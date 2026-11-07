//! The hooks and journal refs (SPEC JR-07, JR-08, D11, D12): the Stop hook captures, then exports
//! the store to the journal refs inside its budget; nothing else exports, and nothing writes a
//! committed journal any more.

use crate::scratch::{Repo, TestResult};
use crate::support::{git, state_of};
use serde_json::{Value, json};
use std::path::Path;
use std::process::{Command, Output};

const JOURNAL_DIR: &str = ".kb/_ledger/journal";

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
fn a_stop_hook_exports_the_sessions_events_to_its_journal_ref() -> TestResult {
    let (repo, remote) = Repo::with_remote(true)?;

    let out = repo.hook(&repo.stop("s"))?;

    exited_0(&out)?;
    let stored = repo.printed_for("s")?.lines().count();
    assert!(stored > 0, "the hook raised items");
    let journal = git_out(&remote, &["show", "refs/redkiln/journal/s:s.jsonl"])?;
    assert_eq!(journal.lines().count(), stored, "{journal}");
    assert_eq!(logged(&repo)?, Vec::<String>::new());
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
fn a_stop_hook_whose_remote_refuses_custom_refs_pushes_an_inbox_branch() -> TestResult {
    let (repo, remote) = Repo::with_remote(true)?;
    pre_receive(
        &remote,
        "#!/bin/sh\nwhile read old new ref; do\n  case \"$ref\" in\n    refs/heads/*) ;;\n    *) echo \"push contains a ref outside refs/heads/*\" >&2; exit 1 ;;\n  esac\ndone\n",
    )?;

    exited_0(&repo.hook(&repo.stop("s"))?)?;

    assert_eq!(
        refs(&remote, "refs/heads/redkiln")?,
        ["refs/heads/redkiln/inbox/s"]
    );
    assert_eq!(refs(&remote, "refs/redkiln")?, Vec::<String>::new());
    assert_eq!(
        logged(&repo)?,
        Vec::<String>::new(),
        "a relayed journal needs no one"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_stop_hook_finding_a_broken_journal_ref_logs_it_once_and_exits_0() -> TestResult {
    let (repo, remote) = Repo::with_remote(true)?;
    // The repository's own commit: its tree is not exactly `s.jsonl`, so the ref breaks JR-05.
    git(
        &repo.root,
        &["push", "-q", "origin", "HEAD:refs/redkiln/journal/s"],
    );
    let before = git_out(&remote, &["rev-parse", "refs/redkiln/journal/s"])?;

    let out = repo.hook(&repo.stop("s"))?;

    exited_0(&out)?;
    let log = logged(&repo)?;
    assert_eq!(log.len(), 1, "{log:?}");
    assert!(
        log.iter()
            .all(|l| l.contains("Stop: refs/redkiln/journal/s breaks JR-05 (tree)")),
        "{log:?}"
    );
    assert_eq!(
        git_out(&remote, &["rev-parse", "refs/redkiln/journal/s"])?,
        before,
        "left alone"
    );
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
    assert!(elapsed < std::time::Duration::from_secs(20), "{elapsed:?}");
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
    // Budget::STOP's worst case is 17.5 s, plus 2 s for the hook's own work (journal::refs).
    assert_eq!(timeout("Stop"), Some(20));
    assert_eq!(timeout("SessionStart"), Some(10));
    Ok(())
}
