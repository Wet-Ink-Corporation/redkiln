use redkiln_core::envelope::{Actor, Envelope};
use redkiln_core::ids::parse_event_ref;
use redkiln_core::journal::{Mapped, import_mapped};
use redkiln_core::legacy::read_ledger;
use redkiln_core::store::{Stored, Wait};
use redkiln_core::trust::Trust;

use super::*;

/// Every event of the store at `env`'s path, as stored.
fn stored(env: &Env) -> Vec<Stored> {
    let store = Store::open(env.store_path().expect("located"), Wait::Command).expect("opens");
    store
        .core()
        .read(
            &Filter::all(),
            0,
            NonZeroUsize::new(10_000).expect("non-zero"),
        )
        .expect("read")
}

/// The store's import records, decoded.
fn records(env: &Env) -> Vec<ImportCompleted> {
    decode_all::<ImportCompleted>(&stored(env))
        .events
        .into_iter()
        .map(|e| e.data)
        .collect()
}

/// A folder outside git whose store is `<dir>/events.sqlite3`, with Core's fixture `name` copied
/// in as an old ledger at `<dir>/old/ledger.sqlite3`.
fn with_ledger(dir: &Path, name: &str) -> (Env, PathBuf) {
    let from = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../core/redkiln-core/tests/fixtures")
        .join(name);
    let old = dir.join("old/ledger.sqlite3");
    std::fs::create_dir_all(dir.join("old")).expect("made");
    std::fs::copy(from, &old).expect("copied");
    let mut env = Env::outside_git(dir, "s1");
    env.old_ledgers = vec![old.clone()];
    (env, old)
}

/// Writes `lines` as the 1.x journal `<root>/.kb/_ledger/journal/<name>`; its path.
fn write_journal(root: &Path, name: &str, lines: &[&[u8]]) -> PathBuf {
    let dir = root.join(journal::DIR);
    std::fs::create_dir_all(&dir).expect("made");
    let path = dir.join(name);
    let mut bytes = Vec::new();
    for line in lines {
        bytes.extend_from_slice(line);
        bytes.push(b'\n');
    }
    std::fs::write(&path, bytes).expect("written");
    path
}

fn ran(legacy: Legacy) -> LegacyReport {
    match legacy {
        Legacy::Ran(report) => report,
        Legacy::NothingDue => panic!("nothing was due"),
    }
}

/// `(read, imported, present, malformed)` of a completed source.
fn counts(source: &SourceRun) -> (u64, u64, u64, u64) {
    match &source.outcome {
        SourceOutcome::Completed { report, .. } => (
            report.read,
            report.imported,
            report.present,
            report.malformed,
        ),
        other => panic!("{}: {other:?}", source.path.display()),
    }
}

/// A 1.x line of no fixture.
const OWN_LINE: &[u8] = br#"{"type":"item_presented","item_id":"wi-b29e54","surface":"chat","reason":"shown","actor":"cli","session_id":"s1","ts":"2026-10-09T03:29:37Z"}"#;

#[test]
fn nothing_due_opens_and_creates_no_store() {
    let dir = tempfile::tempdir().expect("tempdir");
    let env = Env::outside_git(dir.path(), "s1");

    let legacy = import_due(&env, Mode::Command, "cli").expect("ran");

    assert!(matches!(legacy, Legacy::NothingDue), "{legacy:?}");
    let names: Vec<_> = std::fs::read_dir(dir.path()).expect("listed").collect();
    assert!(names.is_empty(), "nothing created: {names:?}");
}

#[test]
fn ledgers_are_imported_before_journals() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (env, old) = with_ledger(dir.path(), "weighin-1.2.0.sqlite3");
    let ledger = read_ledger(&old).expect("reads");
    let first = ledger.events.first().expect("an event");
    // `a.jsonl` sorts first, yet the ledger goes before it.
    write_journal(dir.path(), "a.jsonl", &[&first.data]);

    let report = ran(import_due(&env, Mode::Command, "cli").expect("ran"));

    let kinds: Vec<_> = report.sources.iter().map(|s| s.kind).collect();
    assert_eq!(kinds, [SourceKind::Ledger, SourceKind::Journal]);
    let journal = report.sources.get(1).expect("the journal");
    assert_eq!(
        counts(journal),
        (1, 0, 1, 0),
        "the line is the ledger's event"
    );
    let held: Vec<_> = stored(&env)
        .into_iter()
        .filter(|s| s.event_type.starts_with("weighin."))
        .collect();
    assert_eq!(held.len(), ledger.events.len(), "each event once");
    assert_eq!(held.first().map(|s| s.id), Some(first.id));
}

