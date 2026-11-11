//! One golden envelope per output id (SPEC CLI-04), in `tests/fixtures/cli/`: each test that
//! captures an id's fixed scenario checks it with `support::golden`, and this module checks the
//! set is whole, every golden is an envelope of its own id, and some `golden` call names each id.

use std::collections::BTreeSet;
use std::path::Path;

use bench_contract::cli::OUTPUTS;
use serde_json::Value;

use crate::support::conform::{TestResult, golden_path, schema_errors};

#[test]
fn every_output_id_has_a_golden_and_every_golden_an_output_id() -> TestResult {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cli");
    let mut found = BTreeSet::new();
    for entry in std::fs::read_dir(&dir)? {
        found.insert(entry?.path());
    }

    let expected: BTreeSet<_> = OUTPUTS.iter().map(|id| golden_path(id)).collect();

    assert_eq!(
        found, expected,
        "a golden per bench_contract::cli::OUTPUTS id; write a missing one with BENCH_BLESS=1"
    );
    Ok(())
}

/// Every `.rs` file under `dir`, recursively.
fn sources(dir: &Path, found: &mut Vec<std::path::PathBuf>) -> TestResult {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            sources(&path, found)?;
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.push(path);
        }
    }
    Ok(())
}

/// The ids named as the first argument of every `golden(` call in `text`: a string literal.
fn golden_ids(text: &str) -> Vec<&str> {
    // Built, so this file's own source does not hold the call it looks for.
    const CALL: &str = concat!("golden", "(");
    text.split(CALL)
        .skip(1)
        .filter_map(|after| after.trim_start().strip_prefix('"'))
        .filter_map(|literal| literal.split_once('"').map(|(id, _)| id))
        .collect()
}

/// Each id's golden is compared by some test, not only present: `golden` takes the id it
/// expects as a literal and asserts the envelope is that id's, so a call site naming an id
/// compares that id's golden when it runs. A dynamic record of the ids compared would not do
/// under nextest, which runs each test in its own process, in no fixed order.
#[test]
fn every_output_id_is_compared_by_a_golden_call() -> TestResult {
    let mut files = Vec::new();
    sources(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/it"),
        &mut files,
    )?;
    let mut named = BTreeSet::new();
    for file in &files {
        let text = std::fs::read_to_string(file)?;
        named.extend(golden_ids(&text).into_iter().map(str::to_owned));
    }
    // The definition's own `golden(id: …` names no literal, so it adds nothing.

    let expected: BTreeSet<String> = OUTPUTS.iter().map(|id| (*id).to_owned()).collect();

    assert_eq!(
        named, expected,
        "a `golden(\"<id>\", …)` call per bench_contract::cli::OUTPUTS id, and none for another"
    );
    Ok(())
}

#[test]
fn every_golden_is_an_envelope_of_its_own_id() -> TestResult {
    for id in OUTPUTS {
        let path = golden_path(id);
        let golden: Value = serde_json::from_slice(&std::fs::read(&path)?)?;

        let errors = schema_errors(&golden)?;

        assert_eq!(golden["schema"], *id, "{}", path.display());
        assert!(errors.is_empty(), "{}: {errors:?}", path.display());
    }
    Ok(())
}
