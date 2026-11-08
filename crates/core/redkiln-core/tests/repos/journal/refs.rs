//! Journal refs and the cloud inbox through real repositories (SPEC JR-05 to JR-09).

use std::path::Path;
use std::time::Duration;

use redkiln_core::journal::refs::{
    Breach, Deferral, RefsCheckpoint, RefsExported, RefsScope, RefsTarget, checkpoint_refs,
    checkpoint_refs_since, import_refs,
};
// Every relay test needs a git hook script, so runs only on unix.
#[cfg(unix)]
use redkiln_core::journal::refs::{Deleted, Refusal, Relayed, relay, sweep_refs_since};
use redkiln_core::store::{Store, Wait};
use redkiln_core::suite::RepoClass;
use redkiln_core::trust::Trust;

#[cfg(unix)]
use super::git_fixture::{accept_custom_refs, count_remote_calls, executable, refuse_custom_refs};
use super::git_fixture::{clone, file_at, git, refs, remote};
use super::{
    ImportCompleted, all, decode_all, importer, one_event_store, raised, record_an_import,
    source_store, ts,
};

const TIMEOUT: Duration = Duration::from_secs(30);

/// The deadline of a checkpoint cut short: far enough off for its first journal's local reads on
/// a slow runner, and well before [`SLOW_PUSH`] ends.
#[cfg(unix)]
const CUT: Duration = Duration::from_secs(3);

/// A `pre-receive` hook that takes twice [`CUT`], so the push that starts first ends after the
/// deadline.
#[cfg(unix)]
const SLOW_PUSH: &str = "#!/bin/sh\ncat >/dev/null\nsleep 6\n";

fn target(repo: &Path) -> RefsTarget<'_> {
    RefsTarget {
        repo,
        remote: "origin",
        timeout: TIMEOUT,
        deadline: None,
    }
}

fn written(outcome: RefsCheckpoint) -> RefsExported {
    match outcome {
        RefsCheckpoint::Written(exported) => exported,
        RefsCheckpoint::Client => panic!("an own repository is exported"),
    }
}

fn line_count(text: &str) -> usize {
    text.lines().count()
}

/// A store in `dir/<name>/`.
fn store_in(dir: &Path, name: &str) -> Store {
    let at = dir.join(name);
    std::fs::create_dir_all(&at).expect("dir");
    source_store(&at)
}

#[test]
fn a_client_repository_writes_no_journal_ref() {
    let dir = tempfile::tempdir().expect("tempdir");
    let remote = remote(dir.path());
    let work = clone(dir.path(), &remote, "work");
    let store = store_in(dir.path(), "store");
    let outcome = checkpoint_refs(&store, RepoClass::Client, &target(&work)).expect("no failure");
    assert_eq!(outcome, RefsCheckpoint::Client);
    assert!(refs(&remote, "refs/redkiln").is_empty());
    assert!(refs(&work, "refs/redkiln").is_empty());
}

#[test]
fn a_checkpoint_pushes_one_ref_per_journal_as_a_root_commit_with_one_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let remote = remote(dir.path());
    let work = clone(dir.path(), &remote, "work");
    let store = store_in(dir.path(), "store");
    let report = written(checkpoint_refs(&store, RepoClass::Own, &target(&work)).expect("ok"));
    assert_eq!(report.appended, 4, "{report:?}");
    assert_eq!(report.unexportable, 1, "the raw event has no line");
    assert!(report.relayed.is_empty() && report.deferred.is_empty() && !report.local_only);
    let pushed = refs(&remote, "refs/redkiln/journal");
    assert_eq!(pushed.len(), 3, "a1, b2 and the store's: {pushed:?}");
    assert!(pushed.contains(&"refs/redkiln/journal/a1".to_owned()));
    let a1 = file_at(&remote, "refs/redkiln/journal/a1", "a1.jsonl").expect("a1.jsonl");
    assert_eq!(line_count(&a1), 2);
    assert!(a1.contains("wi-000001") && a1.contains("wi-000004"));
    let tree = git(
        &remote,
        &["ls-tree", "--name-only", "refs/redkiln/journal/a1"],
    );
    assert_eq!(tree, "a1.jsonl", "exactly one file");
    let parents = git(
        &remote,
        &["rev-list", "--parents", "-n1", "refs/redkiln/journal/a1"],
    );
    assert_eq!(parents.split(' ').count(), 1, "a root commit");
    assert_eq!(
        git(&work, &["rev-parse", "refs/redkiln/journal/a1"]),
        git(&remote, &["rev-parse", "refs/redkiln/journal/a1"]),
        "the clone's copy follows the push"
    );
}

