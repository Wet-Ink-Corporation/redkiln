//! Importing journals back from git: journal refs, refs set aside as broken, and inbox branches
//! not yet relayed (SPEC JR-05, JR-07, JR-10 to JR-13).

use std::num::NonZeroUsize;
use std::path::Path;
use std::time::Duration;

use redkiln_core::envelope::{Payload as _, decode_all};
use redkiln_core::journal::refs::{
    ImportedRef, RefsCheckpoint, RefsImport, RefsTarget, checkpoint_refs, import_refs,
};
use redkiln_core::journal::{ImportCompleted, ImportSource, encode_line, import_file};
use redkiln_core::store::{Filter, Store, Wait};
use redkiln_core::suite::RepoClass;
use redkiln_core::trust::Trust;

use super::git_fixture::{clone, git, remote};
use super::{all, importer, one_event_store, raised, source_store, ts};

fn target(repo: &Path) -> RefsTarget<'_> {
    RefsTarget {
        repo,
        remote: "origin",
        timeout: Duration::from_secs(30),
        deadline: None,
    }
}

/// An empty store in `dir/<name>/`.
fn empty_store(dir: &Path, name: &str) -> Store {
    let at = dir.join(name);
    std::fs::create_dir_all(&at).expect("dir");
    Store::open(&at.join("events.sqlite3"), Wait::Command).expect("opens")
}

/// The source store's journals checkpointed from a clone `work` of a new remote; the remote.
fn exported(dir: &Path) -> (Store, std::path::PathBuf) {
    let remote = remote(dir);
    let work = clone(dir, &remote, "work");
    let at = dir.join("source");
    std::fs::create_dir_all(&at).expect("dir");
    let source = source_store(&at);
    let outcome = checkpoint_refs(&source, RepoClass::Own, &target(&work)).expect("ok");
    assert!(matches!(outcome, RefsCheckpoint::Written(_)));
    (source, remote)
}

fn imported(store: &Store, repo: &Path) -> RefsImport {
    import_refs(store, &target(repo), &ts(), &importer()).expect("imported")
}

/// Each source's ref, with the lines it imported.
fn by_source(report: &RefsImport) -> Vec<(&str, u64)> {
    let mut sources: Vec<_> = report
        .sources
        .iter()
        .map(|ImportedRef { source, report, .. }| (source.as_str(), report.imported))
        .collect();
    sources.sort_unstable();
    sources
}

#[test]
fn a_fresh_clone_imports_every_journal_ref_once_per_commit() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (source, remote) = exported(dir.path());
    let reader = clone(dir.path(), &remote, "reader");
    let store = empty_store(dir.path(), "store");
    let first = imported(&store, &reader);
    assert!(
        first.fetch_failed.is_empty() && first.passed_over.is_empty(),
        "{first:?}"
    );
    let sources = by_source(&first);
    assert_eq!(sources.len(), 3, "{sources:?}");
    assert!(sources.contains(&("refs/redkiln/journal/a1", 2)));
    assert!(sources.contains(&("refs/redkiln/journal/b2", 1)));
    let a1 = git(&remote, &["rev-parse", "refs/redkiln/journal/a1"]);
    let expected = ImportSource::Ref {
        name: "refs/redkiln/journal/a1".to_owned(),
        commit: a1,
    };
    let records = decode_all::<ImportCompleted>(&all(&store));
    assert!(
        records
            .events
            .iter()
            .any(|record| record.data.source == expected),
        "the import names the ref and the commit read"
    );

    let again = imported(&store, &reader);
    assert!(again.sources.is_empty(), "already imported: {again:?}");

    source
        .append(
            vec![raised("wi-000005", Some("A1"), Trust::asserted())],
            None,
        )
        .expect("appended");
    let work = dir.path().join("work");
    let outcome = checkpoint_refs(&source, RepoClass::Own, &target(&work)).expect("ok");
    assert!(matches!(outcome, RefsCheckpoint::Written(_)));
    let later = imported(&store, &reader);
    assert_eq!(by_source(&later), [("refs/redkiln/journal/a1", 1)]);
    assert!(later.rewritten.is_empty(), "{later:?}");
}

