//! A scratch repository with a temporary directory of its own, for the tests of session
//! journals and `ci-check`: every run sees its own `TMPDIR`, so each test's journal locks are its
//! own (TST-10), and output comes back with stdout and stderr apart.

use crate::support::{MESSAGE, bin, git, setup};
use serde_json::{Value, json};
use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

pub(crate) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

pub(crate) struct Repo {
    _tmp: tempfile::TempDir,
    pub(crate) root: PathBuf,
    temp: PathBuf,
}

impl Repo {
    /// A repository from `setup`, initialised with `weighin init` when `init` is true.
    pub(crate) fn new(init: bool) -> TestResult<Self> {
        let (scratch, root) = setup();
        let temp = scratch.path().join("tmp");
        std::fs::create_dir_all(&temp)?;
        let repo = Self {
            _tmp: scratch,
            root,
            temp,
        };
        if init {
            repo.ok("s", &["init"], None)?;
        }
        Ok(repo)
    }

    /// A folder beside the repository, for a clone or a folder outside git.
    pub(crate) fn sibling(&self, name: &str) -> TestResult<PathBuf> {
        let dir = self.temp.with_file_name(name);
        std::fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    /// Run `weighin --session <session> <args>` here, with `stdin` when given.
    pub(crate) fn run(
        &self,
        session: &str,
        args: &[&str],
        stdin: Option<&str>,
    ) -> TestResult<Output> {
        let mut child = self
            .command(&self.root)
            .args(["--session", session])
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        if let (Some(text), Some(pipe)) = (stdin, child.stdin.as_mut()) {
            pipe.write_all(text.as_bytes())?;
        }
        drop(child.stdin.take());
        Ok(child.wait_with_output()?)
    }

    /// Run `weighin --session ci <args>` here with a standard output whose reader is already gone
    /// (`weighin … | head -c 0`, without the race); its exit status and stderr.
    pub(crate) fn run_with_stdout_closed(&self, args: &[&str]) -> TestResult<Output> {
        let (reader, writer) = std::io::pipe()?;
        drop(reader);
        Ok(self
            .command(&self.root)
            .args(["--session", "ci"])
            .args(args)
            .stdin(Stdio::null())
            .stdout(writer)
            .stderr(Stdio::piped())
            .output()?)
    }

    /// The lock file the journal writer takes for `session`, in this repository's temp dir.
    pub(crate) fn lock_path(&self, session: &str) -> PathBuf {
        self.temp.join(format!("weigh-in-journal-{session}.lock"))
    }

    /// Run `weighin <args>` in another folder, with no session and nothing on stdin.
    pub(crate) fn run_in(&self, dir: &std::path::Path, args: &[&str]) -> TestResult<Output> {
        Ok(self.command(dir).args(args).stdin(Stdio::null()).output()?)
    }

    /// The binary, started in `dir` with this repository's temporary directory and none of the
    /// session or store variables of the environment the tests run in.
    fn command(&self, dir: &std::path::Path) -> Command {
        let mut cmd = Command::new(bin());
        cmd.current_dir(dir)
            .env_remove("WEIGHIN_STORE")
            .env_remove("WEIGHIN_SESSION")
            .env_remove("CLAUDE_SESSION_ID")
            .env_remove("CLAUDE_CODE_SESSION_ID")
            .env_remove("CLAUDE_CODE_ENTRYPOINT")
            .env("TMPDIR", &self.temp)
            .env("TMP", &self.temp)
            .env("TEMP", &self.temp);
        cmd
    }

    /// [`Repo::run`], failing unless it exits 0; its stdout.
    pub(crate) fn ok(
        &self,
        session: &str,
        args: &[&str],
        stdin: Option<&str>,
    ) -> TestResult<String> {
        let out = self.run(session, args, stdin)?;
        if !out.status.success() {
            return Err(format!(
                "weighin {args:?} failed: {}",
                String::from_utf8_lossy(&out.stderr)
            )
            .into());
        }
        Ok(String::from_utf8(out.stdout)?)
    }

    /// Raise `q` by hand in `session`; the new item's id.
    pub(crate) fn raise(&self, session: &str, q: &str) -> TestResult<String> {
        let out = self.ok(session, &["raise", "--q", q, "--kind", "question"], None)?;
        Ok(out.split_whitespace().nth(1).ok_or("no id")?.to_owned())
    }

    /// Run a hook with `payload`; its output.
    pub(crate) fn hook(&self, payload: &Value) -> TestResult<Output> {
        self.run("unused", &["hook"], Some(&payload.to_string()))
    }

    /// The Stop hook payload for `session` with the two-marker message.
    pub(crate) fn stop(&self, session: &str) -> Value {
        json!({"hook_event_name": "Stop", "session_id": session, "cwd": self.root, "last_assistant_message": MESSAGE})
    }

    pub(crate) fn journal_dir(&self) -> PathBuf {
        self.root.join(".kb/_ledger/journal")
    }

    pub(crate) fn journal_path(&self, session: &str) -> PathBuf {
        self.journal_dir().join(format!("{session}.jsonl"))
    }

    /// The journal of `session`, as bytes.
    pub(crate) fn journal(&self, session: &str) -> TestResult<Vec<u8>> {
        Ok(std::fs::read(self.journal_path(session))?)
    }

    /// The lines `weighin journal` prints for `session`'s events, each with its newline.
    pub(crate) fn printed_for(&self, session: &str) -> TestResult<String> {
        let printed = self.ok("reader", &["journal"], None)?;
        let mut kept = String::new();
        for l in printed.lines() {
            let v: Value = serde_json::from_str(l)?;
            if v.get("session_id").and_then(Value::as_str) == Some(session) {
                kept.push_str(l);
                kept.push('\n');
            }
        }
        Ok(kept)
    }

    /// `git add -A` and a commit with `message`; the new commit's sha.
    pub(crate) fn commit(&self, message: &str) -> TestResult<String> {
        git(&self.root, &["add", "-A"]);
        git(
            &self.root,
            &["commit", "-q", "--allow-empty", "-m", message],
        );
        self.rev("HEAD")
    }

    /// The full sha `rev` names.
    pub(crate) fn rev(&self, rev: &str) -> TestResult<String> {
        let out = Command::new("git")
            .arg("-C")
            .arg(&self.root)
            .args(["rev-parse", "--verify", rev])
            .output()?;
        if !out.status.success() {
            return Err(format!("no revision {rev}").into());
        }
        Ok(String::from_utf8(out.stdout)?.trim().to_owned())
    }
}