#[test]
fn a_checkpoint_with_nothing_new_touches_no_remote() {
    let dir = tempfile::tempdir().expect("tempdir");
    let remote = remote(dir.path());
    let work = clone(dir.path(), &remote, "work");
    let store = store_in(dir.path(), "store");
    written(checkpoint_refs(&store, RepoClass::Own, &target(&work)).expect("ok"));
    let gone = dir.path().join("no-such-remote.git");
    git(
        &work,
        &["remote", "set-url", "origin", &gone.to_string_lossy()],
    );
    let again = written(checkpoint_refs(&store, RepoClass::Own, &target(&work)).expect("ok"));
    assert_eq!(
        again,
        RefsExported {
            unexportable: 1,
            ..RefsExported::default()
        }
    );
}

#[test]
fn a_new_event_is_appended_on_top_of_the_previous_commit() {
    let dir = tempfile::tempdir().expect("tempdir");
    let remote = remote(dir.path());
    let work = clone(dir.path(), &remote, "work");
    let store = store_in(dir.path(), "store");
    written(checkpoint_refs(&store, RepoClass::Own, &target(&work)).expect("ok"));
    let before = git(&remote, &["rev-parse", "refs/redkiln/journal/a1"]);
    let old = file_at(&remote, &before, "a1.jsonl").expect("file");
    store
        .append(
            vec![raised("wi-000005", Some("A1"), Trust::asserted())],
            None,
        )
        .expect("appended");
    let report = written(checkpoint_refs(&store, RepoClass::Own, &target(&work)).expect("ok"));
    assert_eq!(report.appended, 1);
    assert_eq!(report.direct.len(), 1);
    let parents = git(
        &remote,
        &["rev-list", "--parents", "-n1", "refs/redkiln/journal/a1"],
    );
    assert!(parents.ends_with(&before), "chained on the previous commit");
    let new = file_at(&remote, "refs/redkiln/journal/a1", "a1.jsonl").expect("file");
    assert!(new.starts_with(&old), "append-only (JR-05)");
    assert_eq!(line_count(&new), 3);
}

#[test]
fn a_checkpoint_since_a_position_examines_only_the_journals_with_a_later_event() {
    let dir = tempfile::tempdir().expect("tempdir");
    let remote = remote(dir.path());
    let work = clone(dir.path(), &remote, "work");
    let store = store_in(dir.path(), "store");
    let first = checkpoint_refs_since(&store, RepoClass::Own, &target(&work), RefsScope::since(0))
        .expect("ok");
    assert_eq!(
        written(first.outcome).direct.len(),
        3,
        "since 0 is every journal"
    );
    // b2's ref goes from both sides: a scan that examined b2 would push it again.
    git(&remote, &["update-ref", "-d", "refs/redkiln/journal/b2"]);
    git(&work, &["update-ref", "-d", "refs/redkiln/journal/b2"]);
    store
        .append(
            vec![raised("wi-000005", Some("A1"), Trust::asserted())],
            None,
        )
        .expect("appended");

    let scoped = checkpoint_refs_since(
        &store,
        RepoClass::Own,
        &target(&work),
        RefsScope::since(first.through),
    )
    .expect("ok");

    assert!(scoped.through > first.through, "the new event was read");
    let report = written(scoped.outcome);
    assert_eq!(report.direct, [name("a1")], "{report:?}");
    assert_eq!(report.appended, 1);
    assert!(!refs(&remote, "refs/redkiln/journal").contains(&"refs/redkiln/journal/b2".to_owned()));
    let swept = written(checkpoint_refs(&store, RepoClass::Own, &target(&work)).expect("ok"));
    assert_eq!(swept.direct, [name("b2")], "a full checkpoint is the sweep");
}

#[test]
fn a_checkpoint_since_the_last_position_touches_no_git() {
    let dir = tempfile::tempdir().expect("tempdir");
    let remote = remote(dir.path());
    let work = clone(dir.path(), &remote, "work");
    let store = store_in(dir.path(), "store");
    let first = checkpoint_refs_since(&store, RepoClass::Own, &target(&work), RefsScope::since(0))
        .expect("ok");
    // With every journal before the position, not one ref is read: a broken one goes unseen.
    // The clone's own commit: its tree is not exactly `a1.jsonl`, so the ref breaks JR-05.
    git(&work, &["update-ref", "refs/redkiln/journal/a1", "HEAD"]);

    let again = checkpoint_refs_since(
        &store,
        RepoClass::Own,
        &target(&work),
        RefsScope::since(first.through),
    )
    .expect("ok");

    assert_eq!(again.through, first.through);
    assert_eq!(
        written(again.outcome),
        RefsExported {
            unexportable: 1,
            ..RefsExported::default()
        }
    );
}

