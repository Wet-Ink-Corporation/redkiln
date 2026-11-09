//! `read_local` on real repositories, its sources built with git's plumbing.

use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::Command;

use super::*;
use crate::journal::refs::git::{GIT, Repo};

/// One file handed to the caller: its ref, kind, commit and bytes.
type Handed = (String, SourceKind, String, Vec<u8>);

fn git(dir: &Path, args: &[&str]) -> String {
    // Spawned as Core spawns, never while another test's store opener holds its lock.
    let _forking = crate::git::forking();
    let out = Command::new(GIT)
        .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git");
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}

/// An empty repository at `dir/work`.
fn repository(dir: &Path) -> PathBuf {
    let work = dir.join("work");
    std::fs::create_dir_all(&work).expect("dir");
    git(&work, &["init", "-q", "-b", "main"]);
    work
}

fn line(position: u32) -> String {
    format!(
        r#"{{"type":"weighin.item_raised","version":1,"ts":"2026-10-31T04:12:40Z","actor":"agent","trust":{{"level":"asserted"}},"writer":{{"binary":"weighin","version":"2.0.0","spec":"1.0.0"}},"origin":"0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f:{position}","data":{{"item":"wi-{position}"}}}}"#
    ) + "\n"
}

/// The lines `positions`, joined.
fn lines(positions: &[u32]) -> String {
    positions.iter().map(|position| line(*position)).collect()
}

/// A commit holding exactly `files`, on `parents`, built in `work`'s index and worktree.
fn commit(work: &Path, files: &[(&str, &str)], parents: &[&str]) -> String {
    git(work, &["rm", "-rqf", "--cached", "--ignore-unmatch", "."]);
    for (path, content) in files {
        let path = work.join(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("dir");
        }
        std::fs::write(&path, content).expect("write");
        git(work, &["add", "-f", &path.to_string_lossy()]);
    }
    let tree = git(work, &["write-tree"]);
    let mut args = vec!["commit-tree", tree.as_str(), "-m", "c"];
    for parent in parents {
        args.extend(["-p", parent]);
    }
    git(work, &args)
}

/// A chain of journal commits of `a1`, one per content, each on the one before; their ids.
fn chain(work: &Path, contents: &[String]) -> Vec<String> {
    let mut commits: Vec<String> = Vec::new();
    for content in contents {
        let parents: Vec<&str> = commits.last().map(String::as_str).into_iter().collect();
        commits.push(commit(work, &[("a1.jsonl", content)], &parents));
    }
    commits
}

fn set(work: &Path, refname: &str, oid: &str) {
    git(work, &["update-ref", refname, oid]);
}

fn target(work: &Path) -> RefsTarget<'_> {
    RefsTarget {
        repo: work,
        remote: "",
        timeout: Duration::from_secs(30),
        deadline: None,
    }
}

/// `read_local_with` over `ctx` with a history cap of `cap`: what it found, and each file
/// handed over.
fn read_with(ctx: &Ctx<'_>, cap: usize) -> (LocalJournals, Vec<Handed>) {
    let mut handed = Vec::new();
    let found = read_local_with(ctx, cap, |file| {
        handed.push((
            file.refname.to_owned(),
            file.kind,
            file.commit.to_owned(),
            file.bytes.to_vec(),
        ));
    })
    .expect("read");
    (found, handed)
}

fn read(work: &Path) -> (LocalJournals, Vec<Handed>) {
    let mut handed = Vec::new();
    let found = read_local(work, Duration::from_secs(30), |file| {
        handed.push((
            file.refname.to_owned(),
            file.kind,
            file.commit.to_owned(),
            file.bytes.to_vec(),
        ));
    })
    .expect("read");
    (found, handed)
}

const A1: &str = "refs/redkiln/journal/a1";

fn journal_file(commit: &str, content: &str) -> Handed {
    (
        A1.to_owned(),
        SourceKind::Journal,
        commit.to_owned(),
        content.as_bytes().to_vec(),
    )
}

fn at(commit: &str, flaw: Flaw) -> ChainBreak {
    ChainBreak {
        commit: commit.to_owned(),
        flaw,
    }
}

/// The one source read, which must be `refname`'s.
fn only<'a>(found: &'a LocalJournals, refname: &str) -> &'a LocalSource {
    let [source] = found.sources.as_slice() else {
        panic!("one source: {found:?}");
    };
    assert_eq!(source.refname, refname);
    source
}

