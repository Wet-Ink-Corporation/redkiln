//! `SessionStart` imports journal refs before it primes (SPEC JR-10 to JR-13, D10), through the
//! binary, in the cases that hold whether or not the import beats its 2 s deadline: a `client`
//! repository imports nothing, and a store that cannot be opened is logged once. The cases that
//! need the import to run to its end (another clone's items in the prime, a rewritten ref, no
//! second import record, a too-large 1.x journal) and an unreachable remote's silence, which a
//! late import now breaks (wi-6ab291e7fb), are run in process with a budget the test gives the
//! hook, in `src/hooks/session_start/tests.rs`, since a slow runner spawning git passes the
//! deadline (TST-10; Ryan, 2026-11-12).

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::scratch::{Repo, TestResult};
use crate::support::{git, state_of};

/// The `SessionStart` payload for `session` in `repo`.
fn start(repo: &Repo, session: &str) -> Value {
    json!({"hook_event_name": "SessionStart", "session_id": session, "cwd": repo.root,
        "source": "startup"})
}

/// What `SessionStart` printed: its JSON, when it has a message for the user, else `None` and the
/// plain context.
fn started(repo: &Repo, session: &str) -> TestResult<(Option<Value>, String)> {
    let out = repo.hook(&start(repo, session))?;
    if !out.status.success() {
        return Err(format!("the hook failed: {}", String::from_utf8_lossy(&out.stderr)).into());
    }
    let stdout = String::from_utf8(out.stdout)?;
    match serde_json::from_str::<Value>(&stdout) {
        Ok(json) => {
            let context = json
                .get("hookSpecificOutput")
                .and_then(|output| output.get("additionalContext"))
                .and_then(Value::as_str)
                .ok_or("additionalContext")?
                .to_owned();
            Ok((Some(json), context))
        }
        Err(_) => Ok((None, stdout)),
    }
}

/// A repository whose Stop and `journal checkpoint` export to a bare `origin`, holding one item
/// raised in session `a`, exported; and the remote's path.
fn exporting() -> TestResult<(Repo, PathBuf)> {
    let (repo, remote) = Repo::with_remote(true)?;
    repo.raise("a", "Shard the ledger by tenant?")?;
    repo.ok("cli", &["journal", "checkpoint"], None)?;
    Ok((repo, remote))
}

/// Another repository, initialised `own` when `own` is true, whose `origin` is `remote`.
fn other_clone(remote: &Path, own: bool) -> TestResult<Repo> {
    let repo = Repo::new(own)?;
    git(
        &repo.root,
        &["remote", "add", "origin", &remote.to_string_lossy()],
    );
    Ok(repo)
}

/// The store a repository's hooks write.
fn store_file(repo: &Repo) -> PathBuf {
    repo.root.join(".git/redkiln/events.sqlite3")
}

/// The lines of the hook error log, none when there is no log.
fn logged(repo: &Repo) -> TestResult<Vec<String>> {
    match std::fs::read_to_string(state_of(&repo.root).join("hook-errors.log")) {
        Ok(text) => Ok(text.lines().map(str::to_owned).collect()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e.into()),
    }
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn session_start_in_a_client_repository_imports_nothing_and_creates_no_store() -> TestResult {
    let (_exporter, remote) = exporting()?;
    let client = other_clone(&remote, false)?;

    started(&client, "b")?;

    assert!(!store_file(&client).exists(), "no store");
    assert!(!client.has_ref("refs/redkiln/journal/a"), "nothing fetched");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_store_that_cannot_be_opened_is_logged_once_by_session_start() -> TestResult {
    let repo = Repo::new(true)?;
    let store = store_file(&repo);
    std::fs::create_dir_all(store.parent().ok_or("a parent")?)?;
    std::fs::write(&store, "not a database, and longer than a header would be")?;

    started(&repo, "s")?;

    let lines = logged(&repo)?;
    assert_eq!(lines.len(), 1, "the prime's failure only: {lines:?}");
    Ok(())
}
