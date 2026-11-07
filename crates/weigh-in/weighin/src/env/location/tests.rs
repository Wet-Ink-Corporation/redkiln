use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use redkiln_core::location::{LocateError, Source};

use super::{Fallback, Found, Notice, Vars, locate_store};
use crate::git;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A git repository at `<tmp>/repo`, its path as the temporary directory gives it. Paths are
/// compared by [`assert_at`], never as given: on Windows git prints `C:/..` for a directory
/// `canonicalize` gives as `\\?\C:\..`, and on macOS the temporary directory is a symlink.
fn repo() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let repo = tmp.path().join("repo");
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

/// Asserts that `actual` is the directory `existing` followed by the names in `tail`, as Core's
/// `tests/it/location.rs` compares store paths: each name of the tail by name, then what is left
/// against `existing`, both canonicalised, since a store path may not exist yet.
fn assert_at(actual: &Path, existing: &Path, tail: &[&str]) {
    let mut head = actual.to_path_buf();
    for part in tail.iter().rev() {
        assert_eq!(
            head.file_name(),
            Some(OsStr::new(part)),
            "{} ends with {tail:?}",
            actual.display()
        );
        head.pop();
    }
    assert_eq!(
        std::fs::canonicalize(&head).expect("the path's existing directory"),
        std::fs::canonicalize(existing).expect("the expected directory"),
        "{}",
        actual.display()
    );
}

/// Asserts that `actual` and `expected` name the same existing file.
fn assert_same_file(actual: &Path, expected: &Path) {
    assert_eq!(
        std::fs::canonicalize(actual).expect("the found file exists"),
        std::fs::canonicalize(expected).expect("the expected file exists"),
        "{} is {}",
        actual.display(),
        expected.display()
    );
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
    let tail = ["redkiln", "events.sqlite3"];

    let from_main = found(&repo, None, None, &vars(None, None));
    let from_linked = found(&linked, None, None, &vars(None, None));
    let outside = found(&folder, None, None, &vars(None, None));

    assert_eq!(from_main.store.source, Source::GitCommonDir);
    assert_at(&from_main.store.path, &repo.join(".git"), &tail);
    assert_eq!(from_linked.store.source, Source::GitCommonDir);
    assert_at(&from_linked.store.path, &repo.join(".git"), &tail);
    assert_eq!(outside.store.source, Source::Folder);
    assert_at(
        &outside.store.path,
        &folder,
        &[".redkiln", "events.sqlite3"],
    );
    assert_at(from_linked.workspace.root(), &linked, &[]);
    Ok(())
}

#[test]
fn the_flag_wins_over_both_variables_and_redkiln_store_over_weighin_store_silently() {
    let (_tmp, repo) = repo();
    let both = vars(Some("r/events.sqlite3"), Some("w/ledger.sqlite3"));

    let flagged = found(&repo, Some(Path::new("f/events.sqlite3")), None, &both);
    let variable = found(&repo, None, None, &both);

    assert_eq!(flagged.store.source, Source::Flag);
    assert_at(&flagged.store.path, &repo, &["f", "events.sqlite3"]);
    assert_eq!(variable.store.source, Source::Env);
    assert_at(&variable.store.path, &repo, &["r", "events.sqlite3"]);
    assert!(texts(&flagged).is_empty() && texts(&variable).is_empty());
}

#[test]
fn weighin_store_names_the_old_ledger_and_the_store_goes_beside_it() {
    let (_tmp, repo) = repo();

    let set = found(&repo, None, None, &vars(None, Some("x/proj.sqlite3")));
    let blank = found(&repo, None, None, &vars(Some(" "), Some("  ")));

    assert_eq!(set.store.source, Source::Env);
    assert_at(&set.store.path, &repo, &["x", "proj.events.sqlite3"]);
    let [Notice::WeighinStore(Fallback { old, store, .. })] = set.notices.as_slice() else {
        panic!("one WeighinStore notice: {:?}", set.notices);
    };
    assert_at(old, &repo, &["x", "proj.sqlite3"]);
    assert_eq!(store, &set.store.path);
    assert_eq!(blank.store.source, Source::GitCommonDir, "blank is unset");
}

#[test]
fn weighin_store_is_trimmed_as_1x_trimmed_it() {
    let (_tmp, repo) = repo();

    let padded = found(&repo, None, None, &vars(None, Some(" x/proj.sqlite3\n")));

    assert_eq!(padded.store.source, Source::Env);
    assert_at(&padded.store.path, &repo, &["x", "proj.events.sqlite3"]);
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

    assert_eq!(legacy.store.source, Source::Config);
    assert_at(
        &legacy.store.path,
        &repo,
        &["ledger", "proj.events.sqlite3"],
    );
    let [Notice::LegacyStoreKey(Fallback { old, store, .. })] = legacy.notices.as_slice() else {
        panic!("one LegacyStoreKey notice: {:?}", legacy.notices);
    };
    assert_at(old, &repo, &["ledger", "proj.sqlite3"]);
    assert_eq!(store, &legacy.store.path);
    assert_eq!(suite.store.source, Source::Config);
    assert_at(&suite.store.path, &repo, &["s", "events.sqlite3"]);
    assert_eq!(texts(&suite), Vec::<String>::new());
    Ok(())
}

#[test]
fn a_fallback_naming_no_file_is_not_called_a_1x_ledger() -> TestResult {
    let (_tmp, repo) = repo();
    let none = vars(None, None);
    let missing = |found: &Found| -> Vec<String> {
        texts(found)
            .into_iter()
            .filter(|t| !t.starts_with("a weighin 1.x ledger is at"))
            .collect()
    };

    let new_key = found(&repo, None, Some("ledger/proj.sqlite3"), &none);
    let new_var = found(&repo, None, None, &vars(None, Some("x/proj.sqlite3")));
    std::fs::create_dir_all(repo.join("ledger"))?;
    std::fs::write(repo.join("ledger/proj.sqlite3"), b"1.x")?;
    let old_key = found(&repo, None, Some("ledger/proj.sqlite3"), &none);

    for found in [&new_key, &new_var] {
        let [text] = missing(found).try_into().map_err(|t| format!("{t:?}"))?;
        assert!(
            !text.contains("1.x ledger") && text.contains("no file is there"),
            "{text}"
        );
    }
    let [text] = missing(&old_key).try_into().map_err(|t| format!("{t:?}"))?;
    assert!(text.contains("names the 1.x ledger"), "{text}");
    Ok(())
}

#[test]
fn each_old_ledger_that_exists_is_a_notice_and_a_missing_one_is_none() -> TestResult {
    let (tmp, repo) = repo();
    let none = found(&repo, None, None, &vars(None, None));
    let default = repo.join(".git/weighin/ledger.sqlite3");
    let named = tmp.path().join("named.sqlite3");
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
    for (found, expected) in [
        (&by_variable, [&named, &default]),
        (&by_key, [&keyed, &default]),
    ] {
        let paths = old_ledgers(found);
        assert_eq!(paths.len(), expected.len(), "{paths:?}");
        for (path, expected) in paths.iter().zip(expected) {
            assert_same_file(path, expected);
        }
    }
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