#[test]
fn another_clones_lines_are_kept_and_this_ones_appended() {
    let dir = tempfile::tempdir().expect("tempdir");
    let remote = remote(dir.path());
    let first = clone(dir.path(), &remote, "first");
    let second = clone(dir.path(), &remote, "second");
    let theirs = store_in(dir.path(), "theirs");
    written(checkpoint_refs(&theirs, RepoClass::Own, &target(&first)).expect("ok"));
    let before = file_at(&remote, "refs/redkiln/journal/a1", "a1.jsonl").expect("file");
    let ours = Store::open(&dir.path().join("ours.sqlite3"), Wait::Command).expect("opens");
    ours.append(
        vec![raised("wi-000009", Some("a1"), Trust::asserted())],
        None,
    )
    .expect("appended");
    let report = written(checkpoint_refs(&ours, RepoClass::Own, &target(&second)).expect("ok"));
    assert_eq!(report.appended, 1, "{report:?}");
    let after = file_at(&remote, "refs/redkiln/journal/a1", "a1.jsonl").expect("file");
    assert!(after.starts_with(&before));
    assert_eq!(line_count(&after), 3);
}

#[test]
fn a_ref_breaking_jr_05_is_reported_and_left_alone() {
    let dir = tempfile::tempdir().expect("tempdir");
    let remote = remote(dir.path());
    let work = clone(dir.path(), &remote, "work");
    let commit = push_broken_ref(&work, "a1");
    let store = store_in(dir.path(), "store");
    let report = written(checkpoint_refs(&store, RepoClass::Own, &target(&work)).expect("ok"));
    assert_eq!(report.broken, vec![(name("a1"), Breach::Tree)]);
    assert_eq!(
        git(&remote, &["rev-parse", "refs/redkiln/journal/a1"]),
        commit
    );
    assert_eq!(
        report.direct.len(),
        2,
        "the other journals are still written"
    );
}

/// Pushes, from `work`, a journal ref for `name` whose tree holds a second file; its commit.
fn push_broken_ref(work: &Path, name: &str) -> String {
    let blob = git(work, &["hash-object", "-w", "README.md"]);
    let listing = format!("100644 blob {blob}\t{name}.jsonl\n100644 blob {blob}\tother\n");
    let tree = mktree(work, &listing);
    let commit = git(work, &["commit-tree", &tree, "-m", "broken"]);
    let spec = format!("{commit}:refs/redkiln/journal/{name}");
    git(work, &["push", "-q", "origin", &spec]);
    commit
}

#[test]
fn once_a_person_moves_a_broken_ref_aside_the_next_write_starts_a_new_root() {
    let dir = tempfile::tempdir().expect("tempdir");
    let remote = remote(dir.path());
    let work = clone(dir.path(), &remote, "work");
    let broken = push_broken_ref(&work, "a1");
    let store = store_in(dir.path(), "store");
    written(checkpoint_refs(&store, RepoClass::Own, &target(&work)).expect("ok"));
    let aside = format!("refs/redkiln/broken/a1/{broken}");
    git(&remote, &["update-ref", &aside, &broken]);
    git(&remote, &["update-ref", "-d", "refs/redkiln/journal/a1"]);
    let report = written(checkpoint_refs(&store, RepoClass::Own, &target(&work)).expect("ok"));
    assert_eq!(report.direct, vec![name("a1")], "{report:?}");
    assert!(report.broken.is_empty(), "{report:?}");
    let parents = git(
        &remote,
        &["rev-list", "--parents", "-n1", "refs/redkiln/journal/a1"],
    );
    assert_eq!(parents.split(' ').count(), 1, "a new root commit");
}

#[test]
fn past_its_deadline_a_checkpoint_defers_every_journal() {
    let dir = tempfile::tempdir().expect("tempdir");
    let remote = remote(dir.path());
    let work = clone(dir.path(), &remote, "work");
    let store = store_in(dir.path(), "store");
    let late = RefsTarget {
        deadline: Some(std::time::Instant::now()),
        ..target(&work)
    };
    let report = written(checkpoint_refs(&store, RepoClass::Own, &late).expect("ok"));
    assert_eq!(report.deferred.len(), 3, "{report:?}");
    assert!(
        report
            .deferred
            .iter()
            .all(|(_, why)| *why == Deferral::Late)
    );
    assert!(refs(&remote, "refs/redkiln").is_empty());
}