#[test]
fn a_ref_that_no_longer_descends_from_the_commit_imported_is_reported_and_still_read() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (_source, remote) = exported(dir.path());
    let reader = clone(dir.path(), &remote, "reader");
    let store = empty_store(dir.path(), "store");
    let first = imported(&store, &reader);
    assert_eq!(first.sources.len(), 3, "{first:?}");
    // Someone with write access replaces a1 with a new root holding one more line.
    let work = dir.path().join("work");
    let file = git(&work, &["show", "refs/redkiln/journal/a1:a1.jsonl"]);
    let extra = file
        .lines()
        .next()
        .expect("a line")
        .replace("\"item\":\"wi-000001\"", "\"item\":\"wi-rewrite\"");
    let extra = extra.replacen(":1\"", ":99\"", 1);
    let rewritten = format!("{file}\n{extra}\n");
    std::fs::write(work.join("rewritten.jsonl"), rewritten).expect("write");
    let blob = git(&work, &["hash-object", "-w", "rewritten.jsonl"]);
    let tree = super::refs::mktree(&work, &format!("100644 blob {blob}\ta1.jsonl\n"));
    let root = git(&work, &["commit-tree", &tree, "-m", "rewritten"]);
    git(
        &work,
        &[
            "push",
            "-q",
            "-f",
            "origin",
            &format!("{root}:refs/redkiln/journal/a1"),
        ],
    );
    let after = imported(&store, &reader);
    assert_eq!(after.rewritten, ["refs/redkiln/journal/a1"], "{after:?}");
    assert_eq!(by_source(&after), [("refs/redkiln/journal/a1", 1)]);
}

#[test]
fn a_ref_set_aside_as_broken_is_read_commit_by_commit_and_only_its_journal_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (_source, remote) = exported(dir.path());
    let work = dir.path().join("work");
    let a1 = git(&work, &["rev-parse", "refs/redkiln/journal/a1"]);
    let journal = git(&work, &["rev-parse", "refs/redkiln/journal/a1:a1.jsonl"]);
    let readme = git(&work, &["hash-object", "-w", "README.md"]);
    let listing = format!("100644 blob {journal}\ta1.jsonl\n100644 blob {readme}\tother\n");
    let tree = super::refs::mktree(&work, &listing);
    let broken = git(&work, &["commit-tree", &tree, "-p", &a1, "-m", "broken"]);
    let aside = format!("refs/redkiln/broken/a1/{broken}");
    git(
        &work,
        &["push", "-q", "origin", &format!("{broken}:{aside}")],
    );
    let reader = clone(dir.path(), &remote, "reader");
    let store = empty_store(dir.path(), "store");
    let report = imported(&store, &reader);
    let reads: Vec<_> = report
        .sources
        .iter()
        .filter(|imported| imported.source == aside)
        .map(|imported| imported.commit.as_str())
        .collect();
    assert_eq!(reads, [a1.as_str(), broken.as_str()], "oldest first");
    assert!(report.passed_over.is_empty(), "{report:?}");
    let again = imported(&store, &reader);
    assert!(
        again
            .sources
            .iter()
            .all(|imported| imported.source != aside),
        "the tip was imported: {again:?}"
    );
}

#[test]
fn a_journal_ref_whose_file_is_not_a_regular_one_is_passed_over() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (_source, remote) = exported(dir.path());
    let work = dir.path().join("work");
    let journal = git(&work, &["rev-parse", "refs/redkiln/journal/a1:a1.jsonl"]);
    let tree = super::refs::mktree(&work, &format!("100755 blob {journal}\tx9.jsonl\n"));
    let root = git(&work, &["commit-tree", &tree, "-m", "executable"]);
    git(
        &work,
        &[
            "push",
            "-q",
            "origin",
            &format!("{root}:refs/redkiln/journal/x9"),
        ],
    );
    let reader = clone(dir.path(), &remote, "reader");
    let store = empty_store(dir.path(), "store");
    let report = imported(&store, &reader);
    assert_eq!(
        report.passed_over,
        ["refs/redkiln/journal/x9"],
        "{report:?}"
    );
}

