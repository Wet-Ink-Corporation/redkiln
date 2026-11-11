//! The ledger store as the rest of Bench names it: Bench's events in the suite's store.
//!
//! It is opened, read and written only through `redkiln-core` (Move 2 step 3). Every command,
//! hook and test in this crate reaches the store through this module and nothing else.
//!
//! Every event is an EV-01 envelope of one of the 17 `bench.*` types declared in
//! [`bench_contract::event`] (`wire`). Gates and commands above this module see Bench's
//! in-memory [`EventEnvelope`] (the 0.x flat form); `bench ledger` reads Core's envelope into it
//! ([`EventEnvelope::from_ev01`]), and it and every command that records an event print the
//! envelopes as stored ([`StoredEvent`], since step 4).
//!
//! - Every filter and every append condition is restricted to Bench's 17 types (a filter that
//!   names none means all 17), so another plugin's event is never read as Bench's and never
//!   conflicts a Bench write, whatever it is tagged (EV-08).
//! - A stored Bench event this build cannot use is skipped and counted, never a failure of the
//!   read (EV-10). A tail reads the last `n` matching events and then decodes, so an unusable
//!   newest event leaves a gate seeing nothing (fail closed), never an older one in its place.
//! - The store's journal mode, schema stamp and refusals are Core's (ST-04 to ST-07): an anneal
//!   ledger is never opened as a store, only imported (ST-15), and is left as it was.
//!
//! The types: [`Store`], with [`Filter`] for reads, [`Condition`] for conditional appends, and
//! [`Appended`], [`Recorded`], [`StoredEvent`] and [`Read`] for what a write and a read
//! return; [`EventEnvelope`], [`CoreEnvelope`], [`Timestamp`], [`LedgerTag`] and [`TagKey`];
//! [`StoreError`] with [`ReadBack`], [`SchemaError`], [`TagError`] and [`TimestampError`]. The
//! `bench.*` payload types are not re-exported: callers name them through
//! [`bench_contract::event`].

mod envelope;
mod stored;
mod tag;
mod wire;

use std::collections::HashSet;
use std::io;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::time::Instant;

use redkiln_core::approval::ApprovalRecorded;
use redkiln_core::envelope::{
    EnvelopeError, Field, FieldError, Payload as _, Skip, Writer, decode_all,
};
use redkiln_core::ids::IdError;
use redkiln_core::journal::{
    ImportCompleted, ImportError, ImportSource, LineError, Mapped, MappedImport, Recording,
    encode_line, import_mapped,
};
pub use redkiln_core::store::Wait;
use redkiln_core::store::{
    Appended as CoreAppended, Condition as CoreCondition, Filter as CoreFilter, OpenError,
    StoreError as CoreStoreError, Stored, TagError as CoreTagError,
};

pub use bench_contract::event::{BenchEvent as LedgerEvent, EventKind, TYPES, UnknownEventKind};
pub use envelope::{EventEnvelope, SchemaError, Timestamp, TimestampError, WRITTEN_VERSION};
pub use stored::{CoreEnvelope, StoredEvent};
pub use tag::{LedgerTag, TagError, TagKey};
pub(crate) use wire::Context as EventContext;

/// How many events one read of the store returns at most.
const PAGE: NonZeroUsize = match NonZeroUsize::new(512) {
    Some(page) => page,
    None => NonZeroUsize::MIN,
};

