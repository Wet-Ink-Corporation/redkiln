//! Bench on the suite store, through the binary (Move 2 step 3, plan test list 26 to 29): an
//! anneal ledger is left exactly as it was and warned about, never opened; a file Core refuses
//! is named with the import hint; reads and hooks create no store; `bench ledger journal`
//! prints lines Core imports.

use std::error::Error;
use std::io::Write as _;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};

use redkiln_core::journal::read_line;
use redkiln_core::store::{NewEvent, Store, Wait};
use serde_json::Value;

use crate::support::{anneal_in, git};

type TestResult = Result<(), Box<dyn Error>>;

/// The old ledgers a scratch repository is given, each with the files that travel with it.
const LEDGERS: [&[&str]; 5] = [
    &["anneal-0.0.5.sqlite3"],
    &["anneal-older.sqlite3"],
    &["anneal-wal.sqlite3", "anneal-wal.sqlite3-wal"],
    &["bench-ledger-hs032.sqlite3"],
    &["bench-ledger-hs060.sqlite3"],
];

const TASK_DONE: &str = r#"{"type":"task_done","task":"T1"}"#;

/// The committed fixture `name`.
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/anneal")
        .join(name)
}

/// A scratch repository whose `.git/anneal/ledger.sqlite3` is a copy of `files`' first fixture
/// (the others beside it under the same suffix): the old ledger and its directory.
fn repository_with(files: &[&str]) -> Result<(tempfile::TempDir, PathBuf), Box<dyn Error>> {
    let work = tempfile::tempdir()?;
    git(work.path(), &["init", "-q"]);
    let dir = work.path().join(".git/anneal");
    std::fs::create_dir_all(&dir)?;
    let first = files.first().ok_or("a fixture")?;
    for name in files {
        let suffix = name.strip_prefix(first).ok_or("a sibling of the first")?;
        std::fs::copy(fixture(name), dir.join(format!("ledger.sqlite3{suffix}")))?;
    }
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

/// `bench ledger <args>` in `cwd`.
fn ledger(cwd: &Path, args: &[&str]) -> Result<Output, Box<dyn Error>> {
    Ok(anneal_in(cwd).arg("ledger").args(args).output()?)
}

/// `bench hook bootstrap` in `cwd` with a `SessionStart` input naming it.
fn bootstrap(cwd: &Path, store: Option<&Path>) -> Result<Output, Box<dyn Error>> {
    let mut command = anneal_in(cwd);
    command
        .args(["hook", "bootstrap"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(store) = store {
        command.env("REDKILN_STORE", store);
    }
    let mut child = command.spawn()?;
    let input = serde_json::json!({
        "session_id": "s1", "transcript_path": "/dev/null", "cwd": cwd,
        "hook_event_name": "SessionStart", "source": "startup",
    });
    let mut stdin = child.stdin.take().ok_or("stdin is piped")?;
    stdin.write_all(&serde_json::to_vec(&input)?)?;
    drop(stdin);
    Ok(child.wait_with_output()?)
}

/// The `SessionStart` context a bootstrap's output carries.
fn context(output: &Output) -> Result<String, Box<dyn Error>> {
    let json: Value = serde_json::from_slice(&output.stdout)?;
    Ok(json
        .pointer("/hookSpecificOutput/additionalContext")
        .and_then(Value::as_str)
        .ok_or("additionalContext")?
        .to_owned())
}

#[test]
#[cfg_attr(miri, ignore = "runs git and the binary")]
fn a_write_beside_an_old_ledger_lands_in_the_suite_store_and_leaves_the_ledger_untouched()
-> TestResult {
    for files in LEDGERS {
        let (work, old) = repository_with(files)?;
        let before = snapshot(&old)?;

        let output = ledger(work.path(), &["append", TASK_DONE])?;

        assert_eq!(output.status.code(), Some(0), "{files:?}: {output:?}");
        assert!(
            work.path().join(".git/redkiln/events.sqlite3").is_file(),
            "{files:?}: the suite store"
        );
        assert_eq!(
            snapshot(&old)?,
            before,
            "{files:?}: bytes and listing unchanged"
        );
        let stderr = String::from_utf8(output.stderr)?;
        let warnings: Vec<&str> = stderr
            .lines()
            .filter(|line| line.starts_with("bench: warning:"))
            .collect();
        assert!(
            matches!(warnings.as_slice(), [one] if one.contains("ledger.sqlite3") && one.contains("left as it is")),
            "{files:?}: one warning names it: {stderr}"
        );
    }
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "runs git and the binary")]
fn a_store_flag_or_variable_naming_an_old_ledger_is_refused_with_the_import_hint() -> TestResult {
    for files in LEDGERS {
        let (work, old) = repository_with(files)?;
        let before = snapshot(&old)?;
        let path = old.join("ledger.sqlite3");

        let flagged = anneal_in(work.path())
            .args(["ledger", "--store"])
            .arg(&path)
            .args(["query"])
            .output()?;
        let variable = anneal_in(work.path())
            .args(["ledger", "query"])
            .env("REDKILN_STORE", &path)
            .output()?;

        for output in [&flagged, &variable] {
            assert_ne!(
                output.status.code(),
                Some(0),
                "{files:?}: refused: {output:?}"
            );
            let said = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                said.contains("bench imports an anneal ledger rather than opening it"),
                "{files:?}: the hint: {said}"
            );
        }
        assert_eq!(snapshot(&old)?, before, "{files:?}: untouched");
    }
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "runs git and the binary")]
fn a_hook_given_an_old_ledger_as_the_store_fails_open_and_leaves_it() -> TestResult {
    let (work, old) = repository_with(&["anneal-0.0.5.sqlite3"])?;
    let before = snapshot(&old)?;

    let output = bootstrap(work.path(), Some(&old.join("ledger.sqlite3")))?;

    assert_eq!(output.status.code(), Some(0), "fails open: {output:?}");
    let said = context(&output)?;
    assert!(
        said.contains("The ledger could not be read: ") && said.contains("rather than opening it"),
        "says why: {said}"
    );
    assert_eq!(snapshot(&old)?, before, "untouched");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "runs git and the binary")]
