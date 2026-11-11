//! Who decided a human's plan approval, for its Core record (AP-01, AP-05; D2, D3): the
//! suite's owner (ST-12), `[suite] owner` of the worktree's `.redkiln/suite.toml`, else the
//! slug git's `user.name` or `user.email` gives (`Owner::from_git_config`).

use std::path::{Path, PathBuf};
use std::time::Duration;

use redkiln_core::approval::{ApprovalRecorded, ApprovalSubject, Scope, Verdict};
use redkiln_core::subject::Subject;
use redkiln_core::suite::{Owner, Suite};

use crate::envelope::Finding;
use crate::paths::{self, PathError};

/// The subject kind of a plan (AP-01's `subject.kind`).
const PLAN_KIND: &str = "plan";

/// What git's configuration may hold, read with `--get-regexp` (one `<key> <value>` per line).
const USER_KEYS: &str = r"^user\.(name|email)$";

/// Most bytes kept of git's answer: two keys of any sensible length (H-13).
const CONFIG_CAP: u64 = 64 * 1024;

/// The suite owner for a command run in `cwd`, git given `timeout`: `[suite] owner` of the
/// worktree (the folder `cwd` outside git), else the slug git's configuration gives; `None`
/// when neither names one.
///
/// # Errors
/// [`PathError`] when git cannot be run or answered with a failure other than "neither key is
/// set" (its exit 1) or "not a repository".
pub(super) fn owner(cwd: &Path, timeout: Duration) -> Result<Option<Owner>, PathError> {
    let root = match paths::worktree_root_within(cwd, timeout) {
        Ok(root) => PathBuf::from(root),
        Err(PathError::NotARepository { .. }) => cwd.to_path_buf(),
        Err(error) => return Err(error),
    };
    if let Some(owner) = Suite::load(&root).owner() {
        // The record keeps the owner it names; the suite config is dropped here.
        return Ok(Some(owner.clone()));
    }
    let output = paths::run_git_capped(
        cwd,
        &["config", "--get-regexp", USER_KEYS],
        timeout,
        CONFIG_CAP,
    )?;
    // Exit 1 is git's "neither key is set": no owner, not a failure.
    if !output.status.success() && output.status.code() != Some(1) {
        return Err(paths::git_failure(cwd, &output.stderr));
    }
    let listed = String::from_utf8(output.stdout).map_err(PathError::NonUtf8Output)?;
    Ok(Owner::from_git_config(&listed))
}

/// The Core record of `owner`'s approval of the plan whose bytes are `bytes`, at `locator`:
/// repository scope, no gates, no expiry (D3).
pub(super) fn approval(owner: &Owner, bytes: &[u8], locator: &str) -> ApprovalRecorded {
    ApprovalRecorded {
        subject: ApprovalSubject::new(PLAN_KIND, &Subject::sha256(bytes), locator),
        verdict: Verdict::Approved,
        scope: Some(Scope::Repository),
        gates: None,
        decider: format!("human:{}", owner.as_str()),
        expires: None,
        request: None,
    }
}

/// The finding when a human's approval has no decider for its Core record: the Bench event is
/// still written, and other plugins' gates do not see the approval.
pub(super) fn unrecorded(plan: &str, why: &str) -> Finding {
    Finding::new(
        format!("plan/approve/no-owner:{plan}"),
        format!(
            "{plan}: the approval is recorded for bench, but not for other plugins: {why}. fix: \
             set `owner` under `[suite]` in .redkiln/suite.toml, or git's user.name, so later \
             approvals are recorded for them too."
        ),
    )
}

#[cfg(test)]
mod tests;
