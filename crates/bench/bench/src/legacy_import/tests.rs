//! The import over the committed fixtures (`tests/fixtures/anneal/`, read only from copies),
//! into a scratch store (plan test list 32 to 41, at the module's seam).

use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::time::Instant;

use redkiln_core::envelope::Payload as _;
use redkiln_core::journal::{ImportCompleted, ImportSource};
use redkiln_core::legacy::read_ledger;
use redkiln_core::store::{Filter, Stored, Wait};
use redkiln_core::suite::Owner;
use serde_json::Value;

use super::{Budget, Importer, Line, Outcome, Run, Waiting, due, lines, warnings};
use crate::store::{EventKind, Filter as BenchFilter, LedgerEvent, Store};

/// The committed fixture `name`.
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/anneal")
        .join(name)
}

/// A copy of the fixture `name` (with its `-wal`, when `wal`) in `dir` as `ledger.sqlite3`.
fn ledger_in(dir: &Path, name: &str, wal: bool) -> PathBuf {
    let path = dir.join("ledger.sqlite3");
    std::fs::copy(fixture(name), &path).expect("the fixture copies");
    if wal {
        std::fs::copy(
            fixture(&format!("{name}-wal")),
            dir.join("ledger.sqlite3-wal"),
        )
        .expect("its -wal copies");
    }
    path
}

/// Every file in `dir` with its bytes, sorted by name.
fn snapshot(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut files: Vec<(String, Vec<u8>)> = std::fs::read_dir(dir)
        .expect("listed")
        .map(|entry| {
            let entry = entry.expect("an entry");
            let bytes = std::fs::read(entry.path()).expect("read");
            (entry.file_name().to_string_lossy().into_owned(), bytes)
        })
        .collect();
    files.sort();
    files
}

/// The owner a test names, or none.
fn owner_named(name: Option<&str>) -> impl Fn() -> Result<Option<Owner>, String> {
    let listed = name.map(|name| format!("user.name {name}\n"));
    move || Ok(listed.as_deref().and_then(Owner::from_git_config))
}

/// Imports `sources` into the store at `store`, as a command with `owner`.
fn import(store: &Path, sources: &[&Path], owner: Option<&str>) -> Run {
    let store = Store::open(store).expect("the store opens");
    Importer::new(&store, Budget::COMMAND, "cli", owner_named(owner))
        .expect("an importer")
        .run(sources)
}

/// Every event in the store at `path`, as Core holds it.
fn raw(path: &Path) -> Vec<Stored> {
    redkiln_core::store::Store::open_existing(path, Wait::Command)
        .expect("opens")
        .expect("exists")
        .read(
            &Filter::all(),
            0,
            NonZeroUsize::new(4096).expect("non-zero"),
        )
        .expect("reads")
}

/// The JSON of a stored envelope.
fn json(stored: &Stored) -> Value {
    serde_json::from_slice(&stored.data).expect("an envelope")
}

#[test]
fn every_old_event_is_imported_once_as_its_bench_type_under_its_old_identity() {
    let old = tempfile::tempdir().expect("a directory");
    let ledger = ledger_in(old.path(), "anneal-0.0.5.sqlite3", false);
    let before = snapshot(old.path());
    let source = read_ledger(&ledger).expect("Core reads the fixture");
    let work = tempfile::tempdir().expect("a directory");
    let store = work.path().join("events.sqlite3");

    let run = import(&store, &[&ledger], Some("Ryan Britton"));

    let [only] = run.sources.as_slice() else {
        panic!("one source: {run:?}")
    };
    let Outcome::Completed(imported) = &only.outcome else {
        panic!("completed: {run:?}")
    };
    let held = raw(&store);
    let mut bench: Vec<&Stored> = held
        .iter()
        .filter(|stored| stored.event_type.starts_with("bench."))
        .collect();
    bench.sort_by_key(|stored| stored.id.position());
    // The record whose actor Core refuses (`test`) is the one left out.
    let expected: Vec<(String, _)> = source
        .events
        .iter()
        .filter(|old| json(old)["actor"] != "test")
        .map(|old| (format!("bench.{}", old.event_type), old.id))
        .collect();
    let got: Vec<(String, _)> = bench
        .iter()
        .map(|stored| (stored.event_type.clone(), stored.id))
        .collect();
    assert_eq!(got, expected, "bench.<name> under the old identity");
    for stored in &bench {
        assert_eq!(json(stored)["trust"]["level"], "asserted", "{stored:?}");
    }
    let records: Vec<ImportCompleted> = held
        .iter()
        .filter(|stored| stored.event_type == ImportCompleted::TYPE)
        .map(|stored| serde_json::from_value(json(stored)["data"].clone()).expect("a record"))
        .collect();
    let [record] = records.as_slice() else {
        panic!("one import record: {records:?}")
    };
    assert_eq!(
        record.source,
        ImportSource::Legacy {
            path: ledger.to_string_lossy().into_owned(),
            store_id: source.store_id,
        }
    );
    assert_eq!(
        (
            record.read,
            record.imported,
            record.present,
            record.malformed
        ),
        (24, 23, 0, 1),
        "23 events, the human approval's Core record, and the refused actor"
    );
    assert_eq!(imported.report.imported, 23);
    assert_eq!(snapshot(old.path()), before, "the ledger is left as it was");
}

