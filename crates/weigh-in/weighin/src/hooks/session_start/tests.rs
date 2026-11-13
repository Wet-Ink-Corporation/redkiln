//! `SessionStart`'s import of journal refs before it primes (SPEC JR-10 to JR-13, D10), run in
//! process with the budget the test gives the hook, so that what is pinned does not hang on how
//! fast a runner spawns git (TST-10): another clone's items count in the prime, a rewritten ref
//! is told to the user, an unchanged ref is not recorded again, a 1.x journal too large for the
//! hook holds nothing back, and a hook past its deadline imports nothing and tells nobody (D12).
//! The deadline-independent cases run the binary in `tests/it/journal_session_start.rs`.

use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context as _, Result};
use redkiln_core::store::{Filter, Store, Wait};

use crate::env::Env;
use crate::hooks::test_support::{
    checkpoint, git, logged, own_repo, past_deadline, path_str, raise, served, unhurried,
};
use crate::hooks::{Hook, HookEvent};
use crate::journal::refs::Budget;

/// `SessionStart`'s budget with no deadline, each git call given a command's time: the import
/// always runs to its end, whatever the runner's speed.
const UNHURRIED: Budget = unhurried(Budget::SESSION_START);

/// `SessionStart`'s budget with a deadline that has passed when the hook starts.
const PAST_DEADLINE: Budget = past_deadline(Budget::SESSION_START);

/// An own repository `exporter` in `scratch` whose `origin` is a bare `origin.git` beside it,
/// holding one item raised in session `a`, exported; the exporter's and the remote's paths.
fn exporting(scratch: &Path) -> Result<(PathBuf, PathBuf)> {
    let exporter = own_repo(scratch, "exporter")?;
    let remote = scratch.join("origin.git");
    std::fs::create_dir_all(&remote)?;
    git(&remote, &["init", "-q", "--bare", "-b", "main"])?;
    git(&exporter, &["remote", "add", "origin", &path_str(&remote)?])?;
    git(&exporter, &["push", "-q", "origin", "main"])?;
    raise(&exporter, "a", "Shard the ledger by tenant?")?;
    checkpoint(&exporter)?;
    Ok((exporter, remote))
}

/// Another own repository, `reader`, in `scratch`, whose `origin` is `remote`.
fn other_clone(scratch: &Path, remote: &Path) -> Result<PathBuf> {
    let reader = own_repo(scratch, "reader")?;
    git(&reader, &["remote", "add", "origin", &path_str(remote)?])?;
    Ok(reader)
}

/// What `SessionStart` says in `root` for `session` within `budget`: the message for the user,
/// and the context.
fn started(root: &Path, session: &str, budget: Budget) -> (Option<String>, String) {
    let event = HookEvent::SessionStart {
        source: Some("startup".to_owned()),
    };
    let output = served(root, session, event, budget);
    (output.message, output.context.unwrap_or_default())
}

/// How many `core.import_completed` records the repository's store holds.
fn import_records(root: &Path) -> Result<usize> {
    let store = Store::open(&root.join(".git/redkiln/events.sqlite3"), Wait::Command)?;
    let filter = Filter::all().of_type("core.import_completed")?;
    let page = NonZeroUsize::new(1_000).context("a page")?;
    Ok(store.read(&filter, 0, page)?.len())
}

#[test]
fn a_hook_imports_inside_the_session_start_budget() {
    let event = HookEvent::SessionStart { source: None };
    let env = Env::outside_git(Path::new("."), "s");

    let hook = Hook::new(event, env, false, Instant::now());

    assert_eq!(hook.budget, Budget::SESSION_START);
}

#[test]
#[cfg_attr(miri, ignore = "spawns git and opens SQLite through FFI")]
fn a_session_start_past_its_deadline_imports_nothing_and_tells_nobody() -> Result<()> {
    let scratch = tempfile::tempdir()?;
    let (_exporter, remote) = exporting(scratch.path())?;
    let reader = other_clone(scratch.path(), &remote)?;

    let (message, context) = started(&reader, "b", PAST_DEADLINE);

    assert!(
        context.contains("0 open in repo"),
        "nothing was imported: {context}"
    );
    assert_eq!(message, None, "a passed deadline is silent (D12)");
    assert_eq!(import_records(&reader)?, 0);
    assert_eq!(logged(&reader)?, Vec::<String>::new());
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns git and opens SQLite through FFI")]
fn session_start_imports_another_clones_journal_refs_before_priming() -> Result<()> {
    let scratch = tempfile::tempdir()?;
    let (_exporter, remote) = exporting(scratch.path())?;
    let reader = other_clone(scratch.path(), &remote)?;

    let (message, context) = started(&reader, "b", UNHURRIED);

    assert!(
        context.contains("1 open in repo"),
        "the imported item counts in the prime: {context}"
    );
    assert_eq!(message, None, "nothing to tell the user");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns git and opens SQLite through FFI")]
fn session_start_tells_the_user_of_a_rewritten_journal_ref() -> Result<()> {
    let scratch = tempfile::tempdir()?;
    let (exporter, remote) = exporting(scratch.path())?;
    let reader = other_clone(scratch.path(), &remote)?;
    started(&reader, "b", UNHURRIED);
    // The ref goes from both sides and comes back as a new root commit: not a descendant of the
    // commit the reader imported.
    git(&remote, &["update-ref", "-d", "refs/redkiln/journal/a"])?;
    git(&exporter, &["update-ref", "-d", "refs/redkiln/journal/a"])?;
    raise(&exporter, "a", "Keep three retries?")?;
    checkpoint(&exporter)?;

    let (message, context) = started(&reader, "b", UNHURRIED);

    let text = message.context("a message for the user")?;
    assert!(
        text.contains("journal ref(s) rewritten since the last import")
            && text.contains("refs/redkiln/journal/a"),
        "{text}"
    );
    assert!(
        context.contains("2 open in repo"),
        "its new lines were imported: {context}"
    );
    assert!(
        !context.contains("rewritten"),
        "Claude's context is unchanged: {context}"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns git and opens SQLite through FFI")]
fn a_second_session_start_with_no_moved_ref_writes_no_import_record() -> Result<()> {
    let scratch = tempfile::tempdir()?;
    let (_exporter, remote) = exporting(scratch.path())?;
    let reader = other_clone(scratch.path(), &remote)?;
    started(&reader, "b", UNHURRIED);
    let first = import_records(&reader)?;
    assert!(first > 0, "the first SessionStart imported");

    started(&reader, "c", UNHURRIED);

    assert_eq!(import_records(&reader)?, first);
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns git and opens SQLite through FFI")]
fn a_1x_journal_too_large_for_the_hook_does_not_hold_back_the_journal_refs() -> Result<()> {
    let scratch = tempfile::tempdir()?;
    let (_exporter, remote) = exporting(scratch.path())?;
    let reader = other_clone(scratch.path(), &remote)?;
    // A 1.x journal past what SessionStart imports: the hook leaves it for a command, every
    // session, having opened no source (review F1). A sparse file: its length is what counts.
    let dir = reader.join(".kb/_ledger/journal");
    std::fs::create_dir_all(&dir)?;
    std::fs::File::create(dir.join("old.jsonl"))?.set_len(9 * 1024 * 1024)?;

    let (_message, context) = started(&reader, "b", UNHURRIED);

    assert!(
        context.contains("1 open in repo"),
        "the journal ref was imported all the same: {context}"
    );
    Ok(())
}