/// Why an event just appended did not read back ([`StoreError::NotReadBack`]).
#[derive(Debug, thiserror::Error)]
pub enum ReadBack {
    /// The read found no Bench event at the position Core reported.
    #[error("no event at that position")]
    Missing,
    /// The read found it and skipped it, as every read skips an event Bench cannot use.
    #[error("the read skipped it")]
    Skipped(#[source] Skip),
}

/// Why a store operation failed.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// Core refused to open the file as a store: an anneal ledger, an old plugin ledger or
    /// another program's database (ST-07). The file was left as it was.
    #[error(
        "{} is an anneal ledger or another program's database; bench imports an anneal ledger rather than opening it, and left it unchanged",
        path.display()
    )]
    Refused {
        /// The file.
        path: PathBuf,
        /// Core's refusal (boxed: Core's error is large, and every command's error holds this).
        #[source]
        source: Box<CoreStoreError>,
    },
    /// Core could not open, read or write the store (boxed, as above).
    #[error(transparent)]
    Core(Box<CoreStoreError>),
    /// An envelope field Core refuses (the actor). Nothing was written.
    #[error(transparent)]
    Field(FieldError),
    /// A session or worktree that is not an id (ID-05, ID-06). Nothing was written.
    #[error("the event's {field} {value:?} is refused")]
    Id {
        /// The envelope field.
        field: Field,
        /// The value given.
        value: String,
        /// Why it is not an id.
        #[source]
        source: IdError,
    },
    /// A tag value Core refuses, never shortened (EV-11). Nothing was written.
    #[error("tagging the event")]
    Tag(#[source] CoreTagError),
    /// Core would not build the envelope (EV-05): a branch or head it refuses, a payload too
    /// large. Nothing was written.
    #[error("building the {event_type} envelope")]
    Envelope {
        /// The stored type.
        event_type: &'static str,
        /// Core's reason (boxed: it is large and rare).
        #[source]
        source: Box<EnvelopeError>,
    },
    /// Core reported a conflict for a write that had no condition, which it never does.
    #[error("the store reported a conflict for an unconditioned write")]
    UnconditionedConflict,
    /// The event just appended at `position` did not read back as a usable Bench event, which
    /// a store that took it never does. The event was written: appending it again records it
    /// twice.
    #[error(
        "the event was written at position {position} but did not read back; do not append it \
         again"
    )]
    NotReadBack {
        /// Where Core said it wrote the event.
        position: u64,
        /// Why it did not read back.
        #[source]
        cause: ReadBack,
    },
    /// Core's import of an old ledger stopped (ST-15): what it stored stays, and nothing was
    /// recorded, so the next run imports the rest.
    #[error("importing into the store")]
    Import(#[source] Box<ImportError>),
}

impl StoreError {
    /// Whether the event was refused before anything was written: a field, tag or envelope
    /// Core refuses (EV-05), which `bench ledger append` reports as a schema error.
    #[must_use]
    pub const fn is_schema(&self) -> bool {
        matches!(
            self,
            Self::Field(_) | Self::Id { .. } | Self::Tag(_) | Self::Envelope { .. }
        )
    }
}

/// Which events to read: any of `types` (all 17 Bench types when empty) carrying every tag in
/// `tags`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    /// Event types to match; empty matches every Bench type.
    pub types: Vec<EventKind>,
    /// Tags an event must all carry; empty matches every event.
    pub tags: Vec<LedgerTag>,
}

impl Filter {
    /// Core's filter: Bench's types (those named, or all 17), then each tag.
    fn to_core(&self) -> Result<CoreFilter, StoreError> {
        let stored: Vec<&str> = if self.types.is_empty() {
            TYPES.to_vec()
        } else {
            self.types.iter().map(|kind| kind.stored()).collect()
        };
        let typed = stored
            .into_iter()
            .try_fold(CoreFilter::all(), CoreFilter::of_type)?;
        // The filter owns its tags; `self` stays the caller's.
        Ok(self
            .tags
            .iter()
            .fold(typed, |filter, tag| filter.tagged(tag.as_tag().clone())))
    }
}

/// The condition of a first-writer-wins append: no Bench event matching `filter` may exist
/// after `after` (anywhere in the store by default).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Condition {
    filter: Filter,
    after: Option<u64>,
}

impl Condition {
    /// Fails the append when any event matches `filter`.
    #[must_use]
    pub const fn none_matching(filter: Filter) -> Self {
        Self {
            filter,
            after: None,
        }
    }

    /// Only events after `position` (exclusive) count: the caller read the store up to
    /// `position` and decided from that. Position 0 means the whole store.
    #[must_use]
    pub const fn after(mut self, position: u64) -> Self {
        self.after = Some(position);
        self
    }

    /// Core's condition over the same Bench-only filter; never Core's type-less `not_yet` or
    /// `unchanged_since` (EV-08).
    fn to_core(&self) -> Result<CoreCondition, StoreError> {
        let condition = CoreCondition::none_matching(self.filter.to_core()?);
        Ok(match self.after {
            Some(position) => condition.after(position),
            None => condition,
        })
    }
}

/// The outcome of a conditioned append.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Appended {
    /// Written at this position (the last event's, for a write of several).
    At(u64),
    /// Not written: an event matching the condition exists. Someone else wrote first.
    Conflict,
}

/// An event as stored: its position in the suite's store and its envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recorded {
    /// Where the event sits in the store, from 1, among every plugin's events.
    pub position: u64,
    /// The event.
    pub event: EventEnvelope,
}

