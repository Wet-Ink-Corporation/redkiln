//! `bench status` through the binary, in a scratch directory outside git with the ledger at
//! `$REDKILN_STORE` there: the envelope `bench.status/1`, its `data` against the SPEC's
//! `status-1` (CLI-12) and read through `bench_contract::status`.

use std::collections::BTreeMap;
use std::error::Error;
use std::path::Path;
use std::process::Output;

use bench_contract::status::{self, Health, RepoClass, Status, StoreSource};
use serde::Deserialize as _;
use serde_json::{Value, json};

use crate::support::conform::status_errors;
use crate::support::{anneal_in, envelope, golden};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const STORE: &str = "ledger.sqlite3";

/// `output`'s envelope, its `data` passing `status-1`, read through the contract.
fn report(output: &Output) -> TestResult<(Value, Status)> {
    let envelope: Value = envelope(output)?;
    assert_eq!(
        envelope.get("schema"),
        Some(&json!(status::SCHEMA)),
        "CLI-12's output id"
    );
    let data = envelope.get("data").ok_or("a data key")?;
    let errors = status_errors(data)?;
    assert!(errors.is_empty(), "{envelope} breaks status-1: {errors:?}");
    // The contract reads its own copy; the envelope is returned whole for the golden.
    let data = Status::deserialize(data)?;
    Ok((envelope, data))
}

/// The status `bench status` reports in `work`, outside git, with the store `$REDKILN_STORE`
/// names there: CLI-12's keys as Core located it, `store_schema` as given and every section
/// `None`.
fn located_in(work: &Path, store_schema: Option<&str>) -> Status {
    Status {
        plugin: bench_contract::plugin::NAME.to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        spec: redkiln_core::envelope::SPEC_VERSION.to_owned(),
        store: Some(work.join(STORE).to_string_lossy().replace('\\', "/")),
        store_source: Some(StoreSource::Env),
        store_schema: store_schema.map(str::to_owned),
        repo_class: RepoClass::Client,
        unreadable: BTreeMap::new(),
        dropped: 0,
        health: Health::Ok,
        phase: None,
        plan: None,
        tasks: None,
        gates: None,
        verdict: None,
        clef: None,
        context: None,
        ledger_tail: None,
    }
}

#[test]
#[cfg_attr(miri, ignore = "runs the binary")]
fn without_a_ledger_status_exits_0_and_creates_none() -> TestResult {
    let work = tempfile::tempdir()?;

    let output = anneal_in(work.path())
        .args(["status", "--json"])
        .env("REDKILN_STORE", STORE)
        .output()?;

    assert_eq!(
        output.status.code(),
        Some(0),
        "no ledger: a report still exits 0"
    );
    let (envelope, data) = report(&output)?;
    assert_eq!(envelope["ok"], json!(true), "ok agrees with the exit code");
    assert_eq!(
        data,
        located_in(work.path(), None),
        "where the store would be, no store schema, no section: {envelope}"
    );
    assert!(
        output.stderr.is_empty(),
        "no `bench: error:` line: it is an outcome"
    );
    assert!(!work.path().join(STORE).exists(), "no ledger was created");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "runs the binary")]
fn with_a_ledger_status_exits_0_and_lists_its_events() -> TestResult {
    let work = tempfile::tempdir()?;
    let appended = anneal_in(work.path())
        .args([
            "ledger",
            "append",
            r#"{"type":"bench.task_done","data":{"task":"T1"}}"#,
        ])
        .env("REDKILN_STORE", STORE)
        .output()?;
    assert!(appended.status.success(), "the event was appended");
    let tail = anneal_in(work.path())
        .args(["ledger", "tail", "-n", "1"])
        .env("REDKILN_STORE", STORE)
        .output()?;
    golden(
        "bench.ledger-append/1",
        &envelope(&appended)?,
        &[work.path()],
        // The writer's version is the build's, which moves with every release.
        &["/data/event/writer/version"],
    )?;
    golden(
        "bench.ledger-tail/1",
        &envelope(&tail)?,
        &[work.path()],
        &["/data/events/*/event/writer/version"],
    )?;

    let output = anneal_in(work.path())
        .args(["status", "--json", "--fields", "ledger_tail"])
        .env("REDKILN_STORE", STORE)
        .output()?;

    assert_eq!(output.status.code(), Some(0), "a ledger exists");
    let (envelope, mut data) = report(&output)?;
    // The version is the build's, which moves with every release.
    golden(
        "bench.status/1",
        &envelope,
        &[work.path()],
        &["/data/version"],
    )?;
    assert_eq!(envelope["command"], json!("status"), "the command");
    let tail = data.ledger_tail.take().ok_or("a ledger_tail")?;
    assert_eq!(
        data,
        located_in(work.path(), Some("1.0")),
        "the store's schema, and only the section asked for: {envelope}"
    );
    assert_eq!(
        tail.iter()
            .map(|event| event.summary.as_str())
            .collect::<Vec<_>>(),
        ["task_done task:T1"],
        "the one event"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "runs the binary")]
fn an_unknown_section_is_a_usage_error() -> TestResult {
    let work = tempfile::tempdir()?;

    let output = anneal_in(work.path())
        .args(["status", "--fields", "phase,nope"])
        .output()?;

    assert_eq!(output.status.code(), Some(2), "a usage error");
    assert!(output.stdout.is_empty(), "nothing on stdout");
    Ok(())
}
