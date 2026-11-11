//! The one-time import of anneal ledgers, through the binary (Move 2 step 3.5, plan test list 32,
//! 33, 40 and 42): `SessionStart` and a recording command import an old ledger once and leave
//! it as it was; `$ANNEAL_STORE`'s ledger goes into the store beside it; `bench status` reports
//! what is due and imports nothing.

use std::error::Error;
use std::io::Write as _;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use redkiln_core::store::{Filter, Store, Wait};
use serde_json::Value;

use crate::support::{anneal_in, git};

type TestResult = Result<(), Box<dyn Error>>;

const TASK_DONE: &str = r#"{"type":"bench.task_done","data":{"task":"T1"}}"#;

/// The committed fixture `name`.
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/anneal")
        .join(name)
}

/// `bench` in `cwd`, git reading only the repository's own configuration, so the owner is
/// what the test set (here none, unless a test sets one).
fn bench(cwd: &Path) -> Command {
    let mut command = anneal_in(cwd);
    command
        .env("GIT_CONFIG_GLOBAL", cwd.join("no-global-gitconfig"))
        .env("GIT_CONFIG_NOSYSTEM", "1");
    command
}

/// A scratch repository whose `.git/anneal/ledger.sqlite3` is a copy of the 0.0.5 fixture,
/// and that directory.
fn repository() -> Result<(tempfile::TempDir, PathBuf), Box<dyn Error>> {
    let work = tempfile::tempdir()?;
    git(work.path(), &["init", "-q"]);
    let dir = work.path().join(".git/anneal");
    std::fs::create_dir_all(&dir)?;
    std::fs::copy(fixture("anneal-0.0.5.sqlite3"), dir.join("ledger.sqlite3"))?;
    Ok((work, dir))
}

/// A file's name and bytes.
type Snapshot = Vec<(String, Vec<u8>)>;

