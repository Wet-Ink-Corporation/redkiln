//! The ledger store: weighin's events in the suite's store, opened, read and written only
//! through `redkiln-core` (Move 1, #17).
//!
//! Every event is an EV-01 envelope of one of the 13 `weighin.*` types ([`wire`]). Every write
//! is conditioned (DCB): raising an item fails if that item was already raised, and every later
//! event for an item fails if the item changed since the writer read it. Two sessions deciding
//! the same item at once therefore cannot both win; the loser re-reads and sees the decision.
//!
//! Both conditions look only at weighin's types: any namespace may tag an `item:` (EV-12), and an
//! event another plugin wrote about an item never blocks a weighin write. A weighin event this
//! build cannot use still does (fail closed), and a read skips and counts it (EV-10).
//!
//! The store's journal mode, schema stamp and refusals are Core's (ST-04 to ST-07): an old weighin
//! ledger is never opened as a store, only imported (ST-15).

mod wire;

use std::num::NonZeroUsize;
use std::path::Path;

use anyhow::{Context as _, Result};
use redkiln_core::envelope::{Skip, Writer};
use redkiln_core::store::{Appended, Condition, OpenError, StoreError, Wait};
use weighin_core::event::EventEnvelope;
use weighin_core::{ItemId, Ledger};

/// How many events one read of the store returns at most.
const PAGE: NonZeroUsize = match NonZeroUsize::new(512) {
    Some(page) => page,
    None => NonZeroUsize::MIN,
};

/// The store, with the writer every envelope it appends names.
#[derive(Debug)]
pub(crate) struct Store {
    inner: redkiln_core::store::Store,
    writer: Writer,
}

/// What a conditioned append must not find.
#[derive(Debug, Clone)]
pub(crate) enum Guard {
    /// Fail if `item_raised` for this item exists.
    NotYetRaised(ItemId),
    /// Fail if any weighin event for this item was appended after `position`.
    ItemUnchangedSince(ItemId, u64),
}

/// The outcome of an append.
#[derive(Debug)]
pub(crate) enum Written {
    /// Appended; the events, handed back for the ledger, sit at positions up to `last`.
    At {
        /// The last event's position.
        last: u64,
        /// The events appended, in order.
        events: Vec<EventEnvelope>,
    },
    /// The guard matched: someone else wrote first.
    Conflict,
}

/// Every weighin event in position order, and how many this build could not use (EV-10).
#[derive(Debug, Default)]
pub(crate) struct Events {
    pub(crate) events: Vec<(u64, EventEnvelope)>,
    pub(crate) unusable: u64,
}

/// The ledger as read, and how many weighin events this build could not use (EV-10).
#[derive(Debug, Default)]
pub(crate) struct Loaded {
    pub(crate) ledger: Ledger,
    pub(crate) unusable: u64,
}

impl Store {
    /// Open the store at `path`, creating it and its directory when absent.
    ///
    /// # Errors
    /// Fails when Core cannot open it; an old weighin ledger or another program's database is
    /// refused untouched, with a hint that it is imported rather than opened.
    pub(crate) fn open(path: &Path, wait: Wait) -> Result<Self> {
        let inner = redkiln_core::store::Store::open(path, wait).map_err(refused)?;
        Self::over(inner)
    }

    /// Open the store at `path` only when there is one; `None` creates nothing (B-01).
    ///
    /// # Errors
    /// As [`Store::open`].
    pub(crate) fn open_existing(path: &Path, wait: Wait) -> Result<Option<Self>> {
        redkiln_core::store::Store::open_existing(path, wait)
            .map_err(refused)?
            .map(Self::over)
            .transpose()
    }

    fn over(inner: redkiln_core::store::Store) -> Result<Self> {
        Ok(Self {
            inner,
            writer: wire::writer()?,
        })
    }

    /// Every weighin event, in position order; one this build cannot use is skipped and
    /// counted, never a failure of the read (EV-10).
    ///
    /// # Errors
    /// Fails on a store error.
    pub(crate) fn read_all(&self) -> Result<Events> {
        let filter = wire::filter()?;
        let mut read = Events::default();
        let mut after = 0;
        loop {
            let page = self
                .inner
                .read(&filter, after, PAGE)
                .context("reading the store")?;
            for stored in &page {
                after = stored.position;
                match wire::from_stored(stored) {
                    Ok(env) => read.events.push((stored.position, env)),
                    Err(Skip::Unusable(_)) => read.unusable = read.unusable.saturating_add(1),
                    // The filter reads weighin's types only.
                    Err(Skip::Foreign) => {}
                }
            }
            if page.len() < PAGE.get() {
                return Ok(read);
            }
        }
    }

    /// The ledger folded from every usable weighin event.
    ///
    /// # Errors
    /// See [`Store::read_all`].
    pub(crate) fn ledger(&self) -> Result<Loaded> {
        let Events { events, unusable } = self.read_all()?;
        Ok(Loaded {
            ledger: Ledger::fold(events.iter().map(|(p, e)| (*p, e))),
            unusable,
        })
    }

    /// Append `envs` atomically under `guard`, handing them back when written.
    ///
    /// # Errors
    /// Fails when an envelope field or tag is one Core refuses (nothing is written), or on a
    /// store error. A guard match is [`Written::Conflict`], not an error.
    pub(crate) fn append(&self, envs: Vec<EventEnvelope>, guard: &Guard) -> Result<Written> {
        let condition = match guard {
            Guard::NotYetRaised(id) => Condition::not_yet(wire::RAISED, wire::item_tag(id)?)?,
            // Not Core's `unchanged_since`, which matches any type: another plugin's event about
            // the item would block every later weighin write to it.
            Guard::ItemUnchangedSince(id, position) => {
                Condition::none_matching(wire::filter()?.tagged(wire::item_tag(id)?))
                    .after(*position)
            }
        };
        let mut events = Vec::with_capacity(envs.len());
        let mut kept = Vec::with_capacity(envs.len());
        for env in envs {
            let (event, env) = wire::to_new_event(env, &self.writer)?;
            events.push(event);
            kept.push(env);
        }
        match self
            .inner
            .append(events, Some(&condition))
            .context("appending to the store")?
        {
            Appended::At(last) => Ok(Written::At { last, events: kept }),
            Appended::Conflict => Ok(Written::Conflict),
        }
    }
}

/// A store Core would not open, with a hint when it is an old ledger: weighin 2.0 imports a 1.x
/// ledger (Move 1 step 8, ST-15) rather than opening it.
fn refused(error: StoreError) -> anyhow::Error {
    let old = match &error {
        StoreError::Open {
            path,
            source: OpenError::OldLedger { .. } | OpenError::NotAStore,
        } => Some(path.display().to_string()),
        _ => None,
    };
    let error = anyhow::Error::new(error);
    match old {
        Some(path) => error.context(format!(
            "{path} is a weighin 1.x ledger or another program's database; weighin 2.0 imports a 1.x ledger rather than opening it, and left it unchanged"
        )),
        None => error,
    }
}

#[cfg(test)]
mod tests;
