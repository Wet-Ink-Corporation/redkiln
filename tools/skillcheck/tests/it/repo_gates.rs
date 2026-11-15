//! The repository's own skills against the Tier 0, 1 and 3 merge bars (`evals/bench/README.md`).
//! CI runs these tests, so they are what enforce the bars there. Moved from Bench's
//! `tests/it/repo_gates.rs` (Move 2 step 5, PR 5.1b); the debug-leftovers test stays in Bench.

use std::error::Error;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use skillcheck::eval::routing::{RoutingArgs, evaluate};
use skillcheck::exit_codes;
use skillcheck::lint::SkillsArgs;
use skillcheck::lint::finding::Severity;
use skillcheck::profile::DEFAULT_PLUGIN;

use crate::support::lint;

type TestResult = Result<(), Box<dyn Error>>;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn plugin_skills() -> PathBuf {
    repo().join("plugins/bench/skills")
}

#[test]
fn the_plugin_skills_route_at_100_percent_rank_1() -> TestResult {
    let outcome = evaluate(&RoutingArgs {
        skills: plugin_skills(),
        fixtures: repo().join("evals/bench/routing"),
    })?;

    let mut report = outcome.confusion().render().join("\n");
    for problem in &outcome.problems {
        write!(report, "\nproblem: {problem:?}")?;
    }
    for miss in outcome.misses() {
        write!(
            report,
            "\nmiss: {}:{} expected {} got {:?}: {:?}",
            miss.file, miss.line, miss.expected, miss.top, miss.prompt
        )?;
    }
    assert_eq!(
        outcome.code(),
        exit_codes::OK,
        "Tier 1 needs every fixture valid and 100% rank-1 (run `skillcheck eval routing`):\n{report}"
    );
    Ok(())
}

#[test]
fn the_plugin_skills_lint_clean_and_the_snapshot_replays_the_fresh_result() -> TestResult {
    let work = tempfile::tempdir()?;
    let fresh_args = SkillsArgs {
        paths: vec![plugin_skills()],
        index: None,
        hooks: None,
        cache: None,
        plugin: DEFAULT_PLUGIN.to_owned(),
    };
    let cached_args = SkillsArgs {
        cache: Some(work.path().join("lint-cache.json")),
        ..fresh_args.clone()
    };

    let fresh = lint(&fresh_args)?;
    let first = lint(&cached_args)?;
    let replayed = lint(&cached_args)?;

    let errors: Vec<String> = fresh
        .findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .map(|f| format!("{}: {}", f.at, f.message))
        .collect();
    assert_eq!(errors, Vec::<String>::new(), "Tier 0: zero lint errors");
    assert_eq!(
        first.findings, fresh.findings,
        "a snapshot run reports the fresh result"
    );
    assert_eq!(
        replayed.findings, fresh.findings,
        "a replayed run reports the fresh result"
    );
    assert_eq!(
        replayed.cached, fresh.skills,
        "every unchanged skill is replayed"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "runs the binary")]
fn every_entry_skill_has_valid_regression_and_capability_suites() -> TestResult {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_skillcheck"))
        .args(["eval", "uplift", "--check", "--skills"])
        .arg(plugin_skills())
        .arg("--suites")
        .arg(repo().join("evals/bench/uplift"))
        .output()?;
    let envelope: serde_json::Value = crate::support::envelope(&output)?;

    assert_eq!(
        output.status.code(),
        Some(0),
        "Tier 3 suites must all be valid (run `skillcheck eval uplift --check`): {}",
        envelope["findings"]
    );
    let thin: Vec<String> = [
        "grill", "spec", "plan", "build", "diagnose", "verify", "ship",
    ]
    .into_iter()
    .filter(|skill| {
        !envelope["data"]["coverage"].as_array().is_some_and(|rows| {
            rows.iter().any(|row| {
                row["skill"] == *skill
                    && row["regression"].as_u64().is_some_and(|n| n >= 1)
                    && row["capability"].as_u64().is_some_and(|n| n >= 1)
            })
        })
    })
    .map(str::to_owned)
    .collect();
    assert_eq!(
        thin,
        Vec::<String>::new(),
        "each entry skill (#41) needs a regression and a capability case: {}",
        envelope["data"]["coverage"]
    );
    Ok(())
}
