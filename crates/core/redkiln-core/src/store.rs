//! The store: one happenstance SQLite event store per repository (SPEC ST-04 to ST-11).
//!
//! # Opening
//!
//! [`Store::open`] never goes through `SqliteEventStore::open`, which switches a file to WAL.
//! It reads the file's header first and refuses, untouched, a store of a newer major, an old
//! plugin ledger (stamp below 1000, imported instead: ST-15, ST-16) and a database that is not
//! a store (ST-07). A new file gets happenstance's schema, Core's `core_meta` table and the
//! stamp [`STORE_SCHEMA`] in one `BEGIN IMMEDIATE`, copied from a template built on the local
//! temporary disk, so two processes creating the store cannot both write a schema (ST-05,
//! ST-06). A store found in WAL is converted to TRUNCATE, and a stale `-wal` beside it is
//! renamed aside, never deleted (ST-05). A store opened at another path than the one it
//! recorded gets a new identity (ID-01).
//!
//! # Writing
//!
//! Every append is happenstance's own `BEGIN IMMEDIATE`, waiting at most [`Wait`]'s timeout
//! for another writer's lock (ST-09). A [`Condition`] makes it first-writer-wins: of two writers
//! racing on one condition exactly one appends, and the other gets [`Appended::Conflict`]
//! (ST-10). A telemetry event ([`Store::record_telemetry`]) never waits: on a busy lock it is
//! dropped and counted ([`Recorded::Dropped`], [`Store::telemetry_dropped`]), never retried
//! (EV-14, ST-09). Telemetry is kept beside the log, in a side store of its own
//! (`<store>.telemetry`), for [`TELEMETRY_WINDOW`], and read through [`Store::read_telemetry`].
//!
//! # Synchronous only
//!
//! A `Store` drives happenstance's async API on its own current-thread tokio runtime. Inside a
//! tokio runtime context, [`Store::open`] and every method return [`StoreError::InsideRuntime`]
//! rather than let tokio panic; async code runs the store on a plain thread.

mod header;
mod open;
mod retention;
mod tag;
mod telemetry;

use std::fmt;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::time::Duration;

use happenstance_core::{
    AppendCondition, AppendError, Event, EventId, EventType, InvalidEventType, InvalidQuery, Query,
    QueryItem, ReadOptions, SendEventStore, SequencePosition, StoreId, Tags, collect,
};
use happenstance_sqlite::event_store::{SqliteEventStore, SqliteEventStoreError};
use happenstance_sqlite::rusqlite;

pub use open::STORE_SCHEMA;
pub use tag::{CoreKey, Tag, TagError};
pub use telemetry::{Recorded, TELEMETRY_WINDOW, TelemetryEvent};

pub(crate) use header::{Found, read as read_header};
pub(crate) use open::sibling;
pub(crate) use tag::check_caller_tag;

/// How long a writer waits for another's lock (ST-09).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wait {
    /// A command: at most 10 seconds.
    Command,
    /// A hook's durable write: at most 2 seconds.
    Hook,
}

impl Wait {
    /// The busy timeout this wait allows.
    #[must_use]
    pub const fn timeout(self) -> Duration {
        match self {
            Self::Command => Duration::from_secs(10),
            Self::Hook => Duration::from_secs(2),
        }
    }
}

