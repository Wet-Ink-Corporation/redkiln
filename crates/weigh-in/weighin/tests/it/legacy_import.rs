//! The one-time import of weighin 1.x sources (Move 1 step 8, ST-15, JR-07, M1-A): a ledger and
//! its committed journals imported once by the first command or `SessionStart`, ledgers first; a
//! second run appends nothing; the sources and their directories are left as they were; and
//! `ci-check` imports nothing.

use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use redkiln_core::envelope::{Payload as _, decode_all};
use redkiln_core::journal::{ImportCompleted, ImportSource};
use redkiln_core::store::{Filter, Store, Stored, Wait};
use serde_json::{Value, json};

use crate::scratch::{Repo, TestResult};
use crate::support::{Run, store_of, weighin_env};

/// The 1.x ledgers Core keeps as fixtures, one per journal mode and layout 1.x wrote.
const FIXTURES: [&str; 3] = [
    "weighin-0.1.x.sqlite3",
    "weighin-0.2.2.sqlite3",
    "weighin-1.2.0.sqlite3",
];

/// The item each fixture leaves open.
const OPEN_IN_FIXTURE: &str = "wi-930e15";

/// The store id of a 1.x journal line no ledger event matches (M1-A).
const JOURNAL_STORE: &str = "8f161dc3d6d01eac2c7949e3b17ffa04";

/// A 1.x journal line of no fixture.
const OWN_LINE: &str = r#"{"type":"item_presented","item_id":"wi-b29e54","surface":"chat","reason":"shown","actor":"cli","session_id":"s1","worktree":"repo","ts":"2026-10-09T03:29:37Z"}"#;

/// The bytes of Core's fixture `name`.
fn fixture(name: &str) -> TestResult<Vec<u8>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../core/redkiln-core/tests/fixtures")
        .join(name);
    Ok(std::fs::read(path)?)
}

/// The events of fixture `name`, read by Core from a copy in a scratch directory.
fn fixture_events(name: &str) -> TestResult<(String, Vec<Stored>)> {
    let dir = tempfile::tempdir()?;
    let copy = dir.path().join(name);
    std::fs::write(&copy, fixture(name)?)?;
    let ledger = redkiln_core::legacy::read_ledger(&copy)?;
    Ok((ledger.store_id.to_string(), ledger.events))
}

/// A repository with fixture `name` at 1.x's default place; that path.
fn with_old_ledger(repo: &Repo, name: &str) -> TestResult<PathBuf> {
    let path = repo.root.join(".git/weighin/ledger.sqlite3");
    std::fs::create_dir_all(repo.root.join(".git/weighin"))?;
    std::fs::write(&path, fixture(name)?)?;
    Ok(path)
}

/// Writes `lines` as the committed 1.x journal `<repo>/.kb/_ledger/journal/<name>`; its path.
fn with_journal(root: &Path, name: &str, lines: &[&[u8]]) -> TestResult<PathBuf> {
    let dir = root.join(".kb/_ledger/journal");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(name);
    let mut bytes = Vec::new();
    for line in lines {
        bytes.extend_from_slice(line);
        bytes.push(b'\n');
    }
    std::fs::write(&path, bytes)?;
    Ok(path)
}

/// The sorted names in `file`'s directory, and its bytes, to show neither changed.
fn snapshot(file: &Path) -> TestResult<(Vec<String>, Vec<u8>)> {
    let dir = file.parent().ok_or("a file is in a directory")?;
    let mut names = std::fs::read_dir(dir)?
        .map(|entry| Ok(entry?.file_name().to_string_lossy().into_owned()))
        .collect::<std::io::Result<Vec<_>>>()?;
    names.sort();
    Ok((names, std::fs::read(file)?))
}

