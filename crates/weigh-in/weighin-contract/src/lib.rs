//! Weigh-In's contract: what another crate needs to raise, read and answer Weigh-In items without
//! reaching into Weigh-In's internals (AGENTS.md, Layer rules; SPEC DR).
//!
//! * [`vocab`]: an item's kind, door and authority.
//! * [`id`]: the `wi-` item ids.
//! * [`card`]: the grounded decision card and its guards (2 to 4 options, an honest
//!   recommendation).
//! * [`event`]: the 13 event payloads of the ledger.
//! * [`names`]: every `weighin.*` event type with its version.
//! * [`guard`]: triage's rule for routing an item to `default` (two-way judgment calls only).
//! * [`open`] and [`ci_check`]: the JSON `weighin open --json` and `weighin ci-check --json`
//!   write.
//! * [`status`]: the `data` of `weighin status --json` (SPEC CLI-12).
//! * [`cli`]: the binary's subcommands and value options, which prompts may name.
//!
//! With the `core` feature, the event payloads are Core [`Payload`](redkiln_core::envelope::Payload)s
//! and the decision request ([`request`], DR-01 to DR-04) and its answer ([`answer`], DR-05 and
//! DR-06) are available. Without it the crate is pure, so a domain crate can link it.

#[cfg(feature = "core")]
pub mod answer;
pub mod card;
pub mod ci_check;
pub mod cli;
pub mod event;
pub mod guard;
pub mod id;
pub mod names;
pub mod open;
#[cfg(feature = "core")]
pub mod request;
pub mod status;
pub mod vocab;

pub use card::Card;
pub use event::{EventEnvelope, LedgerEvent};
pub use id::ItemId;
