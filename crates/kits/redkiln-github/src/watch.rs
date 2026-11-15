//! The watch engine: long-poll a pull request, a commit's checks or a deployment until it
//! settles or a timeout passes, and report what blocks it and what is still pending.
//!
//! A plugin builds a [`Source`] ([`PullRequestWatch`], [`CommitWatch`] or [`DeployWatch`] over
//! a [`DeployProvider`] such as [`GitHubDeployments`]) and hands it to [`watch`] with a
//! [`Clock`] and a [`Schedule`] (back-off 10 s, doubling, at most 60 s; never sleeping past the
//! deadline). The [`Watched`] it returns holds the last [`Snapshot`]: its [`Blocker`]s and
//! [`Pending`] work. What a blocker means to the caller (an exit code, a finding) is the
//! caller's. A pull request watch ends with the caller's merge gate, behind the [`HeadGate`]
//! port, asked only once nothing else blocks or is pending.
//!
//! GitHub is read only through `gh` ([`crate::tracker::github::GhApi`] for REST `GET`,
//! [`crate::tracker::github::GhGraphql`] for one review thread query), each call bounded by
//! the port's own timeout and output cap; every list read is capped and fails rather than
//! judging part of it (H-13).
//!
//! - `poll`: the loop, the back-off and the [`Clock`] and [`Source`] ports.
//! - `snapshot`: one poll's blockers and pending work.
//! - `github`: the pull request and commit readers (REST).
//! - `threads`: a pull request's unresolved review threads (GraphQL, or the Claude Code
//!   proxy's route where GraphQL is refused).
//! - `deploy`: the [`DeployProvider`] port and its GitHub Deployments reader.
//! - `gate`: the [`HeadGate`] port a pull request watch asks last.

mod deploy;
#[cfg(test)]
mod fake;
mod gate;
mod github;
mod poll;
mod snapshot;
mod threads;

pub use deploy::{DeployProvider, DeployState, DeployWatch, GitHubDeployments};
pub use gate::{GateBlock, HeadGate};
pub use github::{CommitWatch, PullRequestWatch, RepoRef};
pub use poll::{Clock, DEFAULT_TIMEOUT_SECS, Schedule, Source, SystemClock, Watched, watch};
pub use snapshot::{Blocker, Pending, Snapshot};
pub use threads::unresolved as unresolved_threads;

use crate::tracker::github::GhError;

/// Why a watch could not read its target.
#[derive(Debug, thiserror::Error)]
pub enum WatchError {
    /// A `gh` call failed.
    #[error(transparent)]
    Gh(#[from] GhError),
    /// The review threads could be read neither by GraphQL nor from the Claude Code proxy's
    /// route; the GraphQL failure leads, the route's follows as the source.
    #[error("{graphql}; the review thread route of Claude Code's proxy failed too")]
    Threads {
        /// Why the GraphQL query failed.
        graphql: GhError,
        /// Why the proxy's route failed.
        #[source]
        proxy: GhError,
    },
    /// An answer was not the shape expected.
    #[error("gh did not return {what} as expected")]
    Json {
        /// What was asked for.
        what: &'static str,
        /// The decoding failure.
        #[source]
        source: serde_json::Error,
    },
    /// The pull request was closed without being merged: nothing is left to wait for.
    #[error("pull request {pr} is closed without being merged")]
    Closed {
        /// The pull request number.
        pr: u64,
    },
    /// GitHub has no such target.
    #[error("{what} not found")]
    Missing {
        /// The target, as `pull request 7` or `commit main`.
        what: String,
    },
    /// A ref that is empty, `.` or `..`: as a path segment it would name another endpoint, and
    /// git allows none of them as a ref.
    #[error("{git_ref:?} is not a branch, tag or commit")]
    BadRef {
        /// The ref as given.
        git_ref: String,
    },
    /// More items than are read, so the target cannot be judged whole.
    #[error("more {what} than the {limit} anneal reads")]
    TooMany {
        /// What there are too many of.
        what: &'static str,
        /// The most read.
        limit: usize,
    },
}
