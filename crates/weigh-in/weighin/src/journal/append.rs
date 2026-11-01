//! The journal writer: a session's own events, appended to `<DIR>/<session>.jsonl` (B-28).
//!
//! The file is append-only. A complete line already in it stays where it is, whoever wrote it
//! (this writer, or the cloud hook script's whole-ledger dump), and an event whose line is
//! already there is not appended again. The only bytes ever removed are a last line with no
//! newline that does not decode, which a run killed mid-write cut short; its event is appended
//! again in the same write. Writers serialise on a lock file in the temporary directory, the
//! one the hook script locks, so the two take turns.

use anyhow::{Context as _, Result};
use std::collections::HashSet;
use std::fs::{File, OpenOptions, TryLockError};
use std::io::{ErrorKind, Seek as _, SeekFrom, Write as _};
use std::path::Path;
use std::time::{Duration, Instant};
use weighin_core::EventEnvelope;

use super::{DIR, line, lines};
use crate::env::{ConfigSource, Env};
use crate::ops;

/// How long [`sync`] waits for another writer: 3 s, as the hook script's `flock -w 3`, so a
/// capture hook plus the wait stays well inside the hook timeout.
pub(crate) const WAIT: Duration = Duration::from_secs(3);

/// How often a busy lock is tried again.
const POLL: Duration = Duration::from_millis(25);

/// Why [`sync`] wrote nothing. None is a failure: each is a normal state of a folder or session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Skip {
    /// No session is known, so there is no file to keep.
    NoSession,
    /// The session id cannot be a file name (see [`JournalName`]).
    UnusableSession,
    /// Not a git worktree: a journal exists to be committed.
    NotGit,
    /// The folder has no usable `.kb/_ledger/config.json`.
    NotInitialised,
    /// The config sets `"journal": false`.
    OptedOut,
    /// Another writer held the lock for the whole wait; the next run catches up.
    Busy,
}

/// What [`sync`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub(crate) enum Synced {
    /// Nothing was written, for the reason given.
    Skipped(Skip),
    /// The journal already held every event of the session.
    UpToDate,
    /// This many lines were appended.
    Appended(usize),
}

/// A session id usable as a journal's file stem: not empty, no `/` or `\`, and not starting
/// with `.` (API-03).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JournalName(String);

impl JournalName {
    /// The session id.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<&str> for JournalName {
    type Error = Skip;

    fn try_from(session: &str) -> Result<Self, Skip> {
        if session.is_empty() || session.contains(['/', '\\']) || session.starts_with('.') {
            Err(Skip::UnusableSession)
        } else {
            Ok(Self(session.to_owned()))
        }
    }
}

/// Append this session's events that `<repo root>/DIR/<session>.jsonl` lacks, creating the
/// directory and file only when there is something to append. A folder that is not a git
/// worktree, has no config, or opted out, and a session with no usable id, are skipped.
///
/// # Errors
/// Fails when the ledger cannot be read, an event cannot be encoded, or the lock, the directory
/// or the journal cannot be opened, read or written.
pub(crate) fn sync(env: &Env, wait: Duration) -> Result<Synced> {
    if let Err(skip) = gate(env) {
        return Ok(Synced::Skipped(skip));
    }
    sync_events(env, wait, &ops::stored_events(env)?)
}

/// [`sync`] over `events`, the store's events as the caller read them already, so a hook that
/// also needs the ledger reads the store once.
///
/// # Errors
/// As [`sync`], except that the ledger was read by the caller.
pub(crate) fn sync_events(
    env: &Env,
    wait: Duration,
    events: &[(u64, EventEnvelope)],
) -> Result<Synced> {
    let (name, root) = match gate(env) {
        Ok(gated) => gated,
        Err(skip) => return Ok(Synced::Skipped(skip)),
    };
    let candidates = session_lines(events, name.as_str())?;
    if candidates.is_empty() {
        return Ok(Synced::UpToDate);
    }
    let lock_path = std::env::temp_dir().join(format!("weigh-in-journal-{}.lock", name.as_str()));
    // Held until the end of the function; closing the file releases the lock.
    let Some(_held) = lock(&lock_path, wait)? else {
        return Ok(Synced::Skipped(Skip::Busy));
    };
    let path = root.join(DIR).join(format!("{}.jsonl", name.as_str()));
    let existing = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
    };
    let plan = plan_append(&existing, &candidates)?;
    if plan.add.is_empty() {
        return Ok(Synced::UpToDate);
    }
    write_plan(&path, existing.len(), &plan)?;
    Ok(Synced::Appended(plan.added))
}

