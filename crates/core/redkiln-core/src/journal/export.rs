//! The export checkpoint (SPEC JR-08, JR-09) into a directory of journal files,
//! `<dir>/<name>.jsonl`: every durable event a journal lacks is appended to it, in store order.
//!
//! This is the file half of the checkpoint; the journal refs and the cloud inbox (JR-07) are
//! Move 1's step 7, and build on the same lines. A journal file is append-only (JR-05): a
//! complete line is never changed or removed. The only bytes dropped are a last line with no
//! `\n` that does not decode, which a writer killed mid-line left; its event is appended again
//! (JR-03). Writers of one file take turns on a whole-file lock of the file itself, waiting at
//! most the caller's wait; a journal still locked is left for the next checkpoint. On Windows the
//! lock also keeps readers out while a checkpoint writes.
//!
//! The main log never holds telemetry (it lives in its side store), so every event read here is
//! durable (JR-04). An event that is not an envelope (written through
//! [`crate::store::NewEvent`] directly) has no line and is counted, not exported.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{File, OpenOptions, TryLockError};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use happenstance_core::EventId;

use super::line::{Held, encode, held, lines};
use super::name::JournalName;
use crate::store::{Filter, Store, StoreError};
use crate::suite::RepoClass;

/// Events read from the store per page.
const PAGE: NonZeroUsize = match NonZeroUsize::new(512) {
    Some(page) => page,
    None => NonZeroUsize::MIN,
};

/// The largest journal file a checkpoint reads: 256 MiB.
pub const MAX_JOURNAL_BYTES: u64 = 256 * 1024 * 1024;

/// How often a locked journal is tried again.
const POLL: Duration = Duration::from_millis(20);

/// What a checkpoint did.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub enum Checkpoint {
    /// A `client` repository: nothing was written (JR-09, ST-13).
    Client,
    /// The journals were brought up to date, but for any listed as busy.
    Written(Exported),
}

/// The journals a checkpoint wrote to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Exported {
    /// Lines appended, over every journal.
    pub appended: u64,
    /// The journals that gained lines.
    pub journals: Vec<JournalName>,
    /// Journals another writer held for the whole wait; the next checkpoint catches up.
    pub busy: Vec<JournalName>,
    /// Events that are not envelopes, so have no line.
    pub unexportable: u64,
}

