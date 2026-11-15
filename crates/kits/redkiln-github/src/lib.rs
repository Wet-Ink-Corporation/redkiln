//! The Redkiln GitHub kit: tickets and where they come from, the graph they form, and the
//! bounded git reads of a pull request's diff. No harness protocol.
//!
//! - [`ticket`]: [`ticket::Ticket`] as every tracker hands it over, and the id order every
//!   listing uses.
//! - [`tracker`]: the [`tracker::Tracker`] port and its GitHub Issues adapter
//!   ([`tracker::github`]), with the read-only `gh` ports ([`tracker::github::GhApi`],
//!   [`tracker::github::GhGraphql`]) and their real adapter [`tracker::github::GhCli`].
//! - [`graph`]: the ticket graph, its check (dangling references, then cycles) and frontier.
//! - [`estimate`]: the per-ticket token estimate.
//! - [`diff`]: `git diff <base>...<head>` read through bounded git calls: commits, the default
//!   base, ancestry, patch id, numstat and text, within a [`diff::Budget`].
//!
//! Every call out of the process has a timeout and an output cap (H-13); every list read from
//! outside is capped and fails rather than truncating.

pub mod diff;
pub mod estimate;
mod git;
pub mod graph;
mod process;
#[cfg(test)]
mod test_repo;
pub mod ticket;
pub mod tracker;
