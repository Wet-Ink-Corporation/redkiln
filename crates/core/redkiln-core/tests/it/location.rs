//! Where the store is (SPEC ST-01 to ST-03), resolved against real git repositories and folders.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

use redkiln_core::location::{
    GitError, LocateError, Located, STORE_FILE, Search, Source, Workspace, locate,
};
use redkiln_core::suite::RepoClass;

use crate::support::listing;

/// Runs git in `dir` with a fixed identity and no global or system config.
fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=test", "-c", "user.email=test@example.com"])
        .args([
            "-c",
            "init.defaultBranch=main",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?} in {}", dir.display());
}

/// A new repository at `<tempdir>/repo`, with one commit so a worktree can be added.
fn repository() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let repo = dir.path().join("repo");
    std::fs::create_dir(&repo).expect("repo dir");
    git(&repo, &["init", "-q"]);
    git(&repo, &["commit", "-q", "--allow-empty", "-m", "first"]);
    (dir, repo)
}

/// `path` with the components of `tail` taken off its end, canonicalised: so a store path that
/// does not exist yet compares with the directory that does, whatever form git printed it in.
fn assert_store(located: &Located, existing: &Path, tail: &[&str]) {
    let mut head = located.store.clone();
    for part in tail.iter().rev() {
        assert_eq!(
            head.file_name(),
            Some(OsStr::new(part)),
            "{} ends with {tail:?}",
            located.store.display()
        );
        head.pop();
    }
    assert_eq!(
        std::fs::canonicalize(&head).expect("the store's existing directory"),
        std::fs::canonicalize(existing).expect("the expected directory"),
        "{}",
        located.store.display()
    );
}

#[test]
fn in_a_repository_the_store_is_in_the_git_common_dir_from_any_subdirectory() {
    let (_dir, repo) = repository();
    let deep = repo.join("a").join("b");
    std::fs::create_dir_all(&deep).expect("subdirectory");
    let before = listing(&repo.join(".git"));

    let located = locate(&Search::new(&deep)).expect("located");
    assert_eq!(located.source, Source::GitCommonDir);
    assert_store(&located, &repo.join(".git"), &["redkiln", STORE_FILE]);
    let workspace = located.workspace.as_ref().expect("git was asked");
    assert!(matches!(workspace, Workspace::Git { .. }), "{workspace:?}");
    assert_eq!(
        std::fs::canonicalize(workspace.root()).expect("root"),
        std::fs::canonicalize(&repo).expect("repo")
    );
    assert_eq!(
        listing(&repo.join(".git")),
        before,
        "locating creates nothing"
    );
}

#[test]
fn a_linked_worktree_shares_the_main_checkouts_store() {
    let (dir, repo) = repository();
    let linked = dir.path().join("linked");
    git(
        &repo,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "side",
            linked.to_str().expect("UTF-8"),
        ],
    );

    let main = locate(&Search::new(&repo)).expect("main");
    let other = locate(&Search::new(&linked)).expect("linked");
    assert_eq!(other.source, Source::GitCommonDir);
    assert_store(&other, &repo.join(".git"), &["redkiln", STORE_FILE]);
    assert_store(&main, &repo.join(".git"), &["redkiln", STORE_FILE]);
    let root = other.workspace.as_ref().expect("git was asked").root();
    assert_eq!(
        std::fs::canonicalize(root).expect("root"),
        std::fs::canonicalize(&linked).expect("linked"),
        "the worktree root is the linked checkout's own"
    );
}

#[test]
fn outside_git_the_store_is_in_the_folder() {
    let dir = tempfile::tempdir().expect("tempdir");
    let located = locate(&Search::new(dir.path())).expect("located");
    assert_eq!(located.source, Source::Folder);
    assert_eq!(located.store, dir.path().join(".redkiln").join(STORE_FILE));
    assert_eq!(
        located.workspace,
        Some(Workspace::Folder(dir.path().to_path_buf()))
    );
    assert!(listing(dir.path()).is_empty(), "locating creates nothing");
}

/// A `.git` file git cannot parse: git fails, but not with "not a git repository".
fn broken_repository() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join(".git"), "not a gitdir line\n").expect("a broken .git");
    dir
}

#[test]
fn a_git_failure_other_than_no_repository_stops_and_never_falls_back_to_the_folder() {
    let dir = broken_repository();
    let refused = locate(&Search::new(dir.path()));
    match refused {
        Err(LocateError::Git(GitError::Failed { stderr, .. })) => {
            assert!(stderr.contains("gitfile"), "{stderr}");
        }
        other => panic!("expected a git failure, got {other:?}"),
    }
}