/// Every event of the repository's store; the store is closed again before returning.
fn stored(repo: &Repo) -> TestResult<Vec<Stored>> {
    let store = Store::open(&store_of(&repo.root), Wait::Command)?;
    let read = store.read(
        &Filter::all(),
        0,
        NonZeroUsize::new(10_000).ok_or("non-zero")?,
    )?;
    drop(store);
    Ok(read)
}

/// The legacy sources the store recorded importing: `(path, store_id)`.
fn legacy_records(events: &[Stored]) -> Vec<(String, String)> {
    decode_all::<ImportCompleted>(events)
        .events
        .into_iter()
        .filter_map(|e| match e.data.source {
            ImportSource::Legacy { path, store_id } => Some((path, store_id.to_string())),
            _ => None,
        })
        .collect()
}

fn session_start(repo: &Repo) -> TestResult<Value> {
    let start = json!({"hook_event_name": "SessionStart", "session_id": "s1", "cwd": repo.root, "source": "startup"});
    let out = repo.hook(&start)?;
    assert!(out.status.success(), "the hook exits 0");
    let text = String::from_utf8(out.stdout)?;
    Ok(serde_json::from_str(&text).unwrap_or(Value::String(text)))
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_ledger_and_its_journals_import_once_a_second_run_appends_nothing_and_the_sources_are_untouched()
-> TestResult {
    for name in FIXTURES {
        let repo = Repo::new(true)?;
        let old = with_old_ledger(&repo, name)?;
        let (store_id, events) = fixture_events(name)?;
        let copied = events.first().ok_or("the fixture has events")?;
        let journal = with_journal(&repo.root, "s1.jsonl", &[&copied.data, OWN_LINE.as_bytes()])?;
        let before = (snapshot(&old)?, snapshot(&journal)?);

        let open = repo.run("s1", &["open"], None)?;

        let listed = String::from_utf8(open.stdout)?;
        assert!(open.status.success(), "{name}: {listed}");
        assert!(listed.contains(OPEN_IN_FIXTURE), "{name}: {listed}");
        let all = stored(&repo)?;
        let weighin: Vec<&Stored> = all
            .iter()
            .filter(|s| s.event_type.starts_with("weighin."))
            .collect();
        let ids: Vec<String> = weighin.iter().map(|s| s.id.to_string()).collect();
        let expected: Vec<String> = events.iter().map(|e| e.id.to_string()).collect();
        assert_eq!(
            ids.get(..expected.len()),
            Some(expected.as_slice()),
            "{name}"
        );
        assert_eq!(
            ids.len(),
            expected.len().saturating_add(1),
            "{name}: {ids:?}"
        );
        let own = ids.last().ok_or("the journal's own line")?;
        assert!(own.starts_with(JOURNAL_STORE), "{name}: {own}");
        let records = legacy_records(&all);
        assert_eq!(
            records,
            [
                (old.to_string_lossy().into_owned(), store_id),
                (
                    journal.to_string_lossy().into_owned(),
                    JOURNAL_STORE.to_owned()
                ),
            ],
            "{name}: one record per source, the ledger first"
        );
        let last = all.last().map(|s| s.position);

        let again = repo.run("s1", &["open"], None)?;
        let _started = session_start(&repo)?;

        assert!(again.status.success(), "{name}");
        let after = stored(&repo)?;
        assert_eq!(
            after.last().map(|s| s.position),
            last,
            "{name}: nothing appended"
        );
        assert_eq!(
            (snapshot(&old)?, snapshot(&journal)?),
            before,
            "{name}: the sources and their directories are unchanged"
        );
    }
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn journal_import_reports_the_legacy_import_in_its_envelope() -> TestResult {
    let repo = Repo::new(true)?;
    let old = with_old_ledger(&repo, "weighin-1.2.0.sqlite3")?;
    let journal = with_journal(&repo.root, "s1.jsonl", &[OWN_LINE.as_bytes(), b"not json"])?;

    let first = repo.run("s1", &["journal", "import", "--json"], None)?;
    let second = repo.run("s1", &["journal", "import", "--json"], None)?;

    assert_eq!(first.status.code(), Some(0));
    let first: Value = serde_json::from_slice(&first.stdout)?;
    let legacy = first["data"]["legacy"].as_array().ok_or("data.legacy")?;
    let kinds: Vec<(&str, &str, &str)> = legacy
        .iter()
        .map(|s| {
            (
                s["kind"].as_str().unwrap_or(""),
                s["path"].as_str().unwrap_or(""),
                s["outcome"].as_str().unwrap_or(""),
            )
        })
        .collect();
    let (old, journal) = (old.to_string_lossy(), journal.to_string_lossy());
    assert_eq!(
        kinds,
        [
            ("ledger", old.as_ref(), "completed"),
            ("journal", journal.as_ref(), "completed")
        ]
    );
    let line = legacy.get(1).ok_or("the journal")?;
    assert_eq!(
        (
            &line["read"],
            &line["imported"],
            &line["malformed"],
            &line["store_id"]
        ),
        (&json!(2), &json!(1), &json!(1), &json!(JOURNAL_STORE))
    );
    let findings = first["findings"].as_array().ok_or("findings")?;
    assert!(
        findings.iter().any(|f| f["key"]
            .as_str()
            .is_some_and(|k| k.starts_with("legacy-malformed:") && k.ends_with(":2"))),
        "{findings:?}"
    );
    let second: Value = serde_json::from_slice(&second.stdout)?;
    assert_eq!(
        second["data"].get("legacy"),
        None,
        "nothing due the second time"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn ci_check_beside_an_old_ledger_imports_nothing_and_creates_no_store() -> TestResult {
    let repo = Repo::new(true)?;
    with_old_ledger(&repo, "weighin-1.2.0.sqlite3")?;
    with_journal(&repo.root, "s1.jsonl", &[OWN_LINE.as_bytes()])?;

    let out = repo.run("s1", &["ci-check", "--base", "HEAD", "--json"], None)?;

    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!store_of(&repo.root).exists(), "no store");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_client_repository_imports_its_old_ledger() -> TestResult {
    let repo = Repo::new(false)?;
    with_old_ledger(&repo, "weighin-1.2.0.sqlite3")?;

    let open = repo.run("s1", &["open"], None)?;

    let listed = String::from_utf8(open.stdout)?;
    assert!(listed.contains(OPEN_IN_FIXTURE), "{listed}");
    assert_eq!(legacy_records(&stored(&repo)?).len(), 1);
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn the_named_ledger_of_weighin_store_is_imported() -> TestResult {
    let repo = Repo::new(true)?;
    let elsewhere = repo.sibling("elsewhere")?;
    let named = elsewhere.join("proj.sqlite3");
    std::fs::write(&named, fixture("weighin-1.2.0.sqlite3")?)?;
    let before = snapshot(&named)?;
    let named_text = named.to_string_lossy().into_owned();

    let (ok, out) = weighin_env(
        &repo.root,
        &Run {
            session: "s1",
            args: &["open"],
            stdin: None,
            vars: &[("WEIGHIN_STORE", named_text.as_str())],
        },
    );

    assert!(ok && out.contains(OPEN_IN_FIXTURE), "{out}");
    let store = Store::open(&elsewhere.join("proj.events.sqlite3"), Wait::Command)?;
    let all = store.read(
        &Filter::all().of_type(ImportCompleted::TYPE)?,
        0,
        NonZeroUsize::MIN.saturating_add(9),
    )?;
    drop(store);
    let records = legacy_records(&all);
    let paths: Vec<&str> = records.iter().map(|(p, _)| p.as_str()).collect();
    assert_eq!(
        paths.first().copied(),
        Some(named_text.as_str()),
        "{records:?}"
    );
    // The store is written beside the named ledger; the ledger itself is left as it was.
    assert_eq!(snapshot(&named)?.1, before.1);
    Ok(())
}