impl Recorded {
    fn from_stored(stored: &Stored) -> Result<Self, Skip> {
        Ok(Self {
            position: stored.position,
            event: wire::from_stored(stored)?,
        })
    }
}

/// A read: the Bench events it found, each a `T` ([`Recorded`] for the gates, [`StoredEvent`]
/// for `bench ledger`), and how many it skipped as unusable (EV-10).
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct Read<T = Recorded> {
    /// The events, in position order.
    pub events: Vec<T>,
    /// Bench events this build could not use: a newer version, or data that does not decode.
    pub unusable: u64,
}

impl<T> Default for Read<T> {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            unusable: 0,
        }
    }
}

impl<T> Read<T> {
    fn take(&mut self, stored: &Stored, read: fn(&Stored) -> Result<T, Skip>) {
        match read(stored) {
            Ok(event) => self.events.push(event),
            Err(Skip::Unusable(_)) => self.unusable = self.unusable.saturating_add(1),
            // The filter reads Bench's types only.
            Err(Skip::Foreign) => {}
        }
    }
}

/// Why `bench ledger journal` could not write the journal.
#[derive(Debug, thiserror::Error)]
pub enum JournalError {
    /// The store could not be read.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// An event could not be written as a journal line (JR-02).
    #[error("writing the event at position {position} as a journal line")]
    Line {
        /// Where it is stored.
        position: u64,
        /// Core's reason (boxed: it is large and rare).
        #[source]
        source: Box<LineError>,
    },
    /// The output could not be written.
    #[error("writing the journal")]
    Write(#[source] io::Error),
}

/// An open store, with the writer every envelope it appends names.
#[derive(Debug)]
pub struct Store {
    /// Boxed: Core's store is a few hundred bytes, and the open store travels in the gates'
    /// enums and results.
    inner: Box<redkiln_core::store::Store>,
    writer: Writer,
}

impl Store {
    /// Opens the store at `path` for a command, creating it and its directory when absent;
    /// a write waits at most 10 s for another's lock (`Wait::Command`, ST-09).
    ///
    /// # Errors
    /// [`StoreError::Refused`] for an anneal ledger or another program's database, left as it
    /// was; [`StoreError::Core`] when Core cannot open it otherwise.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        Self::open_with(path, Wait::Command)
    }

    /// [`Store::open`], waiting `wait` for another's lock: `Wait::Hook` (2 s) for a hook.
    ///
    /// # Errors
    /// As [`Store::open`].
    pub fn open_with(path: &Path, wait: Wait) -> Result<Self, StoreError> {
        let inner = redkiln_core::store::Store::open(path, wait).map_err(refused)?;
        Self::over(inner)
    }

    /// Opens the store at `path` only when there is one: `None`, with nothing created (no
    /// file, directory or lock), for a missing, empty or never-stamped file. For hooks and
    /// reads, which never create a store.
    ///
    /// # Errors
    /// As [`Store::open`].
    pub fn open_existing(path: &Path, wait: Wait) -> Result<Option<Self>, StoreError> {
        redkiln_core::store::Store::open_existing(path, wait)
            .map_err(refused)?
            .map(Self::over)
            .transpose()
    }

    /// The writer every envelope this store appends names (`bench <version>`).
    pub(crate) const fn writer(&self) -> &Writer {
        &self.writer
    }

    /// Imports `records`, mapped from an old ledger, through Core's import (ST-15), recorded
    /// as `recording` says unless `deadline` passes first ([`MappedImport::Stopped`]).
    ///
    /// # Errors
    /// [`StoreError::Import`] when Core's import stops on a store error.
    pub(crate) fn import(
        &self,
        records: impl IntoIterator<Item = (u64, Mapped)>,
        recording: Recording,
        deadline: Option<Instant>,
    ) -> Result<MappedImport, StoreError> {
        import_mapped(&self.inner, records, recording, deadline)
            .map_err(|error| StoreError::Import(Box::new(error)))
    }

