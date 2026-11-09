//! weighin's real `--json` output against the SPEC's CLI envelope schema,
//! `spec/schemas/cli-envelope-1.schema.json` (CLI-01 to CLI-05; Move 1 step 3, D2 C1): a success
//! and an exit-70 internal error, each one line whose `ok` agrees with the exit code.

use crate::scratch::{Repo, TestResult};
use redkiln_core::envelope::Namespace;
use serde_json::Value;
use std::path::Path;
use std::process::Output;

/// The SPEC's CLI envelope schema.
fn cli_envelope_schema() -> TestResult<Value> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../spec/schemas/cli-envelope-1.schema.json");
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}

/// Asserts `out`'s stdout is one envelope line passing the schema, with `ok` true exactly when
/// the exit code is 0 and an output id in Core's namespace registry; the envelope.
fn conforms(out: &Output) -> TestResult<Value> {
    let text = std::str::from_utf8(&out.stdout)?;
    assert_eq!(text.matches('\n').count(), 1, "CLI-01, one line: {text}");
    assert!(text.ends_with('\n'), "ended by a newline: {text}");
    let envelope: Value = serde_json::from_str(text)?;
    let validator = jsonschema::draft202012::new(&cli_envelope_schema()?)?;
    let errors: Vec<String> = validator
        .iter_errors(&envelope)
        .map(|e| format!("{}: {e}", e.instance_path().as_str()))
        .collect();
    assert!(
        errors.is_empty(),
        "{text} breaks cli-envelope-1: {errors:?}"
    );
    assert_eq!(
        envelope.get("ok").and_then(Value::as_bool),
        Some(out.status.code() == Some(0)),
        "CLI-02: ok agrees with the exit code"
    );
    let id = envelope
        .get("schema")
        .and_then(Value::as_str)
        .ok_or("an output id")?;
    let namespace = id.split_once('.').map_or(id, |(ns, _)| ns);
    assert!(namespace.parse::<Namespace>().is_ok(), "{id}");
    Ok(envelope)
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn journal_success_envelope_conforms() -> TestResult {
    let (repo, _remote) = Repo::with_remote(true)?;
    repo.raise("s1", "Keep retention per series?")?;

    let out = repo.run("s1", &["journal", "checkpoint", "--json"], None)?;

    assert_eq!(out.status.code(), Some(0));
    let envelope = conforms(&out)?;
    assert_eq!(envelope["schema"], "weighin.journal-checkpoint/1");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn internal_error_envelope_conforms() -> TestResult {
    let repo = Repo::new(true)?;
    let bogus = repo.sibling("bogus")?.join("events.sqlite3");
    std::fs::write(&bogus, "not a database")?;
    let store = bogus.to_string_lossy().into_owned();

    let out = repo.run(
        "s1",
        &["--store", &store, "journal", "checkpoint", "--json"],
        None,
    )?;

    assert_eq!(out.status.code(), Some(70), "CLI-05: internal error");
    let envelope = conforms(&out)?;
    assert_eq!(envelope["findings"][0]["severity"], "error");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn status_envelope_conforms() -> TestResult {
    let repo = Repo::new(true)?;

    let out = repo.run("s1", &["status", "--json"], None)?;

    assert_eq!(out.status.code(), Some(0), "CLI-06: a report exits 0");
    let envelope = conforms(&out)?;
    assert_eq!(envelope["schema"], "weighin.status/1");
    Ok(())
}
