//! Shared helpers for the integration tests: the binary, scratch repositories, the launcher
//! harness and the steps of the end-to-end scenario.

use serde_json::{Value, json};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub(crate) fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_weighin")
}

pub(crate) fn git(dir: &Path, args: &[&str]) {
    let st = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .output()
        .unwrap();
    assert!(
        st.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&st.stderr)
    );
}

/// One `weighin` invocation: the session id, the arguments, optional stdin and extra environment.
pub(crate) struct Run<'a> {
    pub(crate) session: &'a str,
    pub(crate) args: &'a [&'a str],
    pub(crate) stdin: Option<&'a str>,
    pub(crate) vars: &'a [(&'a str, &'a str)],
}

pub(crate) fn weighin(
    dir: &Path,
    session: &str,
    args: &[&str],
    stdin: Option<&str>,
) -> (bool, String) {
    weighin_env(
        dir,
        &Run {
            session,
            args,
            stdin,
            vars: &[],
        },
    )
}

pub(crate) fn weighin_env(dir: &Path, run: &Run<'_>) -> (bool, String) {
    let mut cmd = Command::new(bin());
    cmd.current_dir(dir)
        .args(["--session", run.session])
        .args(run.args)
        .env_remove("WEIGHIN_STORE")
        .env_remove("CLAUDE_SESSION_ID")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("CLAUDE_CODE_ENTRYPOINT")
        .envs(run.vars.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    if let Some(s) = run.stdin {
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(s.as_bytes())
            .unwrap();
    }
    drop(child.stdin.take());
    let out = child.wait_with_output().unwrap();
    let text =
        String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    (out.status.success(), text)
}

pub(crate) fn hook(dir: &Path, payload: &Value) -> String {
    let (ok, out) = weighin(dir, "unused", &["hook"], Some(&payload.to_string()));
    assert!(ok, "hook failed: {out}");
    out
}

pub(crate) fn setup() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::create_dir_all(repo.join("docs")).unwrap();
    std::fs::write(
        repo.join("src/retention.rs"),
        "// Retention rules.\n\npub struct RetentionPolicy;\n\nimpl RetentionPolicy {\n    pub fn series_overrides(&self) -> usize {\n        0\n    }\n}\n\npub fn apply() {\n    let p = RetentionPolicy;\n    let _ = p.series_overrides();\n}\n",
    )
    .unwrap();
    std::fs::write(repo.join("docs/plan.md"), "# Plan\n\nWork.\n").unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-q", "-m", "init"]);
    (tmp, repo)
}

pub(crate) const MESSAGE: &str = "Migration done.\n\n```weigh-in\nkind: assumption\ndoor: one-way\nauthority: authority\nq: Should retention overrides attach per record or per series?\ndid: per-series, matching RetentionPolicy\nflip: a tenant needs per-record legal hold\nat: src/retention.rs:6\n```\n\n```weigh-in\nkind: call\ndoor: two-way\nauthority: judgment\nq: Name the helper apply or run?\ndid: named it apply\n```\n";

/// Generous for a slow shared CI runner (a normal run takes under a second); a looping
/// launcher never finishes, so it still fails.
pub(crate) const LAUNCHER_LIMIT: std::time::Duration = std::time::Duration::from_secs(30);

/// Runs `cmd` to completion, killing it and failing the test if it outlives `limit` (a looping
/// launcher). The deadline is kept here rather than with `timeout(1)`, which on Windows can
/// resolve to `TIMEOUT.EXE` from System32, a different program.
pub(crate) fn run_within(cmd: &mut Command, limit: std::time::Duration) -> std::process::Output {
    use std::io::Read as _;
    let deadline = std::time::Instant::now() + limit;
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("sh is on PATH");
    let mut stdout = child.stdout.take().expect("stdout is piped");
    let mut stderr = child.stderr.take().expect("stderr is piped");
    let (tx_out, rx) = std::sync::mpsc::sync_channel::<(bool, Vec<u8>)>(2);
    let tx_err = tx_out.clone();
    std::thread::scope(|s| {
        std::thread::Builder::new()
            .name("launcher-stdout".into())
            .spawn_scoped(s, move || {
                let mut buf = Vec::new();
                stdout.read_to_end(&mut buf).expect("stdout is readable");
                tx_out
                    .send((true, buf))
                    .expect("the test thread is listening");
            })
            .expect("the stdout reader starts");
        std::thread::Builder::new()
            .name("launcher-stderr".into())
            .spawn_scoped(s, move || {
                let mut buf = Vec::new();
                stderr.read_to_end(&mut buf).expect("stderr is readable");
                tx_err
                    .send((false, buf))
                    .expect("the test thread is listening");
            })
            .expect("the stderr reader starts");
        let (mut out, mut err) = (Vec::new(), Vec::new());
        for _ in 0..2 {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            match rx.recv_timeout(left) {
                Ok((true, buf)) => out = buf,
                Ok((false, buf)) => err = buf,
                Err(_) => {
                    child.kill().expect("the looping launcher can be killed");
                    child.wait().expect("the killed launcher is reaped");
                    panic!("the launcher did not finish within {limit:?}");
                }
            }
        }
        let status = child.wait().expect("the launcher exits");
        std::process::Output {
            status,
            stdout: out,
            stderr: err,
        }
    })
}

