//! CLI-12's common keys that depend on the store and the repository: events this build cannot
//! use counted by type, with the warning that degrades `health`, and the repository class
//! read from the worktree's suite config when a variable chose the store.

use redkiln_core::store::{NewEvent, Store as CoreStore, Wait};
use serde_json::{Value, json};

use super::edges::json_in;
use super::{Fixture, document};
use crate::test_repo::Setup;

/// Appends raw `data` as an event of `event_type` through Core, as a newer bench would.
fn append_raw(fixture: &Fixture, event_type: &str, data: &str) -> Setup<()> {
    let store = CoreStore::open(&fixture.store_path(), Wait::Command)?;
    let event = NewEvent::new(event_type, data.as_bytes().to_vec())?;
    store.append(vec![event], None)?;
    Ok(())
}

/// An EV-01 envelope of `event_type` at `version` with `data`.
fn envelope_text(event_type: &str, version: u32, data: &str) -> String {
    format!(
        r#"{{"type":"{event_type}","version":{version},"ts":"2026-11-10T09:30:00Z","actor":"cli","trust":{{"level":"asserted"}},"writer":{{"binary":"bench","version":"9.0.0","spec":"1.0.0"}},"data":{data}}}"#
    )
}

#[test]
fn events_this_build_cannot_use_are_counted_by_type_and_degrade_health() -> Setup<()> {
    let fixture = Fixture::new()?;
    fixture.store()?;
    for pr in [7, 8] {
        let text = envelope_text("bench.verdict_set", 2, &format!(r#"{{"pr":{pr}}}"#));
        append_raw(&fixture, "bench.verdict_set", &text)?;
    }

    let (code, envelope) = fixture.json(&["--fields", "gates"])?;

    assert_eq!(code, 0, "a report: {envelope}");
    let mut expected = document(&fixture, Some("1.0"), "degraded", json!({"gates": []}))?;
    expected["unreadable"] = json!({"bench.verdict_set": 2});
    assert_eq!(envelope["data"], expected, "EV-10, by stored type");
    assert_eq!(
        envelope["findings"],
        json!([{
            "key": "ledger/unusable",
            "message": "2 event(s) in the ledger are of a form this bench cannot read, so every \
                        section skips them; a newer bench wrote them",
            "severity": "warning",
        }]),
        "one warning says why health is degraded"
    );
    Ok(())
}

#[test]
fn the_worktree_s_suite_config_gives_the_class_when_a_variable_chose_the_store() -> Setup<()> {
    let fixture = Fixture::new()?;
    fixture.store()?;
    fixture
        .repo
        .write(".redkiln/suite.toml", "[suite]\nrepo_class = \"own\"\n")?;

    let (_, envelope) = fixture.json(&["--fields", "gates"])?;

    assert_eq!(
        (
            &envelope["data"]["store_source"],
            &envelope["data"]["repo_class"]
        ),
        (&Value::from("env"), &Value::from("own")),
        "ST-13's class from suite.toml, though $REDKILN_STORE chose the store: {envelope}"
    );
    Ok(())
}

/// The `status/unreadable:repo_class` findings of `envelope`, each as `[severity, message]`.
fn repo_class_findings(envelope: &Value) -> Vec<(&Value, &Value)> {
    envelope["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|f| f["key"] == "status/unreadable:repo_class")
        .map(|f| (&f["severity"], &f["message"]))
        .collect()
}

#[test]
fn a_class_outside_its_grammar_in_the_worktree_s_config_is_client_and_degrades_health() -> Setup<()>
{
    let fixture = Fixture::new()?;
    fixture.store()?;
    fixture
        .repo
        .write(".redkiln/suite.toml", "[suite]\nrepo_class = \"onw\"\n")?;

    let (code, envelope) = fixture.json(&["--fields", "gates"])?;

    assert_eq!(code, 0, "a report: {envelope}");
    assert_eq!(
        (&envelope["data"]["repo_class"], &envelope["data"]["health"]),
        (&Value::from("client"), &Value::from("degraded")),
        "ST-13's default, and the reader is told: {envelope}"
    );
    let findings = repo_class_findings(&envelope);
    assert!(
        matches!(
            findings.as_slice(),
            [(severity, message)] if *severity == "warning"
                && message.as_str().is_some_and(|m| m.contains("`repo_class` has a value outside its grammar"))
        ),
        "Core's warning, as one finding: {envelope}"
    );
    Ok(())
}

#[test]
fn a_config_locating_could_not_parse_is_client_and_degrades_health() -> Setup<()> {
    let mut fixture = Fixture::new()?;
    // No variable: locating reads the worktree's suite config itself (ST-01 step 4).
    fixture.vars.remove("REDKILN_STORE");
    fixture
        .repo
        .write(".redkiln/suite.toml", "[suite\nrepo_class = \"own\"\n")?;

    let (code, envelope) = fixture.json(&["--fields", "gates"])?;

    assert_eq!(code, 0, "a report: {envelope}");
    assert_eq!(
        (&envelope["data"]["repo_class"], &envelope["data"]["health"]),
        (&Value::from("client"), &Value::from("degraded")),
        "{envelope}"
    );
    let findings = repo_class_findings(&envelope);
    assert!(
        matches!(
            findings.as_slice(),
            [(severity, message)] if *severity == "warning"
                && message.as_str().is_some_and(|m| m.contains("is not used"))
        ),
        "the location's suite notice, as one finding: {envelope}"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "runs git in a subprocess")]
fn git_failing_while_finding_the_class_is_client_and_a_warning() -> Setup<()> {
    let fixture = Fixture::new()?;
    fixture.store()?;
    let broken = tempfile::tempdir()?;
    // A `.git` file git cannot parse: git fails, but not with "not a git repository".
    std::fs::write(broken.path().join(".git"), "not a gitdir line\n")?;

    let (code, envelope) = json_in(&fixture, broken.path(), &["--fields", "gates"])?;

    assert_eq!(code, 0, "a report: {envelope}");
    assert_eq!(
        (&envelope["data"]["repo_class"], &envelope["data"]["health"]),
        (&Value::from("client"), &Value::from("degraded")),
        "{envelope}"
    );
    let findings = repo_class_findings(&envelope);
    assert!(
        matches!(
            findings.as_slice(),
            [(severity, message)] if *severity == "warning"
                && message.as_str().is_some_and(|m| m.contains("invalid gitfile format"))
        ),
        "git's own words: {envelope}"
    );
    Ok(())
}
