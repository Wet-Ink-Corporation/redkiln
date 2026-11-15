//! The merge gate a pull request watch asks last: the caller's own judgement of whether
//! merging the pull request pinned to GitHub's head would be denied. The kit only asks; what
//! the gate reads (a ledger, a verdict, a worktree) is the caller's.

/// Why the merge gate would deny the merge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateBlock {
    /// The gate's check id (`no-verdict`, `stale`, `review`, `unjudged`, ...).
    pub check: &'static str,
    /// The gate's one-line reason, naming the command that clears it.
    pub reason: String,
}

/// The merge gate's judgement of a pull request's head.
pub trait HeadGate {
    /// Why merging pull request `pr` pinned to `head` (a full commit id) would be denied;
    /// `None` when it would be allowed. A gate that cannot judge denies: it returns a
    /// [`GateBlock`] naming why.
    fn judge(&self, pr: u64, head: &str) -> Option<GateBlock>;
}