#[test]
fn past_its_deadline_a_checkpoint_does_not_fetch() {
    let dir = tempfile::tempdir().expect("tempdir");
    let remote = remote(dir.path());
    let work = clone(dir.path(), &remote, "work");
    let gone = dir.path().join("gone");
    git(
        &work,
        &["remote", "set-url", "origin", &gone.to_string_lossy()],
    );
    let store = store_in(dir.path(), "store");
    let late = RefsTarget {
        deadline: Some(std::time::Instant::now()),
        ..target(&work)
    };
    let report = written(checkpoint_refs(&store, RepoClass::Own, &late).expect("ok"));
    assert_eq!(report.deferred.len(), 3, "{report:?}");
    assert!(
        report
            .deferred
            .iter()
            .all(|(_, why)| *why == Deferral::Late),
        "a fetch would have failed: {report:?}"
    );
}

#[cfg(unix)]
#[test]
fn a_checkpoint_cut_short_by_its_deadline_reads_through_the_journals_it_finished() {
    let dir = tempfile::tempdir().expect("tempdir");
    let remote = remote(dir.path());
    let work = clone(dir.path(), &remote, "work");
    // A slow push: the first journal starts before the deadline and ends after it.
    executable(&remote.join("hooks/pre-receive"), SLOW_PUSH);
    // Positions: 1 a1, 2 b2, 3 the store's, 4 raw, 5 a1; so the oldest last event is b2's.
    let store = store_in(dir.path(), "store");
    let cut = RefsTarget {
        deadline: Some(std::time::Instant::now() + CUT),
        ..target(&work)
    };

    let first =
        checkpoint_refs_since(&store, RepoClass::Own, &cut, RefsScope::since(0)).expect("ok");

    let report = written(first.outcome);
    assert_eq!(
        report.direct,
        [name("b2")],
        "journals go oldest last event first"
    );
    assert!(
        report
            .deferred
            .iter()
            .all(|(_, why)| *why == Deferral::Late),
        "{report:?}"
    );
    assert_eq!(
        first.through, 2,
        "through b2, before the first journal left"
    );
    accept_custom_refs(&remote);
    let rest = checkpoint_refs_since(
        &store,
        RepoClass::Own,
        &target(&work),
        RefsScope::since(first.through),
    )
    .expect("ok");
    assert_eq!(rest.through, 5, "the whole store");
    assert_eq!(written(rest.outcome).direct.len(), 2, "the store's and a1");
}

#[cfg(unix)]
#[test]
fn a_sweep_writes_the_journals_past_its_position_first_and_settles_only_those() {
    let dir = tempfile::tempdir().expect("tempdir");
    let remote = remote(dir.path());
    let work = clone(dir.path(), &remote, "work");
    let store = store_in(dir.path(), "store");
    let first = checkpoint_refs_since(&store, RepoClass::Own, &target(&work), RefsScope::since(0))
        .expect("ok");
    assert_eq!(first.through, 5);
    // a1 and the store's journal lose their refs, so a sweep has them to write again; b2 gains
    // an event past the position, so it is the one new journal.
    for name in refs(&remote, "refs/redkiln/journal") {
        if !name.ends_with("/b2") {
            git(&remote, &["update-ref", "-d", &name]);
            git(&work, &["update-ref", "-d", &name]);
        }
    }
    store
        .append(
            vec![raised("wi-000006", Some("b2"), Trust::asserted())],
            None,
        )
        .expect("appended");
    executable(&remote.join("hooks/pre-receive"), SLOW_PUSH);
    let cut = RefsTarget {
        deadline: Some(std::time::Instant::now() + CUT),
        ..target(&work)
    };

    let swept = sweep_refs_since(
        &store,
        RepoClass::Own,
        &cut,
        RefsScope::since(first.through),
    )
    .expect("ok");

    assert_eq!(swept.through, 6, "the journal past the position is settled");
    let report = written(swept.outcome);
    assert_eq!(report.direct, [name("b2")], "new work first: {report:?}");
    assert_eq!(report.deferred.len(), 2, "the older two, late: {report:?}");
}