#[test]
fn a_repository_with_no_journal_sources_reads_none() {
    let dir = tempfile::tempdir().expect("tempdir");
    let work = repository(dir.path());
    let first = commit(&work, &[("README.md", "r\n")], &[]);
    set(&work, "refs/heads/main", &first);

    let (found, handed) = read(&work);

    assert!(found.sources.is_empty(), "{found:?}");
    assert!(found.passed_over.is_empty(), "{found:?}");
    assert!(handed.is_empty(), "{handed:?}");
}

#[test]
fn a_growing_chain_hands_over_only_its_tip_file_with_no_break() {
    let dir = tempfile::tempdir().expect("tempdir");
    let work = repository(dir.path());
    let contents = [lines(&[1]), lines(&[1, 2]), lines(&[1, 2, 3])];
    let commits = chain(&work, &contents);
    let tip = commits.last().expect("three");
    set(&work, A1, tip);

    let (found, handed) = read(&work);

    let source = only(&found, A1);
    assert_eq!(source.kind, SourceKind::Journal);
    assert_eq!(&source.tip, tip);
    assert_eq!(source.commits, 3);
    assert!(source.breaks.is_empty(), "{source:?}");
    assert!(found.passed_over.is_empty(), "{found:?}");
    assert_eq!(handed, vec![journal_file(tip, &lines(&[1, 2, 3]))]);
}

#[test]
fn a_commit_that_drops_a_line_is_rewritten_and_its_predecessor_file_is_handed_over_first() {
    let dir = tempfile::tempdir().expect("tempdir");
    let work = repository(dir.path());
    let contents = [lines(&[1, 2]), lines(&[1]), lines(&[1, 3])];
    let commits = chain(&work, &contents);
    let [first, second, third] = commits.as_slice() else {
        panic!("three commits");
    };
    set(&work, A1, third);

    let (found, handed) = read(&work);

    let source = only(&found, A1);
    assert_eq!(source.breaks, vec![at(second, Flaw::Rewritten)]);
    assert_eq!(
        handed,
        vec![
            journal_file(first, &lines(&[1, 2])),
            journal_file(third, &lines(&[1, 3])),
        ]
    );
}

#[test]
fn each_merge_commit_is_a_merge_break() {
    let dir = tempfile::tempdir().expect("tempdir");
    let work = repository(dir.path());
    let first = commit(&work, &[("a1.jsonl", &lines(&[1]))], &[]);
    let side = commit(&work, &[("a1.jsonl", &lines(&[9]))], &[]);
    let merged = commit(&work, &[("a1.jsonl", &lines(&[1, 2]))], &[&first, &side]);
    let other = commit(&work, &[("a1.jsonl", &lines(&[8]))], &[]);
    let again = commit(
        &work,
        &[("a1.jsonl", &lines(&[1, 2, 3]))],
        &[&merged, &other],
    );
    set(&work, A1, &again);

    let (found, handed) = read(&work);

    let source = only(&found, A1);
    assert_eq!(
        source.breaks,
        vec![
            at(&merged, Flaw::Breach(Breach::Merge)),
            at(&again, Flaw::Breach(Breach::Merge)),
        ]
    );
    assert_eq!(source.commits, 3, "the first-parent chain");
    assert_eq!(handed, vec![journal_file(&again, &lines(&[1, 2, 3]))]);
}

#[test]
fn a_commit_whose_tree_holds_another_file_is_a_tree_break_and_the_next_file_compares_with_the_last_readable()
 {
    let dir = tempfile::tempdir().expect("tempdir");
    let work = repository(dir.path());
    let first = commit(&work, &[("a1.jsonl", &lines(&[1, 2]))], &[]);
    let extra = commit(
        &work,
        &[("a1.jsonl", &lines(&[1, 2])), ("other.txt", "o\n")],
        &[&first],
    );
    let third = commit(&work, &[("a1.jsonl", &lines(&[3]))], &[&extra]);
    set(&work, A1, &third);

    let (found, handed) = read(&work);

    let source = only(&found, A1);
    assert_eq!(
        source.breaks,
        vec![
            at(&extra, Flaw::Breach(Breach::Tree)),
            at(&third, Flaw::Rewritten),
        ]
    );
    assert_eq!(
        handed,
        vec![
            journal_file(&first, &lines(&[1, 2])),
            journal_file(&third, &lines(&[3])),
        ]
    );
}

