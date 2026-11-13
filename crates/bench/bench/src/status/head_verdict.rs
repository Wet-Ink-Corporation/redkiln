//! `verdict`: the review verdict for `HEAD`, judged as `bench verdict valid` judges it
//! (`verdict::pull_request::read` and `assess`), so a rebase that kept the patch id keeps it.
//!
//! The ledger records verdicts per pull request, and nothing maps `HEAD` to one, so the pull
//! request is the one of the newest `verdict_set` whose commit is `HEAD`, else of the newest
//! `verdict_set` in the ledger. With no verdict at all, or an unborn `HEAD`, the section is
//! `null`.

use std::path::Path;
use std::time::Duration;

use crate::store::{EventKind, Filter, LedgerEvent, Store};
use bench_contract::event::VerdictSet;
use bench_contract::status::{self, FreshVerdict, HeadVerdict};

use super::Unreadable;
use crate::pr_diff::{self, Budget};
use crate::repo_config;
use crate::verdict::assess::{Fresh, Invalid, Judged, Outcome};
use crate::verdict::pull_request::{self, MAX_VERDICTS, Query};

/// How long each git call may take: a UI refreshes often (H-13).
const GIT_CALL: Duration = Duration::from_secs(2);

/// The outcome as the section names it, `verdict valid`'s names.
const fn outcome(outcome: Outcome) -> status::Outcome {
    match outcome {
        Outcome::Valid => status::Outcome::Valid,
        Outcome::NoiseOnly => status::Outcome::NoiseOnly,
        Outcome::Invalid(Invalid::NoVerdict) => status::Outcome::NoVerdict,
        Outcome::Invalid(Invalid::Stale) => status::Outcome::Stale,
        Outcome::Invalid(Invalid::ChangesRequested) => status::Outcome::ChangesRequested,
    }
}

/// Why the verdict is fresh, as the section names it.
const fn fresh(fresh: Fresh) -> status::Fresh {
    match fresh {
        Fresh::BySha => status::Fresh::BySha,
        Fresh::ByPatchId => status::Fresh::ByPatchId,
    }
}

/// The fresh verdict `judged`: what it says, who set it, for which commit, and why it is fresh.
fn fresh_verdict(judged: Judged<'_>) -> FreshVerdict {
    let set = judged.record.set;
    FreshVerdict {
        event: judged.record.position,
        verdict: set.verdict,
        source: set.source,
        // The pull request read is dropped after this; the section keeps its own copy.
        sha: set.sha.clone(),
        fresh: fresh(judged.fresh),
    }
}

/// The verdict for `HEAD` of the worktree at `root`; `None` with no verdict in `store` or an
/// unborn `HEAD`.
pub(super) fn read(store: &Store, root: &Path) -> Result<Option<HeadVerdict>, Unreadable> {
    let budget = Budget::per_call(GIT_CALL);
    let filter = Filter {
        types: vec![EventKind::VerdictSet],
        tags: Vec::new(),
    };
    let sets: Vec<VerdictSet> = store
        .tail(&filter, MAX_VERDICTS)?
        .into_iter()
        .filter_map(|recorded| match recorded.event.event {
            LedgerEvent::VerdictSet(set) => Some(set),
            _ => None,
        })
        .collect();
    let Some(newest) = sets.last() else {
        return Ok(None);
    };
    let timeout = budget.next()?;
    let Some(head) = pr_diff::resolve_commit(root, "HEAD", timeout)? else {
        return Ok(None);
    };
    let pr = sets
        .iter()
        .rev()
        .find(|set| set.sha.as_deref() == Some(head.as_str()))
        .unwrap_or(newest)
        .pr;
    let config = repo_config::gate(root)?;
    let query = Query {
        root,
        pr,
        head: ("HEAD", &head),
        base: None,
        budget,
    };
    let pull = pull_request::read(Some(store), &query)?;
    let assessment = pull.assess(config.require_plugin);
    let outcome = outcome(assessment.outcome());
    Ok(Some(HeadVerdict {
        pr,
        verdict: assessment.latest.map(fresh_verdict),
        stale: assessment.stale,
        valid: outcome == status::Outcome::Valid,
        outcome,
        base: pull.base,
        head: pull.head,
    }))
}
