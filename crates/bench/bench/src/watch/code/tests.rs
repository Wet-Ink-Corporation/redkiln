//! The watch engine tests that assert Bench's exit code: a review-thread read whose two routes
//! both fail, and a refused empty or dot ref.

use super::WatchCode;
use crate::watch::fake::FakeGh;
use redkiln_github::watch::unresolved_threads as unresolved;
use redkiln_github::watch::{CommitWatch, RepoRef, Source, WatchError};

const PR_URL: &str = "https://github.com/octo/app/pull/7";
const REFUSED: &str = "gh: GitHub GraphQL is not available from Claude Code sessions; use the \
                       REST API (HTTP 403)";

#[test]
fn when_both_fail_the_error_line_names_both_failures() {
    let gh = FakeGh::new()
        .failing("graphql", REFUSED)
        .missing("/ccr/review_threads");

    let error = unresolved(&gh, 7, PR_URL).expect_err("both fail");

    assert_eq!(
        crate::error_chain::reason(&error),
        format!(
            "gh api graphql failed: {REFUSED}; the review thread route of Claude Code's proxy \
             failed too: gh api repos/{{owner}}/{{repo}}/pulls/7/ccr/review_threads: not found"
        )
    );
    assert_eq!(error.code().value(), 7);
}

#[test]
fn an_empty_or_dot_ref_is_refused_before_anything_is_read() {
    for git_ref in ["", ".", ".."] {
        let gh = FakeGh::new();

        let result = CommitWatch {
            gh: &gh,
            repo: RepoRef::Current,
            git_ref,
        }
        .poll();

        assert!(
            matches!(&result, Err(WatchError::BadRef { git_ref: refused }) if refused == git_ref),
            "{git_ref:?}: {result:?}"
        );
        assert!(gh.calls.borrow().is_empty(), "{git_ref:?} reached gh");
        assert_eq!(
            result.map_err(|error| error.code().value()).err(),
            Some(7),
            "a query error"
        );
    }
}
