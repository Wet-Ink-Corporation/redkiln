//! What a guard holds back for the store's lock waits, so that its decision is written before
//! its 5 s `hooks.json` `timeout` even when every store wait it meets runs to its full timeout
//! (SPEC HK-5). A guard's entry sets `onFailure: "continue"` (HK-11), so a guard past its
//! timeout lets the call through, losing a deny it had already decided.
//!
//! Each store step of a hook (an open, a read, an append) can wait Core's hook wait
//! ([`STORE_WAIT`], `Wait::Hook`, ST-09) for another process's lock. A guard starts a store step
//! only when that whole wait fits in what is left of its [`HOOK_BUDGET`](super::HOOK_BUDGET),
//! and gives git, before it, only what the waits after it leave.
//!
//! Each Core store call is counted as one `Wait::Hook`. Inside one call SQLite can wait more
//! than once (happenstance's identity re-read at hand-over, a commit's lock escalation), so
//! under a lock held for the full wait at each of those a guard can still pass its timeout.

use std::time::Duration;

use crate::pr_diff::Budget;
use crate::store::Wait;

/// The longest one store step of a hook waits for another process's lock: Core's hook wait.
pub(crate) const STORE_WAIT: Duration = Wait::Hook.timeout();

/// The least time worth giving git to find the ledger; with less, the store step after it is
/// not started.
pub(crate) const MIN_GIT_TIMEOUT: Duration = Duration::from_millis(200);

/// Why a store step was not started: less than it may take is left of the hook's budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("no time is left in the hook's budget to wait {needed:?} for the ledger's lock")]
pub(crate) struct OutOfTime {
    /// What the step may take: its lock waits, and git before them when there is git.
    needed: Duration,
}

/// What `waits` full store waits may take.
pub(crate) const fn waits(waits: u32) -> Duration {
    STORE_WAIT.saturating_mul(waits)
}

/// What is left of `budget`: `Duration::MAX` for a budget with no deadline (a command).
pub(crate) fn left(budget: &Budget) -> Duration {
    budget.cap(Duration::MAX)
}

/// Whether a step that may take `needed` fits in `remaining`.
///
/// # Errors
/// [`OutOfTime`] when it does not, and the step is not started.
pub(crate) fn fits(remaining: Duration, needed: Duration) -> Result<(), OutOfTime> {
    if needed <= remaining {
        Ok(())
    } else {
        Err(OutOfTime { needed })
    }
}

/// How long git may take to find the ledger when `remaining` is left and `count` store waits
/// follow it: what those waits leave, at most `cap`.
///
/// # Errors
/// [`OutOfTime`] when that is under [`MIN_GIT_TIMEOUT`]: neither git nor the waits are started.
pub(crate) fn git_before(
    remaining: Duration,
    count: u32,
    cap: Duration,
) -> Result<Duration, OutOfTime> {
    let reserve = waits(count);
    remaining
        .checked_sub(reserve)
        .filter(|git| *git >= MIN_GIT_TIMEOUT)
        .map(|git| git.min(cap))
        .ok_or_else(|| OutOfTime {
            needed: reserve.saturating_add(MIN_GIT_TIMEOUT),
        })
}

/// Every `remaining` from zero to `budget` in 10 ms steps: the times a test checks a step at.
#[cfg(test)]
pub(crate) fn every_remaining(budget: Duration) -> impl Iterator<Item = Duration> {
    let steps = u32::try_from(budget.as_millis() / 10).expect("a few hundred steps");
    (0..=steps).map(|n| Duration::from_millis(10) * n)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{MIN_GIT_TIMEOUT, OutOfTime, STORE_WAIT, every_remaining, fits, git_before, waits};
    use crate::hook::HOOK_BUDGET;

    #[test]
    fn a_hook_store_wait_is_two_seconds() {
        assert_eq!(STORE_WAIT, Duration::from_secs(2));
    }

    #[test]
    fn git_and_the_waits_after_it_never_pass_what_is_left() {
        for count in 0..=3 {
            let overrun = every_remaining(HOOK_BUDGET)
                .filter_map(|left| {
                    git_before(left, count, Duration::from_secs(1))
                        .ok()
                        .map(|git| (left, git))
                })
                .find(|(left, git)| *git + waits(count) > *left);
            assert_eq!(overrun, None, "{count} waits: (time left, git's timeout)");
        }
    }

    #[test]
    fn git_gets_what_the_waits_leave_at_most_its_cap() {
        let cap = Duration::from_secs(1);
        assert_eq!(git_before(HOOK_BUDGET, 0, cap), Ok(cap));
        assert_eq!(
            git_before(HOOK_BUDGET, 2, cap),
            Ok(Duration::from_millis(500))
        );
        assert_eq!(
            git_before(waits(2) + Duration::from_millis(199), 2, cap),
            Err(OutOfTime {
                needed: waits(2) + MIN_GIT_TIMEOUT
            }),
            "less than the least worth giving git"
        );
    }

    #[test]
    fn a_step_fits_only_when_all_it_may_take_is_left() {
        assert_eq!(fits(STORE_WAIT, STORE_WAIT), Ok(()));
        assert_eq!(
            fits(Duration::from_millis(1_999), STORE_WAIT),
            Err(OutOfTime { needed: STORE_WAIT })
        );
    }
}