#[cfg(unix)]
#[test]
fn a_checkpoint_writes_its_first_journal_before_the_others_and_settles_in_position_order() {
    let dir = tempfile::tempdir().expect("tempdir");
    let remote = remote(dir.path());
    let work = clone(dir.path(), &remote, "work");
    // A slow push: the first journal starts before the deadline and ends after it.
    executable(&remote.join("hooks/pre-receive"), SLOW_PUSH);
    // Positions: 1 a1, 2 b2, 3 the store's, 4 raw, 5 a1; so a1 is the newest journal.
    let store = store_in(dir.path(), "store");
    let cut = RefsTarget {
        deadline: Some(std::time::Instant::now() + CUT),
        ..target(&work)
    };
    let a1 = name("a1");

    let scoped = checkpoint_refs_since(
        &store,
        RepoClass::Own,
        &cut,
        RefsScope {
            first: Some(&a1),
            ..RefsScope::since(0)
        },
    )
    .expect("ok");

    let report = written(scoped.outcome);
    assert_eq!(
        report.direct,
        [a1],
        "the first journal goes first: {report:?}"
    );
    assert_eq!(report.deferred.len(), 2, "the others, late: {report:?}");
    assert_eq!(
        scoped.through, 1,
        "b2, at 2, is not settled, so a1's later position is not either"
    );
}

#[cfg(unix)]
#[test]
fn a_journal_whose_push_fails_after_the_deadline_asks_the_remote_nothing_more() {
    let dir = tempfile::tempdir().expect("tempdir");
    let remote = remote(dir.path());
    let work = clone(dir.path(), &remote, "work");
    executable(
        &remote.join("hooks/pre-receive"),
        "#!/bin/sh\ncat >/dev/null\nsleep 6\necho closed >&2\nexit 1\n",
    );
    // Every call to the remote, in order: fetches and ls-remote run upload-pack, pushes
    // receive-pack, both as this clone's config names them.
    let calls = dir.path().join("calls");
    count_remote_calls(dir.path(), &work, &calls, 0);
    let store = store_in(dir.path(), "store");
    let cut = RefsTarget {
        deadline: Some(std::time::Instant::now() + CUT),
        ..target(&work)
    };

    let report = written(checkpoint_refs(&store, RepoClass::Own, &cut).expect("ok"));

    assert!(
        report.direct.is_empty() && report.relayed.is_empty(),
        "{report:?}"
    );
    assert_eq!(report.deferred.len(), 3, "{report:?}");
    let made = std::fs::read_to_string(&calls).expect("calls");
    assert_eq!(
        made.lines().collect::<Vec<_>>(),
        ["upload", "receive"],
        "the journal refs' fetch, one push, and no relay base read after it"
    );
}

