//! `lint skills --index` against Bench's committed bootstrap index (rule 4).

use std::error::Error;
use std::path::Path;
use std::process::Command;

use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
#[cfg_attr(miri, ignore = "runs the binary")]
fn lint_skills_accepts_the_committed_index() -> TestResult {
    // `lint skills` runs its index rule only when it lints at least one skill, and
    // plugin/skills has none yet, so lint a passing fixture skill with the shipped index.
    let (code, flagged) = lint_index("docs/bench/bootstrap-index.md")?;
    assert_eq!(code, Some(0), "lint skills exits 0");
    assert!(!flagged, "no bootstrap-index finding for the shipped index");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "runs the binary")]
fn lint_skills_rejects_an_index_over_budget_in_the_same_setup() -> TestResult {
    // The control for the test above: the same setup does run the index rule.
    let (code, flagged) = lint_index(
        "tools/skillcheck/tests/fixtures/lint-skills/rule-04-bootstrap-index/fail/index.md",
    )?;
    assert_eq!(code, Some(1), "an error finding exits 1");
    assert!(flagged, "the oversized index is flagged");
    Ok(())
}

/// Runs `skillcheck lint skills <passing fixture skills> --index <index>` from the repository
/// root: the exit code, and whether a `bootstrap-index` finding was reported.
fn lint_index(index: &str) -> Result<(Option<i32>, bool), Box<dyn Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let skills = "tools/skillcheck/tests/fixtures/lint-skills/rule-04-bootstrap-index/pass/skills";
    let output = Command::new(env!("CARGO_BIN_EXE_skillcheck"))
        .current_dir(&root)
        .args(["lint", "skills", skills, "--index", index])
        .output()?;
    let envelope: Value = crate::support::envelope(&output)?;
    let flagged = envelope
        .pointer("/data/details")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|detail| detail.get("checker") == Some(&Value::from("bootstrap-index")));
    Ok((output.status.code(), flagged))
}
