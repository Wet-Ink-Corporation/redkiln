//! `bench hook standing-orders`, the `SubagentStart` hook, through the built binary: the
//! repository's `.anneal/standing-orders.md` reaches every subagent's context, and the hook
//! never blocks.

use std::error::Error;
use std::fs;
use std::io::Write as _;
use std::path::Path;
use std::process::{Output, Stdio};

use serde_json::{Value, json};
use tempfile::TempDir;

use crate::support::{anneal_in, git};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const ORDERS: &str = "# Standing orders\n\n- Never push to main.\n- Report with a Status block.\n";

/// A repository, with `orders` as its standing orders when given, and a `src/` directory.
fn repo(orders: Option<&str>) -> TestResult<TempDir> {
    let dir = tempfile::tempdir()?;
    git(dir.path(), &["init", "-q"]);
    fs::create_dir_all(dir.path().join("src"))?;
    if let Some(orders) = orders {
        fs::create_dir_all(dir.path().join(".anneal"))?;
        fs::write(
            dir.path().join(".anneal").join("standing-orders.md"),
            orders,
        )?;
    }
    Ok(dir)
}

/// Runs the hook as Claude Code does when a subagent starts in `cwd`.
fn start(cwd: &Path) -> TestResult<Output> {
    let input = json!({
        "session_id": "s1",
        "transcript_path": cwd.join("t.jsonl"),
        "cwd": cwd,
        "hook_event_name": "SubagentStart",
        "agent_id": "agent-abc123",
        "agent_type": "Explore",
    });
    let mut child = anneal_in(cwd)
        .args(["hook", "standing-orders"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut stdin = child.stdin.take().ok_or("stdin is piped")?;
    stdin.write_all(&serde_json::to_vec(&input)?)?;
    drop(stdin);
    let output = child.wait_with_output()?;
    assert_eq!(output.status.code(), Some(0), "fails open: {output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    Ok(output)
}

#[test]
#[cfg_attr(miri, ignore = "runs git and the binary")]
fn the_standing_orders_appear_in_the_subagent_context() -> TestResult {
    let dir = repo(Some(ORDERS))?;

    let output = start(&dir.path().join("src"))?;

    let written: Value = serde_json::from_slice(&output.stdout)?;
    let specific = &written["hookSpecificOutput"];
    assert_eq!(
        specific["hookEventName"],
        json!("SubagentStart"),
        "{written}"
    );
    let context = specific["additionalContext"]
        .as_str()
        .ok_or("additionalContext")?;
    assert!(
        context.contains("- Never push to main.\n- Report with a Status block.\n"),
        "the file, found from the worktree root: {context}"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "runs git and the binary")]
fn without_standing_orders_the_hook_is_silent() -> TestResult {
    let dir = repo(None)?;

    let output = start(dir.path())?;

    assert!(output.stdout.is_empty(), "{output:?}");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "runs git and the binary")]
fn standing_orders_over_16_kib_are_one_error_line_and_not_injected() -> TestResult {
    let dir = repo(Some(&"x".repeat(16 * 1024 + 1)))?;

    let output = start(dir.path())?;

    let written: Value = serde_json::from_slice(&output.stdout)?;
    let message = written["systemMessage"].as_str().ok_or("a systemMessage")?;
    assert!(
        message.starts_with("ERROR standing-orders: ") && message.contains("16384"),
        "{message}"
    );
    assert_eq!(
        written.get("hookSpecificOutput"),
        None,
        "nothing injected: {written}"
    );
    Ok(())
}

#[cfg(unix)]
#[test]
#[cfg_attr(miri, ignore = "runs git and the binary")]
fn standing_orders_through_a_symlinked_anneal_directory_are_one_error_line_and_not_injected()
-> TestResult {
    let dir = repo(None)?;
    let outside = tempfile::tempdir()?;
    fs::write(outside.path().join("standing-orders.md"), ORDERS)?;
    std::os::unix::fs::symlink(outside.path(), dir.path().join(".anneal"))?;

    let output = start(dir.path())?;

    let written: Value = serde_json::from_slice(&output.stdout)?;
    let expected = format!(
        "ERROR standing-orders: {} resolves to {}, not this worktree's own \
         .anneal/standing-orders.md; make .anneal a plain directory",
        dir.path()
            .canonicalize()?
            .join(".anneal")
            .join("standing-orders.md")
            .display(),
        outside
            .path()
            .canonicalize()?
            .join("standing-orders.md")
            .display()
    );
    assert_eq!(written["systemMessage"], json!(expected), "{written}");
    assert_eq!(
        written.get("hookSpecificOutput"),
        None,
        "nothing injected: {written}"
    );
    Ok(())
}

/// Writes `orders` as `relative` (`/`-separated) under `root`.
fn write_orders(root: &Path, relative: &str, orders: &str) -> TestResult {
    let path = relative
        .split('/')
        .fold(root.to_path_buf(), |path, part| path.join(part));
    fs::create_dir_all(path.parent().ok_or("a parent")?)?;
    fs::write(path, orders)?;
    Ok(())
}

/// The `additionalContext` the hook wrote.
fn context(output: &Output) -> TestResult<String> {
    let written: Value = serde_json::from_slice(&output.stdout)?;
    Ok(written
        .pointer("/hookSpecificOutput/additionalContext")
        .and_then(Value::as_str)
        .ok_or("additionalContext")?
        .to_owned())
}

#[test]
#[cfg_attr(miri, ignore = "runs git and the binary")]
fn standing_orders_in_bench_s_folder_are_injected_naming_it() -> TestResult {
    let dir = repo(None)?;
    write_orders(dir.path(), ".redkiln/bench/standing-orders.md", ORDERS)?;

    let context = context(&start(&dir.path().join("src"))?)?;

    assert_eq!(
        context,
        format!(
            "Standing orders for every subagent (.redkiln/bench/standing-orders.md):\n\n{ORDERS}"
        )
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "runs git and the binary")]
fn with_both_folders_the_orders_in_bench_s_are_injected_and_anneal_s_left_alone() -> TestResult {
    let dir = repo(Some("old orders\n"))?;
    write_orders(dir.path(), ".redkiln/bench/standing-orders.md", ORDERS)?;

    let context = context(&start(dir.path())?)?;

    assert_eq!(
        (
            context.contains(ORDERS),
            context.contains("old orders"),
            fs::read_to_string(dir.path().join(".anneal").join("standing-orders.md"))?
        ),
        (true, false, "old orders\n".to_owned())
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "runs git and the binary")]
fn anneal_s_standing_orders_alone_are_injected_naming_them() -> TestResult {
    let dir = repo(Some(ORDERS))?;

    let context = context(&start(dir.path())?)?;

    assert_eq!(
        context,
        format!("Standing orders for every subagent (.anneal/standing-orders.md):\n\n{ORDERS}")
    );
    Ok(())
}

#[cfg(unix)]
#[test]
#[cfg_attr(miri, ignore = "runs git and the binary")]
fn standing_orders_through_a_symlinked_bench_directory_are_not_injected() -> TestResult {
    let dir = repo(None)?;
    let outside = tempfile::tempdir()?;
    fs::write(outside.path().join("standing-orders.md"), ORDERS)?;
    fs::create_dir_all(dir.path().join(".redkiln"))?;
    std::os::unix::fs::symlink(outside.path(), dir.path().join(".redkiln").join("bench"))?;

    let output = start(dir.path())?;

    let written: Value = serde_json::from_slice(&output.stdout)?;
    let message = written["systemMessage"].as_str().ok_or("a systemMessage")?;
    assert_eq!(
        (
            message.ends_with(
                "not this worktree's own .redkiln/bench/standing-orders.md; make .redkiln/bench \
                 a plain directory"
            ),
            written.get("hookSpecificOutput"),
        ),
        (true, None),
        "{written}"
    );
    Ok(())
}
