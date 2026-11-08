//! The relay workflow, `.github/workflows/redkiln-journal-relay.yml` (SPEC JR-07, D8, D13): it
//! runs on inbox pushes with the least the token needs, its actions are the ones CI pins, and
//! the command it runs is one the binary accepts.

use crate::scratch::{Repo, TestResult};
use serde_json::Value;
use std::path::Path;

/// The workflow's text.
fn workflow() -> TestResult<String> {
    read_workflow("redkiln-journal-relay.yml")
}

/// The text of `.github/workflows/<name>`.
fn read_workflow(name: &str) -> TestResult<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../.github/workflows")
        .join(name);
    Ok(std::fs::read_to_string(path)?)
}

/// The lines of the top-level block `key:`, up to the next unindented line.
fn block<'a>(text: &'a str, key: &str) -> Vec<&'a str> {
    text.lines()
        .skip_while(|line| *line != key)
        .skip(1)
        .take_while(|line| line.is_empty() || line.starts_with(' '))
        .filter(|line| !line.trim().is_empty())
        .collect()
}

/// What follows `uses: ` on each line that names an action, comment dropped.
fn actions(text: &str) -> Vec<&str> {
    text.lines()
        .filter_map(|line| {
            line.trim_start()
                .trim_start_matches("- ")
                .strip_prefix("uses: ")
        })
        .map(|rest| rest.split_whitespace().next().unwrap_or(rest))
        .collect()
}

#[test]
fn the_relay_runs_on_inbox_pushes_with_contents_write_only() -> TestResult {
    let text = workflow()?;

    assert_eq!(
        block(&text, "on:"),
        ["  push:", "    branches: ['redkiln/inbox/**']"],
        "inbox pushes only"
    );
    assert_eq!(block(&text, "permissions:"), ["  contents: write"]);
    assert_eq!(
        text.matches("permissions:").count(),
        1,
        "no job widens the token"
    );
    assert_eq!(
        block(&text, "concurrency:"),
        [
            "  group: redkiln-journal-relay-${{ github.ref }}",
            "  cancel-in-progress: false"
        ],
        "one run per branch at a time, none cancelled"
    );
    assert!(!text.contains("secrets."), "GITHUB_TOKEN only");
    let used = actions(&text);
    assert!(!used.is_empty(), "the workflow uses actions");
    let ci = read_workflow("ci.yml")?;
    let pinned = actions(&ci);
    for action in used {
        let sha = action.rsplit_once('@').map_or("", |(_, sha)| sha);
        assert!(
            sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit()),
            "{action} is pinned by a full commit sha"
        );
        assert!(
            pinned.contains(&action),
            "{action} is pinned as ci.yml pins it"
        );
    }
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn the_workflows_relay_command_is_one_the_binary_accepts() -> TestResult {
    let text = workflow()?;
    let run = text
        .lines()
        .filter_map(|line| {
            line.trim_start()
                .trim_start_matches("- ")
                .strip_prefix("run: ")
        })
        .find(|run| run.contains(" journal relay "))
        .ok_or("a step runs the relay")?;
    let mut words = run.split_whitespace();
    let program = words.next().ok_or("a program")?;
    assert_eq!(program, "target/debug/weighin", "the binary the step built");
    // The shell's part: the quotes go and the branch variable is the inbox the push named.
    let args: Vec<String> = words
        .map(|word| match word.trim_matches('"') {
            "$GITHUB_REF_NAME" => "redkiln/inbox/x".to_owned(),
            other => other.to_owned(),
        })
        .collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let (repo, _remote) = Repo::with_remote(false)?;

    let out = repo.run_in(&repo.root, &args)?;

    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let envelope: Value = serde_json::from_slice(&out.stdout)?;
    assert_eq!(envelope["data"]["outcome"], "gone", "{envelope}");
    assert_eq!(envelope["data"]["branch"], "redkiln/inbox/x", "{envelope}");
    Ok(())
}