#[cfg(unix)]
#[test]
fn an_inbox_branch_not_yet_relayed_is_imported() {
    let dir = tempfile::tempdir().expect("tempdir");
    let remote = remote(dir.path());
    super::git_fixture::refuse_custom_refs(&remote);
    let cloud = clone(dir.path(), &remote, "cloud");
    let at = dir.path().join("source");
    std::fs::create_dir_all(&at).expect("dir");
    let source = source_store(&at);
    let outcome = checkpoint_refs(&source, RepoClass::Own, &target(&cloud)).expect("ok");
    assert!(matches!(outcome, RefsCheckpoint::Written(_)));
    let reader = clone(dir.path(), &remote, "reader");
    let store = empty_store(dir.path(), "store");
    let report = imported(&store, &reader);
    let sources = by_source(&report);
    assert!(
        sources.contains(&("refs/heads/redkiln/inbox/a1", 2)),
        "{sources:?}"
    );
}

#[test]
fn a_failed_fetch_is_reported_and_the_clones_own_copies_are_read() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (_source, _remote) = exported(dir.path());
    let work = dir.path().join("work");
    let gone = dir.path().join("no-such-remote.git");
    git(
        &work,
        &["remote", "set-url", "origin", &gone.to_string_lossy()],
    );
    let store = empty_store(dir.path(), "store");
    let report = imported(&store, &work);
    assert_eq!(report.fetch_failed.len(), 2, "{report:?}");
    assert_eq!(report.sources.len(), 3, "{report:?}");
}

#[test]
fn records_ingested_from_another_store_do_not_stand_for_this_ones_imports() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (_source, remote) = exported(dir.path());
    let reader = clone(dir.path(), &remote, "reader");
    let other = empty_store(dir.path(), "other");
    assert_eq!(imported(&other, &reader).sources.len(), 3);
    // The other store's import records reach this store through a journal, as one a build
    // that still exported them wrote.
    let of_type = Filter::all()
        .of_type(ImportCompleted::TYPE)
        .expect("a type");
    let records = other
        .read(&of_type, 0, NonZeroUsize::new(100).expect("non-zero"))
        .expect("read");
    assert_eq!(records.len(), 3, "one record per ref");
    let journal: Vec<u8> = records
        .iter()
        .flat_map(|stored| encode_line(stored).expect("a line"))
        .collect();
    let path = dir.path().join("records.jsonl");
    std::fs::write(&path, journal).expect("a journal");
    let store = empty_store(dir.path(), "store");
    let ingested = import_file(&store, &path, ts(), importer()).expect("ingested");
    assert!(!ingested.incomplete, "{ingested:?}");
    let report = imported(&store, &reader);
    assert!(
        by_source(&report)
            .iter()
            .any(|(source, _)| *source == "refs/redkiln/journal/a1"),
        "{report:?}"
    );
    assert!(report.rewritten.is_empty(), "{report:?}");
}

#[test]
fn two_clones_importing_each_other_settle() {
    let dir = tempfile::tempdir().expect("tempdir");
    let remote = remote(dir.path());
    let a_clone = clone(dir.path(), &remote, "a");
    let b_clone = clone(dir.path(), &remote, "b");
    let a = one_event_store(dir.path(), "store-a");
    let b = empty_store(dir.path(), "store-b");
    b.append(
        vec![raised("wi-000002", Some("b2"), Trust::asserted())],
        None,
    )
    .expect("appended");
    let export =
        |store: &Store, repo: &Path| match checkpoint_refs(store, RepoClass::Own, &target(repo))
            .expect("exported")
        {
            RefsCheckpoint::Written(exported) => exported,
            RefsCheckpoint::Client => panic!("an own repository is exported"),
        };

    assert_eq!(export(&a, &a_clone).appended, 1);
    assert_eq!(
        by_source(&imported(&b, &b_clone)),
        [("refs/redkiln/journal/a1", 1)]
    );
    assert_eq!(export(&b, &b_clone).appended, 1, "b2 alone");
    assert_eq!(
        by_source(&imported(&a, &a_clone)),
        [
            ("refs/redkiln/journal/a1", 0),
            ("refs/redkiln/journal/b2", 1)
        ]
    );
    let last = export(&a, &a_clone);
    assert_eq!(last.appended, 0, "{last:?}");
    assert_eq!(
        by_source(&imported(&b, &b_clone)),
        [("refs/redkiln/journal/b2", 0)],
        "its own ref, read once"
    );
    assert_eq!(export(&b, &b_clone).appended, 0);
    assert!(imported(&a, &a_clone).sources.is_empty(), "settled");
    assert!(imported(&b, &b_clone).sources.is_empty(), "settled");
}