#[test]
fn a_ref_naming_a_tree_is_a_tree_break_at_its_tip() {
    let dir = tempfile::tempdir().expect("tempdir");
    let work = repository(dir.path());
    let first = commit(&work, &[("a1.jsonl", &lines(&[1]))], &[]);
    let tree = git(&work, &["rev-parse", &format!("{first}^{{tree}}")]);
    set(&work, A1, &tree);

    let (found, handed) = read(&work);

    let source = only(&found, A1);
    assert_eq!(source.tip, tree);
    assert_eq!(source.commits, 0);
    assert_eq!(source.breaks, vec![at(&tree, Flaw::Breach(Breach::Tree))]);
    assert!(handed.is_empty(), "{handed:?}");
}

#[test]
fn an_unended_file_is_an_unended_break() {
    let dir = tempfile::tempdir().expect("tempdir");
    let work = repository(dir.path());
    let torn = lines(&[1]) + "{\"type\":";
    let commits = chain(&work, &[lines(&[1]), torn.clone()]);
    let [_, second] = commits.as_slice() else {
        panic!("two commits");
    };
    set(&work, A1, second);

    let (found, handed) = read(&work);

    let source = only(&found, A1);
    assert_eq!(
        source.breaks,
        vec![at(second, Flaw::Breach(Breach::Unended))]
    );
    assert_eq!(handed, vec![journal_file(second, &torn)], "still used");
}

#[test]
fn a_file_past_the_cap_is_too_large() {
    let dir = tempfile::tempdir().expect("tempdir");
    let work = repository(dir.path());
    let commits = chain(&work, &[lines(&[1]), lines(&[1, 2])]);
    let [first, second] = commits.as_slice() else {
        panic!("two commits");
    };
    set(&work, A1, second);
    let target = target(&work);
    let mut ctx = Ctx::local(&target);
    ctx.max_bytes = u64::try_from(lines(&[1]).len()).expect("small") + 1;

    let (found, handed) = read_with(&ctx, MAX_HISTORY_COMMITS);

    let source = only(&found, A1);
    assert_eq!(
        source.breaks,
        vec![at(second, Flaw::Breach(Breach::TooLarge))]
    );
    assert_eq!(handed, vec![journal_file(first, &lines(&[1]))]);
}

#[test]
fn a_history_past_the_commit_cap_is_passed_over() {
    let dir = tempfile::tempdir().expect("tempdir");
    let work = repository(dir.path());
    let commits = chain(&work, &[lines(&[1]), lines(&[1, 2]), lines(&[1, 2, 3])]);
    set(&work, A1, commits.last().expect("three"));
    let target = target(&work);
    let ctx = Ctx::local(&target);

    let (found, handed) = read_with(&ctx, 2);

    assert!(found.sources.is_empty(), "{found:?}");
    assert_eq!(found.passed_over, vec![A1.to_owned()]);
    assert!(handed.is_empty(), "{handed:?}");

    let (found, _) = read_with(&ctx, 3);
    assert_eq!(only(&found, A1).commits, 3, "at the cap it is read");
}

#[test]
fn max_count_keeps_the_newest_commits_before_reverse() {
    let dir = tempfile::tempdir().expect("tempdir");
    let work = repository(dir.path());
    let commits = chain(&work, &[lines(&[1]), lines(&[1, 2]), lines(&[1, 2, 3])]);
    let [_, second, third] = commits.as_slice() else {
        panic!("three commits");
    };
    let repo = Repo {
        dir: &work,
        timeout: Duration::from_secs(30),
        program: OsStr::new(GIT),
    };
    let tip = Oid::parse(third).expect("an object name");

    let listed = repo
        .commits(
            &tip,
            &["--first-parent", "--reverse", "--max-count=2"],
            1 << 20,
        )
        .expect("ran")
        .expect("within the cap");

    let listed: Vec<&str> = listed.iter().map(Oid::as_str).collect();
    assert_eq!(listed, vec![second.as_str(), third.as_str()]);
}

#[test]
fn an_inbox_copy_hands_over_its_tip_file_and_has_no_breaks() {
    let dir = tempfile::tempdir().expect("tempdir");
    let work = repository(dir.path());
    let old = commit(&work, &[(".redkiln/inbox/a1.jsonl", &lines(&[1, 2]))], &[]);
    // A replaced inbox: its file no longer starts with the old one's, which is no break.
    let tip = commit(&work, &[(".redkiln/inbox/a1.jsonl", &lines(&[3]))], &[&old]);
    let inbox = "refs/redkiln/inbox/a1";
    set(&work, inbox, &tip);

    let (found, handed) = read(&work);

    let source = only(&found, inbox);
    assert_eq!(source.kind, SourceKind::Inbox);
    assert_eq!(source.tip, tip);
    assert_eq!(source.commits, 1);
    assert!(source.breaks.is_empty(), "{source:?}");
    assert_eq!(
        handed,
        vec![(
            inbox.to_owned(),
            SourceKind::Inbox,
            tip,
            lines(&[3]).into_bytes()
        )]
    );
}

