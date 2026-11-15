//! Where Bench's tickets come from, dispatched through [`AnyTracker`].
//!
//! The GitHub kit's [`Tracker`] port and its GitHub Issues adapter, and Bench's own
//! [`markdown`] adapter (`tickets/*.md` with `blocked_by:` frontmatter).
//!
//! A new tracker (ADO, Jira: #45) is a type implementing [`Tracker`], one more [`AnyTracker`]
//! and [`TrackerError`] variant, and one more `--tracker` value in `dag::TrackerKind`. Nothing
//! else changes: the graph, the estimate and the output only see tickets. See `docs/dag.md`.

pub mod markdown;

use markdown::{Markdown, MarkdownError};
use redkiln_github::ticket::Ticket;
use redkiln_github::tracker::github::{GhCli, GitHub, GitHubError};
use redkiln_github::tracker::{Tickets, Tracker};

/// The tracker `--tracker` selects: a closed set, so `bench dag` dispatches without boxing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnyTracker {
    /// GitHub Issues through the real `gh`.
    GitHub(GitHub<GhCli>),
    /// `<dir>/*.md`.
    Markdown(Markdown),
}

impl Tracker for AnyTracker {
    type Error = TrackerError;

    fn name(&self) -> &'static str {
        match self {
            Self::GitHub(tracker) => tracker.name(),
            Self::Markdown(tracker) => tracker.name(),
        }
    }

    fn read(&self) -> Result<Vec<Ticket>, TrackerError> {
        match self {
            Self::GitHub(tracker) => Ok(tracker.read()?),
            Self::Markdown(tracker) => Ok(tracker.read()?),
        }
    }

    fn read_with_external(&self) -> Result<Tickets, TrackerError> {
        match self {
            Self::GitHub(tracker) => Ok(tracker.read_with_external()?),
            Self::Markdown(tracker) => Ok(tracker.read_with_external()?),
        }
    }
}

/// Why a tracker could not hand over its tickets.
#[derive(Debug, thiserror::Error)]
pub enum TrackerError {
    /// The markdown tracker failed.
    #[error(transparent)]
    Markdown(#[from] MarkdownError),
    /// The GitHub tracker failed.
    #[error(transparent)]
    GitHub(#[from] GitHubError),
}
