//! One golden envelope per output id (SPEC CLI-04), in `tests/fixtures/cli/`: each test that
//! captures an id's fixed scenario checks it with `support::golden`, and this module checks the
//! set is whole and every golden is an envelope of its own id.

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