/// Why a checkpoint failed. Lines appended before the failure stay, and the next checkpoint
/// appends only what is still missing.
#[derive(Debug, thiserror::Error)]
pub enum JournalError {
    /// The store could not be read.
    #[error("reading the store")]
    Store(#[source] StoreError),
    /// A journal file or its directory could not be created, read, locked or written.
    #[error("writing the journal {}", path.display())]
    Io {
        /// The journal file.
        path: PathBuf,
        /// What went wrong.
        #[source]
        source: io::Error,
    },
    /// A journal file is larger than [`MAX_JOURNAL_BYTES`].
    #[error("the journal {} is larger than {MAX_JOURNAL_BYTES} bytes", path.display())]
    TooLarge {
        /// The journal file.
        path: PathBuf,
    },
}

/// Appends every event of `store` that its journal in `dir` lacks (JR-08); in a `client`
/// repository, writes nothing (JR-09). Each journal is locked for at most `wait`.
///
/// # Errors
/// [`JournalError`] when the store cannot be read, or a journal file cannot be read or written.
pub fn checkpoint(
    store: &Store,
    class: RepoClass,
    dir: &Path,
    wait: Duration,
) -> Result<Checkpoint, JournalError> {
    if class == RepoClass::Client {
        return Ok(Checkpoint::Client);
    }
    let mut exported = Exported::default();
    let mut known: HashMap<JournalName, Known> = HashMap::new();
    let mut after = 0;
    loop {
        let page = store
            .read(&Filter::all(), after, PAGE)
            .map_err(JournalError::Store)?;
        let Some(last) = page.last().map(|stored| stored.position) else {
            break;
        };
        let mut by_journal: BTreeMap<JournalName, Vec<(EventId, Vec<u8>)>> = BTreeMap::new();
        for stored in &page {
            match encode(stored) {
                Ok((session, line)) => by_journal
                    .entry(JournalName::for_event(session.as_ref(), stored.id.store()))
                    .or_default()
                    .push((stored.id, line)),
                Err(_) => exported.unexportable = exported.unexportable.saturating_add(1),
            }
        }
        for (name, lines) in by_journal {
            if exported.busy.contains(&name) {
                continue;
            }
            let path = dir.join(name.file_name());
            // The map keeps its own copy of the name, which the report below also takes.
            match append(&path, &lines, known.entry(name.clone()).or_default(), wait)? {
                Appended::Busy => exported.busy.push(name),
                Appended::Lines(0) => {}
                Appended::Lines(n) => {
                    exported.appended = exported.appended.saturating_add(n);
                    if !exported.journals.contains(&name) {
                        exported.journals.push(name);
                    }
                }
            }
        }
        if page.len() < PAGE.get() {
            break;
        }
        after = last;
    }
    Ok(Checkpoint::Written(exported))
}

/// What this checkpoint knows of a journal file: the origins it holds and its length when they
/// were read, so a file no other writer touched since is not read again.
#[derive(Debug, Default)]
struct Known {
    origins: HashSet<EventId>,
    len: Option<u64>,
}

enum Appended {
    Lines(u64),
    Busy,
}

/// Appends to the journal at `path` the `lines` whose origin it lacks, creating it and its
/// directory only when there is something to append.
fn append(
    path: &Path,
    lines: &[(EventId, Vec<u8>)],
    known: &mut Known,
    wait: Duration,
) -> Result<Appended, JournalError> {
    let io = |source| JournalError::Io {
        path: path.to_path_buf(),
        source,
    };
    if known.len.is_some() && lines.iter().all(|(id, _)| known.origins.contains(id)) {
        return Ok(Appended::Lines(0));
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(io)?;
    }
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(io)?;
    if !lock(&file, wait).map_err(io)? {
        return Ok(Appended::Busy);
    }
    let len = file.metadata().map_err(io)?.len();
    let mut end = len;
    if known.len != Some(len) {
        let tail = reread(&mut file, path, known)?;
        end = tail.keep;
        if tail.keep < len {
            // A torn last line: dropped, its event appended again below (JR-03).
            file.set_len(tail.keep).map_err(io)?;
        }
        if tail.unended {
            file.seek(SeekFrom::Start(end)).map_err(io)?;
            file.write_all(b"\n").map_err(io)?;
            end = end.saturating_add(1);
        }
    }
    let mut out = Vec::new();
    let mut added = 0_u64;
    for (id, line) in lines {
        if known.origins.insert(*id) {
            out.extend_from_slice(line);
            added = added.saturating_add(1);
        }
    }
    if !out.is_empty() {
        file.seek(SeekFrom::Start(end)).map_err(io)?;
        file.write_all(&out).map_err(io)?;
        file.sync_data().map_err(io)?;
    }
    let size = u64::try_from(out.len()).unwrap_or(u64::MAX);
    known.len = Some(end.saturating_add(size));
    Ok(Appended::Lines(added))
}

/// Where a journal's complete content ends, and whether its last complete line lacks a `\n`.
struct Tail {
    keep: u64,
    unended: bool,
}

/// Reads the journal at `file` again: every origin its lines hold into `known`.
fn reread(file: &mut File, path: &Path, known: &mut Known) -> Result<Tail, JournalError> {
    let io = |source| JournalError::Io {
        path: path.to_path_buf(),
        source,
    };
    file.seek(SeekFrom::Start(0)).map_err(io)?;
    let mut bytes = Vec::new();
    file.take(MAX_JOURNAL_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(io)?;
    let total = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if total > MAX_JOURNAL_BYTES {
        return Err(JournalError::TooLarge {
            path: path.to_path_buf(),
        });
    }
    known.origins.clear();
    let mut tail = Tail {
        keep: total,
        unended: false,
    };
    for line in lines(&bytes) {
        let found = held(line.text);
        if let Held::Origin(id) = found {
            known.origins.insert(id);
        }
        if !line.complete {
            // Only the last line can lack its `\n`. One that decodes is a complete line, ended
            // before anything is appended; any other (blank included) is dropped.
            match found {
                Held::Origin(_) | Held::NoOrigin => tail.unended = true,
                Held::Torn => tail.keep = u64::try_from(line.start).unwrap_or(total),
            }
        }
    }
    Ok(tail)
}

/// Takes `file`'s lock, waiting at most `wait`; `false` when another writer held it throughout.
/// A file system without whole-file locks writes unlocked.
fn lock(file: &File, wait: Duration) -> io::Result<bool> {
    let deadline = Instant::now().checked_add(wait);
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(true),
            Err(TryLockError::WouldBlock) if deadline.is_some_and(|d| Instant::now() < d) => {}
            Err(TryLockError::WouldBlock) => return Ok(false),
            Err(TryLockError::Error(e)) if e.kind() == io::ErrorKind::Unsupported => {
                return Ok(true);
            }
            Err(TryLockError::Error(e)) => return Err(e),
        }
        std::thread::sleep(POLL);
    }
}