#[test]
fn a_broken_ref_whose_tip_has_no_journal_file_is_reported_and_not_recorded_again() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (_source, remote) = exported(dir.path());
    let work = dir.path().join("work");
    let a1 = git(&work, &["rev-parse", "refs/redkiln/journal/a1"]);
    let readme = git(&work, &["hash-object", "-w", "README.md"]);
    let tree = super::refs::mktree(&work, &format!("100644 blob {readme}\tother\n"));
    let broken = git(&work, &["commit-tree", &tree, "-p", &a1, "-m", "broken"]);
    let aside = format!("refs/redkiln/broken/a1/{broken}");
    git(
        &work,
        &["push", "-q", "origin", &format!("{broken}:{aside}")],
    );
    let reader = clone(dir.path(), &remote, "reader");
    let store = empty_store(dir.path(), "store");
    let first = imported(&store, &reader);
    assert!(first.passed_over.contains(&aside), "{first:?}");
    let records = decode_all::<ImportCompleted>(&all(&store)).events.len();
    let again = imported(&store, &reader);
    assert!(again.sources.is_empty(), "{again:?}");
    assert!(again.passed_over.contains(&aside), "{again:?}");
    assert_eq!(
        decode_all::<ImportCompleted>(&all(&store)).events.len(),
        records,
        "nothing recorded again"
    );
}

#[test]
fn past_its_deadline_an_import_fetches_and_reads_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (_source, remote) = exported(dir.path());
    let reader = clone(dir.path(), &remote, "reader");
    let store = empty_store(dir.path(), "store");
    let late = RefsTarget {
        deadline: Some(std::time::Instant::now()),
        ..target(&reader)
    };
    let report = import_refs(&store, &late, &ts(), &importer()).expect("ran");
    assert!(report.late, "{report:?}");
    assert!(report.sources.is_empty(), "{report:?}");
    assert!(all(&store).is_empty(), "nothing recorded");
}

#[test]
fn a_name_that_is_no_journals_is_passed_over() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (_source, remote) = exported(dir.path());
    let work = dir.path().join("work");
    let a1 = git(&work, &["rev-parse", "refs/redkiln/journal/a1"]);
    let bad = "refs/redkiln/journal/Not.A.Name";
    git(&work, &["push", "-q", "origin", &format!("{a1}:{bad}")]);
    let reader = clone(dir.path(), &remote, "reader");
    let store = empty_store(dir.path(), "store");
    let report = imported(&store, &reader);
    assert_eq!(report.passed_over, [bad], "{report:?}");
}

#[cfg(unix)]
#[test]
fn an_inbox_is_read_once_per_commit_and_a_deleted_one_is_pruned_alone() {
    let dir = tempfile::tempdir().expect("tempdir");
    let remote = remote(dir.path());
    super::git_fixture::refuse_custom_refs(&remote);
    let cloud = clone(dir.path(), &remote, "cloud");
    let at = dir.path().join("source");
    std::fs::create_dir_all(&at).expect("dir");
    let source = source_store(&at);
    let outcome = checkpoint_refs(&source, RepoClass::Own, &target(&cloud)).expect("ok");
    assert!(matches!(outcome, RefsCheckpoint::Written(_)));
    let reader = clone(dir.path(), &remote, "reader");
    let store = empty_store(dir.path(), "store");
    let first = imported(&store, &reader);
    assert!(!first.sources.is_empty(), "{first:?}");
    let again = imported(&store, &reader);
    assert!(again.sources.is_empty(), "unchanged inboxes: {again:?}");
    git(
        &remote,
        &["update-ref", "-d", "refs/heads/redkiln/inbox/a1"],
    );
    let head = git(&reader, &["rev-parse", "HEAD"]);
    git(&reader, &["update-ref", "refs/redkiln/broken/zz/x", &head]);
    let pruned = imported(&store, &reader);
    assert!(pruned.fetch_failed.is_empty(), "{pruned:?}");
    assert_eq!(
        git(&reader, &["for-each-ref", "refs/redkiln/inbox/a1"]),
        "",
        "the deleted inbox's copy is pruned"
    );
    assert_ne!(
        git(&reader, &["for-each-ref", "refs/redkiln/broken/zz/x"]),
        "",
        "a broken ref only this clone holds is not pruned"
    );
}