/// Why a store file could not be opened. A refused file is left as it was.
#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    /// The store's directory could not be created.
    #[error("creating the store's directory")]
    CreateDir(#[source] std::io::Error),
    /// The file's header could not be read.
    #[error("reading the store file's header")]
    Header(#[source] std::io::Error),
    /// The store's canonical path could not be found.
    #[error("finding the store's canonical path")]
    Canonicalize(#[source] std::io::Error),
    /// The store's path has no absolute form (it is empty, or the working directory is gone),
    /// which the telemetry connection opens at.
    #[error("finding the store's absolute path")]
    AbsolutePath(#[source] std::io::Error),
    /// SQLite failed at `step`.
    #[error("SQLite failed while {step}")]
    Sqlite {
        /// What the open was doing.
        step: Step,
        /// SQLite's error.
        #[source]
        source: rusqlite::Error,
    },
    /// The store has happenstance's schema but no identity row to re-mint.
    #[error("the store has no identity row to re-mint")]
    MissingIdentity,
    /// SQLite would not run the store under a TRUNCATE journal.
    #[error("SQLite kept journal mode {found}, not truncate; another process may hold it in WAL")]
    JournalMode {
        /// The mode SQLite reported.
        found: String,
    },
    /// The file is a database, but not a Redkiln store: unstamped with a schema or in WAL, such
    /// as a weighin 0.x ledger or another program's database.
    #[error("the database is not a Redkiln store; it was left unchanged")]
    NotAStore,
    /// The file is stamped below 1000: an old plugin ledger, which is imported into the store and
    /// never opened as one (ST-15, ST-16), or another program's database.
    #[error(
        "the file has schema {found}, below a Redkiln store's (an old plugin ledger, imported rather than opened, or another program's database); it was left unchanged"
    )]
    OldLedger {
        /// The stamp in the file.
        found: i64,
    },
    /// The store was stamped by a build of a newer SPEC major.
    #[error(
        "the store has schema {found}, a newer major than this build reads ({supported}); upgrade"
    )]
    NewerMajor {
        /// The stamp in the file.
        found: i64,
        /// The schema this build writes.
        supported: i64,
    },
    /// A file beside the store could not be renamed aside.
    #[error("setting aside {}", path.display())]
    SetAside {
        /// The file that stayed.
        path: PathBuf,
        /// Why the rename failed.
        #[source]
        source: std::io::Error,
    },
    /// No store at the path (the file is missing, empty or a database never stamped), where
    /// the open needed one: the telemetry connection never creates the store it writes to.
    #[error("there is no store at the path; it was not created")]
    NoStore,
    /// The open lock beside the store (`<store>.lock`) could not be created or taken.
    #[error("taking the store's open lock")]
    Lock(#[source] std::io::Error),
    /// Another opener held the open lock for longer than the wait: it is creating or converting
    /// the store.
    #[error("another process is opening the store; timed out waiting for it")]
    LockBusy,
    /// The temporary directory for the schema template could not be created or removed.
    #[error("creating or removing the schema template's temporary directory")]
    TempDir(#[source] std::io::Error),
    /// happenstance could not build the schema template.
    #[error("building happenstance's schema template")]
    Template(#[source] SqliteEventStoreError),
    /// The schema template's path is not UTF-8, which SQLite's `ATTACH` needs.
    #[error("the schema template's path {} is not UTF-8", path.display())]
    TemplatePath {
        /// The template file.
        path: PathBuf,
    },
    /// happenstance built a schema template with nothing in it.
    #[error("happenstance's schema template is empty")]
    EmptyTemplate,
    /// happenstance refused the store's connection, or its identity could not be re-minted.
    #[error("happenstance refused the store")]
    Store(#[source] SqliteEventStoreError),
    /// The async runtime that drives happenstance could not start.
    #[error("starting the async runtime")]
    Runtime(#[source] std::io::Error),
    /// A step failed, and undoing it (rollback, detach) failed too.
    #[error("cleaning up after a failure also failed ({cleanup})")]
    Cleanup {
        /// The step that failed first.
        #[source]
        failure: Box<OpenError>,
        /// The cleanup's own failure.
        cleanup: rusqlite::Error,
    },
}

/// What an open was doing when SQLite failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Opening the connection.
    Connect,
    /// Setting the busy timeout, durability and foreign keys.
    Configure,
    /// Switching to the TRUNCATE journal.
    JournalMode,
    /// Looking at the file's schema.
    Inspect,
    /// Reading `user_version`.
    ReadStamp,
    /// Attaching the schema template.
    AttachTemplate,
    /// Copying the template's schema and writing Core's table and stamp.
    CopySchema,
    /// Detaching the schema template.
    DetachTemplate,
    /// Reading the recorded path, or re-minting the identity and recording the path (ID-01).
    Identity,
    /// Beginning a write transaction.
    Begin,
    /// Committing it.
    Commit,
    /// Deleting telemetry past its window from the side store (EV-14).
    Prune,
}

impl fmt::Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Connect => "connecting",
            Self::Configure => "configuring the connection",
            Self::JournalMode => "switching to the truncate journal",
            Self::Inspect => "inspecting the schema",
            Self::ReadStamp => "reading the schema version",
            Self::AttachTemplate => "attaching the schema template",
            Self::CopySchema => "copying the schema",
            Self::DetachTemplate => "detaching the schema template",
            Self::Identity => "keeping the store's identity",
            Self::Begin => "beginning a write transaction",
            Self::Commit => "committing",
            Self::Prune => "deleting telemetry past its window",
        })
    }
}

