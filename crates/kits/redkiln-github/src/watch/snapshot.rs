//! What one poll saw: the blockers, and the work still pending.
//!
//! A blocker settles the watch at once; pending work makes it poll again. Every blocker is
//! listed; which one a blocked watch is reported by (an exit code, say) is the caller's.

use serde::Serialize;

/// One poll's view of the target.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Snapshot {
    /// The commit the checks were read for, when the target has one.
    pub head: Option<String>,
    /// What stops the target, each named.
    pub blockers: Vec<Blocker>,
    /// What has not finished yet, each named.
    pub pending: Vec<Pending>,
}

impl Snapshot {
    /// True when polling again cannot change the answer: something blocks, or nothing is
    /// pending.
    pub(crate) fn settled(&self) -> bool {
        !self.blockers.is_empty() || self.pending.is_empty()
    }
}

/// Something that stops the target from being clear.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Blocker {
    /// The pull request conflicts with its base.
    Conflict {
        /// The pull request number.
        pr: u64,
        /// Its web address.
        url: String,
    },
    /// An unresolved review thread.
    Thread {
        /// The thread's first comment.
        url: Option<String>,
        /// The file the thread is on.
        path: Option<String>,
    },
    /// A check run or commit status that did not pass.
    Check {
        /// The check's name (a check run's name, or a status's context).
        name: String,
        /// How it ended, in GitHub's words lower-cased (`failure`, `timed_out`, `error`).
        conclusion: String,
        /// Its details page.
        url: Option<String>,
    },
    /// A deployment whose latest status is not a success.
    Deployment {
        /// The deployment id.
        id: String,
        /// Its latest state (`failure`, `error`, `inactive`).
        state: String,
        /// Its log or target page.
        url: Option<String>,
    },
    /// The pull request is a draft, which GitHub does not merge.
    Draft {
        /// The pull request number.
        pr: u64,
        /// Its web address.
        url: String,
    },
    /// GitHub refuses the merge for a reason the checks and threads read do not show: branch
    /// protection still `blocked` once every check passed and every thread is resolved, and
    /// again on a second read of the pull request (a required review, or a required check
    /// that never reported), `behind`, or a state this version does not know.
    MergeState {
        /// The pull request number.
        pr: u64,
        /// GitHub's `mergeable_state`.
        state: String,
        /// Its web address.
        url: String,
    },
    /// The merge gate would deny a merge of the head.
    Gate {
        /// The gate's check id (`no-verdict`, `stale`, `review`, ...).
        check: &'static str,
        /// The gate's one-line reason, naming the command that clears it.
        reason: String,
    },
}

/// Work that has not finished, so the watch polls again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Pending {
    /// A check run or commit status still running or queued.
    Check {
        /// The check's name.
        name: String,
        /// Its state, lower-cased (`queued`, `in_progress`, `pending`, `expected`).
        status: String,
    },
    /// No check has reported on the commit yet.
    NoChecks,
    /// GitHub has not computed whether the pull request merges cleanly.
    Mergeable,
    /// GitHub does not yet allow the merge (`mergeable_state` `blocked` while checks still run
    /// or until a second read confirms it, or `unstable`).
    MergeState {
        /// GitHub's `mergeable_state`.
        state: String,
    },
    /// A deployment not yet finished, or with no status yet.
    Deployment {
        /// The deployment id.
        id: String,
        /// Its latest state, or `none`.
        state: String,
    },
}

#[cfg(test)]
mod tests {
    use super::{Blocker, Pending, Snapshot};

    fn check(name: &str) -> Blocker {
        Blocker::Check {
            name: name.to_owned(),
            conclusion: "failure".to_owned(),
            url: None,
        }
    }

    #[test]
    fn a_snapshot_settles_on_a_blocker_or_when_nothing_is_pending() {
        let pending = Snapshot {
            pending: vec![Pending::NoChecks],
            ..Snapshot::default()
        };
        let blocked = Snapshot {
            blockers: vec![check("lint")],
            ..pending.clone()
        };

        let settled = [
            Snapshot::default().settled(),
            pending.settled(),
            blocked.settled(),
        ];

        assert_eq!(settled, [true, false, true], "clear, pending, blocked");
    }
}
