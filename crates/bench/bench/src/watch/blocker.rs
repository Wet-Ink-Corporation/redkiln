//! What a watch's blockers and pending work mean to Bench: their exit codes and findings.
//!
//! A blocked watch exits with the code of its first blocker in this order: conflict 8, threads
//! 3, checks and deployments 4, draft, merge state and the merge gate 6; every blocker is
//! listed. The kit's [`Snapshot`], [`Blocker`] and [`Pending`] carry no code; this is the
//! mapping `watch` had before they moved (Move 2 step 5, PR 5.3b).

use std::collections::HashSet;

use redkiln_github::watch::{Blocker, Pending, Snapshot};

use crate::envelope::Finding;
use crate::exit_codes::{self, Code};

/// Which blocker's code a blocked watch exits with, lowest first: a merge conflict, then
/// review threads, then failing checks and deployments, then the draft, merge state and gate.
/// The conflict ranks first although its code (8) is the highest: SPEC CLI-05 moved it off 2,
/// and nothing else about the watch moved with it.
const fn rank(blocker: &Blocker) -> u8 {
    match blocker {
        Blocker::Conflict { .. } => 0,
        Blocker::Thread { .. } => 1,
        Blocker::Check { .. } | Blocker::Deployment { .. } => 2,
        Blocker::Draft { .. } | Blocker::MergeState { .. } | Blocker::Gate { .. } => 3,
    }
}

/// The exit code of a snapshot's blockers.
pub(crate) trait SnapshotCode {
    /// The exit code of the blocker that ranks first; `None` when nothing blocks.
    fn blocker_code(&self) -> Option<Code>;
}

impl SnapshotCode for Snapshot {
    fn blocker_code(&self) -> Option<Code> {
        self.blockers
            .iter()
            .min_by_key(|blocker| rank(blocker))
            .map(Blocker::code)
    }
}

/// A blocker's exit code and finding.
pub(crate) trait BlockerCode {
    /// The exit code this blocker means.
    fn code(&self) -> Code;
    /// The finding that names this blocker, with a stable key.
    fn finding(&self) -> Finding;
}

impl BlockerCode for Blocker {
    fn code(&self) -> Code {
        match self {
            Self::Conflict { .. } => exit_codes::WATCH_CONFLICT,
            Self::Thread { .. } => exit_codes::WATCH_THREADS,
            Self::Check { .. } | Self::Deployment { .. } => exit_codes::WATCH_CHECKS,
            Self::Draft { .. } | Self::MergeState { .. } | Self::Gate { .. } => {
                exit_codes::WATCH_GATE
            }
        }
    }

    fn finding(&self) -> Finding {
        match self {
            Self::Conflict { pr, url } => Finding::new(
                format!("watch/conflict:{pr}"),
                format!("pull request {pr} conflicts with its base ({url}); merge or rebase it"),
            ),
            Self::Thread { url, path } => {
                let on = path
                    .as_deref()
                    .map_or_else(String::new, |path| format!(" on {path}"));
                let (key, at) = url
                    .as_deref()
                    .map_or(("no-comment", "a thread with no comment"), |url| (url, url));
                Finding::new(
                    format!("watch/thread:{key}"),
                    format!("unresolved review thread{on}: {at}"),
                )
            }
            Self::Check {
                name,
                conclusion,
                url,
            } => Finding::new(
                format!("watch/check:{name}"),
                format!("check {name:?} ended {conclusion}{}", at(url.as_deref())),
            ),
            Self::Deployment { id, state, url } => Finding::new(
                format!("watch/deployment:{id}"),
                format!("deployment {id} is {state}{}", at(url.as_deref())),
            ),
            Self::Draft { pr, url } => Finding::new(
                format!("watch/draft:{pr}"),
                format!(
                    "pull request {pr} is a draft ({url}); mark it ready with gh pr ready {pr}"
                ),
            ),
            Self::MergeState { pr, state, url } => Finding::new(
                format!("watch/merge-state:{state}"),
                format!(
                    "GitHub will not merge pull request {pr} (mergeable_state {state}, {url}): \
                     a branch protection rule, such as a required review or check, is not met"
                ),
            ),
            Self::Gate { check, reason } => {
                Finding::new(format!("watch/gate:{check}"), reason.as_str())
            }
        }
    }
}