/// The journal's name and the worktree root, or why there is no journal to keep.
fn gate(env: &Env) -> Result<(JournalName, &Path), Skip> {
    let name = JournalName::try_from(env.session.as_deref().ok_or(Skip::NoSession)?)?;
    let root = env.repo_root.as_deref().ok_or(Skip::NotGit)?;
    if !matches!(env.config_source, ConfigSource::File) {
        return Err(Skip::NotInitialised);
    }
    if !env.config.journal {
        return Err(Skip::OptedOut);
    }
    Ok((name, root))
}

/// Whether [`sync`] keeps a journal for this session and folder, so a caller reads the ledger
/// for it only then.
pub(crate) fn kept(env: &Env) -> bool {
    gate(env).is_ok()
}

/// The journal lines of `session`'s events, in ledger order.
fn session_lines(events: &[(u64, EventEnvelope)], session: &str) -> Result<Vec<Vec<u8>>> {
    events
        .iter()
        .map(|(_, e)| e)
        .filter(|e| e.session_id.as_deref() == Some(session))
        .map(|e| line(e).context("encoding a ledger event"))
        .collect()
}

/// Write `plan` to the journal at `path`, whose current length is `len`: cut it to `keep`, then
/// append `add` in one write.
fn write_plan(path: &Path, len: usize, plan: &AppendPlan) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let context = || format!("writing {}", path.display());
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .with_context(context)?;
    if plan.keep < u64::try_from(len)? {
        file.set_len(plan.keep).with_context(context)?;
    }
    file.seek(SeekFrom::Start(plan.keep))
        .with_context(context)?;
    file.write_all(&plan.add).with_context(context)?;
    file.flush().with_context(context)
}

/// What to do with a journal so it holds every candidate line.
#[derive(Debug, PartialEq, Eq)]
struct AppendPlan {
    /// How many bytes of the file to keep.
    keep: u64,
    /// The bytes to write at `keep`.
    add: Vec<u8>,
    /// How many lines `add` holds.
    added: usize,
}

/// What to do with `existing` so it holds every line of `candidates`, each of which ends in
/// `\n`. Every complete line of `existing` is kept. A last line with no newline is kept and
/// ended when it decodes as an event; otherwise it was cut short, and only the bytes before it
/// are kept. `add` holds each candidate that is not among the kept lines (one trailing `\r`
/// ignored) or earlier in `candidates`, in order.
///
/// # Errors
/// Fails only when the kept length does not fit a `u64`.
fn plan_append(existing: &[u8], candidates: &[Vec<u8>]) -> Result<AppendPlan> {
    let mut have: HashSet<&[u8]> = HashSet::new();
    let mut keep = existing.len();
    let mut add = Vec::new();
    for l in lines(existing) {
        if l.complete {
            have.insert(l.text);
        } else if EventEnvelope::from_json(l.text).is_ok() {
            have.insert(l.text);
            add.push(b'\n');
        } else {
            // The last piece has no newline, so it is `text` exactly and ends the file.
            keep = existing.len().saturating_sub(l.text.len());
        }
    }
    let mut added = 0_usize;
    for candidate in candidates {
        let text = candidate.strip_suffix(b"\n").unwrap_or(candidate);
        if have.insert(text) {
            add.extend_from_slice(candidate);
            added = added.saturating_add(1);
        }
    }
    if added == 0 {
        add.clear();
    }
    Ok(AppendPlan {
        keep: u64::try_from(keep)?,
        add,
        added,
    })
}