    /// The paths of the old ledgers this store recorded importing itself (JR-13): each
    /// `core.import_completed` it appended whose source is `{"legacy": {path, ..}}`. A record
    /// that came in from another store's journal names what that store read, so it does not
    /// count; nor does one this build cannot decode.
    ///
    /// # Errors
    /// [`StoreError::Core`] when the store cannot be read.
    pub(crate) fn imported_sources(&self) -> Result<HashSet<String>, StoreError> {
        let filter = CoreFilter::all().of_type(ImportCompleted::TYPE)?;
        let mine = self.inner.store_id();
        let mut done = HashSet::new();
        let mut after = 0;
        loop {
            let page = self.inner.read(&filter, after, PAGE)?;
            let Some(end) = page.last().map(|stored| stored.position) else {
                return Ok(done);
            };
            let own = page.iter().filter(|stored| stored.id.store() == mine);
            for envelope in decode_all::<ImportCompleted>(own).events {
                if let ImportSource::Legacy { path, .. } = envelope.data.source {
                    done.insert(path);
                }
            }
            if page.len() < PAGE.get() {
                return Ok(done);
            }
            after = end;
        }
    }

    fn over(inner: redkiln_core::store::Store) -> Result<Self, StoreError> {
        Ok(Self {
            inner: Box::new(inner),
            writer: wire::writer()?,
        })
    }

    /// Appends `event` and returns its position.
    ///
    /// # Errors
    /// [`StoreError::Field`], [`StoreError::Id`], [`StoreError::Tag`] or
    /// [`StoreError::Envelope`] for a field Core refuses (nothing is written);
    /// [`StoreError::Core`] when the store refuses the write (a lock held past the wait).
    pub fn append(&self, event: &EventEnvelope) -> Result<u64, StoreError> {
        let stored = wire::to_new_event(event, &self.writer)?;
        match self.inner.append(vec![stored], None)? {
            CoreAppended::At(position) => Ok(position),
            // No condition was given, so Core never reports one matched.
            CoreAppended::Conflict => Err(StoreError::UnconditionedConflict),
        }
    }

    /// [`Store::append`], returning the event as Core stored it, read back from its position:
    /// what `bench ledger append` and the commands that record an event print.
    ///
    /// # Errors
    /// As [`Store::append`]; [`StoreError::NotReadBack`] when the event, written, is not found
    /// at the position Core reported or does not read as a usable Bench event.
    pub fn append_stored(&self, event: &EventEnvelope) -> Result<StoredEvent, StoreError> {
        let position = self.append(event)?;
        let filter = Filter::default().to_core()?;
        let after = position.saturating_sub(1);
        let read = self.inner.read(&filter, after, NonZeroUsize::MIN)?;
        let not_read_back = |cause| StoreError::NotReadBack { position, cause };
        let stored = read
            .first()
            .filter(|stored| stored.position == position)
            .ok_or_else(|| not_read_back(ReadBack::Missing))?;
        StoredEvent::from_stored(stored).map_err(|skip| not_read_back(ReadBack::Skipped(skip)))
    }

    /// Appends `event` unless a Bench event matching `condition` exists: first writer wins.
    /// The condition is checked inside the append's transaction.
    ///
    /// # Errors
    /// As [`Store::append`]; a matching event is [`Appended::Conflict`], not an error.
    pub fn append_if(
        &self,
        event: &EventEnvelope,
        condition: &Condition,
    ) -> Result<Appended, StoreError> {
        self.append_with_approval_if(event, None, condition)
    }

    /// [`Store::append_if`], writing first, in the same transaction and with the same
    /// context, the Core record of a human's approval (AP-05, D3) when `approval` is given:
    /// both or neither are written.
    ///
    /// # Errors
    /// As [`Store::append_if`].
    pub fn append_with_approval_if(
        &self,
        event: &EventEnvelope,
        approval: Option<&ApprovalRecorded>,
        condition: &Condition,
    ) -> Result<Appended, StoreError> {
        let context = wire::Context::of(event, &self.writer)?;
        let mut events = Vec::with_capacity(2);
        if let Some(approval) = approval {
            // Both envelopes carry the one context; the Core envelope owns its copy of the
            // record, which a caller that lost a race appends again.
            events.push(wire::approval_event(context.clone(), approval.clone())?);
        }
        events.push(wire::to_new_event_in(context, &event.event)?);
        match self.inner.append(events, Some(&condition.to_core()?))? {
            CoreAppended::At(position) => Ok(Appended::At(position)),
            CoreAppended::Conflict => Ok(Appended::Conflict),
        }
    }

    /// Every usable event matching `filter`, in position order; unusable ones are skipped
    /// (EV-10). A caller that reports them reads [`Store::query_read`].
    ///
    /// # Errors
    /// [`StoreError::Core`] when the store cannot be read.
    pub fn query(&self, filter: &Filter) -> Result<Vec<Recorded>, StoreError> {
        Ok(self.query_read(filter)?.events)
    }