#[test]
fn a_second_run_appends_nothing_and_records_nothing() {
    let old = tempfile::tempdir().expect("a directory");
    let ledger = ledger_in(old.path(), "anneal-0.0.5.sqlite3", false);
    let work = tempfile::tempdir().expect("a directory");
    let store_path = work.path().join("events.sqlite3");
    let first = import(&store_path, &[&ledger], Some("ryan"));
    assert!(matches!(first.sources[0].outcome, Outcome::Completed(_)));
    let held = raw(&store_path).len();
    let store = Store::open(&store_path).expect("opens");

    let left = due(Some(&store), std::slice::from_ref(&ledger)).expect("reads");

    assert!(left.is_empty(), "nothing is due: {left:?}");
    let again = Importer::new(&store, Budget::COMMAND, "cli", owner_named(Some("ryan")))
        .expect("an importer")
        .run(&[&ledger]);
    let Outcome::Completed(imported) = &again.sources[0].outcome else {
        panic!("{again:?}")
    };
    assert_eq!(
        (imported.report.imported, imported.report.present),
        (0, 23),
        "every identity is present, the approval's too"
    );
    assert_eq!(
        raw(&store_path).len(),
        held.saturating_add(1),
        "only a record of the forced re-run"
    );
}

#[test]
fn only_the_human_approval_is_recorded_for_other_plugins_decided_by_the_owner() {
    let old = tempfile::tempdir().expect("a directory");
    let ledger = ledger_in(old.path(), "anneal-0.0.5.sqlite3", false);
    let source = read_ledger(&ledger).expect("reads");
    let work = tempfile::tempdir().expect("a directory");
    let store = work.path().join("events.sqlite3");

    let run = import(&store, &[&ledger], Some("Ryan Britton"));

    let Outcome::Completed(imported) = &run.sources[0].outcome else {
        panic!("{run:?}")
    };
    let tally = &imported.tally;
    assert_eq!(
        (
            tally.approvals,
            tally.no_owner,
            tally.model_or_unknown,
            tally.bad_hash
        ),
        (1, 0, 3, 0),
        "human; model, no `by` and `via` only"
    );
    let human = source
        .events
        .iter()
        .find(|old| old.event_type == "plan_approved" && json(old)["by"]["actor"] == "human")
        .expect("the fixture's human approval");
    let approvals: Vec<Stored> = raw(&store)
        .into_iter()
        .filter(|stored| stored.event_type == "core.approval_recorded")
        .collect();
    let [approval] = approvals.as_slice() else {
        panic!("one: {approvals:?}")
    };
    assert_eq!(
        approval.id.store(),
        super::map::approval_store(source.store_id),
        "D4's derived store"
    );
    assert_eq!(
        approval.id.position(),
        human.id.position(),
        "at the old position"
    );
    let data = &json(approval)["data"];
    assert_eq!(data["decider"], "human:ryan-britton");
    assert_eq!(data["verdict"], "approved");
    assert_eq!(data["scope"], "repository");
    assert_eq!(data["subject"]["kind"], "plan");
    assert_eq!(data["subject"]["hash"], json(human)["hash"]);
    assert_eq!(data["subject"]["locator"], json(human)["plan"]);
    assert_eq!(data.get("gates"), None, "D3: no gates");
}

