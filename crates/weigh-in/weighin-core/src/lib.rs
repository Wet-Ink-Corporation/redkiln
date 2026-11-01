//! Weigh-In's ledger domain, with no I/O.
//!
//! Everything a question goes through between an agent raising it and a decision landing in
//! `.kb` is an event in this crate's vocabulary ([`event`]). Current state is always a fold of
//! those events ([`projection`]); nothing here reads a clock, a file or a process, so the same
//! crate can be linked into redkiln-rs unchanged.
//!
//! * [`id`] mints the stable `wi-xxxxxx` item ids.
//! * [`marker`] parses the `weigh-in` fenced blocks agents emit.
//! * [`scan`] finds candidate questions in documents and newly written TODOs.
//! * [`card`] is the grounded decision card (the board and the atoms both render from it).
//! * [`atom`] renders `.kb` decision and open-question atoms that pass Redkiln's `KbFrontmatter`.
//! * [`board`] is the exchange format with the Artifact board.

#![forbid(unsafe_code)]

pub mod atom;
pub mod board;
pub mod card;
pub mod event;
pub mod id;
pub mod marker;
pub mod projection;
pub mod scan;

pub use card::Card;
pub use event::{EventEnvelope, LedgerEvent};
pub use id::ItemId;
pub use projection::{Item, Ledger, Status};
