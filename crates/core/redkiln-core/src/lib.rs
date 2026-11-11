//! Redkiln Core: the suite's one event store, as the SPEC (`spec/`) defines it.
//!
//! - [`location`] finds the store from a working directory: flag, `$REDKILN_STORE`,
//!   `suite.toml`, the git common dir, or the folder outside git (ST-01 to ST-03).
//! - [`suite`] reads `.redkiln/suite.toml`: the repository class, which is `client` unless a
//!   file says `own`, the store and the owner (ST-12, ST-13).
//! - [`store`] opens the store under a TRUNCATE journal, stamps and checks its schema version,
//!   and appends events, conditioned where a write must not race (SPEC ST-04 to ST-11); a
//!   telemetry write never waits on a busy lock (ST-09), and telemetry is kept 30 days in a side
//!   store (EV-14).
//! - [`envelope`] builds events strictly and reads them leniently (EV-01 to EV-16), and
//!   [`store::Tag`]'s registry holds Core's tag keys (EV-11, EV-12).
//! - [`trust`] is the `trust` object every envelope carries (TR-01).
//! - [`approval`] is `core.approval_recorded`'s payload, the one approval record (AP-01).
//! - [`ids`] holds the SPEC's ids and digests (ID-01 to ID-07), and [`canonical`] the canonical
//!   JSON digest (ID-08, RFC 8785).
//! - [`legacy`] reads an old plugin ledger (weighin 0.1.x to 1.x) from a copy, never touching
//!   the file itself, so the import (ST-15) can read it while a 1.x binary still uses it.
//!
//! Every operation is synchronous: a [`store::Store`] drives happenstance's async API on its
//! own current-thread runtime, and refuses to run inside another runtime's context.

pub mod approval;
pub mod canonical;
pub mod envelope;
mod git;
pub mod ids;
pub mod journal;
pub mod legacy;
pub mod location;
pub mod store;
pub mod subject;
pub mod suite;
pub mod trust;

pub use happenstance_core::{EventId, StoreId};
