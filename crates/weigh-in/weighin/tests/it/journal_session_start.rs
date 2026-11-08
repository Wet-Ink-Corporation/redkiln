//! `SessionStart` imports journal refs before it primes (SPEC JR-10 to JR-13, D10): another
//! clone's items count in the prime, a rewritten ref is told to the user, an unreachable remote
//! is silent, a `client` repository imports nothing, and an unchanged ref is not recorded again.

use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use redkiln_core::store::{Filter, Store, Wait};
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

/// How many `core.import_completed` records the repository's store holds.
fn import_records(repo: &Repo) -> TestResult<usize> {
    let store = Store::open(&store_file(repo), Wait::Command)?;
    let filter = Filter::all().of_type("core.import_completed")?;
    let page = NonZeroUsize::new(1_000).ok_or("a page")?;
    Ok(store.read(&filter, 0, page)?.len())
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
fn session_start_imports_another_clones_journal_refs_before_priming() -> TestResult {
    let (_exporter, remote) = exporting()?;
    let reader = other_clone(&remote, true)?;

    let (message, context) = started(&reader, "b")?;

    assert!(
        context.contains("1 open in repo"),
        "the imported item counts in the prime: {context}"
    );
    assert_eq!(message, None, "nothing to tell the user");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn session_start_with_an_unreachable_remote_primes_as_before_and_tells_nobody() -> TestResult {
    let repo = Repo::new(true)?;
    let gone = repo.sibling("gone.git")?.join("missing");
    git(
        &repo.root,
        &["remote", "add", "origin", &gone.to_string_lossy()],
    );

    let (message, context) = started(&repo, "s")?;

    assert_eq!(message, None, "an offline laptop is normal");
    assert!(context.contains("0 open in repo"), "{context}");
    assert_eq!(logged(&repo)?, Vec::<String>::new());
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn session_start_tells_the_user_of_a_rewritten_journal_ref() -> TestResult {
    let (exporter, remote) = exporting()?;
    let reader = other_clone(&remote, true)?;
    started(&reader, "b")?;
    // The ref goes from both sides and comes back as a new root commit: not a descendant of the
    // commit the reader imported.
    git(&remote, &["update-ref", "-d", "refs/redkiln/journal/a"]);
    git(
        &exporter.root,
        &["update-ref", "-d", "refs/redkiln/journal/a"],
    );
    exporter.raise("a", "Keep three retries?")?;
    exporter.ok("cli", &["journal", "checkpoint"], None)?;

    let (message, context) = started(&reader, "b")?;

    let message = message.ok_or("a message for the user")?;
    let text = message["systemMessage"].as_str().ok_or("systemMessage")?;
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
fn a_second_session_start_with_no_moved_ref_writes_no_import_record() -> TestResult {
    let (_exporter, remote) = exporting()?;
    let reader = other_clone(&remote, true)?;
    started(&reader, "b")?;
    let first = import_records(&reader)?;
    assert!(first > 0, "the first SessionStart imported");

    started(&reader, "c")?;

    assert_eq!(import_records(&reader)?, first);
    Ok(())
}