    /// Every event matching `filter`, read a page at a time: the usable ones in position
    /// order, and how many were unusable.
    ///
    /// # Errors
    /// As [`Store::query`].
    pub fn query_read(&self, filter: &Filter) -> Result<Read, StoreError> {
        self.read_all(filter, Recorded::from_stored)
    }

    /// [`Store::query_read`], each event as Core stores it: `bench ledger query`'s listing.
    ///
    /// # Errors
    /// As [`Store::query`].
    pub fn query_stored(&self, filter: &Filter) -> Result<Read<StoredEvent>, StoreError> {
        self.read_all(filter, StoredEvent::from_stored)
    }

    /// Every event matching `filter`, a page at a time, each read with `read`.
    fn read_all<T>(
        &self,
        filter: &Filter,
        read: fn(&Stored) -> Result<T, Skip>,
    ) -> Result<Read<T>, StoreError> {
        let filter = filter.to_core()?;
        let mut found = Read::default();
        let mut after = 0;
        loop {
            let page = self.inner.read(&filter, after, PAGE)?;
            for stored in &page {
                after = stored.position;
                found.take(stored, read);
            }
            if page.len() < PAGE.get() {
                return Ok(found);
            }
        }
    }

    /// The usable ones among the last `n` events matching `filter`, in position order: fewer
    /// than `n` when some are unusable (Invariant 4). A caller that reports them reads
    /// [`Store::tail_read`].
    ///
    /// # Errors
    /// As [`Store::query`].
    pub fn tail(&self, filter: &Filter, n: usize) -> Result<Vec<Recorded>, StoreError> {
        Ok(self.tail_read(filter, n)?.events)
    }

    /// The last `n` events matching `filter`, read and then decoded.
    ///
    /// # Errors
    /// As [`Store::query`].
    pub fn tail_read(&self, filter: &Filter, n: usize) -> Result<Read, StoreError> {
        self.read_last(filter, n, Recorded::from_stored)
    }

    /// [`Store::tail_read`], each event as Core stores it: `bench ledger tail`'s listing.
    ///
    /// # Errors
    /// As [`Store::query`].
    pub fn tail_stored(&self, filter: &Filter, n: usize) -> Result<Read<StoredEvent>, StoreError> {
        self.read_last(filter, n, StoredEvent::from_stored)
    }

    /// The last `n` events matching `filter`, read and then each read with `read`.
    fn read_last<T>(
        &self,
        filter: &Filter,
        n: usize,
        read: fn(&Stored) -> Result<T, Skip>,
    ) -> Result<Read<T>, StoreError> {
        let mut found = Read::default();
        let Some(n) = NonZeroUsize::new(n) else {
            return Ok(found);
        };
        for stored in &self.inner.read_last(&filter.to_core()?, n)? {
            found.take(stored, read);
        }
        Ok(found)
    }

    /// Writes every Bench event as its journal line (JR-04), in position order, through
    /// `write`: the importable form. Unusable events are written too; another plugin's never.
    ///
    /// # Errors
    /// [`JournalError`] when the store cannot be read, an event cannot be a line, or `write`
    /// fails.
    pub fn journal(
        &self,
        mut write: impl FnMut(&[u8]) -> io::Result<()>,
    ) -> Result<(), JournalError> {
        let filter = Filter::default().to_core()?;
        let mut after = 0;
        loop {
            let page = self
                .inner
                .read(&filter, after, PAGE)
                .map_err(StoreError::from)?;
            for stored in &page {
                after = stored.position;
                let line = encode_line(stored).map_err(|source| JournalError::Line {
                    position: stored.position,
                    source: Box::new(source),
                })?;
                write(&line).map_err(JournalError::Write)?;
            }
            if page.len() < PAGE.get() {
                return Ok(());
            }
        }
    }
}

/// Core's error, with the "imports rather than opens" hint for a file it refused as an old
/// ledger or another program's database.
fn refused(error: CoreStoreError) -> StoreError {
    match error {
        CoreStoreError::Open {
            path,
            source: source @ (OpenError::OldLedger { .. } | OpenError::NotAStore),
        } => StoreError::Refused {
            // Kept twice: in the hint, and in Core's own error beneath it.
            path: path.clone(),
            source: Box::new(CoreStoreError::Open { path, source }),
        },
        other => StoreError::from(other),
    }
}

impl From<CoreStoreError> for StoreError {
    fn from(error: CoreStoreError) -> Self {
        Self::Core(Box::new(error))
    }
}

#[cfg(test)]
mod tests;
