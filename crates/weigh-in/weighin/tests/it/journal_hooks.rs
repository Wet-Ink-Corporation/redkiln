//! The hooks and journal refs (SPEC JR-07, JR-08, D11, D12): the Stop hook captures, then exports
//! the store to the journal refs inside its budget; nothing else exports, and nothing writes a
//! committed journal any more. The cases that hold only when the Stop's export beats
//! `Budget::STOP`'s 4 s deadline, which a slow runner spawning git can pass, run in process with
//! a budget the test gives the hook (`src/hooks/turn/tests/export.rs`, TST-10).

use crate::scratch::{Repo, TestResult};
use crate::support::{git, state_of};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const JOURNAL_DIR: &str = ".kb/_ledger/journal";

/// `Budget::STOP.worst_case()` (`journal/refs.rs`): 4 s + 7 x 2 s. The binary's constants are
/// out of this test's reach, so this changes with them.
#[cfg(unix)]
const STOP_WORST_CASE: std::time::Duration = std::time::Duration::from_secs(18);

/// The export watermarks in the state dir `state`, one per repository and store, sorted, with
/// what each holds.
fn watermarks(state: &Path) -> TestResult<Vec<(PathBuf, String)>> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(state)? {
        let path = entry?.path();
        let named = path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with("journal-watermark-"));
        if named {
            let text = std::fs::read_to_string(&path)?;
            found.push((path, text));
        }
    }
    found.sort();
    Ok(found)
}

/// Where weighin records how far the store's events are exported: the one watermark in the
/// repository's state dir.
fn watermark(repo: &Repo) -> TestResult<PathBuf> {
    match watermarks(&state_of(&repo.root))?.as_slice() {
        [(path, _)] => Ok(path.clone()),
        found => Err(format!("one watermark, not {found:?}").into()),
    }
}

/// The store position the watermark records.
fn watermark_at(repo: &Repo) -> TestResult<u64> {
    let text = std::fs::read_to_string(watermark(repo)?)?;
    Ok(text.lines().next().ok_or("a position")?.parse()?)
}

/// Puts a directory where the repository's watermark is: reading it then fails with something
/// other than "not found".
fn unreadable_watermark(repo: &Repo) -> TestResult {
    let path = watermark(repo)?;
    std::fs::remove_file(&path)?;
    std::fs::create_dir(&path)?;
    Ok(())
}

/// git's stdout in `dir`, trimmed; the run must succeed.
fn git_out(dir: &Path, args: &[&str]) -> TestResult<String> {
    let out = Command::new("git").arg("-C").arg(dir).args(args).output()?;
    if !out.status.success() {
        return Err(format!("git {args:?}: {}", String::from_utf8_lossy(&out.stderr)).into());
    }
    Ok(String::from_utf8(out.stdout)?.trim().to_owned())
}

/// The refs of `dir` under `prefix`, sorted.
fn refs(dir: &Path, prefix: &str) -> TestResult<Vec<String>> {
    let listed = git_out(dir, &["for-each-ref", "--format=%(refname)", prefix])?;
    let mut names: Vec<String> = listed.lines().map(str::to_owned).collect();
    names.sort();
    Ok(names)
}

/// The lines of the hook error log, none when there is no log.
fn logged(repo: &Repo) -> TestResult<Vec<String>> {
    match std::fs::read_to_string(state_of(&repo.root).join("hook-errors.log")) {
        Ok(text) => Ok(text.lines().map(str::to_owned).collect()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e.into()),
    }
}

/// The hook ran and exited 0.
fn exited_0(out: &Output) -> TestResult {
    if out.status.success() {
        Ok(())
    } else {
        Err(format!("the hook failed: {}", String::from_utf8_lossy(&out.stderr)).into())
    }
}

/// A `PostToolUse` payload for a `Write` of a plan with one open question.
fn markdown_write(repo: &Repo) -> TestResult<Value> {
    let plan = "# Plan\n\n## Open questions\n- Should we shard the ledger by tenant?\n";
    let path = repo.root.join("docs/plan.md");
    std::fs::write(&path, plan)?;
    Ok(
        json!({"hook_event_name": "PostToolUse", "session_id": "s", "cwd": repo.root,
            "tool_name": "Write", "tool_input": {"file_path": path, "content": plan}}),
    )
}