fn reads_and_session_start_create_no_store_beside_an_old_ledger_or_nothing() -> TestResult {
    let (with_old, _) = repository_with(&["anneal-0.0.5.sqlite3"])?;
    let empty = tempfile::tempdir()?;
    git(empty.path(), &["init", "-q"]);

    for work in [with_old.path(), empty.path()] {
        let query = ledger(work, &["query"])?;
        let tail = ledger(work, &["tail"])?;
        let journal = ledger(work, &["journal"])?;
        let status = anneal_in(work).args(["status", "--json"]).output()?;

        for output in [&query, &tail, &journal] {
            assert_eq!(output.status.code(), Some(0), "{output:?}");
        }
        assert_eq!(status.status.code(), Some(0), "no store: {status:?}");
        assert!(
            !work.join(".git/redkiln").exists(),
            "{}: neither the store nor its directory",
            work.display()
        );
    }
    // SessionStart creates nothing where there is nothing to import; beside an old ledger it
    // imports it (step 3.5, Ryan's call 2026-11-11).
    let started = bootstrap(empty.path(), None)?;
    assert_eq!(started.status.code(), Some(0), "{started:?}");
    assert!(
        !empty.path().join(".git/redkiln").exists(),
        "SessionStart with nothing to import creates neither the store nor its directory"
    );
    let said = context(&bootstrap(with_old.path(), None)?)?;
    assert!(
        said.contains("the anneal ledger")
            && said.contains("is left as it is: its events are imported"),
        "SessionStart says it imported the old ledger: {said}"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "runs git and the binary")]
fn the_journal_prints_one_importable_line_per_bench_event_and_no_other() -> TestResult {
    let work = tempfile::tempdir()?;
    let store = work.path().join("events.sqlite3");
    for event in [TASK_DONE, r#"{"type":"grill_confirmed","plan":"p.md"}"#] {
        let output = anneal_in(work.path())
            .args(["ledger", "--store"])
            .arg(&store)
            .args(["append", event])
            .output()?;
        assert_eq!(output.status.code(), Some(0), "{output:?}");
    }
    let foreign = br#"{"type":"weighin.item_raised","version":1,"ts":"2026-11-10T09:30:00Z","actor":"cli","trust":{"level":"asserted"},"writer":{"binary":"weighin","version":"2.0.0","spec":"1.0.0"},"data":{"item":"wi-3fa9c01b2e"}}"#;
    Store::open(&store, Wait::Command)?.append(
        vec![NewEvent::new("weighin.item_raised", foreign.to_vec())?],
        None,
    )?;

    let output = anneal_in(work.path())
        .args(["ledger", "--store"])
        .arg(&store)
        .arg("journal")
        .output()?;

    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let types = output
        .stdout
        .split_inclusive(|byte| *byte == b'\n')
        .map(|line| {
            let text = line
                .strip_suffix(b"\n")
                .ok_or("each line ends in \\n (JR-01)")?;
            Ok(read_line(text)?.event_type)
        })
        .collect::<Result<Vec<String>, Box<dyn Error>>>()?;
    assert_eq!(
        types,
        ["bench.task_done", "bench.grill_confirmed"],
        "JR-02 lines Core reads back, Bench's only"
    );
    let stored = Store::open_existing(&store, Wait::Command)?
        .ok_or("the store")?
        .read(
            &redkiln_core::store::Filter::all(),
            0,
            NonZeroUsize::new(8).ok_or("non-zero")?,
        )?;
    assert_eq!(
        stored.len(),
        3,
        "the foreign event is in the store, not the journal"
    );
    Ok(())
}
