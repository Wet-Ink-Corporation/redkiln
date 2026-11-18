//! How a guard bounds the store's lock waits by what is left of its budget, so that its
//! decision is written before its 5 s `hooks.json` `timeout` even when every lock it meets is
//! held (SPEC HK-5). A guard's entry sets `onFailure: "continue"` (HK-11), so a guard past its
//! timeout lets the call through, losing a deny it had already decided.
//!
//! Each store step of a hook (an open, a read, an append) can wait for another process's lock.
//! A guard opens its store with a lock wait of its own: what is left of its
//! [`HOOK_BUDGET`](super::HOOK_BUDGET), less what must follow, shared among the steps that
//! connection serves ([`share`]), at most Core's hook wait ([`STORE_WAIT`], `Wait::Hook`,
//! ST-09) and at least [`FLOOR`]. Git before the open gets what leaves each of those steps its
//! floor, at most its own cap ([`git_timeout`]). Before each later step the guard checks that
//! the step's wait, the open's share, still fits in what is left ([`step`]); when it does not,
//! the step is skipped or the call denied, as each guard says.
//!
//! A later step's wait is the open's share, not what is left at that moment. Core fixes the
//! busy timeout when it hands the connection to happenstance, and happenstance 0.6.0 has no
//! setter on an open store. A SQLite busy handler installed before the hand-over could change
//! it later, but it would reset `PRAGMA busy_timeout`, which Core's open pins. A setter in
//! happenstance is a possible follow-up.
//!
//! Each Core store call is counted as one wait. Inside one call SQLite can wait more than once
//! (happenstance's identity re-read at hand-over, a commit's lock escalation), each time up to
//! the open's share, so under a lock held for the full share at each of those a guard can still
//! pass its timeout.

use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

use crate::existing_ledger::{self, LedgerError};
use crate::pr_diff::Budget;
use crate::store::{Store, Wait};

/// The longest one store step of a hook waits for another process's lock: Core's hook wait.
pub(crate) const STORE_WAIT: Duration = Wait::Hook.timeout();

/// The least time worth giving git to find the ledger; with less, the store steps after it are
/// not started.
pub(crate) const MIN_GIT_TIMEOUT: Duration = Duration::from_millis(200);

/// The least lock wait worth giving a store step: with less left, after what must follow it,
/// the step is not started.
pub(crate) const FLOOR: Duration = Duration::from_millis(100);

/// Why a store step was not started: less than it may take is left of the hook's budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("no time is left in the hook's budget to wait {needed:?} for the ledger's lock")]
pub(crate) struct OutOfTime {
    /// What the step may take: its lock wait, and what must follow it or git before it.
    needed: Duration,
}

/// What a store a hook opens will serve: its steps, the open among them, and what the hook
/// needs besides them once the open has started (git before a later read, say).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Steps {
    /// The store steps the connection serves, the open included.
    pub(crate) count: u32,
    /// What the hook needs besides the steps' lock waits.
    pub(crate) after: Duration,
}

/// Why a hook did not open the ledger.
#[derive(Debug, thiserror::Error)]
pub(crate) enum OpenRefused {
    /// Less than git and the steps' floors was left of the hook's budget.
    #[error(transparent)]
    OutOfTime(OutOfTime),
    /// It could not be found or opened.
    #[error(transparent)]
    Ledger(LedgerError),
}

/// What is left of `budget`: `Duration::MAX` for a budget with no deadline (a command).
pub(crate) fn left(budget: &Budget) -> Duration {
    budget.cap(Duration::MAX)
}

/// The longest the next store step may wait when `after` must follow it: what is left of
/// `budget`, less `after`.
///
/// # Errors
/// [`OutOfTime`] when that is under [`FLOOR`]: the step is not started.
pub(crate) fn cap(budget: &Budget, after: Duration) -> Result<Duration, OutOfTime> {
    within(left(budget), after)
}

/// [`cap`], with `remaining` left.
fn within(remaining: Duration, after: Duration) -> Result<Duration, OutOfTime> {
    remaining
        .checked_sub(after)
        .filter(|wait| *wait >= FLOOR)
        .ok_or_else(|| OutOfTime {
            needed: after.saturating_add(FLOOR),
        })
}

impl Steps {
    /// The least the steps need once git has answered: each step's [`FLOOR`], and `after`.
    pub(crate) const fn floors(self) -> Duration {
        FLOOR.saturating_mul(self.count).saturating_add(self.after)
    }
}