#[test]
fn journals_wait_while_a_found_ledger_is_not_imported() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (env, old) = with_ledger(dir.path(), "weighin-1.2.0.sqlite3");
    let mut hot = old.as_os_str().to_owned();
    hot.push("-journal");
    std::fs::write(PathBuf::from(hot), b"a commit in progress").expect("written");
    write_journal(dir.path(), "s1.jsonl", &[OWN_LINE]);

    let report = ran(import_due(&env, Mode::Command, "cli").expect("ran"));

    let [source] = report.sources.as_slice() else {
        panic!("only the ledger: {:?}", report.sources);
    };
    assert!(
        matches!(&source.outcome, SourceOutcome::Failed(e) if format!("{e:#}").contains("hot journal")),
        "{source:?}"
    );
    assert_eq!(report.waiting, Some(Waiting::Ledger));
    assert_eq!(records(&env), Vec::new(), "nothing recorded");
}

#[test]
fn a_source_with_an_own_record_is_not_read_again() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (env, _old) = with_ledger(dir.path(), "weighin-0.2.2.sqlite3");
    write_journal(dir.path(), "s1.jsonl", &[OWN_LINE]);
    let first = ran(import_due(&env, Mode::Command, "cli").expect("ran"));
    let after = stored(&env).len();

    let second = import_due(&env, Mode::Command, "cli").expect("ran");

    assert_eq!(first.sources.len(), 2);
    assert!(matches!(second, Legacy::NothingDue), "{second:?}");
    assert_eq!(stored(&env).len(), after, "nothing appended");
    assert_eq!(records(&env).len(), 2, "one record per source");
}

#[test]
fn a_record_ingested_from_another_store_does_not_mark_a_source_done() {
    let dir = tempfile::tempdir().expect("tempdir");
    let env = Env::outside_git(dir.path(), "s1");
    let path = write_journal(dir.path(), "s1.jsonl", &[OWN_LINE]);
    let store = Store::open(env.store_path().expect("located"), Wait::Command).expect("opens");
    let author = Author {
        actor: Actor::Cli,
        trust: Trust::asserted(),
        writer: redkiln_core::envelope::Writer::new("weighin", "2.0.0").expect("valid"),
    };
    let elsewhere = ImportCompleted {
        source: ImportSource::Legacy {
            path: path.to_string_lossy().into_owned(),
            store_id: JOURNAL_STORE,
        },
        read: 1,
        imported: 1,
        present: 0,
        malformed: 0,
    };
    let ts = Timestamp::parse("2026-11-08T10:00:00Z").expect("valid");
    let event = Envelope::new(ts.clone(), author.clone(), elsewhere)
        .to_event([])
        .expect("an event");
    let id = parse_event_ref("0123456789abcdef0123456789abcdef:1").expect("a ref");
    let recording = Recording {
        source: ImportSource::Artifact("another clone".to_owned()),
        ts,
        author,
    };
    let ingested = import_mapped(
        store.core(),
        [(1, Mapped::Event { id, event })],
        recording,
        None,
    )
    .expect("ingested");
    assert!(
        matches!(ingested, MappedImport::Completed(_)),
        "{ingested:?}"
    );
    drop(store);

    let report = ran(import_due(&env, Mode::Command, "cli").expect("ran"));

    let [source] = report.sources.as_slice() else {
        panic!("the journal: {:?}", report.sources);
    };
    assert_eq!(counts(source), (1, 1, 0, 0));
}

#[test]
fn a_journal_file_added_later_is_imported_on_the_next_run() {
    let dir = tempfile::tempdir().expect("tempdir");
    let env = Env::outside_git(dir.path(), "s1");
    write_journal(dir.path(), "a.jsonl", &[OWN_LINE]);
    let first = ran(import_due(&env, Mode::Command, "cli").expect("ran"));
    let later = write_journal(dir.path(), "b.jsonl", &[OWN_LINE]);

    let second = ran(import_due(&env, Mode::Command, "cli").expect("ran"));

    assert_eq!(first.sources.len(), 1);
    let [source] = second.sources.as_slice() else {
        panic!("only b: {:?}", second.sources);
    };
    assert_eq!(source.path, later);
    assert_eq!(counts(source), (1, 0, 1, 0), "the same line, held");
}

#[test]
fn a_second_run_after_the_ledger_is_gone_appends_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (env, old) = with_ledger(dir.path(), "weighin-1.2.0.sqlite3");
    let ledger = read_ledger(&old).expect("reads");
    let lines: Vec<&[u8]> = ledger.events.iter().map(|e| e.data.as_slice()).collect();
    write_journal(dir.path(), "s1.jsonl", &lines);
    let _first = ran(import_due(&env, Mode::Command, "cli").expect("ran"));
    let held = stored(&env).len();
    // A linked worktree: the same journal bytes at another path, the same store, no ledger.
    let other = tempfile::tempdir().expect("tempdir");
    write_journal(other.path(), "s1.jsonl", &lines);
    let mut linked = Env::outside_git(other.path(), "s1");
    linked.store = Ok(env.store.as_ref().expect("located").clone());

    let second = ran(import_due(&linked, Mode::Command, "cli").expect("ran"));

    let [source] = second.sources.as_slice() else {
        panic!("the journal: {:?}", second.sources);
    };
    let count = u64::try_from(lines.len()).expect("fits");
    assert_eq!(counts(source), (count, 0, count, 0), "every line held");
    assert_eq!(
        stored(&env).len(),
        held.saturating_add(1),
        "its record only"
    );
}

