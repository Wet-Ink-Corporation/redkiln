//! `SessionStart`'s one-time import of an anneal ledger (Move 2 step 3.5, plan test list 32), run
//! in process with a budget that has no deadline, so that what is pinned does not hang on how fast
//! a runner spawns git (TST-10): moved from `tests/it/legacy_import.rs`, where the binary's 2 s
//! deadline passed before the import on slow Windows runners. A hook past its deadline is
//! `legacy_import`'s own test.

use std::error::Error;
use std::ffi::OsString;
use std::io;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::time::Instant;

use redkiln_core::store::{Filter, Store as CoreStore, Wait};

use super::serve;
use crate::harness::claude_code::keys::{
    ADDITIONAL_CONTEXT, HOOK_EVENT_NAME, HOOK_SPECIFIC_OUTPUT,
};
use crate::hook::HookEnv;
use crate::legacy_import::{self, Budget, MAX_HOOK_SOURCE_BYTES};
use crate::paths::{self, GIT_TIMEOUT};
use crate::test_repo::ScratchRepo;

type TestResult = Result<(), Box<dyn Error>>;

/// A hook's budget with no deadline: the import always runs to its end, whatever the runner's
/// speed.
const fn unhurried(_started: Instant) -> Budget {
    Budget {
        deadline: None,
        max_bytes: Some(MAX_HOOK_SOURCE_BYTES),
    }
}

/// A scratch repository whose `.git/anneal/ledger.sqlite3` is a copy of the 0.0.5 fixture, and
/// that directory. Its `suite.toml` names the owner: in process the hook's git reads the
/// machine's own configuration, whose `user.name` would otherwise decide whether one plan
/// approval is also recorded for other plugins (23 imported rather than 22).
fn repository() -> Result<(ScratchRepo, PathBuf), Box<dyn Error>> {
    let repo = ScratchRepo::new()?;
    repo.write(".redkiln/suite.toml", "[suite]\nowner = \"ryan\"\n")?;
    let dir = repo.path().join(".git/anneal");
    std::fs::create_dir_all(&dir)?;
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/anneal/anneal-0.0.5.sqlite3");
    std::fs::copy(fixture, dir.join("ledger.sqlite3"))?;
    Ok((repo, dir))
}

/// A file's name and bytes.
type Snapshot = Vec<(String, Vec<u8>)>;

/// Every file in `dir` with its bytes, sorted by name.
fn snapshot(dir: &Path) -> Result<Snapshot, Box<dyn Error>> {
    let mut files = std::fs::read_dir(dir)?
        .map(|entry| {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            Ok((name, std::fs::read(entry.path())?))
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    files.sort();
    Ok(files)
}

/// How many events in the store at `path` are Bench's, and how many are import records.
fn counts(path: &Path) -> Result<(usize, usize), Box<dyn Error>> {
    let store = CoreStore::open_existing(path, Wait::Command)?.ok_or("the store")?;
    let limit = NonZeroUsize::new(4096).ok_or("non-zero")?;
    let types: Vec<String> = store
        .read(&Filter::all(), 0, limit)?
        .into_iter()
        .map(|stored| stored.event_type)
        .collect();
    Ok((
        types.iter().filter(|t| t.starts_with("bench.")).count(),
        types
            .iter()
            .filter(|t| t.as_str() == "core.import_completed")
            .count(),
    ))
}

/// The variables the hook reads in `cwd`: only an anneal home inside it.
fn lookup_in(cwd: &Path) -> impl Fn(&str) -> Option<OsString> {
    let home = cwd.join(".anneal-home");
    move |var: &str| (var == "ANNEAL_HOME").then(|| OsString::from(&home))
}

/// The `SessionStart` hook in `cwd` with no deadline: its context.
fn session_start(cwd: &Path) -> Result<String, Box<dyn Error>> {
    let input = serde_json::json!({
        "session_id": "s1", "cwd": cwd,
        HOOK_EVENT_NAME: "SessionStart", "source": "startup",
    });
    let stdin = serde_json::to_vec(&input)?;
    let env = HookEnv {
        stdin: stdin.as_slice(),
        current_dir: || Err(io::Error::other("the input names the cwd")),
        lookup: lookup_in(cwd),
    };
    let mut out = Vec::new();
    serve(env, &mut out, unhurried)?;
    let json: serde_json::Value = serde_json::from_slice(&out)?;
    Ok(json[HOOK_SPECIFIC_OUTPUT][ADDITIONAL_CONTEXT]
        .as_str()
        .ok_or("the context")?
        .to_owned())
}

#[test]
#[cfg_attr(miri, ignore = "runs git and opens SQLite through FFI")]
fn session_start_imports_an_old_ledger_once_and_a_later_run_appends_nothing() -> TestResult {
    let (work, old) = repository()?;
    let before = snapshot(&old)?;
    let store = work.path().join(".git/redkiln/events.sqlite3");

    let said = session_start(work.path())?;

    assert!(
        said.contains("is left as it is: its events are imported into the suite store, once")
            && said.contains("23 imported, 0 already there, 1 skipped"),
        "SessionStart says what it imported: {said}"
    );
    assert!(
        said.contains("actor \"test\""),
        "and the refused actor: {said}"
    );
    assert_eq!(counts(&store)?, (22, 1), "22 events, one record");
    assert_eq!(snapshot(&old)?, before, "the ledger is left as it was");

    let again = session_start(work.path())?;
    let location =
        paths::ledger_store_within(None, lookup_in(work.path()), work.path(), GIT_TIMEOUT)?;
    let opened = legacy_import::open_for_command(&location)?;

    assert!(
        !again.contains("anneal ledger"),
        "nothing due, nothing said: {again}"
    );
    assert!(
        legacy_import::due(Some(&opened), &location.old_ledgers)?.is_empty(),
        "nor by a command, which says only what is due"
    );
    assert_eq!(counts(&store)?, (22, 1), "a command imports nothing again");
    assert_eq!(snapshot(&old)?, before, "still as it was");
    Ok(())
}