/// Every file in `dir` with its bytes, sorted by name.
fn snapshot(dir: &Path) -> Result<Snapshot, Box<dyn Error>> {
    let mut files = std::fs::read_dir(dir)?
        .map(|entry| {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            Ok((name, std::fs::read(entry.path())?))
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    files.sort();
    Ok(files)
}

/// The type of every event in the store at `path`, in position order.
fn types(path: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let store = Store::open_existing(path, Wait::Command)?.ok_or("the store")?;
    let limit = NonZeroUsize::new(4096).ok_or("non-zero")?;
    Ok(store
        .read(&Filter::all(), 0, limit)?
        .into_iter()
        .map(|stored| stored.event_type)
        .collect())
}

/// How many of `types` are Bench's, and how many import records.
fn counts(types: &[String]) -> (usize, usize) {
    (
        types.iter().filter(|t| t.starts_with("bench.")).count(),
        types
            .iter()
            .filter(|t| t.as_str() == "core.import_completed")
            .count(),
    )
}

/// `bench hook bootstrap` in `cwd` with a `SessionStart` input naming it: its context.
fn session_start(cwd: &Path) -> Result<String, Box<dyn Error>> {
    let mut child = bench(cwd)
        .args(["hook", "bootstrap"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let input = serde_json::json!({
        "session_id": "s1", "transcript_path": "/dev/null", "cwd": cwd,
        "hook_event_name": "SessionStart", "source": "startup",
    });
    let mut stdin = child.stdin.take().ok_or("stdin is piped")?;
    stdin.write_all(&serde_json::to_vec(&input)?)?;
    drop(stdin);
    let output = child.wait_with_output()?;
    assert_eq!(output.status.code(), Some(0), "fails open: {output:?}");
    let json: Value = serde_json::from_slice(&output.stdout)?;
    Ok(json
        .pointer("/hookSpecificOutput/additionalContext")
        .and_then(Value::as_str)
        .ok_or("additionalContext")?
        .to_owned())
}

/// `bench ledger append` of [`TASK_DONE`] in `cwd`.
fn append(cwd: &Path) -> Result<Output, Box<dyn Error>> {
    Ok(bench(cwd).args(["ledger", "append", TASK_DONE]).output()?)
}

#[test]
#[cfg_attr(miri, ignore = "runs git and the binary")]
fn session_start_imports_an_old_ledger_once_and_a_later_run_appends_nothing() -> TestResult {
    let (work, old) = repository()?;
    let before = snapshot(&old)?;
    let store = work.path().join(".git/redkiln/events.sqlite3");

    let said = session_start(work.path())?;

    assert!(
        said.contains("is left as it is: its events are imported into the suite store, once")
            && said.contains("22 imported, 0 already there, 1 skipped"),
        "SessionStart says what it imported: {said}"
    );
    assert!(
        said.contains("actor \"test\""),
        "and the refused actor: {said}"
    );
    assert_eq!(counts(&types(&store)?), (22, 1), "22 events, one record");
    assert_eq!(snapshot(&old)?, before, "the ledger is left as it was");

    let again = session_start(work.path())?;
    let appended = append(work.path())?;

    assert!(
        !again.contains("anneal ledger"),
        "nothing due, nothing said: {again}"
    );
    let stderr = String::from_utf8(appended.stderr)?;
    assert!(
        !stderr.contains("anneal ledger"),
        "nor by a command: {stderr}"
    );
    assert_eq!(
        counts(&types(&store)?),
        (23, 1),
        "only the command's own event"
    );
    assert_eq!(snapshot(&old)?, before, "still as it was");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "runs git and the binary")]
fn a_recording_command_imports_with_one_warning_and_its_problems_beneath() -> TestResult {
    let (work, old) = repository()?;
    let before = snapshot(&old)?;

    let output = append(work.path())?;

    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stderr = String::from_utf8(output.stderr)?;
    let warnings: Vec<&str> = stderr
        .lines()
        .filter(|line| line.starts_with("bench: warning:"))
        .collect();
    assert!(
        matches!(warnings.as_slice(), [one] if one.contains("22 imported")),
        "one warning for the source: {stderr}"
    );
    assert!(
        warnings
            .iter()
            .any(|line| line.contains("; record ") && line.contains("actor \"test\"")),
        "the refused actor after it, on its line (CLI-08): {stderr}"
    );
    let store = work.path().join(".git/redkiln/events.sqlite3");
    assert_eq!(counts(&types(&store)?), (23, 1), "imported, then appended");
    assert_eq!(snapshot(&old)?, before, "the ledger is left as it was");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "runs the binary")]
fn the_ledger_anneal_store_names_is_imported_into_the_store_beside_it() -> TestResult {
    let dir = tempfile::tempdir()?;
    let old = dir.path().join("a.sqlite3");
    std::fs::copy(fixture("anneal-older.sqlite3"), &old)?;

    let output = bench(dir.path())
        .env("ANNEAL_STORE", &old)
        .args(["ledger", "append", TASK_DONE])
        .output()?;

    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(
        counts(&types(&dir.path().join("a.events.sqlite3"))?),
        (8, 1),
        "the 7 old events and the command's"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "runs git and the binary")]
fn status_reports_an_old_ledger_while_it_is_due_and_imports_nothing() -> TestResult {
    let (work, _old) = repository()?;
    let status = |work: &Path| -> Result<Value, Box<dyn Error>> {
        let output = bench(work).args(["status", "--json"]).output()?;
        crate::support::envelope(&output)
    };
    let keys = |envelope: &Value| -> Vec<String> {
        envelope["findings"]
            .as_array()
            .map(|findings| {
                findings
                    .iter()
                    .filter_map(|f| f["key"].as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    };

    let due = status(work.path())?;

    assert!(
        keys(&due).contains(&"ledger/old-ledger".to_owned()),
        "reported: {due}"
    );
    assert!(
        !work.path().join(".git/redkiln").exists(),
        "status imported nothing and created no store"
    );
    assert_eq!(append(work.path())?.status.code(), Some(0));
    let imported = status(work.path())?;
    assert!(
        !keys(&imported).contains(&"ledger/old-ledger".to_owned()),
        "not once imported: {imported}"
    );
    Ok(())
}
