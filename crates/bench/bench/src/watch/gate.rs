//! The merge gate as `watch pr` asks it: would `gh pr merge <n> --match-head-commit <head>`,
//! run from this worktree, be denied? The real answer is `hook::merge_gate::judge_head`, the
//! hook's own checks, behind the GitHub kit's `HeadGate` port; tests fake the port.

use std::ffi::OsString;
use std::path::Path;

use redkiln_github::watch::{GateBlock, HeadGate};

use crate::hook::merge_gate::{self, Pinned};
use crate::paths::GIT_TIMEOUT;
use crate::pr_diff::Budget;

/// The gate the hook runs, over the ledger and worktree seen from `cwd`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LedgerGate<'a, L> {
    /// Where the worktree and the ledger are found.
    pub(crate) cwd: &'a Path,
    /// The process's environment.
    pub(crate) lookup: L,
}

impl<L: Fn(&str) -> Option<OsString>> HeadGate for LedgerGate<'_, L> {
    fn judge(&self, pr: u64, head: &str) -> Option<GateBlock> {
        let budget = Budget::per_call(GIT_TIMEOUT);
        let pinned = Pinned { pr, head };
        merge_gate::judge_head(self.cwd, &self.lookup, pinned, budget).map(|denial| GateBlock {
            check: denial.check_id(),
            reason: denial.into_reason(),
        })
    }
}