#[test]
fn with_no_owner_no_approval_is_recorded_for_other_plugins_and_each_is_counted() {
    let old = tempfile::tempdir().expect("a directory");
    let ledger = ledger_in(old.path(), "anneal-0.0.5.sqlite3", false);
    let work = tempfile::tempdir().expect("a directory");
    let store = work.path().join("events.sqlite3");

    let run = import(&store, &[&ledger], None);

    let Outcome::Completed(imported) = &run.sources[0].outcome else {
        panic!("{run:?}")
    };
    assert_eq!((imported.tally.approvals, imported.tally.no_owner), (0, 1));
    assert!(
        raw(&store)
            .iter()
            .all(|stored| stored.event_type != "core.approval_recorded"),
        "none"
    );
    let said: Vec<String> = lines(&run).into_iter().map(|line| line.text).collect();
    assert!(
        said.iter().any(|line| line.contains("no owner: 1")),
        "the report counts it: {said:?}"
    );
}

#[test]
fn fields_core_refuses_are_normalised_dropped_or_reported() {
    let old = tempfile::tempdir().expect("a directory");
    let ledger = ledger_in(old.path(), "anneal-0.0.5.sqlite3", false);
    let work = tempfile::tempdir().expect("a directory");
    let store_path = work.path().join("events.sqlite3");

    let run = import(&store_path, &[&ledger], Some("ryan"));

    let Outcome::Completed(imported) = &run.sources[0].outcome else {
        panic!("{run:?}")
    };
    let [(_, why)] = imported.tally.problems.as_slice() else {
        panic!("one problem: {:?}", imported.tally.problems)
    };
    assert!(why.contains("actor \"test\""), "the refused actor: {why}");
    let store = Store::open(&store_path).expect("opens");
    let runs = store
        .query(&BenchFilter {
            types: vec![EventKind::TestRun, EventKind::SubagentStatus],
            tags: Vec::new(),
        })
        .expect("reads");
    let stamps: Vec<&str> = runs.iter().map(|r| r.event.ts.as_str()).collect();
    assert_eq!(
        stamps,
        ["2026-11-02T10:04:00.120Z", "2026-11-02T10:12:00.500Z"],
        "anneal's dropped zeros, restored"
    );
    let entered = crate::mode::active(&store, "/home/user/anneal-fixture")
        .expect("reads")
        .expect("the old mode, under the id this build computes for its worktree");
    let modes = store
        .query(&BenchFilter {
            types: vec![EventKind::ModeSet],
            tags: Vec::new(),
        })
        .expect("reads");
    let [mode_set] = modes.as_slice() else {
        panic!("one mode_set: {modes:?}")
    };
    assert!(
        matches!(mode_set.event.event, LedgerEvent::ModeSet(_)),
        "{modes:?}"
    );
    assert_eq!(entered.position, mode_set.position, "the imported mode_set");
    assert!(
        matches!(entered.mode, crate::mode::Active::IncompleteBuild),
        "build with a plan and no design tree, as anneal recorded it: {entered:?}"
    );
    assert_eq!(
        imported.tally.context_dropped, 0,
        "the fixture's worktree, branch and head are all Core's"
    );
}

/// An old record at `position` of the store `store_id`, holding `data`.
fn old_record(
    store_id: redkiln_core::StoreId,
    position: u64,
    event_type: &str,
    data: &str,
) -> Stored {
    let id = redkiln_core::ids::parse_event_ref(&format!("{store_id}:{position}")).expect("a ref");
    Stored {
        position,
        id,
        recorded_at_millis: 0,
        event_type: event_type.to_owned(),
        data: data.as_bytes().to_vec(),
        tags: Vec::new(),
    }
}