/// A sparse 1.x journal of `len` bytes: its size is read, never its bytes.
fn sparse_journal(root: &Path, len: u64) -> PathBuf {
    let path = write_journal(root, "big.jsonl", &[]);
    File::options()
        .write(true)
        .open(&path)
        .and_then(|f| f.set_len(len))
        .expect("sized");
    path
}

#[test]
fn a_journal_over_the_cap_is_reported_and_not_read() {
    let dir = tempfile::tempdir().expect("tempdir");
    let env = Env::outside_git(dir.path(), "s1");
    sparse_journal(dir.path(), MAX_JOURNAL_FILE_BYTES.saturating_add(1));

    let report = ran(import_due(&env, Mode::Command, "cli").expect("ran"));

    let [source] = report.sources.as_slice() else {
        panic!("the journal: {:?}", report.sources);
    };
    assert!(
        matches!(source.outcome, SourceOutcome::TooLarge { bytes } if bytes > MAX_JOURNAL_FILE_BYTES),
        "{source:?}"
    );
    assert!(notes(&report).iter().any(|n| n.contains("not imported")));
    assert_eq!(records(&env), Vec::new(), "not recorded, so tried again");
}

#[test]
fn the_hook_leaves_sources_over_its_cap_for_a_command_and_says_so() {
    let dir = tempfile::tempdir().expect("tempdir");
    let env = Env::outside_git(dir.path(), "s1");
    sparse_journal(dir.path(), MAX_HOOK_SOURCE_BYTES.saturating_add(1));
    let hook = Mode::Hook {
        started: Instant::now(),
    };

    let report = ran(import_due(&env, hook, "hook").expect("ran"));

    assert!(report.sources.is_empty(), "{:?}", report.sources);
    assert!(
        matches!(report.waiting, Some(Waiting::TooLargeForHook { bytes }) if bytes > MAX_HOOK_SOURCE_BYTES)
    );
    assert!(
        notes(&report)
            .iter()
            .any(|n| n.contains("next weighin command"))
    );
    assert!(
        !env.store_path().expect("located").exists(),
        "the cap is checked before the store is made"
    );
}

#[test]
fn the_hook_past_its_deadline_records_nothing_and_the_next_run_finishes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (env, old) = with_ledger(dir.path(), "weighin-1.2.0.sqlite3");
    let events = read_ledger(&old).expect("reads").events.len();
    let late = Mode::Hook {
        started: Instant::now()
            .checked_sub(HOOK_START_BY.saturating_mul(2))
            .expect("the clock has run that long"),
    };

    let hooked = ran(import_due(&env, late, "hook").expect("ran"));
    let held_after_hook = stored(&env).len();
    let finished = ran(import_due(&env, Mode::Command, "cli").expect("ran"));

    assert!(
        matches!(
            hooked.sources.first().map(|s| &s.outcome),
            Some(SourceOutcome::Stopped { .. })
        ),
        "{:?}",
        hooked.sources
    );
    assert_eq!(hooked.waiting, Some(Waiting::Deadline));
    assert_eq!(held_after_hook, 0, "no batch stored, nothing recorded");
    let source = finished.sources.first().expect("the ledger");
    let count = u64::try_from(events).expect("fits");
    assert_eq!(counts(source), (count, count, 0, 0));
    assert_eq!(records(&env).len(), 1);
}

#[test]
fn the_hook_past_its_deadline_reads_no_journal_and_leaves_them_waiting() {
    let dir = tempfile::tempdir().expect("tempdir");
    let env = Env::outside_git(dir.path(), "s1");
    write_journal(dir.path(), "a.jsonl", &[OWN_LINE]);
    let late = Mode::Hook {
        started: Instant::now()
            .checked_sub(HOOK_START_BY.saturating_mul(2))
            .expect("the clock has run that long"),
    };

    let report = ran(import_due(&env, late, "hook").expect("ran"));

    assert!(
        report.sources.is_empty(),
        "the store's events are not read for a journal past the deadline (review F3): {:?}",
        report.sources
    );
    assert_eq!(report.waiting, Some(Waiting::Deadline));
    assert!(!report.imported_a_source(), "no budget spent on a source");
    assert_eq!(records(&env), Vec::new());
}