#[test]
fn a_broken_ref_hands_over_its_kept_files_and_has_no_breaks() {
    let dir = tempfile::tempdir().expect("tempdir");
    let work = repository(dir.path());
    let without = commit(&work, &[("other.txt", "o\n")], &[]);
    let one = commit(&work, &[("a1.jsonl", &lines(&[1, 2]))], &[&without]);
    let two = commit(
        &work,
        &[("a1.jsonl", &lines(&[3])), ("other.txt", "o\n")],
        &[&one],
    );
    let broken = format!("refs/redkiln/broken/a1/{one}");
    set(&work, &broken, &two);

    let (found, handed) = read(&work);

    let source = only(&found, &broken);
    assert_eq!(source.kind, SourceKind::Broken);
    assert_eq!(source.tip, two);
    assert_eq!(source.commits, 3);
    assert!(source.breaks.is_empty(), "{source:?}");
    let file = |commit: &str, content: String| {
        (
            broken.clone(),
            SourceKind::Broken,
            commit.to_owned(),
            content.into_bytes(),
        )
    };
    assert_eq!(
        handed,
        vec![file(&one, lines(&[1, 2])), file(&two, lines(&[3]))]
    );
}

#[test]
fn a_ref_whose_name_is_no_journal_is_passed_over() {
    let dir = tempfile::tempdir().expect("tempdir");
    let work = repository(dir.path());
    let first = commit(&work, &[("a1.jsonl", &lines(&[1]))], &[]);
    let refs = [
        "refs/redkiln/journal/Upper",
        "refs/redkiln/inbox/Upper",
        "refs/redkiln/broken/no-sha",
    ];
    for refname in refs {
        set(&work, refname, &first);
    }

    let (found, handed) = read(&work);

    assert!(found.sources.is_empty(), "{found:?}");
    assert_eq!(found.passed_over, refs.map(str::to_owned).to_vec());
    assert!(handed.is_empty(), "{handed:?}");
}

/// A wrapper around git that logs each call's first argument to `log`.
fn wrapper(dir: &Path, log: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt as _;
    let path = dir.join("logging-git");
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$1\" >> '{}'\nexec git \"$@\"\n",
        log.display()
    );
    std::fs::write(&path, script).expect("write");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    path
}

#[test]
fn read_local_runs_only_local_read_steps() {
    let dir = tempfile::tempdir().expect("tempdir");
    let work = repository(dir.path());
    // A remote with a URL, which a writer or an import would fetch from.
    let remote = dir.path().join("remote");
    std::fs::create_dir_all(&remote).expect("dir");
    git(&remote, &["init", "-q", "--bare"]);
    git(
        &work,
        &["remote", "add", "origin", &remote.to_string_lossy()],
    );
    let commits = chain(&work, &[lines(&[1]), lines(&[1, 2])]);
    let tip = commits.last().expect("two");
    set(&work, A1, tip);
    let inbox = commit(&work, &[(".redkiln/inbox/b2.jsonl", &lines(&[5]))], &[]);
    set(&work, "refs/redkiln/inbox/b2", &inbox);
    set(&work, &format!("refs/redkiln/broken/c3/{tip}"), tip);
    let before = git(&work, &["for-each-ref"]);
    let log = dir.path().join("calls");
    let program = wrapper(dir.path(), &log);
    let target = target(&work);
    let mut ctx = Ctx::local(&target);
    ctx.repo.program = program.as_os_str();

    let (found, _) = read_with(&ctx, MAX_HISTORY_COMMITS);

    let kinds: Vec<SourceKind> = found.sources.iter().map(|source| source.kind).collect();
    assert_eq!(
        kinds,
        vec![SourceKind::Journal, SourceKind::Inbox, SourceKind::Broken],
        "journal refs, then inboxes, then broken refs"
    );
    let calls = std::fs::read_to_string(&log).expect("logged");
    let allowed = [
        "for-each-ref",
        "show-ref",
        "rev-parse",
        "cat-file",
        "rev-list",
        "ls-tree",
    ];
    for call in calls.lines() {
        assert!(allowed.contains(&call), "{call} ran: {calls}");
    }
    assert_eq!(git(&work, &["for-each-ref"]), before, "no ref moved");
}