#[cfg(unix)]
#[test]
fn a_remote_that_hangs_defers_the_journals_instead_of_failing_the_checkpoint() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = tempfile::tempdir().expect("tempdir");
    let remote = remote(dir.path());
    let work = clone(dir.path(), &remote, "work");
    let hook = remote.join("hooks/pre-receive");
    // Only a push runs the hook, so fetches and local steps finish well inside the limit; the
    // hook ends by itself soon after the test, whichever process outlives the killed push.
    std::fs::write(&hook, "#!/bin/sh\nsleep 12\n").expect("hook");
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let store = store_in(dir.path(), "store");
    let quick = RefsTarget {
        timeout: Duration::from_secs(4),
        ..target(&work)
    };
    let report = written(checkpoint_refs(&store, RepoClass::Own, &quick).expect("not an error"));
    let timed_out = |said: &str| said.contains("did not answer");
    assert!(
        report.relay_reason.as_deref().is_some_and(timed_out),
        "{report:?}"
    );
    assert!(
        report.direct.is_empty() && report.relayed.is_empty(),
        "{report:?}"
    );
    assert_eq!(report.deferred.len(), 3, "{report:?}");
    assert!(report.broken.is_empty(), "{report:?}");
    for (name, deferral) in &report.deferred {
        assert!(
            matches!(deferral, Deferral::Failed(said) if timed_out(said)),
            "{name}: {deferral:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn the_relay_leaves_an_inbox_whose_journal_ref_is_broken() {
    let dir = tempfile::tempdir().expect("tempdir");
    let remote = remote(dir.path());
    refuse_custom_refs(&remote);
    let cloud = clone(dir.path(), &remote, "cloud");
    let store = store_in(dir.path(), "store");
    written(checkpoint_refs(&store, RepoClass::Own, &target(&cloud)).expect("ok"));
    accept_custom_refs(&remote);
    let actions = clone(dir.path(), &remote, "actions");
    push_broken_ref(&actions, "a1");
    let outcome = relay(&target(&actions), "redkiln/inbox/a1").expect("ran");
    assert_eq!(outcome, Relayed::Broken(Breach::Tree));
    assert!(
        refs(&remote, "refs/heads/redkiln").contains(&"refs/heads/redkiln/inbox/a1".to_owned())
    );
}

#[cfg(unix)]
#[test]
fn the_relay_refuses_an_inbox_commit_with_no_parent() {
    let dir = tempfile::tempdir().expect("tempdir");
    let remote = remote(dir.path());
    let work = clone(dir.path(), &remote, "work");
    let blob = git(&work, &["hash-object", "-w", "README.md"]);
    let tree = mktree(&work, &format!("100644 blob {blob}\tREADME.md\n"));
    let root = git(&work, &["commit-tree", &tree, "-m", "orphan"]);
    let spec = format!("{root}:refs/heads/redkiln/inbox/a1");
    git(&work, &["push", "-q", "origin", &spec]);
    let outcome = relay(&target(&work), "redkiln/inbox/a1").expect("ran");
    assert_eq!(outcome, Relayed::Refused(Refusal::Parents(0)));
}

#[test]
fn with_no_remote_only_local_refs_move() {
    let dir = tempfile::tempdir().expect("tempdir");
    let work = dir.path().join("work");
    std::fs::create_dir_all(&work).expect("dir");
    git(&work, &["init", "-q"]);
    let store = store_in(dir.path(), "store");
    let report = written(checkpoint_refs(&store, RepoClass::Own, &target(&work)).expect("ok"));
    assert!(report.local_only);
    assert_eq!(report.direct.len(), 3);
    let a1 = git(&work, &["show", "refs/redkiln/journal/a1:a1.jsonl"]);
    assert_eq!(line_count(&a1), 2);
}

#[cfg(unix)]
#[test]
fn where_refs_are_refused_the_checkpoint_pushes_inbox_branches_on_the_default_branch() {
    let dir = tempfile::tempdir().expect("tempdir");
    let remote = remote(dir.path());
    refuse_custom_refs(&remote);
    let work = clone(dir.path(), &remote, "work");
    let store = store_in(dir.path(), "store");
    let report = written(checkpoint_refs(&store, RepoClass::Own, &target(&work)).expect("ok"));
    assert!(report.direct.is_empty(), "{report:?}");
    assert_eq!(report.relayed.len(), 3, "{report:?}");
    assert!(
        report
            .relay_reason
            .as_deref()
            .is_some_and(|r| r.contains("outside refs/heads")),
        "{report:?}"
    );
    assert!(refs(&remote, "refs/redkiln").is_empty());
    let branch = "refs/heads/redkiln/inbox/a1";
    let main = git(&remote, &["rev-parse", "main"]);
    let parents = git(&remote, &["rev-list", "--parents", "-n1", branch]);
    assert!(
        parents.ends_with(&main),
        "its parent is the default branch's tip"
    );
    let changed = git(
        &remote,
        &["diff-tree", "-r", "--name-status", "main", branch],
    );
    assert_eq!(changed, "A\t.redkiln/inbox/a1.jsonl");
    let file = file_at(&remote, branch, ".redkiln/inbox/a1.jsonl").expect("inbox file");
    assert_eq!(line_count(&file), 2);

    let again = written(checkpoint_refs(&store, RepoClass::Own, &target(&work)).expect("ok"));
    assert!(
        again.relayed.is_empty() && again.deferred.is_empty(),
        "{again:?}"
    );
}

#[cfg(unix)]
#[test]
fn an_inbox_another_clone_wrote_under_the_same_name_is_kept() {
    let dir = tempfile::tempdir().expect("tempdir");
    let remote = remote(dir.path());
    refuse_custom_refs(&remote);
    let first = clone(dir.path(), &remote, "first");
    let second = clone(dir.path(), &remote, "second");
    let theirs = store_in(dir.path(), "theirs");
    written(checkpoint_refs(&theirs, RepoClass::Own, &target(&first)).expect("ok"));
    let ours = Store::open(&dir.path().join("ours.sqlite3"), Wait::Command).expect("opens");
    ours.append(
        vec![raised("wi-000009", Some("a1"), Trust::asserted())],
        None,
    )
    .expect("appended");
    let report = written(checkpoint_refs(&ours, RepoClass::Own, &target(&second)).expect("ok"));
    assert_eq!(report.relayed, vec![name("a1")], "{report:?}");
    let file = file_at(&remote, "redkiln/inbox/a1", ".redkiln/inbox/a1.jsonl").expect("file");
    assert_eq!(line_count(&file), 3, "theirs kept, ours added");
}

#[cfg(unix)]
#[test]
fn the_relay_merges_an_inbox_into_its_ref_and_deletes_the_branch() {
    let dir = tempfile::tempdir().expect("tempdir");
    let remote = remote(dir.path());
    refuse_custom_refs(&remote);
    let cloud = clone(dir.path(), &remote, "cloud");
    let store = store_in(dir.path(), "store");
    written(checkpoint_refs(&store, RepoClass::Own, &target(&cloud)).expect("ok"));
    let inbox = file_at(&remote, "redkiln/inbox/a1", ".redkiln/inbox/a1.jsonl").expect("file");
    accept_custom_refs(&remote);
    let actions = clone(dir.path(), &remote, "actions");
    let outcome = relay(&target(&actions), "refs/heads/redkiln/inbox/a1").expect("relayed");
    assert_eq!(
        outcome,
        Relayed::Merged {
            added: 2,
            deleted: Deleted::Yes,
        }
    );
    let journal = file_at(&remote, "refs/redkiln/journal/a1", "a1.jsonl").expect("journal");
    assert_eq!(journal, inbox);
    assert!(
        !refs(&remote, "refs/heads/redkiln").contains(&"refs/heads/redkiln/inbox/a1".to_owned())
    );
    let again = relay(&target(&actions), "redkiln/inbox/a1").expect("ran");
    assert_eq!(again, Relayed::Gone);
}

#[cfg(unix)]
#[test]
fn the_relay_leaves_an_inbox_branch_a_newer_commit_reached_meanwhile() {
    let dir = tempfile::tempdir().expect("tempdir");
    let remote = remote(dir.path());
    refuse_custom_refs(&remote);
    let cloud = clone(dir.path(), &remote, "cloud");
    let store = store_in(dir.path(), "store");
    written(checkpoint_refs(&store, RepoClass::Own, &target(&cloud)).expect("ok"));
    accept_custom_refs(&remote);
    // Once the journal ref lands, a newer inbox commit (here, main's tip) takes the branch.
    newer_inbox_after_journal_push(&remote, "a1");
    let actions = clone(dir.path(), &remote, "actions");
    let outcome = relay(&target(&actions), "redkiln/inbox/a1").expect("relayed");
    assert_eq!(
        outcome,
        Relayed::Merged {
            added: 2,
            deleted: Deleted::NewerArrived,
        }
    );
    assert!(
        refs(&remote, "refs/heads/redkiln").contains(&"refs/heads/redkiln/inbox/a1".to_owned())
    );
}

/// A `post-receive` hook on `remote` that moves `redkiln/inbox/<name>` to `main` once
/// `refs/redkiln/journal/<name>` is pushed.
#[cfg(unix)]
fn newer_inbox_after_journal_push(remote: &Path, name: &str) {
    use std::os::unix::fs::PermissionsExt as _;
    let hook = remote.join("hooks/post-receive");
    let script = format!(
        "#!/bin/sh\nwhile read old new ref; do\n  if [ \"$ref\" = refs/redkiln/journal/{name} ]; then\n    git update-ref refs/heads/redkiln/inbox/{name} refs/heads/main\n  fi\ndone\n"
    );
    std::fs::write(&hook, script).expect("hook");
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).expect("chmod");
}

