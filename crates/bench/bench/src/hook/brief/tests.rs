use std::fs;

use super::{BriefReadError, MAX_BRIEF_BYTES, read_brief};

#[test]
fn a_brief_within_the_cap_is_read_whole() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("36.md");
    fs::write(&path, "## Goal\nShip it.\n").expect("written");

    assert_eq!(
        read_brief(&path).expect("read"),
        "## Goal\nShip it.\n",
        "the whole file"
    );
}

#[test]
fn a_brief_over_the_cap_is_refused_rather_than_cut() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("36.md");
    let size = usize::try_from(MAX_BRIEF_BYTES).expect("fits") + 1;
    fs::write(&path, "x".repeat(size)).expect("written");

    assert!(
        matches!(read_brief(&path), Err(BriefReadError::TooLarge)),
        "one byte over"
    );
}

#[test]
fn a_directory_named_as_the_brief_is_not_a_file() {
    let dir = tempfile::tempdir().expect("a directory");

    assert!(
        matches!(read_brief(dir.path()), Err(BriefReadError::NotAFile)),
        "a directory is never opened as a brief"
    );
}

#[test]
fn the_record_starts_only_when_git_and_both_store_waits_fit() {
    use crate::hook::HOOK_BUDGET;
    use crate::hook::store_wait::{STORE_WAIT, every_remaining, fits};

    for left in every_remaining(HOOK_BUDGET) {
        if let Ok(git) = super::record_git(left) {
            assert!(
                git + STORE_WAIT * 2 <= left,
                "the open and the append, {left:?} left"
            );
        }
        if fits(left, super::APPEND_NEEDS).is_ok() {
            assert!(STORE_WAIT <= left, "the append, {left:?} left");
        }
    }
    assert_eq!(super::APPEND_NEEDS, STORE_WAIT);
}

/// Runs the hook on input it cannot read, with the ledger at `store`, within `budget`.
fn denied_within(store: &std::path::Path, budget: crate::pr_diff::Budget) -> String {
    use std::ffi::OsString;

    let lookup = |var: &str| (var == "REDKILN_STORE").then(|| OsString::from(store));
    let env = crate::hook::HookEnv {
        stdin: &b"not json"[..],
        // An absolute directory on every platform; the store is `$REDKILN_STORE`'s.
        current_dir: || Ok(std::env::temp_dir()),
        lookup,
    };
    let mut out = Vec::new();
    super::run_within(env, &mut out, budget).expect("a Vec accepts every write");
    String::from_utf8(out).expect("UTF-8")
}

#[test]
fn with_too_little_left_for_both_store_waits_the_deny_goes_out_unrecorded() {
    use std::time::{Duration, Instant};

    use crate::hook::guard::GIT_TIMEOUT;
    use crate::pr_diff::Budget;
    use crate::store::{Filter, Store};

    let dir = tempfile::tempdir().expect("a temporary directory");
    let store = dir.path().join("ledger.sqlite3");
    drop(Store::open(&store).expect("a ledger"));
    let short = Budget::within(Instant::now(), Duration::from_millis(4_100), GIT_TIMEOUT);
    let full = Budget::within(Instant::now(), crate::hook::HOOK_BUDGET, GIT_TIMEOUT);

    let skipped = denied_within(&store, short);
    let recorded = denied_within(&store, full);

    assert!(
        skipped.contains("\"deny\"")
            && skipped.contains("(the block was not recorded in the ledger: no time is left"),
        "{skipped}"
    );
    assert!(
        recorded.contains("\"deny\"") && !recorded.contains("not recorded"),
        "{recorded}"
    );
    let events = Store::open(&store)
        .and_then(|ledger| ledger.query(&Filter::default()))
        .expect("the ledger reads");
    assert_eq!(
        events.len(),
        1,
        "only the run with time left records: {events:?}"
    );
}
