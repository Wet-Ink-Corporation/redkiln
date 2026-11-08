//! `weighin-contract`'s decision request against the SPEC's schema,
//! `spec/schemas/decision-request-1.schema.json`: what `Request::check` accepts the schema
//! accepts, and each per-field shape the schema refuses `check` refuses too. Here because weighin
//! already has `jsonschema` as a dev-dependency and the contract with its `core` feature.

use crate::scratch::TestResult;
use redkiln_core::envelope::Timestamp;
use serde::Deserialize as _;
use serde_json::{Value, json};
use std::path::Path;
use weighin_contract::request::Request;

const TS: &str = "2026-11-08T12:00:00Z";

fn schema_errors(record: &Value) -> TestResult<Vec<String>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../spec/schemas/decision-request-1.schema.json");
    let schema: Value = serde_json::from_slice(&std::fs::read(path)?)?;
    let validator = jsonschema::draft202012::new(&schema)?;
    Ok(validator
        .iter_errors(record)
        .map(|e| format!("{}: {e}", e.instance_path().as_str()))
        .collect())
}

/// A request with every optional field set, which passes every guard.
fn full() -> Value {
    json!({
        "question": "Ship the bench baseline?",
        "options": [
            {"id": "a", "label": "Ship", "holds_when": "The run is stable", "cost_if_wrong": "A noisy gate"},
            {"id": "b", "label": "Rerun", "holds_when": "The run was noisy", "cost_if_wrong": "An hour"}
        ],
        "recommendation": "a",
        "door": "two-way",
        "authority": "judgment",
        "blast": "module",
        "default": "b",
        "deadline": "2026-11-09T12:00:00.000Z",
        "subject": {
            "kind": "bench.baseline",
            "method": "git-commit",
            "hash": format!("git-commit:{}", "a".repeat(40)),
            "base": "b".repeat(40),
            "locator": "benches/baseline.json"
        },
        "blocks": "run",
        "trust_needed": "witnessed",
        "raiser": {"namespace": "bench", "key": "baseline/main", "run": "r-1", "session": "s-1"},
        "attachments": [{"kind": "log", "locator": "target/bench.log", "hash": "sha256:0a1b"}]
    })
}

fn checked(record: &Value) -> TestResult<bool> {
    let request = Request::deserialize(record)?;
    Ok(request.check(&Timestamp::parse(TS)?).is_ok())
}

#[test]
fn a_request_check_accepts_passes_the_schema() -> TestResult {
    let record = full();
    assert!(checked(&record)?, "the full request passes every guard");
    assert_eq!(schema_errors(&record)?, Vec::<String>::new());
    Ok(())
}

#[test]
fn each_field_shape_the_schema_refuses_check_refuses() -> TestResult {
    let cases: [(&str, &str, Value); 9] = [
        ("attachments", "", json!([])),
        ("raiser", "run", json!("")),
        ("raiser", "session", json!("")),
        ("subject", "kind", json!("commit")),
        ("subject", "base", json!("abc")),
        ("subject", "locator", json!("")),
        ("subject", "locator", json!("a\u{1}b")),
        ("question", "", json!("")),
        ("attachments", "hash", json!("SHA256:0a")),
    ];
    for (field, inner, value) in cases {
        let mut record = full();
        let slot = match (field, inner) {
            (_, "") => record.get_mut(field),
            ("attachments", _) => record
                .get_mut(field)
                .and_then(|a| a.get_mut(0))
                .and_then(|a| a.get_mut(inner)),
            _ => record.get_mut(field).and_then(|f| f.get_mut(inner)),
        }
        .ok_or("the field is in the full request")?;
        *slot = value;
        assert!(
            !schema_errors(&record)?.is_empty(),
            "{field}.{inner}: the schema refuses it"
        );
        assert!(!checked(&record)?, "{field}.{inner}: check refuses it too");
    }
    Ok(())
}