#[cfg(unix)]
#[test]
fn the_relay_refuses_an_inbox_commit_that_changes_anything_else() {
    let dir = tempfile::tempdir().expect("tempdir");
    let remote = remote(dir.path());
    let work = clone(dir.path(), &remote, "work");
    git(&work, &["checkout", "-q", "-b", "redkiln/inbox/a1"]);
    std::fs::create_dir_all(work.join(".redkiln/inbox")).expect("dir");
    std::fs::write(work.join(".redkiln/inbox/a1.jsonl"), "").expect("write");
    std::fs::write(work.join("README.md"), "changed\n").expect("write");
    git(&work, &["add", "-A"]);
    git(&work, &["commit", "-q", "-m", "sneaky"]);
    git(&work, &["push", "-q", "origin", "redkiln/inbox/a1"]);
    let outcome = relay(&target(&work), "redkiln/inbox/a1").expect("ran");
    assert!(
        matches!(outcome, Relayed::Refused(Refusal::Changes(_))),
        "{outcome:?}"
    );
    assert!(
        refs(&remote, "refs/heads/redkiln").contains(&"refs/heads/redkiln/inbox/a1".to_owned())
    );
    assert!(refs(&remote, "refs/redkiln").is_empty());
}

#[test]
fn checkpoint_refs_never_exports_an_import_record() {
    let dir = tempfile::tempdir().expect("tempdir");
    let remote = remote(dir.path());
    let work = clone(dir.path(), &remote, "work");
    let store = one_event_store(dir.path(), "store");
    record_an_import(&store, dir.path());
    let report = written(checkpoint_refs(&store, RepoClass::Own, &target(&work)).expect("ok"));
    assert_eq!((report.appended, report.unexportable), (1, 0), "{report:?}");
    assert_eq!(
        refs(&remote, "refs/redkiln/journal"),
        ["refs/redkiln/journal/a1"],
        "no store journal ref"
    );
    let a1 = file_at(&remote, "refs/redkiln/journal/a1", "a1.jsonl").expect("a1.jsonl");
    assert_eq!(line_count(&a1), 1, "{a1}");
    assert!(a1.contains("wi-000001"), "{a1}");
}

