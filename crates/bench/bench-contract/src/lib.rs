//! Bench's contract: what other crates may name of Bench without depending on its internals
//! (M2-C).
//!
//! * [`event`]: the 17 `bench.*` event types, their payloads and the 0.x flat form
//!   ([`BenchEvent`]) the `bench ledger` CLI reads and writes.
//! * [`cli`]: the `bench` command line's binary, subcommands and value options.
//! * [`plugin`]: the `rk-bench` plugin's name that its skills are namespaced under.
//!
//! With the `core` feature the payloads are Core [`Payload`](redkiln_core::envelope::Payload)s
//! and [`event::decode`] reads a stored Bench event. Without it the crate is pure (serde and
//! thiserror only), so a tool links it without linking Core's store.

pub mod cli;
pub mod event;
pub mod plugin;
pub mod status;

pub use event::{BenchEvent, EventKind};
