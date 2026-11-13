//! The `clef` section's finding when Clef's configuration or the answered-decision counts
//! cannot be read: its exact wording, pinned so the shared reader (`decide::modes`, #158)
//! words it as the section's own reader did.

use crate::harness::claude_code::MODE_VAR;
use serde_json::{Value, json};

use super::{Fixture, document};
use crate::test_repo::Setup;

/// The one finding `bench status --json --fields clef` gives, with `clef` absent.
fn clef_finding(fixture: &Fixture) -> Setup<Value> {
    fixture.store()?;
    let (code, envelope) = fixture.json(&["--fields", "clef"])?;
    assert_eq!(code, 0, "an unreadable section does not fail the command");
    assert_eq!(
        envelope["data"],
        document(fixture, Some("1.0"), "degraded", json!({}))?,
        "clef is null, and the warning degrades health"
    );
    let findings = envelope["findings"].as_array().ok_or("findings")?;
    assert_eq!(findings.len(), 1, "{envelope}");
    Ok(findings.first().cloned().unwrap_or(Value::Null))
}

#[test]
fn a_clef_mode_that_does_not_parse_is_named_in_the_clef_finding() -> Setup<()> {
    let mut fixture = Fixture::new()?;
    fixture.vars.insert(MODE_VAR, "sometimes".into());

    let finding = clef_finding(&fixture)?;

    assert_eq!(finding["key"], json!("status/unreadable:clef"));
    assert_eq!(
        finding["message"],
        json!(format!(
            "clef could not be read: {MODE_VAR} \"sometimes\" is not \
             `<off|shadow|enforce>[,<D1..D7>=<off|shadow|enforce>]...`"
        ))
    );
    Ok(())
}

#[test]
fn corrupt_decision_counts_are_named_in_the_clef_finding() -> Setup<()> {
    let fixture = Fixture::new()?;
    let counts = fixture.home().join("decisions").join("counts.json");
    std::fs::create_dir_all(counts.parent().ok_or("a parent")?)?;
    std::fs::write(&counts, "not json")?;

    let finding = clef_finding(&fixture)?;

    assert_eq!(finding["key"], json!("status/unreadable:clef"));
    assert_eq!(
        finding["message"],
        json!(format!(
            "clef could not be read: {} is not a decision counts file: \
             expected ident at line 1 column 2",
            counts.display()
        ))
    );
    Ok(())
}
