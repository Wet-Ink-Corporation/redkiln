//! Session journals written by the binary (B-28): after a capture hook or a command that changes
//! the ledger, `.kb/_ledger/journal/<session>.jsonl` holds the session's own events, append-only
//! and each line once, in initialised git repositories only.

use crate::scratch::{Repo, TestResult};
use serde_json::{Value, json};

/// A batch that raises one question, triages it and dismisses it.
fn raise_triage_dismiss(q: &str) -> String {
    json!([
        {"op": "raise", "q": q, "door": "two-way", "authority": "judgment"},
        {"op": "triage", "id": "@1", "door": "two-way", "blast": "file", "route": "dismiss", "reason": "settled"},
        {"op": "dismiss", "id": "@1", "why": "recoverable", "reason": "a config line"}
    ])
    .to_string()
}

/// Whether some line of `text` appears more than once.
fn has_repeated_line(text: &str) -> bool {
    let mut seen = std::collections::HashSet::new();
    text.lines().any(|l| !seen.insert(l))
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_stop_hook_writes_the_sessions_events_as_journal_prints_them() -> TestResult {
    let repo = Repo::new(true)?;

    let out = repo.hook(&repo.stop("s"))?;

    assert!(out.status.success(), "the hook exits 0");
    let expected = repo.printed_for("s")?;
    assert!(!expected.is_empty(), "the hook raised items");
    assert_eq!(String::from_utf8(repo.journal("s")?)?, expected);
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_post_tool_use_hook_on_a_markdown_write_journals_what_it_raised() -> TestResult {
    let repo = Repo::new(true)?;
    let plan = "# Plan\n\n## Open questions\n- Should we shard the ledger by tenant?\n";
    let path = repo.root.join("docs/plan.md");
    std::fs::write(&path, plan)?;
    let payload = json!({"hook_event_name": "PostToolUse", "session_id": "s", "cwd": repo.root,
        "tool_name": "Write", "tool_input": {"file_path": path, "content": plan}});

    let out = repo.hook(&payload)?;

    assert!(out.status.success(), "the hook exits 0");
    let expected = repo.printed_for("s")?;
    assert_eq!(
        expected.lines().count(),
        1,
        "the scan raised one item: {expected}"
    );
    assert_eq!(String::from_utf8(repo.journal("s")?)?, expected);
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git, and locks a file")]
fn a_journal_locked_by_another_writer_warns_busy_and_the_command_succeeds() -> TestResult {
    let repo = Repo::new(true)?;
    let held = std::fs::File::create(repo.lock_path("s"))?;
    held.lock()?;

    let out = repo.run(
        "s",
        &["batch"],
        Some(&raise_triage_dismiss("Keep three retries?")),
    )?;

    let stderr = String::from_utf8(out.stderr)?;
    assert_eq!(
        (out.status.code(), stderr.as_str()),
        (
            Some(0),
            "weighin: warning: session journal busy; the next command or hook catches it up\n"
        )
    );
    assert!(
        !repo.journal_path("s").exists(),
        "nothing written while busy"
    );
    drop(held);
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_batch_writes_each_event_once_and_a_read_adds_nothing() -> TestResult {
    let repo = Repo::new(true)?;

    repo.ok(
        "s",
        &["batch"],
        Some(&raise_triage_dismiss("Keep three retries?")),
    )?;
    let after_batch = repo.journal("s")?;
    repo.ok("s", &["open"], None)?;
    repo.ok("s", &["stats"], None)?;

    let text = std::str::from_utf8(&after_batch)?;
    assert_eq!(text, repo.printed_for("s")?);
    assert_eq!(
        text.lines().count(),
        3,
        "raised, triaged, dismissed: {text}"
    );
    assert_eq!(repo.journal("s")?, after_batch, "reads add nothing");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_later_decide_appends_only_its_new_events() -> TestResult {
    let repo = Repo::new(true)?;
    let raised = repo.ok("s", &["raise", "--q", "Name it apply or run?"], None)?;
    let id = raised.split_whitespace().nth(1).ok_or("no id")?.to_owned();
    let before = repo.journal("s")?;

    repo.ok("s", &["decide", &id, "apply"], None)?;

    let after = repo.journal("s")?;
    assert!(after.starts_with(&before), "the earlier bytes stay put");
    assert_eq!(String::from_utf8(after)?, repo.printed_for("s")?);
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn the_scripts_whole_ledger_file_gains_only_the_sessions_new_events() -> TestResult {
    let repo = Repo::new(true)?;
    repo.ok("t", &["raise", "--q", "Another session's question?"], None)?;
    repo.ok(
        "s",
        &["raise", "--q", "This session's first question?"],
        None,
    )?;
    let whole = repo.ok("reader", &["journal"], None)?;
    std::fs::write(repo.journal_path("s"), &whole)?;

    repo.ok(
        "s",
        &["batch"],
        Some(&raise_triage_dismiss("Keep three retries?")),
    )?;

    let text = String::from_utf8(repo.journal("s")?)?;
    let added = text
        .strip_prefix(&whole)
        .ok_or("the old bytes stay first")?;
    let printed = repo.printed_for("s")?;
    let new_of_s: String = printed
        .lines()
        .filter(|l| !whole.lines().any(|w| w == *l))
        .flat_map(|l| [l, "\n"])
        .collect();
    assert_eq!(added, new_of_s);
    assert!(!has_repeated_line(&text), "no line twice: {text}");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_cut_short_last_line_is_replaced_by_its_whole_event() -> TestResult {
    let repo = Repo::new(true)?;
    repo.ok("s", &["raise", "--q", "First question?"], None)?;
    repo.ok("s", &["raise", "--q", "Second question?"], None)?;
    let printed = repo.printed_for("s")?;
    let mut lines = printed.lines();
    let (first, second) = (
        lines.next().ok_or("one line")?,
        lines.next().ok_or("two lines")?,
    );
    let cut = second.get(..30).ok_or("a long line")?;
    std::fs::write(repo.journal_path("s"), format!("{first}\n{cut}"))?;

    repo.ok(
        "s",
        &["batch"],
        Some(&raise_triage_dismiss("Keep three retries?")),
    )?;

    let text = String::from_utf8(repo.journal("s")?)?;
    assert_eq!(text, repo.printed_for("s")?);
    assert!(text.starts_with(&format!("{first}\n{second}\n")), "{text}");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn journal_false_in_the_config_writes_nothing() -> TestResult {
    let repo = Repo::new(true)?;
    let config = repo.root.join(".kb/_ledger/config.json");
    let mut c: Value = serde_json::from_str(&std::fs::read_to_string(&config)?)?;
    c["journal"] = json!(false);
    std::fs::write(&config, c.to_string())?;

    repo.ok(
        "s",
        &["batch"],
        Some(&raise_triage_dismiss("Keep three retries?")),
    )?;

    assert!(!repo.journal_dir().exists(), "no journal directory");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn an_uninitialised_repository_gets_no_journal() -> TestResult {
    let repo = Repo::new(false)?;

    let hook = repo.hook(&repo.stop("s"))?;
    repo.ok(
        "s",
        &["batch"],
        Some(&raise_triage_dismiss("Keep three retries?")),
    )?;

    assert!(hook.status.success(), "the hook exits 0");
    assert!(!repo.root.join(".kb/_ledger").exists(), "nothing under .kb");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_session_id_that_cannot_be_a_file_name_gets_no_journal() -> TestResult {
    let repo = Repo::new(true)?;

    for session in [".x", "a/b"] {
        repo.ok(
            session,
            &["batch"],
            Some(&raise_triage_dismiss("Keep three retries?")),
        )?;
    }

    assert!(
        !repo.journal_dir().exists(),
        "no journal anywhere under .kb"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_journal_failure_warns_and_leaves_the_command_alone() -> TestResult {
    let repo = Repo::new(true)?;
    std::fs::create_dir_all(repo.journal_path("s"))?;

    let out = repo.run(
        "s",
        &["batch"],
        Some(&raise_triage_dismiss("Keep three retries?")),
    )?;

    let stdout = String::from_utf8(out.stdout)?;
    let stderr = String::from_utf8(out.stderr)?;
    assert!(out.status.success(), "{stderr}");
    assert_eq!(
        stdout.lines().filter(|l| l.starts_with('✓')).count(),
        3,
        "{stdout}"
    );
    assert!(
        stderr.starts_with("weighin: warning: session journal not updated: "),
        "{stderr}"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_journal_failure_in_a_hook_is_logged_and_changes_nothing_else() -> TestResult {
    let reference = Repo::new(true)?;
    let expected = reference.hook(&reference.stop("s"))?;
    let repo = Repo::new(true)?;
    std::fs::create_dir_all(repo.journal_path("s"))?;

    let out = repo.hook(&repo.stop("s"))?;

    assert_eq!(
        (out.status.success(), &out.stdout, &out.stderr),
        (true, &expected.stdout, &expected.stderr)
    );
    let log = std::fs::read_to_string(repo.root.join(".git/weighin/hook-errors.log"))?;
    assert!(log.contains("Stop: updating the session journal"), "{log}");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_command_that_fails_before_writing_leaves_no_ledger_and_no_journal() -> TestResult {
    let repo = Repo::new(true)?;

    let out = repo.run("s", &["batch"], Some("not json"))?;

    assert!(!out.status.success(), "bad JSON fails the batch");
    let stores = [
        repo.root.join(".git/weighin/ledger.sqlite3"),
        repo.journal_dir(),
    ];
    assert_eq!(stores.map(|p| p.exists()), [false, false]);
    Ok(())
}