#[test]
fn the_flag_and_the_variable_win_without_asking_git() {
    let dir = broken_repository();
    let elsewhere = tempfile::tempdir().expect("tempdir");
    let absolute = elsewhere.path().join("events.sqlite3");
    let mut search = Search::new(dir.path());
    search.flag = Some(Path::new("here.sqlite3"));
    search.env = Some(absolute.as_os_str());
    let flagged = locate(&search).expect("the flag needs no git");
    assert_eq!(flagged.source, Source::Flag);
    assert_eq!(flagged.store, dir.path().join("here.sqlite3"));
    assert_eq!(flagged.workspace, None);
    assert_eq!(Source::Flag.as_str(), "flag");

    search.flag = None;
    let from_env = locate(&search).expect("the variable needs no git");
    assert_eq!(from_env.source, Source::Env);
    assert_eq!(from_env.store, absolute);
    assert!(from_env.suite.is_none(), "no config read either");
}

#[test]
fn a_relative_variable_is_relative_to_the_working_directory_and_a_blank_one_is_unset() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut search = Search::new(dir.path());
    search.env = Some(OsStr::new("state/events.sqlite3"));
    let located = locate(&search).expect("located");
    assert_eq!(
        located.store,
        dir.path().join("state").join("events.sqlite3")
    );

    for blank in ["", "  ", "\t"] {
        search.env = Some(OsStr::new(blank));
        let located = locate(&search).expect("located");
        assert_eq!(located.source, Source::Folder, "{blank:?} is unset");
    }
}

#[test]
fn a_leading_tilde_is_the_home_directory_given() {
    let dir = tempfile::tempdir().expect("tempdir");
    let home = tempfile::tempdir().expect("home");
    let mut search = Search::new(dir.path());
    search.env = Some(OsStr::new("~/stores/events.sqlite3"));
    search.home = Some(home.path());
    let located = locate(&search).expect("located");
    assert_eq!(
        located.store,
        home.path().join("stores").join("events.sqlite3")
    );

    search.home = None;
    assert!(
        matches!(
            locate(&search),
            Err(LocateError::NoHome { step: Source::Env })
        ),
        "no home, no guess"
    );

    search.env = Some(OsStr::new("~user/events.sqlite3"));
    let located = locate(&search).expect("only `~/` is expanded");
    assert_eq!(
        located.store,
        dir.path().join("~user").join("events.sqlite3")
    );
}

#[test]
fn a_relative_working_directory_is_refused() {
    let refused = locate(&Search::new(Path::new("relative/dir")));
    assert!(
        matches!(&refused, Err(LocateError::RelativeCwd { cwd }) if cwd == Path::new("relative/dir")),
        "{refused:?}"
    );
}

fn write_suite(root: &Path, text: &str) {
    let dir = root.join(".redkiln");
    std::fs::create_dir_all(&dir).expect(".redkiln");
    std::fs::write(dir.join("suite.toml"), text).expect("suite.toml");
}

#[test]
fn suite_toml_store_is_relative_to_the_worktree_root_not_the_working_directory() {
    let (_dir, repo) = repository();
    write_suite(
        &repo,
        "[suite]\nrepo_class = \"own\"\nstore = \"data/events.sqlite3\"\n",
    );
    let deep = repo.join("src");
    std::fs::create_dir(&deep).expect("subdirectory");

    let located = locate(&Search::new(&deep)).expect("located");
    assert_eq!(located.source, Source::Config);
    assert_store(&located, &repo, &["data", "events.sqlite3"]);
    let suite = located.suite.as_ref().expect("the config was read");
    assert_eq!(suite.repo_class(), RepoClass::Own);
    assert_eq!(Source::Config.as_str(), "config");
}

#[test]
fn suite_toml_with_a_tilde_store_and_no_home_is_an_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_suite(dir.path(), "[suite]\nstore = \"~/events.sqlite3\"\n");
    assert!(matches!(
        locate(&Search::new(dir.path())),
        Err(LocateError::NoHome {
            step: Source::Config
        })
    ));
}

#[test]
fn outside_git_suite_toml_is_read_in_the_folder() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_suite(dir.path(), "[suite]\nstore = \"state/events.sqlite3\"\n");
    let located = locate(&Search::new(dir.path())).expect("located");
    assert_eq!(located.source, Source::Config);
    assert_eq!(
        located.store,
        dir.path().join("state").join("events.sqlite3")
    );
}

#[test]
fn a_suite_toml_that_does_not_parse_leaves_the_default_store_and_says_so() {
    let (_dir, repo) = repository();
    write_suite(&repo, "[suite\nstore = \"data/events.sqlite3\"\n");
    let located = locate(&Search::new(&repo)).expect("located");
    assert_eq!(located.source, Source::GitCommonDir);
    let suite = located.suite.as_ref().expect("the config was read");
    assert_eq!(suite.repo_class(), RepoClass::Client);
    assert_eq!(suite.warnings().len(), 1, "{:?}", suite.warnings());
}

#[test]
fn every_step_has_its_spec_name() {
    let names = [
        Source::Flag,
        Source::Env,
        Source::Config,
        Source::GitCommonDir,
        Source::Folder,
    ]
    .map(Source::as_str);
    assert_eq!(names, ["flag", "env", "config", "git_common_dir", "folder"]);
}
