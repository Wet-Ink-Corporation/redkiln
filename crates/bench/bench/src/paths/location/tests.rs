//! Where the store is, and the anneal ledgers noticed beside it (plan test list 23 and 24).

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use super::{Notice, locate, store_beside};
use crate::paths::StoreSource;

const TIMEOUT: Duration = Duration::from_secs(30);

fn env(pairs: &[(&str, &Path)]) -> impl Fn(&str) -> Option<OsString> + use<> {
    let map: HashMap<String, OsString> = pairs
        .iter()
        .map(|(key, value)| ((*key).to_owned(), value.as_os_str().to_owned()))
        .collect();
    move |key| map.get(key).cloned()
}

/// Runs `git <args>` in `dir`, isolated from the caller's repository and identity.
fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args([
            "-c",
            "user.name=bench test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_COMMON_DIR")
        .env_remove("GIT_INDEX_FILE")
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// A repository at `<root>/main` with one commit and a linked worktree at `<root>/linked`.
fn repository(root: &Path) -> (PathBuf, PathBuf) {
    let main = root.join("main");
    std::fs::create_dir(&main).expect("mkdir");
    git(&main, &["init", "-q"]);
    git(&main, &["commit", "-q", "--allow-empty", "-m", "init"]);
    git(
        &main,
        &["worktree", "add", "-q", "-b", "linked", "../linked"],
    );
    (main, root.join("linked"))
}

fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).expect("exists")
}

/// `path`'s parent canonicalised, with its file name: for a store not created yet.
fn canonical_file(path: &Path) -> PathBuf {
    canonical(path.parent().expect("a parent")).join(path.file_name().expect("a name"))
}

#[test]
#[cfg_attr(miri, ignore = "runs git in a subprocess")]
fn the_main_checkout_and_a_linked_worktree_locate_the_store_in_the_common_dir() {
    let root = tempfile::tempdir().expect("temp dir");
    let (main, linked) = repository(root.path());
    std::fs::create_dir_all(main.join(".git/redkiln")).expect("mkdir");
    let expected = canonical(&main.join(".git/redkiln")).join("events.sqlite3");

    let from_main = locate(None, env(&[]), &main, TIMEOUT).expect("locates");
    let from_linked = locate(None, env(&[]), &linked, TIMEOUT).expect("locates");

    for found in [&from_main, &from_linked] {
        assert_eq!(canonical_file(&found.path), expected, "ST-01's default");
        assert_eq!(found.source, StoreSource::GitCommonDir, "git chose it");
        assert_eq!(found.notices, [], "nothing to say");
    }
}

#[test]
#[cfg_attr(miri, ignore = "runs git in a subprocess")]
fn the_flag_beats_both_variables_and_redkiln_store_beats_anneal_store_silently() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (redkiln, anneal) = (dir.path().join("r.sqlite3"), dir.path().join("a.sqlite3"));
    let both = [
        ("REDKILN_STORE", redkiln.as_path()),
        ("ANNEAL_STORE", anneal.as_path()),
    ];

    let flagged = locate(
        Some(Path::new("f.sqlite3")),
        env(&both),
        dir.path(),
        TIMEOUT,
    )
    .expect("locates");
    let variable = locate(None, env(&both), dir.path(), TIMEOUT).expect("locates");

    assert_eq!(
        (flagged.path, flagged.source),
        (dir.path().join("f.sqlite3"), StoreSource::Flag),
        "--store first"
    );
    assert_eq!(
        (variable.path, variable.source, variable.notices),
        (redkiln, StoreSource::Env, Vec::new()),
        "$REDKILN_STORE over $ANNEAL_STORE, with no notice"
    );
}

#[test]
#[cfg_attr(miri, ignore = "runs git in a subprocess")]
fn anneal_store_alone_puts_the_store_beside_the_ledger_it_names_with_a_notice() {
    let dir = tempfile::tempdir().expect("temp dir");
    let old = dir.path().join("a.sqlite3");

    let found = locate(None, env(&[("ANNEAL_STORE", &old)]), dir.path(), TIMEOUT).expect("locates");

    let store = dir.path().join("a.events.sqlite3");
    assert_eq!(
        (&found.path, found.source),
        (&store, StoreSource::Env),
        "D5: beside it"
    );
    assert_eq!(
        found.notices,
        [Notice::AnnealStore { old, store }],
        "said once; the missing ledger itself is no old ledger"
    );
}

