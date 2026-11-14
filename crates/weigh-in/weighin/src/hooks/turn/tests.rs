//! The Stop hook's export of journal refs (SPEC JR-08, D3, D12), run in process with the budget
//! the test gives the hook, so that what is pinned does not hang on how fast a runner spawns git
//! (TST-10): an `own` repository, whether by suite.toml or by weigh-in 1.x's config.json,
//! exports the session's journal ref at Stop, and a Stop past its deadline exports nothing and
//! logs no error, but the next session start says once that it ran out of time
//! (wi-6ab291e7fb). The cases that hold whatever the deadline does run the binary in
//! `tests/it/repo_class.rs`.

use std::path::Path;
use std::time::Instant;

use anyhow::{Context as _, Result};

use crate::env::{BoardMode, Env, RepoClass};
use crate::hooks::test_support::{
    has_ref, init, logged, own_repo, past_deadline, raise, repo, served, unhurried,
};
use crate::hooks::{Hook, HookEvent, Turn};
use crate::journal::refs::Budget;

mod export;

/// The Stop's budget with no deadline, each git call given a command's time: the export always
/// runs to its end, whatever the runner's speed.
const UNHURRIED: Budget = unhurried(Budget::STOP);

/// The Stop's budget with a deadline that has passed when the hook starts.
const PAST_DEADLINE: Budget = past_deadline(Budget::STOP);

/// The last message of the turn the Stop ends: a one-way assumption and a two-way call, as the
/// binary tests' `Repo::stop` sends.
const MESSAGE: &str = "Migration done.\n\n```weigh-in\nkind: assumption\ndoor: one-way\nauthority: authority\nq: Should retention overrides attach per record or per series?\ndid: per-series, matching RetentionPolicy\nflip: a tenant needs per-record legal hold\nat: src/retention.rs:6\n```\n\n```weigh-in\nkind: call\ndoor: two-way\nauthority: judgment\nq: Name the helper apply or run?\ndid: named it apply\n```\n";

/// The Stop hook of session `s` in `root`, ending a turn with [`MESSAGE`], within `budget`.
fn stopped(root: &Path, budget: Budget) {
    let event = HookEvent::Stop(Turn {
        message: MESSAGE.to_owned(),
        agent: None,
    });
    // What the Stop says is the capture's nudge, which these tests do not pin.
    drop(served(root, "s", event, budget));
}

#[test]
fn a_stop_hook_takes_the_stop_budget() {
    let event = HookEvent::Stop(Turn {
        message: String::new(),
        agent: None,
    });
    let env = Env::outside_git(Path::new("."), "s");

    let hook = Hook::new(event, env, false, Instant::now());

    assert_eq!(hook.budget, Budget::STOP);
}

#[test]
#[cfg_attr(miri, ignore = "spawns git and opens SQLite through FFI")]
fn a_stop_past_its_deadline_exports_nothing_and_logs_nothing() -> Result<()> {
    let scratch = tempfile::tempdir()?;
    let root = own_repo(scratch.path(), "repo")?;
    raise(&root, "s", "Keep retention per series?")?;

    stopped(&root, PAST_DEADLINE);

    assert!(
        !has_ref(&root, "refs/redkiln/journal/s"),
        "a passed deadline exports nothing"
    );
    assert_eq!(
        logged(&root)?,
        Vec::<String>::new(),
        "a late journal retries unlogged (D12)"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns git and opens SQLite through FFI")]
fn the_next_session_start_tells_once_that_a_stop_ran_out_of_time() -> Result<()> {
    let scratch = tempfile::tempdir()?;
    let root = own_repo(scratch.path(), "repo")?;
    raise(&root, "s", "Keep retention per series?")?;
    stopped(&root, PAST_DEADLINE);
    let start = || HookEvent::SessionStart {
        source: Some("startup".to_owned()),
    };

    let told = served(&root, "t", start(), UNHURRIED).message;
    let again = served(&root, "u", start(), UNHURRIED).message;

    let told = told.context("the late Stop is told (wi-6ab291e7fb)")?;
    assert!(
        told.starts_with("Weigh-In: hooks ran out of time 1 time(s) since the last session")
            && told.contains("Stop: 1 journal(s) left for the next export."),
        "{told}"
    );
    assert_eq!(again, None, "told once");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns git and opens SQLite through FFI")]
fn config_jsons_class_below_no_suite_toml_exports_the_journal_ref_at_stop() -> Result<()> {
    let scratch = tempfile::tempdir()?;
    let root = repo(scratch.path(), "repo")?;
    let config = root.join(".kb/_ledger/config.json");
    std::fs::create_dir_all(config.parent().ok_or_else(|| anyhow::anyhow!("a parent"))?)?;
    std::fs::write(&config, r#"{"repo_class":"own"}"#)?;
    raise(&root, "s", "Keep retention per series?")?;

    stopped(&root, UNHURRIED);

    assert!(
        has_ref(&root, "refs/redkiln/journal/s"),
        "an own repository exports its journal ref"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns git and opens SQLite through FFI")]
fn an_own_repository_exports_its_journal_ref_at_stop() -> Result<()> {
    let scratch = tempfile::tempdir()?;
    let root = repo(scratch.path(), "repo")?;
    init(&root, RepoClass::Own, Some(BoardMode::Artifact))?;
    raise(&root, "s", "Keep retention per series?")?;

    stopped(&root, UNHURRIED);

    assert!(
        has_ref(&root, "refs/redkiln/journal/s"),
        "own: a journal ref"
    );
    Ok(())
}