/// The target directory the `sh` launcher looks in first on this platform.
pub(crate) fn launcher_target() -> Option<(&'static str, &'static str)> {
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some(("x86_64-unknown-linux-musl", ""))
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some(("aarch64-apple-darwin", ""))
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        Some(("x86_64-pc-windows-msvc", ".exe"))
    } else {
        None
    }
}

/// A copy of the `sh` launcher in a scratch `bin` directory with no binary beside it, and a
/// `PATH` that finds it first.
pub(crate) fn isolated_launcher() -> (tempfile::TempDir, PathBuf, std::ffi::OsString) {
    let tmp = tempfile::tempdir().unwrap();
    let bin_dir = tmp.path().join("bin");
    std::fs::create_dir_all(&bin_dir).unwrap();
    let launcher = bin_dir.join("weighin");
    std::fs::copy(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../plugin/libexec/weighin"),
        &launcher,
    )
    .unwrap();
    let parent = std::env::var_os("PATH").unwrap_or_default();
    let path = std::env::join_paths(std::iter::once(bin_dir).chain(std::env::split_paths(&parent)))
        .unwrap();
    (tmp, launcher, path)
}

/// Runs `args` with `--dir` on a fresh folder with no ledger, and says whether it succeeded,
/// what it printed and whether it left a `.weighin/` behind. An `<answers>` argument becomes an
/// empty directory in that folder.
pub(crate) fn in_a_folder_with_no_ledger(
    args: &[&str],
    stdin: Option<&str>,
) -> (bool, String, bool) {
    let tmp = tempfile::tempdir().unwrap();
    let folder = tmp.path().join("empty");
    std::fs::create_dir_all(folder.join("answers")).unwrap();
    let dir = folder.to_str().unwrap().to_owned();
    let answers = folder.join("answers").to_str().unwrap().to_owned();
    let args: Vec<&str> = args
        .iter()
        .map(|a| {
            if *a == "<answers>" {
                answers.as_str()
            } else {
                a
            }
        })
        .collect();

    let (ok, out) = weighin(
        tmp.path(),
        "s1",
        &[&["--dir", &dir], &args[..]].concat(),
        stdin,
    );

    (ok, out, folder.join(".weighin").exists())
}

/// Scans `file` in `repo` and returns the id of the one item it raised.
pub(crate) fn scan_one(repo: &Path, file: &str) -> String {
    let (ok, out) = weighin(repo, "s1", &["scan", file], None);
    assert!(ok && out.starts_with("raised "), "{out}");
    out.split_whitespace().nth(1).unwrap().to_owned()
}

/// The receipt's note on a code file.
pub(crate) const CODE_FILE: &str = "code file: the decision is in the ledger";

/// Decides the one item scanned from `file`, written with `source`, as `option`, and checks the
/// receipt says the code file was not annotated and the file is unchanged.
pub(crate) fn decide_leaves_code_alone(file: &str, source: &str, option: &str) {
    let (_tmp, repo) = setup();
    std::fs::write(repo.join(file), source).unwrap();
    let id = scan_one(&repo, file);

    let (ok, out) = weighin(&repo, "s1", &["decide", &id, option], None);

    assert!(
        ok && out.contains(&format!("decided {id} → {option}"))
            && out.contains(&format!("not annotated {file} ({CODE_FILE})"))
            && out.contains(&format!("trailer `Weigh-In: {id} decided {option}`")),
        "{file} {option:?}: {out}"
    );
    assert_eq!(
        std::fs::read(repo.join(file)).unwrap(),
        source.as_bytes(),
        "{file} {option:?}"
    );
}

/// The date `decide` stamped on `id`'s arrow in `text`.
pub(crate) fn stamped_date(text: &str, id: &str, option: &str) -> String {
    text.split(&format!("{id}: decided {option} ("))
        .nth(1)
        .and_then(|rest| rest.get(..10))
        .expect("the file holds the arrow")
        .to_owned()
}