/// Why a store operation failed.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// Called inside a tokio runtime context, where driving the store's own runtime would panic.
    #[error("the store is synchronous and was called inside an async runtime")]
    InsideRuntime,
    /// The store could not be opened.
    #[error("opening the store {}", path.display())]
    Open {
        /// The store file.
        path: PathBuf,
        /// What went wrong.
        #[source]
        source: OpenError,
    },
    /// An event type happenstance does not accept.
    #[error("invalid event type")]
    EventType(#[source] InvalidEventType),
    /// The filter could not be turned into a store query.
    #[error("building the store query")]
    Query(#[source] InvalidQuery),
    /// happenstance refused the append.
    #[error("appending to the store")]
    Append(#[source] AppendError<SqliteEventStoreError>),
    /// happenstance could not read the store.
    #[error("reading the store")]
    Read(#[source] SqliteEventStoreError),
    /// Telemetry past its window could not be deleted from the side store for a reason other
    /// than a busy lock (EV-14). The event was not written.
    #[error("pruning the telemetry store {}", path.display())]
    Prune {
        /// The side store.
        path: PathBuf,
        /// What went wrong.
        #[source]
        source: OpenError,
    },
    /// The file at the store's path now has another identity than the one this store opened:
    /// it was replaced or restored since. Telemetry is not written beside it.
    #[error("the store file now has identity {found}, not the {opened} this store opened")]
    IdentityChanged {
        /// The identity this store opened.
        opened: StoreId,
        /// The identity the file has now.
        found: StoreId,
    },
}

/// An event to append: its type, its payload bytes and its tags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewEvent {
    event: Event,
    tags: Vec<Tag>,
}

impl NewEvent {
    /// An event of `event_type` carrying `data`, with no tags yet.
    ///
    /// # Errors
    /// [`StoreError::EventType`] when happenstance does not accept the type.
    pub fn new(event_type: &str, data: Vec<u8>) -> Result<Self, StoreError> {
        let event = Event::new(event_type, data).map_err(StoreError::EventType)?;
        Ok(Self {
            event,
            tags: Vec::new(),
        })
    }

    /// The same event, also carrying `tag`.
    #[must_use]
    pub fn with_tag(mut self, tag: Tag) -> Self {
        self.tags.push(tag);
        self
    }

    fn into_event(self) -> Event {
        let tags: Tags = self.tags.into_iter().map(Tag::into_happenstance).collect();
        self.event.with_tags(tags)
    }
}

/// Which events to read, or what a [`Condition`] must not find: events of any of its types
/// (every type when none is named) carrying every one of its tags.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    types: Vec<EventType>,
    tags: Vec<Tag>,
}

impl Filter {
    /// Every event.
    #[must_use]
    pub fn all() -> Self {
        Self::default()
    }

    /// The same filter, also matching events of `event_type`.
    ///
    /// # Errors
    /// [`StoreError::EventType`] when happenstance does not accept the type.
    pub fn of_type(mut self, event_type: &str) -> Result<Self, StoreError> {
        self.types
            .push(EventType::new(event_type).map_err(StoreError::EventType)?);
        Ok(self)
    }

    /// The same filter, matching only events that also carry `tag`.
    #[must_use]
    pub fn tagged(mut self, tag: Tag) -> Self {
        self.tags.push(tag);
        self
    }

    fn to_query(&self) -> Result<Query, StoreError> {
        if self.types.is_empty() && self.tags.is_empty() {
            return Ok(Query::all());
        }
        // The query owns its tags; the filter stays usable for a re-read after a conflict.
        let tags: Tags = self
            .tags
            .iter()
            .map(|t| t.as_happenstance().clone())
            .collect();
        QueryItem::new(self.types.iter().cloned(), tags)
            .map(Query::from_item)
            .map_err(StoreError::Query)
    }
}

/// The condition of a first-writer-wins append (ST-10): no event matching its filter may exist,
/// anywhere or after the position the writer read.
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

    /// *Not yet raised*: fails when an event of `event_type` carrying `subject` exists.
    ///
    /// # Errors
    /// [`StoreError::EventType`] when happenstance does not accept the type.
    pub fn not_yet(event_type: &str, subject: Tag) -> Result<Self, StoreError> {
        Ok(Self::none_matching(
            Filter::all().of_type(event_type)?.tagged(subject),
        ))
    }

    /// *Unchanged since*: fails when any event carrying `subject` sits after `position`.
    #[must_use]
    pub fn unchanged_since(subject: Tag, position: u64) -> Self {
        Self::none_matching(Filter::all().tagged(subject)).after(position)
    }

    /// Only events after `position` (exclusive) count. Position 0 means the whole store.
    #[must_use]
    pub const fn after(mut self, position: u64) -> Self {
        self.after = Some(position);
        self
    }

    fn to_append_condition(&self) -> Result<AppendCondition, StoreError> {
        let after = self.after.and_then(SequencePosition::new);
        Ok(AppendCondition::new(self.filter.to_query()?).after_every_guard_opt(after))
    }
}