#[test]
fn a_context_field_core_refuses_is_dropped_and_counted_and_the_event_kept() {
    let old = redkiln_core::ids::parse_store_id("0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f").expect("an id");
    let events = [
        old_record(
            old,
            1,
            "task_done",
            r#"{"type":"task_done","task":"T1","actor":"cli","worktree":"relative/wt","branch":"main","head":"abc123","ts":"2026-11-02T12:05:00+02:00"}"#,
        ),
        old_record(
            old,
            2,
            "task_done",
            r#"{"type":"task_done","task":"T2","actor":"cli","ts":"2026-11-02T10:06:00Z","extra":"ignored"}"#,
        ),
        old_record(
            old,
            3,
            "task_done",
            r#"{"type":"no_such_type","actor":"cli","ts":"2026-11-02T10:07:00Z"}"#,
        ),
    ];
    let work = tempfile::tempdir().expect("a directory");
    let store = Store::open(&work.path().join("events.sqlite3")).expect("opens");
    let importer =
        Importer::new(&store, Budget::COMMAND, "cli", owner_named(None)).expect("an importer");
    let mut records = super::map::Records::new(&events, old, store.writer(), &importer.owner);

    let ended = store
        .import(
            &mut records,
            importer.recording(Path::new("old.sqlite3"), old),
            None,
        )
        .expect("imports");

    let tally = records.into_tally();
    assert_eq!(
        tally.context_dropped, 2,
        "the relative worktree and the short head"
    );
    assert_eq!(
        tally.problems.iter().map(|(n, _)| *n).collect::<Vec<_>>(),
        [3],
        "the unknown type is reported"
    );
    let redkiln_core::journal::MappedImport::Completed(report) = ended else {
        panic!("completed: {ended:?}")
    };
    assert_eq!((report.imported, report.malformed), (2, 1));
    let kept = store.query(&BenchFilter::default()).expect("reads");
    let first = &kept.first().expect("the first event").event;
    assert_eq!(
        (
            first.worktree.as_deref(),
            first.branch.as_deref(),
            first.head.as_deref(),
            first.ts.as_str()
        ),
        (None, Some("main"), None, "2026-11-02T10:05:00.000Z"),
        "worktree and head dropped, branch kept, the offset normalised to UTC"
    );
}

#[test]
fn the_older_registry_s_shapes_import() {
    let old = tempfile::tempdir().expect("a directory");
    let ledger = ledger_in(old.path(), "anneal-older.sqlite3", false);
    let work = tempfile::tempdir().expect("a directory");
    let store = work.path().join("events.sqlite3");

    let run = import(&store, &[&ledger], Some("ryan"));

    let Outcome::Completed(imported) = &run.sources[0].outcome else {
        panic!("{run:?}")
    };
    assert_eq!(
        (
            imported.report.read,
            imported.report.imported,
            imported.report.malformed
        ),
        (7, 7, 0),
        "{:?}",
        imported.tally.problems
    );
    let types: Vec<String> = raw(&store)
        .into_iter()
        .map(|stored| stored.event_type)
        .filter(|t| t.starts_with("bench."))
        .collect();
    assert!(
        types.contains(&"bench.round_started".to_owned()),
        "{types:?}"
    );
    assert!(
        types.contains(&"bench.grill_confirmed".to_owned()),
        "{types:?}"
    );
}

#[test]
fn a_wal_ledger_imports_with_its_wal_frames_and_is_left_as_it_was() {
    let old = tempfile::tempdir().expect("a directory");
    let ledger = ledger_in(old.path(), "anneal-wal.sqlite3", true);
    let before = snapshot(old.path());
    let work = tempfile::tempdir().expect("a directory");
    let store = work.path().join("events.sqlite3");

    let run = import(&store, &[&ledger], Some("ryan"));

    let Outcome::Completed(imported) = &run.sources[0].outcome else {
        panic!("{run:?}")
    };
    assert_eq!(imported.report.imported, 23, "the -wal's events too");
    assert_eq!(
        snapshot(old.path()),
        before,
        "file, -wal and listing unchanged"
    );
}

#[test]
fn both_happenstance_schemas_of_dev_bench_ledgers_import() {
    // Each ledger's events kept and refused (hs032's actor `test` is refused on every one).
    for (name, kept, refused) in [
        ("bench-ledger-hs060.sqlite3", 5_usize, 0_u64),
        ("bench-ledger-hs032.sqlite3", 0, 3),
    ] {
        let old = tempfile::tempdir().expect("a directory");
        let ledger = ledger_in(old.path(), name, false);
        let work = tempfile::tempdir().expect("a directory");
        let store = work.path().join("events.sqlite3");

        let run = import(&store, &[&ledger], Some("ryan"));

        let Outcome::Completed(imported) = &run.sources[0].outcome else {
            panic!("{name}: {run:?}")
        };
        let bench = raw(&store)
            .iter()
            .filter(|stored| stored.event_type.starts_with("bench."))
            .count();
        assert_eq!(
            (bench, imported.report.malformed),
            (kept, refused),
            "{name}: {:?}",
            imported.tally.problems
        );
    }
}

