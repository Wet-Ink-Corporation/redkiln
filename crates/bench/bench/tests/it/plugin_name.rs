//! Pins `bench_contract::plugin::NAME`, the name Claude Code namespaces the plugin's skills and
//! commands under (`rk-bench:build`, `/rk-bench:handoff`), against the plugin's manifest, so the
//! route hook's outcome labels and the bootstrap index's commands follow the install id.

use std::error::Error;
use std::fs;
use std::path::Path;

use serde_json::Value;

#[test]
fn the_contract_names_the_plugin_as_its_manifest_does() -> Result<(), Box<dyn Error>> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../plugins/bench/.claude-plugin/plugin.json");
    let json: Value = serde_json::from_str(&fs::read_to_string(manifest)?)?;

    assert_eq!(json["name"], Value::from(bench_contract::plugin::NAME));
    assert_eq!(bench_contract::plugin::NAME, "rk-bench", "the install id");
    Ok(())
}