/// The outcome of an append.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Appended {
    /// Written; the last event sits at this position.
    At(u64),
    /// Not written: an event matching the condition exists. Someone else wrote first.
    Conflict,
}

/// An event as stored: where it sits, its identity, when it was recorded, and the event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stored {
    /// Its position in this store, from 1.
    pub position: u64,
    /// Its identity: the store that first accepted it and its position there (ID-02).
    pub id: EventId,
    /// When this store recorded it, in milliseconds since the Unix epoch. Advisory (EV-15).
    pub recorded_at_millis: i64,
    /// Its type.
    pub event_type: String,
    /// Its payload bytes.
    pub data: Vec<u8>,
    /// Its tags.
    pub tags: Vec<Tag>,
}

impl Stored {
    pub(crate) fn from_sequenced(se: happenstance_core::SequencedEvent) -> Self {
        let parts = se.event.into_parts();
        Self {
            position: u64::from(se.position),
            id: se.id,
            recorded_at_millis: se.recorded_at.as_millis(),
            event_type: parts.event_type.as_str().to_owned(),
            data: parts.data.to_vec(),
            tags: parts.tags.into_iter().map(Tag::stored).collect(),
        }
    }
}

/// An open store.
pub struct Store {
    inner: SqliteEventStore,
    runtime: Driver,
    /// The store's absolute path; the telemetry side store sits beside it.
    path: PathBuf,
    /// The open's busy timeout, which a telemetry read waits too.
    wait: Duration,
    /// The side store's telemetry connection, with no busy wait; opened on the first telemetry
    /// write.
    telemetry: Option<SqliteEventStore>,
    /// Telemetry events dropped on a busy lock since the store was opened (ST-09).
    dropped: u64,
}

impl fmt::Debug for Store {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Store")
            .field("store_id", &self.inner.store_id())
            .finish_non_exhaustive()
    }
}

impl Store {
    /// Opens the store at `path`, creating it and its directory when absent, with every
    /// statement waiting at most `wait`'s timeout for another process's lock.
    ///
    /// # Errors
    /// [`StoreError::InsideRuntime`] inside a tokio runtime context. [`StoreError::Open`] when
    /// the file cannot be created or opened under TRUNCATE; among others when it is a newer
    /// major ([`OpenError::NewerMajor`]), an old plugin ledger ([`OpenError::OldLedger`]) or not
    /// a store ([`OpenError::NotAStore`]), each left as it was.
    pub fn open(path: &Path, wait: Wait) -> Result<Self, StoreError> {
        refuse_inside_runtime()?;
        // Absolute, so the telemetry connection opens the same file whatever the working
        // directory becomes; not canonicalised, so Windows keeps the form SQLite was given.
        // Found first, so a path with no absolute form opens nothing.
        let opened = std::path::absolute(path)
            .map_err(OpenError::AbsolutePath)
            .and_then(|absolute| {
                let inner = open::open(path, wait.timeout(), open::Mode::Create)?;
                let runtime = Driver::new().map_err(OpenError::Runtime)?;
                Ok((inner, runtime, absolute))
            });
        match opened {
            Ok((inner, runtime, path)) => Ok(Self {
                inner,
                runtime,
                path,
                wait: wait.timeout(),
                telemetry: None,
                dropped: 0,
            }),
            Err(source) => Err(StoreError::Open {
                path: path.to_path_buf(),
                source,
            }),
        }
    }

