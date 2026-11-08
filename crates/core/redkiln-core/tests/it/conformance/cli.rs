//! CLI envelope goldens (`spec/fixtures/cli/`, CLI-01 to CLI-03). Core has no CLI envelope type:
//! the schema checks the shape (`schema.rs`), weighin's integration tests check its real output
//! against the same schema, and here each output id is tied to Core's namespace registry.

use redkiln_core::envelope::Namespace;

use super::{one_line, value_of};

const VALID: [&str; 3] = [
    "cli/valid/gate-failed.json",
    "cli/valid/ok-with-warning.json",
    "cli/valid/internal-error.json",
];

/// The namespace an output id names: the text before its first dot (CLI-02, EV-03).
fn namespace_of(rel: &str) -> String {
    let doc = value_of(&one_line(rel));
    let id = doc["schema"].as_str().expect("an output id");
    id.split_once('.').map_or(id, |(ns, _)| ns).to_owned()
}

#[test]
fn golden_schema_ids_name_registered_namespaces() {
    for rel in VALID {
        let namespace = namespace_of(rel);
        assert!(namespace.parse::<Namespace>().is_ok(), "{rel}: {namespace}");
    }
    let unregistered = namespace_of("cli/invalid/namespace-unregistered.json");
    assert!(unregistered.parse::<Namespace>().is_err(), "{unregistered}");
}

#[test]
fn goldens_are_one_line() {
    for rel in VALID {
        // `one_line` asserts a single line ended by `\n` (CLI-01).
        assert!(value_of(&one_line(rel)).is_object(), "{rel} is one object");
    }
}
