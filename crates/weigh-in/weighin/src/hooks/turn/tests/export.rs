//! The Stop hook's export to journal refs and its watermark (SPEC JR-07, JR-08, D11, D12), in
//! the cases that hold only when the export runs to its end: moved from the binary tests in
//! `tests/it/journal_hooks.rs`, whose runs on a slow runner pass `Budget::STOP`'s 4 s deadline
//! before the ref is written (TST-10). Each runs the hook in process with no deadline. In process
//! the hook has no exit code: that a hook exits 0 is pinned through the binary there.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use redkiln_core::store::Wait;

use super::{MESSAGE, UNHURRIED};
use crate::env::{Env, Vars};
use crate::hooks::test_support::{
    checkpoint, env_of, git, git_out, has_ref, logged, own_repo, own_with_remote, printed_for,
    raise, served_in,
};
use crate::hooks::{HookEvent, Turn};

/// The Stop hook in `env`, ending a turn with [`MESSAGE`], with no deadline.
fn stop_in(env: Env) {
    let event = HookEvent::Stop(Turn {
        message: MESSAGE.to_owned(),
        agent: None,
    });
    // What the Stop says is the capture's nudge, which these tests do not pin.
    drop(served_in(env, event, UNHURRIED));
}

/// The Stop hook of `session` in `root`, with no deadline.
fn stop(root: &Path, session: &str) {
    stop_in(env_of(root, session));
}

/// The state dir of `root`'s default store.
fn state_of(root: &Path) -> PathBuf {
    root.join(".git/redkiln/weighin")
}

/// The export watermarks in the state dir `state`, one per repository and store, sorted, with
/// what each holds.
fn watermarks(state: &Path) -> Result<Vec<(PathBuf, String)>> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(state)? {
        let path = entry?.path();
        let named = path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with("journal-watermark-"));
        if named {
            let text = std::fs::read_to_string(&path)?;
            found.push((path, text));
        }
    }
    found.sort();
    Ok(found)
}

/// The one watermark in `root`'s state dir.
fn watermark(root: &Path) -> Result<PathBuf> {
    match watermarks(&state_of(root))?.as_slice() {
        [(path, _)] => Ok(path.clone()),
        found => anyhow::bail!("one watermark, not {found:?}"),
    }
}

/// Sets the store position the watermark records to `position`, keeping the rest of it.
fn set_watermark(root: &Path, position: impl FnOnce(u64) -> u64) -> Result<()> {
    let path = watermark(root)?;
    let text = std::fs::read_to_string(&path)?;
    let (first, rest) = text.split_once('\n').unwrap_or((text.as_str(), ""));
    let moved = position(first.trim().parse()?);
    std::fs::write(&path, format!("{moved}\n{rest}"))?;
    Ok(())
}

/// The store position the watermark records.
fn watermark_at(root: &Path) -> Result<u64> {
    let text = std::fs::read_to_string(watermark(root)?)?;
    Ok(text.lines().next().context("a position")?.parse()?)
}

/// Replaces the watermark's last line, the store it was written for, with `store`.
fn set_watermark_store(root: &Path, store: &str) -> Result<()> {
    let path = watermark(root)?;
    let text = std::fs::read_to_string(&path)?;
    let mut lines: Vec<&str> = text.lines().collect();
    lines.pop().context("a store line")?;
    lines.push(store);
    std::fs::write(&path, format!("{}\n", lines.join("\n")))?;
    Ok(())
}

/// The environment of `session` in `root` in a process whose `$REDKILN_STORE` is `store`.
fn env_on(root: &Path, session: &str, store: &Path) -> Env {
    Env::discover(
        root,
        Some((session.to_owned(), "the test")),
        None,
        Vars::redkiln_store(store),
    )
    .with_wait(Wait::Hook)
}

