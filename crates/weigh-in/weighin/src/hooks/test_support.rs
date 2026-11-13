//! Fixtures the hooks' in-process tests share: scratch git repositories, the commands that set
//! them up (`init`, `raise`, `journal checkpoint`), and a hook run inside the budget the test
//! gives it, so that what a test pins does not hang on how fast the runner spawns git (TST-10).

use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, bail};
use redkiln_core::store::Wait;

use super::{Hook, HookEvent, HookOutput, serve};
use crate::env::{BoardMode, Env, RepoClass, Vars};
use crate::journal::refs::{self, Budget, Checkpointed, Scope};
use crate::ops::{RaiseInput, Session};

/// `budget` with no deadline and each git call given a command's time: the hook's journal-ref
/// work always runs to its end, whatever the runner's speed.
pub(super) const fn unhurried(budget: Budget) -> Budget {
    Budget {
        start_by: None,
        per_call: Budget::COMMAND.per_call,
        ..budget
    }
}

/// `budget` with a deadline that has passed when the hook starts.
pub(super) const fn past_deadline(budget: Budget) -> Budget {
    Budget {
        start_by: Some(Duration::ZERO),
        ..budget
    }
}

/// Run git in `dir` under a fixed identity, failing on a non-zero exit.
pub(super) fn git(dir: &Path, args: &[&str]) -> Result<()> {
    crate::git::run_env(
        dir,
        args,
        &[
            ("GIT_AUTHOR_NAME", "t"),
            ("GIT_AUTHOR_EMAIL", "t@t"),
            ("GIT_COMMITTER_NAME", "t"),
            ("GIT_COMMITTER_EMAIL", "t@t"),
        ],
    )
    .map(drop)
}

/// The environment of `session` in `dir`, as the hook discovers it, with none of the process's
/// store variables.
pub(super) fn env_of(dir: &Path, session: &str) -> Env {
    Env::discover(
        dir,
        Some((session.to_owned(), "the test")),
        None,
        Vars::default(),
    )
    .with_wait(Wait::Hook)
}

/// A repository with one commit on `main` at `<scratch>/<name>`, not initialised.
pub(super) fn repo(scratch: &Path, name: &str) -> Result<PathBuf> {
    let root = scratch.join(name);
    std::fs::create_dir_all(&root)?;
    std::fs::write(root.join("README.md"), "# Scratch\n")?;
    git(&root, &["init", "-q", "-b", "main"])?;
    git(&root, &["add", "."])?;
    git(&root, &["commit", "-q", "-m", "init"])?;
    Ok(root)
}

/// `weighin init --class <class>`, with `--board <board>` when given, in `root`.
pub(super) fn init(root: &Path, class: RepoClass, board: Option<BoardMode>) -> Result<()> {
    let args = crate::InitArgs {
        class: Some(class),
        board,
        board_url: None,
        owner: None,
        ledger: None,
    };
    crate::commands::init(&mut env_of(root, "s"), args, &mut io::sink())
}

/// A repository at `<scratch>/<name>` initialised `own` (`weighin init --class own`).
pub(super) fn own_repo(scratch: &Path, name: &str) -> Result<PathBuf> {
    let root = repo(scratch, name)?;
    init(&root, RepoClass::Own, None)?;
    Ok(root)
}

/// Raise `q` in `session` of `root`, as `weighin raise --kind question` does.
pub(super) fn raise(root: &Path, session: &str, q: &str) -> Result<()> {
    let input = RaiseInput {
        q: q.to_owned(),
        kind: Some("question".to_owned()),
        ..RaiseInput::default()
    };
    Session::open(&env_of(root, session), "cli")?.raise(&input)?;
    Ok(())
}

/// Export `root`'s journals, as `weighin journal checkpoint` does.
pub(super) fn checkpoint(root: &Path) -> Result<()> {
    match refs::checkpoint(
        &env_of(root, "s"),
        Budget::COMMAND,
        Instant::now(),
        Scope::All,
    )? {
        Checkpointed::Ran(..) => Ok(()),
        Checkpointed::Skipped(skip) => bail!("the checkpoint was skipped: {}", skip.as_str()),
    }
}

/// `path` as a string git takes.
pub(super) fn path_str(path: &Path) -> Result<String> {
    path.to_str()
        .map(str::to_owned)
        .context("a UTF-8 scratch path")
}

/// What the hook for `event` says in `root` for `session` within `budget`.
pub(super) fn served(root: &Path, session: &str, event: HookEvent, budget: Budget) -> HookOutput {
    served_in(env_of(root, session), event, budget)
}

/// What the hook for `event` says in `env` within `budget`.
pub(super) fn served_in(env: Env, event: HookEvent, budget: Budget) -> HookOutput {
    let hook = Hook::new(event, env, false, Instant::now()).budget(budget);
    serve(&hook, None)
}

/// A repository from [`own_repo`] at `<scratch>/<name>`, with a bare `origin` beside it that
/// holds its one commit on `main`; the repository's and the remote's paths.
pub(super) fn own_with_remote(scratch: &Path, name: &str) -> Result<(PathBuf, PathBuf)> {
    let root = own_repo(scratch, name)?;
    let remote = scratch.join(format!("{name}-origin.git"));
    std::fs::create_dir_all(&remote)?;
    git(&remote, &["init", "-q", "--bare", "-b", "main"])?;
    git(&root, &["remote", "add", "origin", &path_str(&remote)?])?;
    git(&root, &["push", "-q", "origin", "main"])?;
    Ok((root, remote))
}

/// git's stdout in `dir`, trimmed; the run must succeed.
pub(super) fn git_out(dir: &Path, args: &[&str]) -> Result<String> {
    Ok(crate::git::run(dir, args)?.trim().to_owned())
}

/// The refs of `dir` under `prefix`, sorted.
#[cfg(unix)]
pub(super) fn refs(dir: &Path, prefix: &str) -> Result<Vec<String>> {
    let listed = git_out(dir, &["for-each-ref", "--format=%(refname)", prefix])?;
    let mut names: Vec<String> = listed.lines().map(str::to_owned).collect();
    names.sort();
    Ok(names)
}

/// Whether `root` holds `refname`.
pub(super) fn has_ref(root: &Path, refname: &str) -> bool {
    git(root, &["rev-parse", "--verify", "-q", refname]).is_ok()
}

/// The lines `weighin journal` prints for `session`'s events in `root`, each with its newline.
pub(super) fn printed_for(root: &Path, session: &str) -> Result<String> {
    let mut printed = Vec::new();
    crate::journal::write(
        &crate::ops::stored_events(&env_of(root, "reader"))?,
        &mut printed,
    )?;
    let mut kept = String::new();
    for l in String::from_utf8(printed)?.lines() {
        let v: serde_json::Value = serde_json::from_str(l)?;
        if v.get("session_id").and_then(serde_json::Value::as_str) == Some(session) {
            kept.push_str(l);
            kept.push('\n');
        }
    }
    Ok(kept)
}

/// Makes `remote` run `script` as its `pre-receive` hook.
#[cfg(unix)]
pub(super) fn pre_receive(remote: &Path, script: &str) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    let hook = remote.join("hooks/pre-receive");
    std::fs::write(&hook, script)?;
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755))?;
    Ok(())
}

/// The lines of the hook error log, none when there is no log.
pub(super) fn logged(root: &Path) -> Result<Vec<String>> {
    let log = root.join(".git/redkiln/weighin/hook-errors.log");
    match std::fs::read_to_string(log) {
        Ok(text) => Ok(text.lines().map(str::to_owned).collect()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e.into()),
    }
}
