//! Redkiln Core: the suite's one event store, as the SPEC (`spec/`) defines it.
//!
//! - [`store`] opens the store under a TRUNCATE journal, stamps and checks its schema version,
//!   and appends events, conditioned where a write must not race (SPEC ST-04 to ST-11).
//! - [`legacy`] reads an old plugin ledger (weighin 0.1.x to 1.x) from a copy, never touching
//!   the file itself, so the import (ST-15) can read it while a 1.x binary still uses it.
//!
//! Every operation is synchronous: a [`store::Store`] drives happenstance's async API on its
//! own current-thread runtime, and refuses to run inside another runtime's context.

pub mod legacy;
pub mod store;

pub use happenstance_core::{EventId, StoreId};