#[test]
#[cfg_attr(miri, ignore = "spawns git and opens SQLite through FFI")]
fn a_stop_hook_exports_the_sessions_events_to_its_journal_ref() -> Result<()> {
    let scratch = tempfile::tempdir()?;
    let (root, remote) = own_with_remote(scratch.path(), "repo")?;

    stop(&root, "s");

    let stored = printed_for(&root, "s")?.lines().count();
    assert!(stored > 0, "the hook raised items");
    let journal = git_out(&remote, &["show", "refs/redkiln/journal/s:s.jsonl"])?;
    assert_eq!(journal.lines().count(), stored, "{journal}");
    assert_eq!(logged(&root)?, Vec::<String>::new());
    Ok(())
}

#[cfg(unix)]
#[test]
#[cfg_attr(miri, ignore = "spawns git and opens SQLite through FFI")]
fn a_stop_hook_whose_remote_refuses_custom_refs_pushes_an_inbox_branch() -> Result<()> {
    use crate::hooks::test_support::{pre_receive, refs};
    let scratch = tempfile::tempdir()?;
    let (root, remote) = own_with_remote(scratch.path(), "repo")?;
    pre_receive(
        &remote,
        "#!/bin/sh\nwhile read old new ref; do\n  case \"$ref\" in\n    refs/heads/*) ;;\n    *) echo \"push contains a ref outside refs/heads/*\" >&2; exit 1 ;;\n  esac\ndone\n",
    )?;

    stop(&root, "s");

    assert_eq!(
        refs(&remote, "refs/heads/redkiln")?,
        ["refs/heads/redkiln/inbox/s"]
    );
    assert_eq!(refs(&remote, "refs/redkiln")?, Vec::<String>::new());
    assert_eq!(
        logged(&root)?,
        Vec::<String>::new(),
        "a relayed journal needs no one"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns git and opens SQLite through FFI")]
fn a_stop_hook_finding_a_broken_journal_ref_logs_it_once() -> Result<()> {
    let scratch = tempfile::tempdir()?;
    let (root, remote) = own_with_remote(scratch.path(), "repo")?;
    // The repository's own commit: its tree is not exactly `s.jsonl`, so the ref breaks JR-05.
    git(
        &root,
        &["push", "-q", "origin", "HEAD:refs/redkiln/journal/s"],
    )?;
    let before = git_out(&remote, &["rev-parse", "refs/redkiln/journal/s"])?;

    stop(&root, "s");

    let log = logged(&root)?;
    assert_eq!(log.len(), 1, "{log:?}");
    assert!(
        log.iter()
            .all(|l| l.contains("Stop: refs/redkiln/journal/s breaks JR-05 (tree)")),
        "{log:?}"
    );
    assert_eq!(
        git_out(&remote, &["rev-parse", "refs/redkiln/journal/s"])?,
        before,
        "left alone"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns git and opens SQLite through FFI")]
fn a_watermark_past_the_store_head_is_ignored() -> Result<()> {
    let scratch = tempfile::tempdir()?;
    let root = own_repo(scratch.path(), "repo")?;
    raise(&root, "u", "Keep the cache warm?")?;
    checkpoint(&root)?;
    set_watermark(&root, |at| at.saturating_add(1_000))?;
    raise(&root, "t", "Keep three retries?")?;

    stop(&root, "s");

    assert!(
        has_ref(&root, "refs/redkiln/journal/t"),
        "a store behind its watermark is not the one it was written for: everything is examined"
    );
    Ok(())
}

#[cfg(unix)]
#[test]
#[cfg_attr(miri, ignore = "spawns git and opens SQLite through FFI")]
fn a_stop_after_a_deferred_export_examines_the_deferred_journals_again() -> Result<()> {
    use crate::hooks::test_support::{pre_receive, refs};
    let scratch = tempfile::tempdir()?;
    let (root, remote) = own_with_remote(scratch.path(), "repo")?;
    raise(&root, "u", "Keep the cache warm?")?;
    checkpoint(&root)?;
    assert!(watermark(&root).is_ok(), "the sweep moved the watermark");
    raise(&root, "t", "Keep three retries?")?;
    pre_receive(
        &remote,
        "#!/bin/sh\necho \"closed for maintenance\" >&2\nexit 1\n",
    )?;
    stop(&root, "s");
    assert_eq!(
        refs(&remote, "refs/redkiln")?,
        ["refs/redkiln/journal/u"],
        "t and s deferred"
    );
    assert_eq!(
        refs(&remote, "refs/heads/redkiln")?,
        Vec::<String>::new(),
        "deferred"
    );
    std::fs::remove_file(remote.join("hooks/pre-receive"))?;

    stop(&root, "s");

    assert!(
        refs(&remote, "refs/redkiln/journal")?.contains(&"refs/redkiln/journal/t".to_owned()),
        "t's events are past no clean export, so the next Stop examines t again"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns git and opens SQLite through FFI")]
fn a_watermark_written_for_another_store_is_ignored() -> Result<()> {
    let scratch = tempfile::tempdir()?;
    let root = own_repo(scratch.path(), "repo")?;
    raise(&root, "u", "Keep the cache warm?")?;
    checkpoint(&root)?;
    // A full scan would export u again; one scoped by the watermark does not look at it.
    git(&root, &["update-ref", "-d", "refs/redkiln/journal/u"])?;
    stop(&root, "s");
    assert!(
        !has_ref(&root, "refs/redkiln/journal/u"),
        "scoped: u not examined"
    );
    set_watermark_store(&root, "00000000000000000000000000000000")?;

    stop(&root, "s");

    assert!(
        has_ref(&root, "refs/redkiln/journal/u"),
        "another store's watermark: full scan"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns git and opens SQLite through FFI")]
fn two_repositories_on_one_store_each_keep_their_own_watermark() -> Result<()> {
    let scratch = tempfile::tempdir()?;
    let a = own_repo(scratch.path(), "a")?;
    let b = own_repo(scratch.path(), "b")?;
    let shared = scratch.path().join("shared");
    std::fs::create_dir_all(&shared)?;
    let store = shared.join("events.sqlite3");
    let state = store.with_file_name("weighin");
    stop_in(env_on(&a, "sa", &store));
    stop_in(env_on(&b, "sb", &store));
    let both = watermarks(&state)?;
    assert_eq!(both.len(), 2, "one per repository: {both:?}");

    stop_in(env_on(&a, "sa2", &store));

    assert!(
        has_ref(&a, "refs/redkiln/journal/sa2"),
        "a exports its session"
    );
    let after = watermarks(&state)?;
    let kept = after.iter().filter(|mark| both.contains(mark)).count();
    assert_eq!(kept, 1, "b's is untouched, a's moved on: {after:?}");
    stop_in(env_on(&b, "sb2", &store));
    assert!(
        has_ref(&b, "refs/redkiln/journal/sb2"),
        "b exports its session"
    );
    let last = watermarks(&state)?;
    assert_eq!(last.len(), 2, "{last:?}");
    assert!(
        last.iter()
            .any(|mark| after.contains(mark) && !both.contains(mark)),
        "a's moved watermark is untouched by b: {last:?}"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns git and opens SQLite through FFI")]
fn a_stop_finding_a_broken_ref_leaves_the_watermark_unchanged() -> Result<()> {
    let scratch = tempfile::tempdir()?;
    let root = own_repo(scratch.path(), "repo")?;
    raise(&root, "u", "Keep the cache warm?")?;
    checkpoint(&root)?;
    let before = watermark_at(&root)?;
    raise(&root, "t", "Keep three retries?")?;
    // The repository's own commit: its tree is not exactly `t.jsonl`, so the ref breaks JR-05.
    git(&root, &["update-ref", "refs/redkiln/journal/t", "HEAD"])?;

    stop(&root, "s");

    assert_eq!(logged(&root)?.len(), 1, "the broken ref is logged");
    assert_eq!(
        watermark_at(&root)?,
        before,
        "t is not settled, so nothing after it is"
    );
    Ok(())
}