/// How long git may take to find the ledger with `remaining` left when `steps` follow it: what
/// leaves them their [`Steps::floors`], at most `cap`.
///
/// # Errors
/// [`OutOfTime`] when that is under [`MIN_GIT_TIMEOUT`]: neither git nor the steps are
/// started.
pub(crate) fn git_timeout(
    remaining: Duration,
    steps: Steps,
    cap: Duration,
) -> Result<Duration, OutOfTime> {
    remaining
        .checked_sub(steps.floors())
        .filter(|git| *git >= MIN_GIT_TIMEOUT)
        .map(|git| git.min(cap))
        .ok_or_else(|| OutOfTime {
            needed: steps.floors().saturating_add(MIN_GIT_TIMEOUT),
        })
}

/// The lock wait a store is opened with, with `remaining` left: what is left less
/// `steps.after`, shared among `steps.count`, at most [`STORE_WAIT`].
///
/// # Errors
/// [`OutOfTime`] when a step's share is under [`FLOOR`] (or there is no step): the store is
/// not opened.
pub(crate) fn share(remaining: Duration, steps: Steps) -> Result<Duration, OutOfTime> {
    within(remaining, steps.after)
        .ok()
        .and_then(|left| left.checked_div(steps.count))
        .filter(|wait| *wait >= FLOOR)
        .map(|wait| wait.min(STORE_WAIT))
        .ok_or_else(|| OutOfTime {
            needed: steps.floors(),
        })
}

/// Whether a later step of a store opened with `wait` fits in what is left of `budget`, with
/// `besides` (git before it, or what must follow it) taken too.
///
/// # Errors
/// [`OutOfTime`] when it does not, and the step is not started.
pub(crate) fn step(budget: &Budget, wait: Duration, besides: Duration) -> Result<(), OutOfTime> {
    fits(cap(budget, besides), wait, besides)
}

/// [`step`], with `remaining` left: for a hook that reads what is left from its own clock.
///
/// # Errors
/// As [`step`].
pub(crate) fn step_within(
    remaining: Duration,
    wait: Duration,
    besides: Duration,
) -> Result<(), OutOfTime> {
    fits(within(remaining, besides), wait, besides)
}

/// Whether a step's `wait` fits under `capped`, the longest it may wait with `besides` taken.
fn fits(
    capped: Result<Duration, OutOfTime>,
    wait: Duration,
    besides: Duration,
) -> Result<(), OutOfTime> {
    match capped {
        Ok(cap) if wait <= cap => Ok(()),
        Ok(_) | Err(_) => Err(OutOfTime {
            needed: wait.saturating_add(besides),
        }),
    }
}

/// The store seen from `cwd` when there is one, opened for `steps` within `budget`: git gets
/// [`git_timeout`] (at most `budget`'s per-call cap), then the store [`share`] of what git
/// left.
///
/// # Errors
/// [`OpenRefused::OutOfTime`] when too little is left for git or the open, which is then not
/// started; [`OpenRefused::Ledger`] when the ledger cannot be found or opened.
pub(crate) fn open_existing(
    cwd: &Path,
    lookup: impl Fn(&str) -> Option<OsString>,
    budget: &Budget,
    steps: Steps,
) -> Result<Option<Store>, OpenRefused> {
    let refused = OutOfTime {
        needed: steps.floors().saturating_add(MIN_GIT_TIMEOUT),
    };
    let git = match budget.next() {
        Ok(per_call) => git_timeout(left(budget), steps, per_call),
        // Under git's least call is left: under its floor too.
        Err(_spent) => Err(refused),
    }
    .map_err(OpenRefused::OutOfTime)?;
    let path = existing_ledger::locate(cwd, lookup, git).map_err(OpenRefused::Ledger)?;
    let wait = share(left(budget), steps).map_err(OpenRefused::OutOfTime)?;
    Store::open_existing_within(&path, Wait::Hook, wait)
        .map_err(|error| OpenRefused::Ledger(LedgerError::Open(error)))
}

/// Every `remaining` from zero to `budget` in 10 ms steps: the times a test checks a step at.
#[cfg(test)]
pub(crate) fn every_remaining(budget: Duration) -> impl Iterator<Item = Duration> {
    let steps = u32::try_from(budget.as_millis() / 10).expect("a few hundred steps");
    (0..=steps).map(|n| Duration::from_millis(10) * n)
}

#[cfg(test)]
mod tests;
