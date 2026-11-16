//! Redkiln Core's calibration crate (blueprint, Core: `redkiln-calibrate`): how well a
//! producer's probabilities match the outcomes later labelled, and where its per-user decision
//! rows live.
//!
//! - [`scope`]: the user scope `~/.redkiln/` (`$REDKILN_HOME`) and each producer's decision
//!   folder in it, with anneal's old `~/.anneal/decisions/` named to read alongside (D1).
//! - [`log`]: the rotating, capped, append-only decision log in one folder.
//! - [`rows`]: anneal's v1 decision and label rows read from one or more logs.
//! - [`score`]: the Brier score, reliability bins, expected cost and the proposed threshold.
//! - [`promotion`]: the bar a question clears to leave shadow mode; only ever reported.
//!
//! No I/O but the log's and the rows' files; the environment only through the lookup a binary
//! hands [`scope::UserScope::from_lookup`].

pub mod log;
pub mod promotion;
pub mod rows;
pub mod scope;
pub mod score;