/// Makes `remote` run `script` as its `pre-receive` hook.
#[cfg(unix)]
fn pre_receive(remote: &Path, script: &str) -> TestResult {
    use std::os::unix::fs::PermissionsExt as _;
    let hook = remote.join("hooks/pre-receive");
    std::fs::write(&hook, script)?;
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755))?;
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn no_hook_or_command_writes_a_committed_journal() -> TestResult {
    let repo = Repo::new(true)?;

    exited_0(&repo.hook(&repo.stop("s"))?)?;
    exited_0(&repo.hook(&markdown_write(&repo)?)?)?;
    let batch = json!([{"op": "raise", "q": "Shard by tenant?", "door": "two-way",
        "authority": "judgment"}]);
    repo.ok("s", &["batch"], Some(&batch.to_string()))?;
    let id = repo.raise("s", "Keep three retries?")?;
    repo.ok("s", &["decide", &id, "A"], None)?;

    assert!(!repo.root.join(JOURNAL_DIR).exists());
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_subagent_stop_hook_captures_but_exports_nothing() -> TestResult {
    let (repo, remote) = Repo::with_remote(true)?;
    let mut payload = repo.stop("s");
    payload["hook_event_name"] = json!("SubagentStop");

    exited_0(&repo.hook(&payload)?)?;

    assert!(!repo.printed_for("s")?.is_empty(), "the hook raised items");
    assert_eq!(refs(&repo.root, "refs/redkiln")?, Vec::<String>::new());
    assert_eq!(refs(&remote, "refs/redkiln")?, Vec::<String>::new());
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_post_tool_use_hook_exports_nothing() -> TestResult {
    let (repo, remote) = Repo::with_remote(true)?;

    exited_0(&repo.hook(&markdown_write(&repo)?)?)?;

    assert_eq!(repo.printed_for("s")?.lines().count(), 1, "one item raised");
    assert_eq!(refs(&repo.root, "refs/redkiln")?, Vec::<String>::new());
    assert_eq!(refs(&remote, "refs/redkiln")?, Vec::<String>::new());
    assert!(!repo.root.join(JOURNAL_DIR).exists());
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn an_appending_command_prints_no_journal_warning() -> TestResult {
    let repo = Repo::new(true)?;

    let out = repo.run("s", &["raise", "--q", "Keep three retries?"], None)?;

    assert!(out.status.success());
    assert_eq!(String::from_utf8(out.stderr)?, "");
    Ok(())
}

#[cfg(unix)]
#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_stop_hook_whose_remote_hangs_ends_inside_its_timeout_and_the_next_stop_exports() -> TestResult
{
    let (repo, remote) = Repo::with_remote(true)?;
    // Only a push runs the hook; it ends by itself soon after the test, whichever process
    // outlives the killed push.
    pre_receive(&remote, "#!/bin/sh\nsleep 12\n")?;

    let started = std::time::Instant::now();
    let out = repo.hook(&repo.stop("s"))?;
    let elapsed = started.elapsed();

    exited_0(&out)?;
    // The budget's worst case, and a second for starting the binary and capturing.
    let bound = STOP_WORST_CASE.saturating_add(std::time::Duration::from_secs(1));
    assert!(elapsed < bound, "{elapsed:?}");
    assert!(
        !repo.printed_for("s")?.is_empty(),
        "the store keeps the events"
    );
    assert_eq!(refs(&remote, "refs/redkiln")?, Vec::<String>::new());
    assert_eq!(
        logged(&repo)?,
        Vec::<String>::new(),
        "a failed push retries unlogged"
    );

    std::fs::remove_file(remote.join("hooks/pre-receive"))?;
    exited_0(&repo.hook(&repo.stop("s"))?)?;

    assert_eq!(
        refs(&remote, "refs/redkiln/journal")?,
        ["refs/redkiln/journal/s"]
    );
    Ok(())
}

/// Sessions already exported before the Stop under test: enough that a scan of every journal
/// shows in git's trace, few enough to raise quickly on every platform.
const EXPORTED_SESSIONS: usize = 30;

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_stop_examines_only_the_journals_with_events_since_the_last_clean_export() -> TestResult {
    let repo = Repo::new(true)?;
    for n in 0..EXPORTED_SESSIONS {
        repo.raise(&format!("old{n}"), "Keep three retries?")?;
    }
    repo.ok("cli", &["journal", "checkpoint"], None)?;
    assert_eq!(
        refs(&repo.root, "refs/redkiln/journal")?.len(),
        EXPORTED_SESSIONS,
        "the sweep exported every session"
    );
    repo.raise("s", "Shard the ledger by tenant?")?;
    let trace = repo.sibling("trace")?.join("git.trace");

    let out = repo.hook_with(&repo.stop("s"), "GIT_TRACE", &trace.to_string_lossy())?;

    exited_0(&out)?;
    assert!(
        repo.has_ref("refs/redkiln/journal/s"),
        "the current session is exported"
    );
    let traced = std::fs::read_to_string(&trace)?;
    assert!(
        traced.lines().any(|l| l.contains("refs/redkiln/journal/s")),
        "the trace shows the current journal read: {traced}"
    );
    let examined: Vec<&str> = traced
        .lines()
        .filter(|l| l.contains("refs/redkiln/journal/old"))
        .collect();
    assert_eq!(
        examined,
        Vec::<&str>::new(),
        "no exported journal is read again"
    );
    assert_eq!(logged(&repo)?, Vec::<String>::new());
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_watermark_that_cannot_be_read_is_one_warning_and_one_logged_line() -> TestResult {
    let repo = Repo::new(true)?;
    repo.raise("u", "Keep the cache warm?")?;
    repo.ok("cli", &["journal", "checkpoint"], None)?;
    unreadable_watermark(&repo)?;
    repo.raise("t", "Keep three retries?")?;

    let out = repo.run("cli", &["journal", "checkpoint", "--json"], None)?;

    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let envelope: Value = serde_json::from_slice(&out.stdout)?;
    let keys: Vec<&str> = envelope["findings"]
        .as_array()
        .ok_or("findings")?
        .iter()
        .filter_map(|f| f["key"].as_str())
        .collect();
    assert_eq!(keys, ["journal/watermark"], "{envelope}");
    assert_eq!(envelope["findings"][0]["severity"], "warning", "{envelope}");
    assert!(repo.has_ref("refs/redkiln/journal/t"), "the export stands");
    exited_0(&repo.hook(&repo.stop("s"))?)?;
    let log = logged(&repo)?;
    assert_eq!(log.len(), 1, "{log:?}");
    assert!(
        log.iter()
            .all(|l| l.contains("reading the export watermark")
                && l.contains("every Stop examines every journal until it can be read")),
        "the line says what it costs until fixed: {log:?}"
    );
    let finding = envelope["findings"][0]["message"]
        .as_str()
        .ok_or("a message")?;
    assert!(
        finding.contains("reading the export watermark"),
        "{finding}"
    );
    Ok(())
}

#[cfg(unix)]
#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn with_an_unreadable_watermark_a_stop_exports_its_own_session_first() -> TestResult {
    let (repo, remote) = Repo::with_remote(true)?;
    for n in 0..8 {
        repo.raise(&format!("old{n}"), "Keep three retries?")?;
    }
    repo.ok("cli", &["journal", "checkpoint"], None)?;
    // History to write again, older than the current session: the old journals lose their refs.
    for name in refs(&remote, "refs/redkiln/journal")? {
        git(&remote, &["update-ref", "-d", &name]);
        git(&repo.root, &["update-ref", "-d", &name]);
    }
    unreadable_watermark(&repo)?;
    // Each push takes a second, so the Stop's deadline passes long before the history is done.
    pre_receive(&remote, "#!/bin/sh\ncat >/dev/null\nsleep 1\n")?;

    exited_0(&repo.hook(&repo.stop("s"))?)?;

    let pushed = refs(&remote, "refs/redkiln/journal")?;
    assert!(
        pushed.contains(&"refs/redkiln/journal/s".to_owned()),
        "the current session first: {pushed:?}"
    );
    assert!(pushed.len() < 9, "cut short: {pushed:?}");
    Ok(())
}

#[cfg(unix)]
#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_sweep_cut_short_writes_new_work_first_and_never_moves_the_watermark_back() -> TestResult {
    let (repo, remote) = Repo::with_remote(true)?;
    for n in 0..8 {
        repo.raise(&format!("old{n}"), "Keep three retries?")?;
    }
    repo.ok("cli", &["journal", "checkpoint"], None)?;
    let before = watermark_at(&repo)?;
    // History to write again: the old journals lose their refs on both sides.
    for name in refs(&remote, "refs/redkiln/journal")? {
        git(&remote, &["update-ref", "-d", &name]);
        git(&repo.root, &["update-ref", "-d", &name]);
    }
    repo.raise("t", "Shard the ledger by tenant?")?;
    // Each push takes a second, so pre-push's deadline passes long before the history is done.
    pre_receive(&remote, "#!/bin/sh\ncat >/dev/null\nsleep 1\n")?;
    let url = remote.to_string_lossy();
    let pushing = format!(
        "refs/heads/main {} refs/heads/main {}\n",
        "1".repeat(40),
        "0".repeat(40)
    );

    let out = repo.run(
        "cli",
        &["journal", "pre-push", "origin", &url],
        Some(&pushing),
    )?;

    assert_eq!(out.status.code(), Some(0));
    let pushed = refs(&remote, "refs/redkiln/journal")?;
    assert!(
        pushed.contains(&"refs/redkiln/journal/t".to_owned()),
        "new work first: {pushed:?}"
    );
    assert!(pushed.len() < 9, "cut short: {pushed:?}");
    assert!(
        watermark_at(&repo)? >= before,
        "the watermark only moves forward"
    );
    Ok(())
}

#[cfg(unix)]
#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn stops_cut_short_by_their_deadline_each_move_the_watermark_on() -> TestResult {
    let (repo, remote) = Repo::with_remote(true)?;
    for n in 0..8 {
        repo.raise(&format!("old{n}"), "Keep three retries?")?;
    }
    // Each push takes a second, so the Stop's deadline passes after a few journals.
    pre_receive(&remote, "#!/bin/sh\ncat >/dev/null\nsleep 1\n")?;

    exited_0(&repo.hook(&repo.stop("s"))?)?;

    let first = watermark_at(&repo)?;
    let pushed = refs(&remote, "refs/redkiln/journal")?.len();
    assert!(first > 0, "the journals it finished are settled");
    assert!((1..9).contains(&pushed), "cut short: {pushed} pushed");
    exited_0(&repo.hook(&repo.stop("s"))?)?;
    assert!(
        watermark_at(&repo)? > first,
        "the next Stop goes on from there"
    );
    assert_eq!(
        logged(&repo)?,
        Vec::<String>::new(),
        "a late journal retries unlogged"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn in_cowork_a_stop_hook_exports_nothing() -> TestResult {
    let (repo, remote) = Repo::with_remote(true)?;

    let out = repo.hook_with(&repo.stop("s"), "CLAUDE_CODE_ENTRYPOINT", "remote_cowork")?;

    exited_0(&out)?;
    assert_eq!(refs(&repo.root, "refs/redkiln")?, Vec::<String>::new());
    assert_eq!(refs(&remote, "refs/redkiln")?, Vec::<String>::new());
    Ok(())
}

#[test]
fn hooks_json_timeouts_match_the_budgets() -> TestResult {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../plugins/weigh-in/hooks/hooks.json");
    let hooks: Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
    let timeout = |event: &str| hooks["hooks"][event][0]["hooks"][0]["timeout"].as_u64();
    // Budget::STOP's worst case is 18 s, plus 2 s for the hook's own work (journal::refs).
    assert_eq!(timeout("Stop"), Some(20));
    assert_eq!(timeout("SessionStart"), Some(10));
    Ok(())
}
