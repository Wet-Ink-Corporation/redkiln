//! Past decisions brought back when an edit touches the code they are anchored to (#116): the
//! `decisions` command and the context the `PostToolUse` hook adds.

use crate::support::{hook, setup, weighin};
use serde_json::{Value, json};
use std::path::Path;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Raise `q` anchored `at` and decide it with `option`; the decided item's id.
fn decided(repo: &Path, q: &str, at: &str, option: &str) -> Result<String, String> {
    let batch = json!([
        {"op": "raise", "q": q, "at": at, "kind": "question", "door": "two-way"},
        {"op": "decide", "id": "@1", "option": option}
    ]);
    let (ok, out) = weighin(repo, "s1", &["batch"], Some(&batch.to_string()));
    if !ok {
        return Err(out);
    }
    out.split_whitespace()
        .find(|w| w.starts_with("wi-"))
        .map(str::to_owned)
        .ok_or(out)
}

/// The `PostToolUse` payload of an `Edit` to `file` that wrote `new_string`.
fn edit(repo: &Path, file: &str, new_string: &str) -> Value {
    json!({
        "hook_event_name": "PostToolUse", "session_id": "s1", "cwd": repo, "tool_name": "Edit",
        "tool_input": {"file_path": repo.join(file), "old_string": "0", "new_string": new_string}
    })
}

