//! Bench's per-user state through the built binary (PR 5.6): written to `<root>/bench/` in the
//! user scope (`$REDKILN_HOME`), read from anneal's home (`$ANNEAL_HOME`) while absent there, and
//! never written in anneal's home.

use std::error::Error;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};

use serde_json::{Value, json};

use crate::support::anneal_in;

type TestResult = Result<(), Box<dyn Error>>;

/// What the bootstrap's setup hint says.
const HINT: &str = "the context monitor is off";

/// A reading 130k of 200k tokens (65%), past the warning.
const PAST_THE_WARNING: &[u8] =
    br#"{"session_id":"s1","context_window":{"used_percentage":65,"context_window_size":200000}}"#;

/// Runs `bench <args>` in `dir` with `stdin`, the user scope's root `root` and anneal's home
/// `anneal`.
fn bench(
    dir: &Path,
    homes: (&Path, &Path),
    args: &[&str],
    stdin: &[u8],
) -> Result<Output, Box<dyn Error>> {
    let (root, anneal) = homes;
    let mut child = anneal_in(dir)
        .args(args)
        .env("REDKILN_HOME", root)
        .env("ANNEAL_HOME", anneal)
        .env("REDKILN_STORE", dir.join("ledger.sqlite3"))
        .env_remove("CLAUDE_CODE_AUTO_COMPACT_WINDOW")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut input = child.stdin.take().ok_or("stdin is piped")?;
    match input.write_all(stdin) {
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => {}
        written => written?,
    }
    drop(input);
    Ok(child.wait_with_output()?)
}

/// Every file under `dir`, relative to it, sorted; empty when `dir` does not exist.
fn files(dir: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        let entries = match std::fs::read_dir(&next) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        for entry in entries {
            let path = entry?.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                found.push(path.strip_prefix(dir)?.to_path_buf());
            }
        }
    }
    found.sort();
    Ok(found)
}

#[test]
#[cfg_attr(miri, ignore = "runs the binary")]
fn the_status_line_saves_in_bench_s_state_folder_and_nothing_in_anneal_s_home() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (root, anneal) = (dir.path().join("root"), dir.path().join("anneal"));

    let output = bench(
        dir.path(),
        (&root, &anneal),
        &["statusline"],
        PAST_THE_WARNING,
    )?;

    assert_eq!(
        (
            output.status.code(),
            files(&root.join("bench"))?,
            files(&anneal)?
        ),
        (Some(0), vec![PathBuf::from("context-usage.json")], vec![]),
        "{output:?}"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "runs the binary")]
fn the_monitor_reads_anneal_s_snapshot_and_keeps_its_level_in_bench_s_folder() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (root, old_root) = (dir.path().join("root"), dir.path().join("old"));
    // A status line run with the user scope at `old` leaves its snapshot in `old/bench`, which
    // then plays anneal's home.
    let anneal = old_root.join("bench");
    bench(
        dir.path(),
        (&old_root, &old_root),
        &["statusline"],
        PAST_THE_WARNING,
    )?;
    let before = std::fs::read(anneal.join("context-usage.json"))?;

    let monitor = bench(
        dir.path(),
        (&root, &anneal),
        &["hook", "context-monitor"],
        br#"{"session_id":"s1","hook_event_name":"PostToolBatch"}"#,
    )?;

    let warned: Value = serde_json::from_slice(&monitor.stdout)?;
    assert!(
        warned
            .pointer("/hookSpecificOutput/additionalContext")
            .and_then(Value::as_str)
            .is_some_and(|text| text.contains("65%")),
        "anneal's snapshot is judged: {warned}"
    );
    assert_eq!(
        (
            files(&root.join("bench"))?,
            files(&anneal)?,
            std::fs::read(anneal.join("context-usage.json"))?
        ),
        (
            vec![PathBuf::from("context-monitor.json")],
            vec![PathBuf::from("context-usage.json")],
            before
        ),
        "the level is kept in bench's folder; anneal's home is left as it was"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "runs the binary")]
fn a_hint_shown_under_anneal_is_not_shown_again_and_a_new_one_is_recorded_in_bench_s_folder()
-> TestResult {
    let (shown, fresh) = (tempfile::tempdir()?, tempfile::tempdir()?);
    let marker = json!({"kind": "anneal-statusline-hint", "version": 1}).to_string();
    let shown_anneal = shown.path().join("anneal");
    std::fs::create_dir(&shown_anneal)?;
    std::fs::write(shown_anneal.join("statusline-hint.json"), &marker)?;
    let start = |dir: &Path| -> Result<String, Box<dyn Error>> {
        let input = json!({"session_id": "s2", "cwd": dir, "hook_event_name": "SessionStart"});
        let homes = (dir.join("root"), dir.join("anneal"));
        let output = bench(
            dir,
            (&homes.0, &homes.1),
            &["hook", "bootstrap"],
            &serde_json::to_vec(&input)?,
        )?;
        let json: Value = serde_json::from_slice(&output.stdout)?;
        Ok(json
            .pointer("/hookSpecificOutput/additionalContext")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned())
    };

    let (again, first) = (start(shown.path())?, start(fresh.path())?);

    assert_eq!(
        (
            again.contains(HINT),
            first.contains(HINT),
            files(&shown.path().join("anneal"))?,
            files(&shown.path().join("root"))?,
            files(&fresh.path().join("root").join("bench"))?,
            files(&fresh.path().join("anneal"))?,
        ),
        (
            false,
            true,
            vec![PathBuf::from("statusline-hint.json")],
            vec![],
            vec![PathBuf::from("statusline-hint.json")],
            vec![],
        ),
        "{again}\n---\n{first}"
    );
    Ok(())
}