#[test]
#[cfg_attr(miri, ignore = "runs git in a subprocess")]
fn a_blank_anneal_store_is_unset() {
    let dir = tempfile::tempdir().expect("temp dir");

    let found = locate(
        None,
        env(&[("ANNEAL_STORE", Path::new("  "))]),
        dir.path(),
        TIMEOUT,
    )
    .expect("locates");

    assert_eq!(found.source, StoreSource::Folder, "outside git, the folder");
    assert_eq!(found.notices, [], "no notice");
}

#[test]
#[cfg_attr(miri, ignore = "runs git in a subprocess")]
fn the_suite_configs_store_is_honoured() {
    let root = tempfile::tempdir().expect("temp dir");
    let (main, _) = repository(root.path());
    std::fs::create_dir(main.join(".redkiln")).expect("mkdir");
    std::fs::write(
        main.join(".redkiln/suite.toml"),
        "[suite]\nstore = \"../shared/events.sqlite3\"\n",
    )
    .expect("write");

    let found = locate(None, env(&[]), &main, TIMEOUT).expect("locates");

    assert_eq!(found.source, StoreSource::Config, "ST-01 step 4");
    assert!(
        found.path.ends_with("shared/events.sqlite3"),
        "{}",
        found.path.display()
    );
}

#[test]
#[cfg_attr(miri, ignore = "runs git in a subprocess")]
fn an_anneal_ledger_in_the_common_dir_is_an_old_ledger_notice() {
    let root = tempfile::tempdir().expect("temp dir");
    let (main, linked) = repository(root.path());
    let old = main.join(".git/anneal/ledger.sqlite3");
    std::fs::create_dir_all(old.parent().expect("a parent")).expect("mkdir");
    std::fs::write(&old, b"").expect("write");

    let found = locate(None, env(&[]), &linked, TIMEOUT).expect("locates");

    assert_eq!(
        found
            .old_ledgers
            .iter()
            .map(|path| canonical(path))
            .collect::<Vec<_>>(),
        [canonical(&old)],
        "found from a linked worktree too"
    );
    assert!(
        matches!(found.notices.as_slice(), [Notice::OldLedger { .. }]),
        "{:?}",
        found.notices
    );
}

#[test]
#[cfg_attr(miri, ignore = "runs git in a subprocess")]
fn outside_git_an_anneal_folder_ledger_is_an_old_ledger_and_a_missing_one_none() {
    let with = tempfile::tempdir().expect("temp dir");
    let without = tempfile::tempdir().expect("temp dir");
    let old = with.path().join(".anneal/ledger.sqlite3");
    std::fs::create_dir_all(old.parent().expect("a parent")).expect("mkdir");
    std::fs::write(&old, b"").expect("write");

    let found = locate(None, env(&[]), with.path(), TIMEOUT).expect("locates");
    let none = locate(None, env(&[]), without.path(), TIMEOUT).expect("locates");

    assert_eq!(
        found.old_ledgers,
        std::slice::from_ref(&old),
        "the folder's anneal ledger"
    );
    assert_eq!(found.notices, [Notice::OldLedger { path: old }], "said");
    assert_eq!(
        (none.old_ledgers, none.notices),
        (Vec::new(), Vec::new()),
        "nothing"
    );
}

#[test]
#[cfg_attr(miri, ignore = "runs git in a subprocess")]
fn the_ledger_anneal_store_names_is_an_old_ledger_when_it_exists() {
    let dir = tempfile::tempdir().expect("temp dir");
    let old = dir.path().join("a.sqlite3");
    std::fs::write(&old, b"").expect("write");

    let found = locate(None, env(&[("ANNEAL_STORE", &old)]), dir.path(), TIMEOUT).expect("locates");

    assert_eq!(
        found.old_ledgers,
        std::slice::from_ref(&old),
        "to import later"
    );
    assert_eq!(
        found.notices,
        [
            Notice::AnnealStore {
                old: old.clone(),
                store: store_beside(&old)
            },
            Notice::OldLedger { path: old },
        ],
        "both said"
    );
}
