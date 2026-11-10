//! Events another store minted, stored under their own identity (SPEC ID-02, JR-11), through
//! happenstance's ingest port: the write path that keeps a foreign `EventId` and its recorded
//! time instead of minting them, deduplicating on the identity inside the write.
//!
//! Crate-internal on purpose: the ability to write an event under any identity is the ability to
//! forge history, so only an import holds it.

use happenstance_core::{EventId, RecordedAt};
use happenstance_sqlite::happenstance_sync::{
    IngestError, IngestGroup, ReplicatedEvent, SendIngestStore,
};

use super::{NewEvent, Store, StoreError};

/// An event to store under the identity another store gave it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Foreign {
    /// Its identity (ID-02).
    pub(crate) id: EventId,
    /// When its first store recorded it, in milliseconds since the Unix epoch.
    pub(crate) recorded_at_millis: i64,
    /// The event.
    pub(crate) event: NewEvent,
}

/// What an ingest did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ingest {
    /// Written in one transaction: this many appended, this many already held.
    Done {
        /// Appended to the log.
        appended: usize,
        /// Skipped: the store already held their identity.
        skipped: usize,
    },
    /// Nothing written: this event claims the store's own identity, which the store does not
    /// hold (it was restored without a re-mint). The caller drops it and tries the rest again.
    UnheldOwn(EventId),
}

impl Store {
    /// Stores `events` under their own identities, in one transaction; each already held is
    /// skipped, inside the write. It gives way as an append does (ST-09), so an import's
    /// batches, each at most happenstance's batch limit, have the pause between them.
    ///
    /// # Errors
    /// [`StoreError::InsideRuntime`]; [`StoreError::Ingest`] when happenstance refuses the batch
    /// (a store limit, a busy lock past the open's timeout, I/O). Nothing is written then.
    pub(crate) fn ingest(&self, events: &[Foreign]) -> Result<Ingest, StoreError> {
        let replicated: Vec<ReplicatedEvent> = events
            .iter()
            .map(|foreign| {
                // happenstance takes its events by value; a payload is a shared `Bytes`, so the
                // copy is the tags and a handle.
                ReplicatedEvent::new(
                    foreign.id,
                    RecordedAt::from_millis(foreign.recorded_at_millis),
                    foreign.event.clone().into_event(),
                )
            })
            .collect();
        let groups = [IngestGroup::new(&replicated, &[])];
        let ingested = self.give_way.write(replicated.len(), || {
            self.runtime.block_on(self.inner.ingest(&groups))
        })?;
        match ingested {
            Ok(done) => Ok(Ingest::Done {
                appended: done.appended,
                skipped: done.skipped,
            }),
            Err(IngestError::UnheldOwnIdentity { id, .. }) => Ok(Ingest::UnheldOwn(id)),
            Err(other) => Err(StoreError::Ingest(other)),
        }
    }
}