/// Settles the CRLF question in `docs/spec.md` and returns its id.
pub(crate) fn settle_crlf_question(repo: &Path) -> String {
    let spec = "# Spec\r\n\r\n## Open questions\r\n- Keep the queue durable?\r\n";
    std::fs::write(repo.join("docs/spec.md"), spec).unwrap();
    let (_, out) = weighin(repo, "s1", &["scan", "docs/spec.md"], None);
    let id = out.split_whitespace().nth(1).unwrap().to_owned();
    let (ok, out) = weighin(repo, "s1", &["decide", &id, "yes"], None);
    assert!(ok, "{out}");
    id
}

// Steps of the end-to-end scenario `a_session_from_capture_to_record`.

pub(crate) fn capture_both_markers(repo: &Path) {
    let (ok, out) = weighin(
        repo,
        "s1",
        &["init", "--class", "own", "--owner", "ryan"],
        None,
    );
    assert!(ok, "{out}");

    // SessionStart primes the convention.
    let prime = hook(
        repo,
        &json!({"hook_event_name": "SessionStart", "session_id": "s1", "cwd": repo, "source": "startup"}),
    );
    assert!(
        prime.contains("```weigh-in") && prime.contains("session s1"),
        "{prime}"
    );
    assert!(
        prime.len() < 1100,
        "prime is ~150 tokens, got {} chars",
        prime.len()
    );

    // Stop captures both markers; the one-way one nudges once; the two-way judgment call is defaulted.
    let stop = json!({"hook_event_name": "Stop", "session_id": "s1", "cwd": repo, "last_assistant_message": MESSAGE});
    let nudge = hook(repo, &stop);
    assert!(
        nudge.contains("systemMessage") && nudge.contains("one-way"),
        "{nudge}"
    );
    assert_eq!(
        hook(repo, &stop),
        "",
        "a repeated Stop raises nothing and does not nudge again"
    );
}

pub(crate) fn scan_written_plan(repo: &Path) {
    // PostToolUse scans only what was written.
    let plan = "# Plan\n\nWork.\n\n## Open questions\n- Should we shard the ledger by tenant?\n";
    std::fs::write(repo.join("docs/plan.md"), plan).unwrap();
    hook(
        repo,
        &json!({"hook_event_name": "PostToolUse", "session_id": "s1", "cwd": repo, "tool_name": "Write", "tool_input": {"file_path": repo.join("docs/plan.md"), "content": plan}}),
    );

    let (_, open) = weighin(repo, "s1", &["open"], None);
    assert!(open.contains("2 open in this session"), "{open}");
    assert!(open.contains("1 default(s) to review"), "{open}");
}

pub(crate) fn items_seen_from_a_worktree(repo: &Path) -> (String, String) {
    // A linked worktree sees the same ledger (it lives under the common dir).
    let wt = repo.parent().unwrap().join("wt");
    git(
        repo,
        &["worktree", "add", "-q", wt.to_str().unwrap(), "-b", "feat"],
    );
    let (_, from_wt) = weighin(&wt, "s2", &["open", "--repo", "--json"], None);
    let items: Vec<Value> = serde_json::from_str(from_wt.trim()).unwrap();
    assert_eq!(items.len(), 2, "{from_wt}");
    let one_way = items.iter().find(|i| i["door_hint"] == "one-way").unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let scanned = items.iter().find(|i| i["channel"] == "scan").unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        !repo.join(".weighin").exists() && !wt.join(".weighin").exists(),
        "nothing is written into the working trees"
    );
    (one_way, scanned)
}

pub(crate) fn context_pack_for(repo: &Path, one_way: &str) {
    // Context pack: excerpt, symbol, call sites.
    let (_, ctx) = weighin(repo, "s1", &["context", one_way], None);
    assert!(
        ctx.contains("series_overrides") && ctx.contains("src/retention.rs:13"),
        "{ctx}"
    );
    assert!(ctx.contains("vscode://file/"), "{ctx}");
}

