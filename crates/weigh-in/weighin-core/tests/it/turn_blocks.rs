//! SPEC ET-08 against its one implementation: `parse_markers` finds exactly the `weigh-in`
//! blocks each end-of-turn case's reading lists (`spec/fixtures/turn/`), so the shared fixtures
//! pin Weigh-In's fence rules: a registered block opens at any indent and runs to the end when
//! unclosed; any other fence opens only at three spaces or fewer, is consumed whole, and is an
//! ordinary line when unclosed.

use std::path::{Path, PathBuf};

use serde_json::Value;
use weighin_core::marker::parse_markers;

use crate::support::TestResult;

/// The SPEC's end-of-turn cases, beside the workspace's crates.
fn turn_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../spec/fixtures/turn")
}

#[test]
fn parse_markers_finds_the_blocks_each_turn_case_names() -> TestResult {
    let mut cases = 0_usize;
    for entry in std::fs::read_dir(turn_dir())? {
        let path = entry?.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let message = std::fs::read_to_string(&path)?;
        let reading: Value =
            serde_json::from_slice(&std::fs::read(path.with_extension("reading.json"))?)?;
        let blocks = reading["blocks"]
            .as_array()
            .ok_or_else(|| format!("{}: blocks is a list", path.display()))?;
        let expected: Vec<&str> = blocks
            .iter()
            .map(|block| {
                assert_eq!(
                    block["name"],
                    "weigh-in",
                    "{}: weigh-in is the one registered block",
                    path.display()
                );
                block["q"].as_str().unwrap_or_default()
            })
            .collect();

        let found = parse_markers(&message);
        let questions: Vec<&str> = found.iter().map(|m| m.question.as_str()).collect();
        assert_eq!(questions, expected, "{}", path.display());
        cases += 1;
    }
    assert!(cases >= 19, "every case was read, found {cases}");
    Ok(())
}