/// An exclusive lock on `path`, tried every [`POLL`] until `wait` has passed; `None` when it
/// stayed busy. The lock file is created when missing and never removed, and the lock lasts
/// until the returned file is closed.
///
/// # Errors
/// Fails when the lock file cannot be opened or locking fails for another reason than a holder.
fn lock(path: &Path, wait: Duration) -> Result<Option<File>> {
    let context = || format!("locking {}", path.display());
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(path)
        .with_context(context)?;
    let start = Instant::now();
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(Some(file)),
            Err(TryLockError::WouldBlock) => {}
            Err(TryLockError::Error(e)) => return Err(e).with_context(context),
        }
        let left = wait.saturating_sub(start.elapsed());
        if left.is_zero() {
            return Ok(None);
        }
        std::thread::sleep(POLL.min(left));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RAISED: &[u8] = br#"{"type":"item_raised","item_id":"wi-b29e54","kind":"question","question":"Keep the cache warm or cold?","door_hint":"unsure","authority_hint":"authority","channel":"manual","quote":"Keep the cache warm or cold?","actor":"cli","session_id":"s","ts":"2026-10-09T03:29:36Z"}"#;
    const PRESENTED: &[u8] = br#"{"type":"item_presented","item_id":"wi-b29e54","surface":"chat","reason":"","actor":"cli","session_id":"s","ts":"2026-10-09T03:29:37Z"}"#;
    const OTHER: &[u8] = br#"{"type":"item_presented","item_id":"wi-b29e54","surface":"chat","reason":"","actor":"cli","session_id":"t","ts":"2026-10-09T03:29:38Z"}"#;

    /// `text` and a newline: one journal line.
    fn nl(text: &[u8]) -> Vec<u8> {
        let mut v = text.to_vec();
        v.push(b'\n');
        v
    }

    fn concat(parts: &[&[u8]]) -> Vec<u8> {
        parts.concat()
    }

    #[test]
    fn a_journal_name_refuses_what_cannot_be_a_file_stem() {
        let refused = ["", "a/b", "a\\b", ".hidden"].map(JournalName::try_from);

        assert_eq!(refused, [const { Err(Skip::UnusableSession) }; 4]);
    }

    #[test]
    fn a_journal_name_accepts_a_uuid() {
        let id = "0b6f5c1e-2a8d-4c4f-9a57-3d7e2f1b9c04";

        let name = JournalName::try_from(id);

        assert_eq!(name.as_ref().map(JournalName::as_str), Ok(id));
    }

    #[test]
    fn an_empty_file_gets_every_candidate_in_order() -> Result<()> {
        let candidates = [nl(RAISED), nl(PRESENTED)];

        let plan = plan_append(b"", &candidates)?;

        let expected = AppendPlan {
            keep: 0,
            add: concat(&[RAISED, b"\n", PRESENTED, b"\n"]),
            added: 2,
        };
        assert_eq!(plan, expected);
        Ok(())
    }

    #[test]
    fn candidates_already_present_add_nothing_even_as_crlf_lines() -> Result<()> {
        let existing = concat(&[RAISED, b"\r\n", PRESENTED, b"\n"]);

        let plan = plan_append(&existing, &[nl(RAISED), nl(PRESENTED)])?;

        let expected = AppendPlan {
            keep: u64::try_from(existing.len())?,
            add: Vec::new(),
            added: 0,
        };
        assert_eq!(plan, expected);
        Ok(())
    }

    #[test]
    fn a_repeated_candidate_is_added_once() -> Result<()> {
        let plan = plan_append(b"", &[nl(RAISED), nl(RAISED)])?;

        let expected = AppendPlan {
            keep: 0,
            add: nl(RAISED),
            added: 1,
        };
        assert_eq!(plan, expected);
        Ok(())
    }

    #[test]
    fn a_cut_short_last_line_is_dropped_and_its_event_added_again() -> Result<()> {
        let cut = PRESENTED
            .get(..40)
            .ok_or_else(|| anyhow::anyhow!("short"))?;
        let existing = concat(&[RAISED, b"\n", cut]);

        let plan = plan_append(&existing, &[nl(RAISED), nl(PRESENTED)])?;

        let expected = AppendPlan {
            keep: u64::try_from(RAISED.len() + 1)?,
            add: nl(PRESENTED),
            added: 1,
        };
        assert_eq!(plan, expected);
        Ok(())
    }

    #[test]
    fn an_unterminated_last_line_that_decodes_is_kept_and_ended() -> Result<()> {
        let existing = concat(&[OTHER, b"\n", RAISED]);

        let plan = plan_append(&existing, &[nl(RAISED), nl(PRESENTED)])?;

        let expected = AppendPlan {
            keep: u64::try_from(existing.len())?,
            add: concat(&[b"\n", PRESENTED, b"\n"]),
            added: 1,
        };
        assert_eq!(plan, expected);
        Ok(())
    }

    #[test]
    fn a_whole_ledger_file_keeps_other_sessions_and_gains_only_missing_lines() -> Result<()> {
        let existing = concat(&[RAISED, b"\n", OTHER, b"\n"]);

        let plan = plan_append(&existing, &[nl(RAISED), nl(PRESENTED)])?;

        let expected = AppendPlan {
            keep: u64::try_from(existing.len())?,
            add: nl(PRESENTED),
            added: 1,
        };
        assert_eq!(plan, expected);
        Ok(())
    }

    #[test]
    #[cfg_attr(miri, ignore = "locks a file through the operating system")]
    fn a_lock_held_elsewhere_is_busy_after_the_wait() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("j.lock");
        let held = lock(&path, Duration::ZERO)?;
        assert!(held.is_some(), "the first lock is free");

        let second = lock(&path, Duration::ZERO)?;

        assert!(second.is_none(), "a held lock is busy");
        Ok(())
    }
}
