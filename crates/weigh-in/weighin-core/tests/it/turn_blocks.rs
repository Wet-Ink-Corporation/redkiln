//! SPEC ET-08 against its one implementation: `parse_markers` finds exactly the `weigh-in`
//! blocks each end-of-turn case's reading lists (`spec/fixtures/turn/`), so the shared fixtures
//! pin Weigh-In's fence rules (indent, unclosed fences, other fences consumed whole).

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
        assert!(
            blocks.iter().all(|name| name == "weigh-in"),
            "{}: weigh-in is the one registered block",
            path.display()
        );

        assert_eq!(
            parse_markers(&message).len(),
            blocks.len(),
            "{}",
            path.display()
        );
        cases += 1;
    }
    assert!(cases >= 17, "every case was read, found {cases}");
    Ok(())
}