    /// The identity this store mints event ids under (ID-01).
    #[must_use]
    pub const fn store_id(&self) -> StoreId {
        self.inner.store_id()
    }

    /// Appends `events` in one transaction, unless `condition` matches an event.
    ///
    /// # Errors
    /// [`StoreError::InsideRuntime`]; [`StoreError::Query`] for a condition happenstance cannot
    /// query; [`StoreError::Append`] when happenstance refuses the append (no events, a store
    /// limit, a busy lock past the timeout, I/O). A matched condition is
    /// [`Appended::Conflict`], not an error.
    pub fn append(
        &self,
        events: Vec<NewEvent>,
        condition: Option<&Condition>,
    ) -> Result<Appended, StoreError> {
        let condition = condition.map(Condition::to_append_condition).transpose()?;
        let events: Vec<Event> = events.into_iter().map(NewEvent::into_event).collect();
        match self
            .runtime
            .block_on(self.inner.append(&events, condition.as_ref()))?
        {
            Ok(position) => Ok(Appended::At(u64::from(position))),
            Err(AppendError::ConditionViolated(_)) => Ok(Appended::Conflict),
            Err(other) => Err(StoreError::Append(other)),
        }
    }

    /// At most `limit` events matching `filter` after `position` (exclusive), in store order.
    ///
    /// # Errors
    /// [`StoreError::InsideRuntime`]; [`StoreError::Query`] for a filter happenstance cannot
    /// query; [`StoreError::Read`] when happenstance cannot read the store.
    pub fn read(
        &self,
        filter: &Filter,
        position: u64,
        limit: NonZeroUsize,
    ) -> Result<Vec<Stored>, StoreError> {
        let page = Page {
            after: position,
            limit,
        };
        read_page(&self.runtime, &self.inner, filter, page)
    }
}

/// One page of a read: the events after a position, at most a limit of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Page {
    /// The position the page starts after (exclusive); 0 is the whole store.
    pub after: u64,
    /// The most events the page holds.
    pub limit: NonZeroUsize,
}

/// At most `page.limit` events of `store` matching `filter` after `page.after`, in store order.
fn read_page(
    runtime: &Driver,
    store: &SqliteEventStore,
    filter: &Filter,
    page: Page,
) -> Result<Vec<Stored>, StoreError> {
    let Some(from) = page.after.checked_add(1).and_then(SequencePosition::new) else {
        return Ok(Vec::new());
    };
    let query = filter.to_query()?;
    let options = ReadOptions::new().from(from).limit(page.limit.get());
    let events = runtime
        .block_on(collect(store.read(&query, options)))?
        .map_err(StoreError::Read)?;
    Ok(events.into_iter().map(Stored::from_sequenced).collect())
}

/// Whether this thread is in a tokio runtime context, where a nested `block_on` panics. The one
/// check the store and the old-ledger reader share.
pub(crate) fn inside_runtime() -> bool {
    tokio::runtime::Handle::try_current().is_ok()
}

/// Refuses to run inside a tokio runtime context.
fn refuse_inside_runtime() -> Result<(), StoreError> {
    if inside_runtime() {
        Err(StoreError::InsideRuntime)
    } else {
        Ok(())
    }
}

/// The store's current-thread runtime, shut down without blocking when it is dropped inside
/// another runtime's context, where tokio's blocking shutdown panics.
pub(crate) struct Driver {
    /// Always `Some` until [`Drop`] takes it.
    runtime: Option<tokio::runtime::Runtime>,
}

impl Driver {
    pub(crate) fn new() -> std::io::Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread().build()?;
        Ok(Self {
            runtime: Some(runtime),
        })
    }

    /// Runs `future` to completion, unless this thread is in a tokio runtime context.
    pub(crate) fn block_on<F: Future>(&self, future: F) -> Result<F::Output, StoreError> {
        refuse_inside_runtime()?;
        match &self.runtime {
            Some(runtime) => Ok(runtime.block_on(future)),
            // Only `drop` empties the driver, and nothing runs on a dropped store.
            None => Err(StoreError::InsideRuntime),
        }
    }
}

impl Drop for Driver {
    fn drop(&mut self) {
        let Some(runtime) = self.runtime.take() else {
            return;
        };
        if inside_runtime() {
            // The store only `block_on`s, so no task of its own is left to wait for.
            runtime.shutdown_background();
        } else {
            drop(runtime);
        }
    }
}
