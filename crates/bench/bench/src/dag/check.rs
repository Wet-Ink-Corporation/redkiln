//! The exit code of the GitHub kit's graph check: Bench's mapping of a
//! [`Check`] onto `dag`'s exit-code table.
//!
//! ```
//! use bench::dag::check::CheckCode;
//! use redkiln_github::graph::Graph;
//! use redkiln_github::ticket::{Status, Ticket, TicketId};
//!
//! let ticket = |id: &str, blocked_by: &[&str], status| Ticket {
//!     id: TicketId::new(id),
//!     title: id.to_owned(),
//!     body: String::new(),
//!     blocked_by: blocked_by.iter().copied().map(TicketId::new).collect(),
//!     files: Vec::new(),
//!     status,
//! };
//! let graph = Graph::new(vec![
//!     ticket("1", &[], Status::Done),
//!     ticket("2", &["1"], Status::Open),
//!     ticket("3", &["2"], Status::Open),
//! ])?;
//!
//! assert!(graph.check().code().is_success());
//! let ready: Vec<&str> = graph.frontier().map(|t| t.id.as_str()).collect();
//! assert_eq!(ready, ["2"]);
//! # Ok::<(), redkiln_github::graph::GraphError>(())
//! ```

use crate::exit_codes::{self, Code};
use redkiln_github::graph::Check;

/// A graph check's exit code.
pub trait CheckCode {
    /// The exit code: [`exit_codes::DAG_DANGLING`] when any reference dangles (a cycle through
    /// a missing ticket cannot be judged, so dangling comes first), else
    /// [`exit_codes::DAG_CYCLE`] for a cycle, else [`exit_codes::OK`].
    #[must_use]
    fn code(&self) -> Code;
}

impl CheckCode for Check<'_> {
    fn code(&self) -> Code {
        if !self.dangling.is_empty() {
            exit_codes::DAG_DANGLING
        } else if self.cycle.is_some() {
            exit_codes::DAG_CYCLE
        } else {
            exit_codes::OK
        }
    }
}

#[cfg(test)]
mod tests;
