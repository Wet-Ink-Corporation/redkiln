use std::ffi::OsString;
use std::path::{Path, PathBuf};

use redkiln_core::location::{LocateError, Source};

use super::{Found, Notice, StoreAt, Vars, locate_store};
use crate::git;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A git repository at `<tmp>/repo`, its path as git prints it (resolved).
fn repo() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let repo = std::fs::canonicalize(tmp.path())
        .expect("resolved")
        .join("repo");
    std::fs::create_dir_all(&repo).expect("repo dir");
    git::run(&repo, &["init", "-q"]).expect("git init");
    (tmp, repo)
}

fn vars(redkiln: Option<&str>, weighin: Option<&str>) -> Vars {
    Vars {
        redkiln_store: redkiln.map(OsString::from),
        weighin_store: weighin.map(OsString::from),
        home: None,
    }
}

fn found(cwd: &Path, flag: Option<&Path>, key: Option<&str>, vars: &Vars) -> Found {
    locate_store(cwd, flag, vars, |_| key.map(str::to_owned)).expect("the store is located")
}

fn at(path: impl Into<PathBuf>, source: Source) -> StoreAt {
    StoreAt {
        path: path.into(),
        source,
    }
}

fn texts(found: &Found) -> Vec<String> {
    found.notices.iter().map(ToString::to_string).collect()
}

#[test]
fn the_default_store_is_in_the_common_dir_from_every_worktree_and_beside_a_folder() -> TestResult {
    let (tmp, repo) = repo();
    let commit = ["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q"];
    git::run(
        &repo,
        &[&commit[..], &["--allow-empty", "-m", "c"]].concat(),
    )?;
    let linked = repo.with_file_name("linked");
    git::run(&repo, &["worktree", "add", "-q", &linked.to_string_lossy()])?;
    let folder = tmp.path().join("folder");
    std::fs::create_dir_all(&folder)?;
    let common = repo.join(".git/redkiln/events.sqlite3");

    let from_main = found(&repo, None, None, &vars(None, None));
    let from_linked = found(&linked, None, None, &vars(None, None));
    let outside = found(&folder, None, None, &vars(None, None));

    assert_eq!(&from_main.store, &at(&common, Source::GitCommonDir));
    assert_eq!(&from_linked.store, &at(&common, Source::GitCommonDir));
    assert_eq!(
        &outside.store,
        &at(folder.join(".redkiln/events.sqlite3"), Source::Folder)
    );
    assert_eq!(from_linked.workspace.root(), linked);
    Ok(())
}

#[test]
fn the_flag_wins_over_both_variables_and_redkiln_store_over_weighin_store_silently() {
    let (_tmp, repo) = repo();
    let both = vars(Some("/r/events.sqlite3"), Some("/w/ledger.sqlite3"));

    let flagged = found(&repo, Some(Path::new("/f/events.sqlite3")), None, &both);
    let variable = found(&repo, None, None, &both);

    assert_eq!(&flagged.store, &at("/f/events.sqlite3", Source::Flag));
    assert_eq!(&variable.store, &at("/r/events.sqlite3", Source::Env));
    assert!(texts(&flagged).is_empty() && texts(&variable).is_empty());
}

#[test]
fn weighin_store_names_the_old_ledger_and_the_store_goes_beside_it() {
    let (_tmp, repo) = repo();

    let set = found(&repo, None, None, &vars(None, Some("/x/proj.sqlite3")));
    let blank = found(&repo, None, None, &vars(Some(" "), Some("  ")));

    assert_eq!(&set.store, &at("/x/proj.events.sqlite3", Source::Env));
    assert_eq!(
        set.notices,
        [Notice::WeighinStore {
            old: PathBuf::from("/x/proj.sqlite3"),
            store: PathBuf::from("/x/proj.events.sqlite3"),
        }]
    );
    assert_eq!(blank.store.source, Source::GitCommonDir, "blank is unset");
}

#[test]
fn suite_store_wins_over_the_config_key_which_alone_puts_the_store_beside_its_ledger() -> TestResult
{
    let (_tmp, repo) = repo();

    let legacy = found(&repo, None, Some("ledger/proj.sqlite3"), &vars(None, None));
    std::fs::create_dir_all(repo.join(".redkiln"))?;
    std::fs::write(
        repo.join(".redkiln/suite.toml"),
        "[suite]\nstore = \"s/events.sqlite3\"\n",
    )?;
    let suite = found(&repo, None, Some("ledger/proj.sqlite3"), &vars(None, None));

    let beside = repo.join("ledger/proj.events.sqlite3");
    assert_eq!(&legacy.store, &at(&beside, Source::Config));
    assert_eq!(
        legacy.notices,
        [Notice::LegacyStoreKey {
            old: repo.join("ledger/proj.sqlite3"),
            store: beside,
        }]
    );
    assert_eq!(
        &suite.store,
        &at(repo.join("s/events.sqlite3"), Source::Config)
    );
    assert_eq!(texts(&suite), Vec::<String>::new());
    Ok(())
}

#[test]
fn each_old_ledger_that_exists_is_a_notice_and_a_missing_one_is_none() -> TestResult {
    let (tmp, repo) = repo();
    let none = found(&repo, None, None, &vars(None, None));
    let default = repo.join(".git/weighin/ledger.sqlite3");
    let named = std::fs::canonicalize(tmp.path())?.join("named.sqlite3");
    let keyed = repo.join("keyed.sqlite3");
    std::fs::create_dir_all(repo.join(".git/weighin"))?;
    for old in [&default, &named, &keyed] {
        std::fs::write(old, b"1.x")?;
    }
    let named_text = named.to_string_lossy().into_owned();

    let by_variable = found(&repo, None, None, &vars(None, Some(&named_text)));
    let by_key = found(&repo, None, Some("keyed.sqlite3"), &vars(None, None));

    assert_eq!(texts(&none), Vec::<String>::new());
    let old_ledgers = |found: &Found| -> Vec<PathBuf> {
        found
            .notices
            .iter()
            .filter_map(|n| match n {
                Notice::OldLedger { path } => Some(path.clone()),
                _ => None,
            })
            .collect()
    };
    assert_eq!(old_ledgers(&by_variable), [named, default.clone()]);
    assert_eq!(old_ledgers(&by_key), [keyed, default]);
    Ok(())
}

#[test]
fn a_repository_git_refuses_is_an_error_not_the_folder_store() -> TestResult {
    let tmp = tempfile::tempdir()?;
    std::fs::write(tmp.path().join(".git"), "gitdir: /nowhere/at/all\n")?;

    let err = locate_store(tmp.path(), None, &vars(None, None), |_| None)
        .err()
        .ok_or("a broken .git file is refused")?;

    assert!(matches!(err, LocateError::Git(_)), "{err:?}");
    Ok(())
}
