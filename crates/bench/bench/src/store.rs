//! The ledger store as the rest of Bench names it: the one module that reaches `bench-ledger`.
//!
//! Every command, hook and test in this crate takes the store's types from here, so the store
//! behind them can change in one place (Move 2 step 3: `bench-ledger` today, Core's store
//! next). The `bench.*` payload types are not re-exported: they are Bench's contract's, and
//! callers name them through [`bench_contract::event`].
//!
//! - [`Store`]: an open ledger, with [`Filter`] for reads, [`Condition`] for conditional
//!   appends, and [`Appended`] and [`Recorded`] for what a write and a read return.
//! - [`EventEnvelope`]: an event with its writer and [`Timestamp`]; [`LedgerEvent`] and
//!   [`EventKind`] are the event it carries and its kind, [`REGISTRY_VERSION`] the version of
//!   the event registry the envelope is read against.
//! - [`LedgerTag`] and [`TagKey`]: the tags a filter or condition matches on.
//! - [`StoreError`], [`SchemaError`], [`TagError`] and [`TimestampError`]: what can go wrong.

pub use bench_ledger::{
    Appended, Condition, EventEnvelope, EventKind, Filter, LedgerEvent, LedgerTag,
    REGISTRY_VERSION, Recorded, SchemaError, Store, StoreError, TagError, TagKey, Timestamp,
    TimestampError,
};
