//! `SessionStart`'s import of journal refs before it primes (SPEC JR-10 to JR-13, D10), run in
//! process with the budget the test gives the hook, so that what is pinned does not hang on how
//! fast a runner spawns git (TST-10): another clone's items count in the prime, a rewritten ref
//! is told to the user, an unchanged ref is not recorded again, a 1.x journal too large for the
//! hook holds nothing back, and a hook past its deadline imports nothing and tells nobody (D12).
//! The deadline-independent cases run the binary in `tests/it/journal_session_start.rs`.

use std::io;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, bail};
use redkiln_core::store::{Filter, Store, Wait};

use crate::env::{Env, RepoClass, Vars};
use crate::hooks::{Hook, HookEvent, serve};
use crate::journal::refs::{self, Budget, Checkpointed, Scope};
use crate::ops::{RaiseInput, Session};

/// `SessionStart`'s budget with no deadline, each git call given a command's time: the import
/// always runs to its end, whatever the runner's speed.
const UNHURRIED: Budget = Budget {
    start_by: None,
    per_call: Budget::COMMAND.per_call,
    ..Budget::SESSION_START
};

/// `SessionStart`'s budget with a deadline that has passed when the hook starts.
const PAST_DEADLINE: Budget = Budget {
    start_by: Some(Duration::ZERO),
    ..Budget::SESSION_START
};

/// Run git in `dir` under a fixed identity, failing on a non-zero exit.
fn git(dir: &Path, args: &[&str]) -> Result<()> {
    crate::git::run_env(
        dir,
        args,
        &[
            ("GIT_AUTHOR_NAME", "t"),
            ("GIT_AUTHOR_EMAIL", "t@t"),
            ("GIT_COMMITTER_NAME", "t"),
            ("GIT_COMMITTER_EMAIL", "t@t"),
        ],
    )
    .map(drop)
}

/// The environment of `session` in `dir`, as the hook discovers it, with none of the process's
/// store variables.
fn env_of(dir: &Path, session: &str) -> Env {
    Env::discover(
        dir,
        Some((session.to_owned(), "the test")),
        None,
        Vars::default(),
    )
    .with_wait(Wait::Hook)
}

/// A repository with one commit on `main` at `<scratch>/<name>`, initialised `own` when `own`
/// is true (`weighin init --class own`).
fn repo(scratch: &Path, name: &str, own: bool) -> Result<PathBuf> {
    let root = scratch.join(name);
    std::fs::create_dir_all(&root)?;
    std::fs::write(root.join("README.md"), "# Scratch\n")?;
    git(&root, &["init", "-q", "-b", "main"])?;
    git(&root, &["add", "."])?;
    git(&root, &["commit", "-q", "-m", "init"])?;
    if own {
        let args = crate::InitArgs {
            class: Some(RepoClass::Own),
            board: None,
            board_url: None,
            owner: None,
            ledger: None,
        };
        crate::commands::init(&mut env_of(&root, "s"), args, &mut io::sink())?;
    }
    Ok(root)
}

/// Raise `q` in `session` of `root`, as `weighin raise --kind question` does.
fn raise(root: &Path, session: &str, q: &str) -> Result<()> {
    let input = RaiseInput {
        q: q.to_owned(),
        kind: Some("question".to_owned()),
        ..RaiseInput::default()
    };
    Session::open(&env_of(root, session), "cli")?.raise(&input)?;
    Ok(())
}

/// Export `root`'s journals, as `weighin journal checkpoint` does.
fn checkpoint(root: &Path) -> Result<()> {
    match refs::checkpoint(
        &env_of(root, "s"),
        Budget::COMMAND,
        Instant::now(),
        Scope::All,
    )? {
        Checkpointed::Ran(..) => Ok(()),
        Checkpointed::Skipped(skip) => bail!("the checkpoint was skipped: {}", skip.as_str()),
    }
}

/// An own repository `exporter` in `scratch` whose `origin` is a bare `origin.git` beside it,
/// holding one item raised in session `a`, exported; the exporter's and the remote's paths.
fn exporting(scratch: &Path) -> Result<(PathBuf, PathBuf)> {
    let exporter = repo(scratch, "exporter", true)?;
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
    let reader = repo(scratch, "reader", true)?;
    git(&reader, &["remote", "add", "origin", &path_str(remote)?])?;
    Ok(reader)
}

fn path_str(path: &Path) -> Result<String> {
    path.to_str()
        .map(str::to_owned)
        .context("a UTF-8 scratch path")
}

/// What `SessionStart` says in `root` for `session` within `budget`: the message for the user,
/// and the context.
fn started(root: &Path, session: &str, budget: Budget) -> (Option<String>, String) {
    let event = HookEvent::SessionStart {
        source: Some("startup".to_owned()),
    };
    let hook = Hook::new(event, env_of(root, session), false, Instant::now()).budget(budget);
    let output = serve(&hook, None);
    (output.message, output.context.unwrap_or_default())
}

/// How many `core.import_completed` records the repository's store holds.
fn import_records(root: &Path) -> Result<usize> {
    let store = Store::open(&root.join(".git/redkiln/events.sqlite3"), Wait::Command)?;
    let filter = Filter::all().of_type("core.import_completed")?;
    let page = NonZeroUsize::new(1_000).context("a page")?;
    Ok(store.read(&filter, 0, page)?.len())
}

/// The lines of the hook error log, none when there is no log.
fn logged(root: &Path) -> Result<Vec<String>> {
    let log = root.join(".git/redkiln/weighin/hook-errors.log");
    match std::fs::read_to_string(log) {
        Ok(text) => Ok(text.lines().map(str::to_owned).collect()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e.into()),
    }
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