pub(crate) fn sit_and_decide(repo: &Path, one_way: &str) {
    // The sitting: triage, ground and present in one call.
    let card = json!({
        "why": "It fixes what a tenant can hold.",
        "timing": "Decide now.",
        "evidence": [{"src": "src/retention.rs:6", "text": "series_overrides exists; no per-record path"}],
        "options": [
            {"id": "A", "title": "Per series", "holds_if": "no tenant needs per-record holds", "if_wrong": "a migration of every override"},
            {"id": "B", "title": "Per record", "holds_if": "legal hold is per record", "if_wrong": "slower queries"}
        ],
        "recommendation": {"option": "A", "flip": "a tenant needs per-record legal hold", "stability": {"agree": 4, "of": 4}}
    });
    let batch = json!([
        {"op": "triage", "id": one_way, "door": "one-way", "blast": "system", "route": "present", "reason": "authority-bound one-way door"},
        {"op": "ground", "id": one_way, "card": card},
        {"op": "present", "id": one_way, "surface": "chat"},
        {"op": "decide", "id": one_way, "option": "Z"}
    ]);
    let (ok, out) = weighin(repo, "s1", &["batch"], Some(&batch.to_string()));
    assert!(
        !ok && out.contains("has no option `Z`") && out.contains("✓ presented"),
        "bad option refused, the rest applied: {out}"
    );

    let (ok, out) = weighin(
        repo,
        "s1",
        &[
            "decide",
            one_way,
            "A",
            "--note",
            "series is what the schema has",
            "--via",
            "chat",
        ],
        None,
    );
    assert!(ok && out.contains("atom .kb/_intake/decisions/"), "{out}");
    assert_eq!(
        weighin(repo, "s1", &["trailer", one_way], None),
        (true, format!("Weigh-In: {one_way} decided A\n")),
        "the trailer command prints the decide receipt's trailer"
    );
    let atom_dir = repo.join(".kb/_intake/decisions");
    let atom = std::fs::read_dir(&atom_dir)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let text = std::fs::read_to_string(&atom).unwrap();
    assert!(
        text.contains("kind: decision")
            && text.contains("reversibility: low")
            && text.contains("provenance: user-approved"),
        "{text}"
    );
    assert!(
        text.contains("tree_hash: \""),
        "records the tree the human saw"
    );
}

pub(crate) fn annotate_scanned_and_rescan(repo: &Path, scanned: &str) {
    // Deciding a scanned item annotates its source line.
    let batch = json!([
        {"op": "ground", "id": scanned, "card": {"options": [{"id": "A", "title": "Shard"}, {"id": "B", "title": "One store"}], "recommendation": {"option": "B"}}},
        {"op": "decide", "id": scanned, "option": "B"}
    ]);
    let (ok, out) = weighin(repo, "s1", &["batch"], Some(&batch.to_string()));
    assert!(ok, "{out}");
    let plan_after = std::fs::read_to_string(repo.join("docs/plan.md")).unwrap();
    assert!(
        plan_after.contains(&format!("→ Weigh-In {scanned}: decided B")),
        "{plan_after}"
    );

    // Rescanning the annotated file raises nothing new.
    hook(
        repo,
        &json!({"hook_event_name": "PostToolUse", "session_id": "s1", "cwd": repo, "tool_name": "Write", "tool_input": {"file_path": repo.join("docs/plan.md"), "content": plan_after}}),
    );
    let (_, open) = weighin(repo, "s1", &["open"], None);
    assert!(open.contains("No decisions need you."), "{open}");
}

pub(crate) fn reopen_default_from_the_board(repo: &Path) {
    // Digest shows the default; the board can reopen it.
    let (_, digest) = weighin(repo, "s1", &["digest", "--json"], None);
    let d: Value = serde_json::from_str(digest.trim()).unwrap();
    let default_id = d["defaults"][0]["id"].as_str().unwrap().to_owned();
    let (ok, out) = weighin(
        repo,
        "s1",
        &["board", "export", "--out", "board.json"],
        None,
    );
    assert!(
        ok && out.contains("2 decided") && out.contains("1 default(s)"),
        "{out}"
    );
    let answers = repo.join("answers.json");
    std::fs::write(&answers, json!([
        {"id": default_id, "data": {"item_id": default_id, "action": "reopen", "note": "prefer run"}},
        {"id": "wi-000000", "data": {"item_id": "wi-000000", "action": "choose", "option": "A"}}
    ]).to_string()).unwrap();
    let (ok, out) = weighin(
        repo,
        "s1",
        &["board", "apply", answers.to_str().unwrap()],
        None,
    );
    assert!(
        ok && out.contains(&format!("reopened {default_id}")) && out.contains("✗ wi-000000"),
        "{out}"
    );

    let (_, stats) = weighin(repo, "s1", &["stats", "--json"], None);
    let h: Value = serde_json::from_str(stats.trim()).unwrap();
    assert_eq!(h["presented"], 1);
    assert_eq!(h["acted"], 1);
}