/// The context lines a hook's output adds, or `None` when it printed nothing.
fn added_context(printed: &str) -> Result<Option<Vec<String>>, serde_json::Error> {
    if printed.is_empty() {
        return Ok(None);
    }
    let out: Value = serde_json::from_str(printed)?;
    let specific = out.get("hookSpecificOutput");
    let field = |name: &str| specific.and_then(|s| s.get(name)).and_then(Value::as_str);
    assert_eq!(field("hookEventName"), Some("PostToolUse"), "{printed}");
    let context = field("additionalContext").unwrap_or_default();
    Ok(Some(context.lines().map(str::to_owned).collect()))
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn decisions_at_a_path_lists_the_decision_anchored_there_as_json() -> TestResult {
    let (_tmp, repo) = setup();
    let id = decided(
        &repo,
        "Per series or per record?",
        "src/retention.rs:6",
        "per series",
    )?;

    let (ok, out) = weighin(
        &repo,
        "s1",
        &["decisions", "--path", "src/retention.rs", "--json"],
        None,
    );

    assert!(ok, "{out}");
    let listed: Value = serde_json::from_str(out.trim())?;
    assert_eq!(
        listed,
        json!([{
            "id": id, "option": "per series", "title": null,
            "question": "Per series or per record?", "at": "src/retention.rs:6", "line": 6,
            "via": "chat",
            "decided_at": listed[0]["decided_at"],
        }])
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn decisions_at_a_path_resolve_an_absolute_path_and_print_one_line_each() -> TestResult {
    let (_tmp, repo) = setup();
    let id = decided(
        &repo,
        "Per series or per record?",
        "src/retention.rs:6",
        "per series",
    )?;
    let file = repo.join("src/retention.rs");

    let (ok, out) = weighin(
        &repo,
        "s1",
        &["decisions", "--path", file.to_str().ok_or("path")?],
        None,
    );

    assert!(ok, "{out}");
    assert_eq!(
        out,
        format!("{id} decided per series: Per series or per record? (src/retention.rs:6)\n")
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn decisions_at_a_path_with_none_anchored_say_so() -> TestResult {
    let (_tmp, repo) = setup();
    decided(
        &repo,
        "Per series or per record?",
        "src/retention.rs:6",
        "per series",
    )?;

    let (ok, out) = weighin(&repo, "s1", &["decisions", "--path", "docs/plan.md"], None);

    assert_eq!(
        (ok, out.as_str()),
        (true, "no decisions anchored at docs/plan.md\n")
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn an_edit_to_an_anchored_file_adds_the_decision_and_raises_nothing() -> TestResult {
    let (_tmp, repo) = setup();
    let id = decided(
        &repo,
        "Per series or per record?",
        "src/retention.rs:6",
        "per series",
    )?;
    let (_, before) = weighin(&repo, "s1", &["open", "--all", "--json"], None);

    let printed = hook(&repo, &edit(&repo, "src/retention.rs", "0\n"));

    assert_eq!(
        added_context(&printed)?,
        Some(vec![format!(
            "{id} decided per series: Per series or per record? (src/retention.rs:6)"
        )])
    );
    let (_, after) = weighin(&repo, "s1", &["open", "--all", "--json"], None);
    assert_eq!(after, before, "the hook raised nothing");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn an_edit_to_a_file_with_no_decision_prints_nothing() -> TestResult {
    let (_tmp, repo) = setup();
    decided(&repo, "What goes in the plan?", "docs/plan.md:1", "work")?;

    let printed = hook(&repo, &edit(&repo, "src/retention.rs", "0\n"));

    assert_eq!(printed, "");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn an_edit_to_a_file_whose_decision_was_reopened_prints_nothing() -> TestResult {
    let (_tmp, repo) = setup();
    let id = decided(
        &repo,
        "Per series or per record?",
        "src/retention.rs:6",
        "per series",
    )?;
    let (ok, out) = weighin(&repo, "s1", &["reopen", &id], None);
    assert!(ok, "{out}");

    let printed = hook(&repo, &edit(&repo, "src/retention.rs", "0\n"));

    assert_eq!(printed, "");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn an_edit_adds_at_most_three_decisions_nearest_the_edited_line_first() -> TestResult {
    let (_tmp, repo) = setup();
    let far = decided(&repo, "Far?", "src/retention.rs:1", "A")?;
    let near = decided(&repo, "Near?", "src/retention.rs:5", "A")?;
    let nearest = decided(&repo, "Nearest?", "src/retention.rs:8", "A")?;
    let farthest = decided(&repo, "Farthest?", "src/retention.rs:40", "A")?;
    // `0` is on line 7 of the file `setup` wrote.

    let printed = hook(&repo, &edit(&repo, "src/retention.rs", "0\n"));

    let lines = added_context(&printed)?.ok_or("no context added")?;
    let shown: Vec<&str> = lines
        .iter()
        .filter_map(|l| l.split_whitespace().next())
        .collect();
    assert_eq!(shown, [nearest.as_str(), near.as_str(), far.as_str()]);
    assert!(!printed.contains(&farthest), "{printed}");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn an_edit_that_writes_a_todo_to_an_anchored_file_still_raises_it() -> TestResult {
    let (_tmp, repo) = setup();
    decided(
        &repo,
        "Per series or per record?",
        "src/retention.rs:6",
        "per series",
    )?;
    let text = std::fs::read_to_string(repo.join("src/retention.rs"))?;
    let todo = "        // TODO: cap the overrides per tenant";
    std::fs::write(
        repo.join("src/retention.rs"),
        text.replacen("        0\n", &format!("{todo}\n        0\n"), 1),
    )?;

    let printed = hook(
        &repo,
        &edit(&repo, "src/retention.rs", &format!("{todo}\n        0")),
    );

    assert_eq!(added_context(&printed)?.map(|l| l.len()), Some(1));
    let (_, open) = weighin(&repo, "s1", &["open", "--repo"], None);
    assert!(open.contains("cap the overrides per tenant"), "{open}");
    Ok(())
}

/// The `PostToolUse` payload of a `Write` of `content` to `file`.
fn write(repo: &Path, file: &str, content: &str) -> Value {
    json!({
        "hook_event_name": "PostToolUse", "session_id": "s1", "cwd": repo, "tool_name": "Write",
        "tool_input": {"file_path": repo.join(file), "content": content}
    })
}

/// The hook error log's lines, none when it was never written.
fn logged(repo: &Path) -> std::io::Result<Vec<String>> {
    match std::fs::read_to_string(repo.join(".git/weighin/hook-errors.log")) {
        Ok(log) => Ok(log.lines().map(str::to_owned).collect()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e),
    }
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_write_to_an_anchored_file_adds_the_newest_decisions_first() -> TestResult {
    let (_tmp, repo) = setup();
    let older = decided(&repo, "Older, nearer the top?", "src/retention.rs:7", "A")?;
    let newer = decided(&repo, "Newer, further down?", "src/retention.rs:40", "A")?;
    let text = std::fs::read_to_string(repo.join("src/retention.rs"))?;

    let printed = hook(&repo, &write(&repo, "src/retention.rs", &text));

    let lines = added_context(&printed)?.ok_or("no context added")?;
    let shown: Vec<&str> = lines
        .iter()
        .filter_map(|l| l.split_whitespace().next())
        .collect();
    assert_eq!(shown, [newer.as_str(), older.as_str()]);
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_ledger_that_cannot_be_read_after_an_edit_prints_nothing_and_is_logged() -> TestResult {
    let (_tmp, repo) = setup();
    // A directory where the store should be: it exists, so it is opened, and the open fails.
    std::fs::create_dir_all(repo.join(".git/weighin/ledger.sqlite3"))?;

    let printed = hook(&repo, &edit(&repo, "src/retention.rs", "0\n"));

    assert_eq!(printed, "");
    let log = logged(&repo)?;
    assert!(
        log.len() == 1 && log.iter().all(|l| l.contains("PostToolUse: ")),
        "{log:?}"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_ledger_that_cannot_be_read_is_logged_once_for_the_journal_and_the_recall() -> TestResult {
    let (_tmp, repo) = setup();
    let (ok, out) = weighin(&repo, "s1", &["init", "--class", "own"], None);
    assert!(ok, "{out}");
    std::fs::create_dir_all(repo.join(".git/weighin/ledger.sqlite3"))?;

    let printed = hook(&repo, &edit(&repo, "src/retention.rs", "0\n"));

    assert_eq!(printed, "");
    let log = logged(&repo)?;
    assert_eq!(log.len(), 1, "one read, one failure: {log:?}");
    Ok(())
}
