//! A committed 1.x journal file, checked and read under [`MAX_JOURNAL_FILE_BYTES`] (H-13),
//! opened read-only (ST-16, B-01).

use std::fs::File;
use std::io::Read as _;
use std::path::Path;

use anyhow::{Context as _, Result};

use super::{JOURNAL_STORE, MAX_JOURNAL_FILE_BYTES, SourceKind, SourceOutcome, SourceRun};

/// A due journal as checked before the store's events are read.
pub(super) enum Checked<'p> {
    /// It may import: read it against the held events.
    Due(&'p Path),
    /// Over the cap or not openable: its outcome needs no held event.
    Settled(SourceRun),
}

impl Checked<'_> {
    /// The settled source, if it is one.
    pub(super) fn settled(self) -> Option<SourceRun> {
        match self {
            Checked::Due(_) => None,
            Checked::Settled(source) => Some(source),
        }
    }
}

/// The journal at `path` settled by opening it and reading its length only: too large, or
/// failing; else due.
pub(super) fn check(path: &Path) -> Checked<'_> {
    let measured = File::open(path)
        .and_then(|file| file.metadata())
        .with_context(|| format!("opening {}", path.display()));
    let outcome = match measured {
        Ok(meta) if meta.len() > MAX_JOURNAL_FILE_BYTES => {
            SourceOutcome::TooLarge { bytes: meta.len() }
        }
        Ok(_) => return Checked::Due(path),
        Err(e) => SourceOutcome::Failed(e),
    };
    Checked::Settled(SourceRun {
        kind: SourceKind::Journal,
        path: path.to_path_buf(),
        store_id: Some(JOURNAL_STORE),
        outcome,
    })
}

/// A journal file as read.
pub(super) enum Read {
    Bytes(Vec<u8>),
    TooLarge(u64),
}

/// The bytes of the journal at `path`, unless it is over [`MAX_JOURNAL_FILE_BYTES`].
pub(super) fn read_journal(path: &Path) -> Result<Read> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let len = file
        .metadata()
        .with_context(|| format!("reading {}", path.display()))?
        .len();
    if len > MAX_JOURNAL_FILE_BYTES {
        return Ok(Read::TooLarge(len));
    }
    let mut bytes = Vec::new();
    file.take(MAX_JOURNAL_FILE_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .with_context(|| format!("reading {}", path.display()))?;
    let read = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if read > MAX_JOURNAL_FILE_BYTES {
        // It grew past the cap while it was read.
        return Ok(Read::TooLarge(read));
    }
    Ok(Read::Bytes(bytes))
}