#[test]
fn a_store_whose_only_new_event_is_an_import_record_touches_no_remote() {
    let dir = tempfile::tempdir().expect("tempdir");
    let remote = remote(dir.path());
    let work = clone(dir.path(), &remote, "work");
    let store = one_event_store(dir.path(), "store");
    written(checkpoint_refs(&store, RepoClass::Own, &target(&work)).expect("ok"));
    let before = refs(&remote, "refs/redkiln");
    let report = import_refs(&store, &target(&work), &ts(), &importer()).expect("imported");
    assert_eq!(report.sources.len(), 1, "{report:?}");
    assert_eq!(
        decode_all::<ImportCompleted>(&all(&store)).events.len(),
        1,
        "the import wrote its record"
    );
    let gone = dir.path().join("no-such-remote.git");
    git(
        &work,
        &["remote", "set-url", "origin", &gone.to_string_lossy()],
    );
    let again = written(checkpoint_refs(&store, RepoClass::Own, &target(&work)).expect("ok"));
    assert_eq!(again, RefsExported::default());
    assert_eq!(
        refs(&remote, "refs/redkiln"),
        before,
        "the remote is unchanged"
    );
}

/// What makes GitHub start no `on: push` workflow for a commit, in its message.
#[cfg(unix)]
const CI_SKIP_DIRECTIVES: [&str; 6] = [
    "[skip ci]",
    "[ci skip]",
    "[no ci]",
    "[skip actions]",
    "[actions skip]",
    "skip-checks",
];

/// An inbox push must start the relay workflow, so its commit asks GitHub to skip nothing.
#[cfg(unix)]
#[test]
fn an_inbox_commit_carries_no_ci_skip_directive() {
    let dir = tempfile::tempdir().expect("tempdir");
    let remote = remote(dir.path());
    refuse_custom_refs(&remote);
    let work = clone(dir.path(), &remote, "work");
    let store = one_event_store(dir.path(), "store");
    let report = written(checkpoint_refs(&store, RepoClass::Own, &target(&work)).expect("ok"));
    assert_eq!(report.relayed, vec![name("a1")], "{report:?}");
    let message = git(
        &remote,
        &["log", "-1", "--format=%B", "refs/heads/redkiln/inbox/a1"],
    );
    assert_eq!(message, "redkiln inbox a1");
    for directive in CI_SKIP_DIRECTIVES {
        assert!(!message.contains(directive), "{message}");
    }
}

/// A journal-ref push starts no workflow either way; its marker documents the intent.
#[test]
fn a_journal_ref_commit_keeps_its_ci_skip_directive() {
    let dir = tempfile::tempdir().expect("tempdir");
    let remote = remote(dir.path());
    let work = clone(dir.path(), &remote, "work");
    let store = one_event_store(dir.path(), "store");
    let report = written(checkpoint_refs(&store, RepoClass::Own, &target(&work)).expect("ok"));
    assert_eq!(report.direct, vec![name("a1")], "{report:?}");
    let message = git(
        &remote,
        &["log", "-1", "--format=%B", "refs/redkiln/journal/a1"],
    );
    assert_eq!(message, "redkiln journal a1 [skip ci]");
}

fn name(text: &str) -> redkiln_core::journal::JournalName {
    redkiln_core::journal::JournalName::parse(text).expect("a name")
}

/// `git mktree` of `listing` in `dir`.
pub(super) fn mktree(dir: &Path, listing: &str) -> String {
    use std::io::Write as _;
    let mut child = std::process::Command::new("git")
        .arg("mktree")
        .current_dir(dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("git mktree");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(listing.as_bytes())
        .expect("written");
    let out = child.wait_with_output().expect("ran");
    assert!(out.status.success());
    String::from_utf8(out.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}