#[test]
fn a_hook_leaves_sources_over_its_budget_to_a_command_which_imports_them() {
    let old = tempfile::tempdir().expect("a directory");
    let ledger = ledger_in(old.path(), "anneal-0.0.5.sqlite3", false);
    let work = tempfile::tempdir().expect("a directory");
    let location = crate::paths::StoreLocation {
        path: work.path().join("events.sqlite3"),
        source: crate::paths::StoreSource::Flag,
        old_ledgers: vec![ledger],
        from: work.path().to_path_buf(),
        notices: Vec::new(),
        repo_class: None,
    };
    let tight = Budget {
        max_bytes: Some(1),
        ..Budget::hook(Instant::now())
    };

    let hook = super::at_session_start(&location, tight, crate::paths::GIT_TIMEOUT)
        .expect("runs")
        .expect("something is due");

    assert!(
        matches!(hook.waiting, Some(Waiting::TooLarge { .. })) && hook.sources.is_empty(),
        "{hook:?}"
    );
    assert!(!location.path.exists(), "the hook created no store");
    let said: Vec<String> = lines(&hook).into_iter().map(|line| line.text).collect();
    assert!(
        said.iter()
            .any(|line| line.contains("the next bench command that records imports them")),
        "SessionStart says so: {said:?}"
    );
    let store = super::open_for_command(&location).expect("a command opens and imports");
    assert_eq!(
        due(Some(&store), &location.old_ledgers).expect("reads"),
        Vec::<&Path>::new(),
        "the command imported it"
    );
}

#[test]
fn an_unreadable_source_is_reported_and_retried_and_a_later_one_still_imports() {
    let old = tempfile::tempdir().expect("a directory");
    let not_sqlite = old.path().join("not-sqlite.sqlite3");
    std::fs::write(&not_sqlite, b"not a database").expect("written");
    let good_dir = tempfile::tempdir().expect("a directory");
    let good = ledger_in(good_dir.path(), "anneal-older.sqlite3", false);
    let work = tempfile::tempdir().expect("a directory");
    let store_path = work.path().join("events.sqlite3");
    let core_store = work.path().join("other.sqlite3");
    drop(Store::open(&core_store).expect("a Core store"));

    let run = import(
        &store_path,
        &[&not_sqlite, &core_store, &good],
        Some("ryan"),
    );

    let outcomes: Vec<&str> = run
        .sources
        .iter()
        .map(|source| match source.outcome {
            Outcome::Completed(_) => "completed",
            Outcome::Stopped(_) => "stopped",
            Outcome::Failed(_) => "failed",
        })
        .collect();
    assert_eq!(outcomes, ["failed", "failed", "completed"]);
    let store = Store::open(&store_path).expect("opens");
    // The array owns its paths; the assertion below borrows the first two again.
    let sources = [not_sqlite.clone(), core_store.clone(), good];
    let left = due(Some(&store), &sources).expect("reads");
    assert_eq!(
        left,
        [not_sqlite.as_path(), core_store.as_path()],
        "the failed ones stay due, nothing recorded for them"
    );
}

/// Plan test 43, run by hand: `BENCH_LIVE_LEDGER=<path> cargo test -p bench --lib
/// legacy_import::tests::a_live -- --ignored`. The ledger is read only from Core's copy.
#[test]
#[ignore = "manual (move 2 step 3): reads the anneal ledger BENCH_LIVE_LEDGER names"]
fn a_live_anneal_ledger_reads_untouched_and_every_event_maps() {
    let live = PathBuf::from(std::env::var_os("BENCH_LIVE_LEDGER").expect("BENCH_LIVE_LEDGER"));
    let before = std::fs::read(&live).expect("the live ledger reads");
    let work = tempfile::tempdir().expect("a directory");

    let run = import(&work.path().join("events.sqlite3"), &[&live], Some("ryan"));

    let Outcome::Completed(imported) = &run.sources[0].outcome else {
        panic!("{run:?}")
    };
    assert_eq!(
        imported.report.malformed, 0,
        "every event maps: {:?}",
        imported.tally.problems
    );
    assert_eq!(std::fs::read(&live).expect("reads"), before, "untouched");
}

#[test]
fn a_commands_report_is_one_warning_per_source_with_its_details_joined() {
    let line = |headline: bool, text: &str| Line {
        headline,
        text: text.to_owned(),
    };
    let report = vec![
        line(true, "a imported"),
        line(false, "record 1 refused"),
        line(false, "record 2 refused"),
        line(true, "b not imported"),
    ];

    assert_eq!(
        warnings(report),
        [
            "a imported; record 1 refused; record 2 refused",
            "b not imported"
        ],
        "CLI-08: one line each"
    );
}
