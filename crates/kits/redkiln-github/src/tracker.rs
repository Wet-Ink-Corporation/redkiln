//! Where tickets come from: the [`Tracker`] port and the GitHub Issues adapter ([`github`]),
//! which also fetches blockers outside the issues read as external tickets.
//!
//! A new tracker (ADO, Jira: #45) is a type implementing [`Tracker`] with its own error type,
//! in the kit or in the plugin that needs it (Bench's markdown tracker lives in Bench). The
//! graph, the estimate and the output only see [`Ticket`]s.

pub mod github;

use crate::ticket::Ticket;

/// The most tickets a tracker hands over (H-13); past it the read fails rather than
/// returning part of the graph.
pub const MAX_TICKETS: usize = 10_000;

/// The tickets in scope and the external tickets their blockers name.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Tickets {
    /// The tickets the graph is about: checked, counted and listed in the frontier.
    pub in_scope: Vec<Ticket>,
    /// Tickets outside the scope that an in-scope ticket is blocked by. They resolve
    /// references (a done one is a satisfied blocker, an open one blocks) but are never
    /// counted or ready themselves, and their own blockers are not followed.
    pub external: Vec<Ticket>,
}

/// A source of tickets.
///
/// Contract for implementations:
/// - Read only: never change the tracker.
/// - Ids are unique; a ticket's `blocked_by` uses the same ids, and may name tickets that do not
///   exist (the graph reports them as dangling, so do not drop them).
/// - Map the tracker's own states onto [`Status`](crate::ticket::Status): `Done` only for
///   finished work.
/// - Bound everything read from outside: at most [`MAX_TICKETS`] tickets, capped sizes, a
///   timeout on any call out of the process. Fail instead of truncating.
/// - Order does not matter: the graph sorts by id.
pub trait Tracker {
    /// Why this tracker could not hand over its tickets.
    type Error: std::error::Error + Send + Sync + 'static;

    /// The tracker's name as `--tracker` spells it (`"markdown"`).
    fn name(&self) -> &'static str;

    /// Reads every ticket in scope.
    ///
    /// # Errors
    /// [`Tracker::Error`] when the tickets cannot be read or one of them cannot be understood.
    fn read(&self) -> Result<Vec<Ticket>, Self::Error>;

    /// Reads every ticket in scope, plus the tickets outside it that they are blocked by. A
    /// tracker whose scope is a subset of a larger one (GitHub sub-issues) overrides this to
    /// look up each outside reference; a reference that names nothing stays dangling. The
    /// default has no outside: every ticket the tracker knows is in scope.
    ///
    /// # Errors
    /// As [`Tracker::read`], plus any failure looking up an outside reference.
    fn read_with_external(&self) -> Result<Tickets, Self::Error> {
        Ok(Tickets {
            in_scope: self.read()?,
            external: Vec::new(),
        })
    }
}