/// A pending item's finding.
pub(crate) trait PendingFinding {
    /// The finding that names this pending item when the watch times out.
    fn finding(&self) -> Finding;
}

impl PendingFinding for Pending {
    fn finding(&self) -> Finding {
        match self {
            Self::Check { name, status } => Finding::new(
                format!("watch/pending:check:{name}"),
                format!("check {name:?} is still {status}"),
            ),
            Self::NoChecks => Finding::new(
                "watch/pending:no-checks",
                "no check has reported on the commit yet",
            ),
            Self::Mergeable => Finding::new(
                "watch/pending:mergeable",
                "GitHub has not yet computed whether the pull request merges cleanly",
            ),
            Self::MergeState { state } => Finding::new(
                format!("watch/pending:merge-state:{state}"),
                format!("GitHub still reports the pull request {state}"),
            ),
            Self::Deployment { id, state } => Finding::new(
                format!("watch/pending:deployment:{id}"),
                format!("deployment {id} is still {state}"),
            ),
        }
    }
}

/// ` (<url>)`, or nothing.
fn at(url: Option<&str>) -> String {
    url.map_or_else(String::new, |url| format!(" ({url})"))
}

/// Makes the keys of `findings` distinct, in order: a key already used gets the first free
/// `:<n>` suffix from `:2`. Two blockers can name the same thing (two threads with no comment,
/// a check run and a status of one name), and each finding needs its own key.
pub(crate) fn unique_keys(findings: &mut [Finding]) {
    let mut used: HashSet<String> = HashSet::with_capacity(findings.len());
    for finding in findings {
        if used.contains(&finding.key) {
            let mut n = 2_usize;
            let mut key = format!("{}:{n}", finding.key);
            while used.contains(&key) {
                n = n.saturating_add(1);
                key = format!("{}:{n}", finding.key);
            }
            finding.key = key;
        }
        // The set and the finding each keep the key.
        used.insert(finding.key.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::{SnapshotCode, unique_keys};
    use crate::envelope::Finding;
    use crate::exit_codes::Code;
    use redkiln_github::watch::{Blocker, Snapshot};

    fn check(name: &str) -> Blocker {
        Blocker::Check {
            name: name.to_owned(),
            conclusion: "failure".to_owned(),
            url: None,
        }
    }

    #[test]
    fn the_lowest_code_among_several_blockers_wins() {
        let snapshot = Snapshot {
            head: None,
            blockers: vec![
                Blocker::Gate {
                    check: "no-verdict",
                    reason: "merge-gate: none".to_owned(),
                },
                check("test"),
                Blocker::Thread {
                    url: None,
                    path: None,
                },
            ],
            pending: Vec::new(),
        };

        let code = snapshot.blocker_code().map(Code::value);

        assert_eq!(
            code,
            Some(3),
            "threads (3) before checks (4) and the gate (6)"
        );
    }

    #[test]
    fn a_merge_conflict_wins_over_every_other_blocker_though_its_code_is_highest() {
        let snapshot = Snapshot {
            head: None,
            blockers: vec![
                Blocker::Thread {
                    url: None,
                    path: None,
                },
                check("test"),
                Blocker::Conflict {
                    pr: 7,
                    url: "https://github.com/octo/app/pull/7".to_owned(),
                },
            ],
            pending: Vec::new(),
        };

        let code = snapshot.blocker_code().map(Code::value);

        assert_eq!(code, Some(8), "the conflict (8) before threads (3)");
    }

    #[test]
    fn a_repeated_key_takes_the_first_free_suffix() {
        let mut findings: Vec<Finding> =
            ["watch/check:lint", "watch/check:lint:2", "watch/check:lint"]
                .into_iter()
                .map(|key| Finding::new(key, "m"))
                .collect();

        unique_keys(&mut findings);

        let keys: Vec<&str> = findings
            .iter()
            .map(|finding| finding.key.as_str())
            .collect();
        assert_eq!(
            keys,
            [
                "watch/check:lint",
                "watch/check:lint:2",
                "watch/check:lint:3"
            ]
        );
    }
}
