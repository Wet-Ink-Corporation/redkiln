use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::{
    ClefMode, DecisionPoint, Fresh, Health, Level, Outcome, PlanStatus, RepoClass, SCHEMA, Status,
    StoreSource, WindowSource,
};
use crate::event::Mode;

const OK: &str = include_str!("../../../../../spec/fixtures/status/valid/bench-ok.json");

/// The golden envelope.
fn envelope() -> Value {
    serde_json::from_str(OK).expect("the golden is JSON")
}

/// The golden envelope's `data`, read through the contract.
fn read() -> Status {
    serde_json::from_value(envelope()["data"].take()).expect("the golden reads")
}

/// The bytes of the golden's `data`, as written: between `"data":` and `,"findings":`.
fn data_bytes() -> &'static str {
    let (_, data) = OK.split_once("\"data\":").expect("a data key");
    let (data, _) = data.split_once(",\"findings\":").expect("a findings key");
    data
}

/// The golden's status with every section and nested option `None`: no store yet.
fn bare() -> Status {
    Status {
        store_schema: None,
        unreadable: BTreeMap::new(),
        phase: None,
        plan: None,
        tasks: None,
        gates: None,
        verdict: None,
        clef: None,
        context: None,
        ledger_tail: None,
        ..read()
    }
}

#[test]
fn the_golden_reads_and_writes_back_byte_for_byte() {
    let status = read();

    assert_eq!(
        serde_json::to_string(&status).expect("encoded"),
        data_bytes(),
        "field order is wire order"
    );
    assert_eq!(
        envelope()["schema"],
        json!(SCHEMA),
        "the golden's output id"
    );
}

#[test]
fn the_golden_s_values_read_as_their_types() {
    let status = read();

    assert_eq!(
        (
            status.plugin.as_str(),
            status.store_source,
            status.repo_class,
            status.health,
            status.dropped,
        ),
        (
            crate::plugin::NAME,
            Some(StoreSource::GitCommonDir),
            RepoClass::Own,
            Health::Ok,
            0
        ),
        "CLI-12's keys"
    );
    let phase = status.phase.expect("a phase");
    assert_eq!(phase.mode, Mode::Build, "the mode");
    let plan = status.plan.expect("a plan");
    assert_eq!(plan.status, PlanStatus::Approved, "the plan");
    let verdict = status.verdict.expect("a verdict");
    assert_eq!(
        (verdict.outcome, verdict.verdict.map(|fresh| fresh.fresh)),
        (Outcome::Valid, Some(Fresh::BySha)),
        "the verdict"
    );
    let clef = status.clef.expect("a clef");
    let first = clef.points.first().expect("D1");
    assert_eq!(
        (first.point, first.configured_mode, first.mode),
        (DecisionPoint::D1, ClefMode::Enforce, ClefMode::Shadow),
        "D1"
    );
    let context = status.context.expect("a context");
    assert_eq!(
        (context.window_source, context.level),
        (WindowSource::Model, Some(Level::Warn)),
        "the context"
    );
}

#[test]
fn a_later_key_is_accepted_at_the_top_and_in_a_section() {
    let mut data = envelope()["data"].take();
    data["added_in_a_later_build"] = json!({"any": [1, 2]});
    data["phase"]["added_too"] = json!(true);

    let status: Status = serde_json::from_value(data).expect("CLI-04: additive");

    assert_eq!(status, read(), "the later keys are ignored");
}

#[test]
fn every_option_is_written_as_null() {
    let written = serde_json::to_value(bare()).expect("encoded");

    let nulls: Vec<&str> = [
        "store_schema",
        "phase",
        "plan",
        "tasks",
        "gates",
        "verdict",
        "clef",
        "context",
        "ledger_tail",
    ]
    .into_iter()
    .filter(|key| written.get(key) == Some(&Value::Null))
    .collect();
    assert_eq!(
        nulls,
        [
            "store_schema",
            "phase",
            "plan",
            "tasks",
            "gates",
            "verdict",
            "clef",
            "context",
            "ledger_tail"
        ],
        "never omitted: {written}"
    );
}

#[test]
fn nested_options_are_written_as_null() {
    let mut status = read();
    let phase = status.phase.as_mut().expect("a phase");
    phase.plan = None;
    phase.tree = None;
    phase.clean = None;
    status.plan.as_mut().expect("a plan").approved = None;
    status.tasks.as_mut().expect("tasks").next = None;
    let verdict = status.verdict.as_mut().expect("a verdict");
    verdict.base = None;
    verdict.verdict = None;
    let context = status.context.as_mut().expect("a context");
    context.used_tokens = None;
    context.percent = None;
    context.level = None;

    let written = serde_json::to_value(&status).expect("encoded");

    assert_eq!(
        [
            &written["phase"]["plan"],
            &written["phase"]["tree"],
            &written["phase"]["clean"],
            &written["plan"]["approved"],
            &written["tasks"]["next"],
            &written["verdict"]["base"],
            &written["verdict"]["verdict"],
            &written["context"]["used_tokens"],
            &written["context"]["percent"],
            &written["context"]["level"],
        ],
        [&Value::Null; 10],
        "each key present and null: {written}"
    );
    let back: Status = serde_json::from_value(written).expect("reads back");
    assert_eq!(back, status, "null reads as None");
}

#[test]
fn enums_spell_as_the_schema_and_the_sections_say() {
    let spelled = |v: Value| v.as_str().map(str::to_owned);
    let health = [Health::Ok, Health::Degraded, Health::Off]
        .map(|h| spelled(serde_json::to_value(h).expect("encoded")));
    let sources = [
        StoreSource::Flag,
        StoreSource::Env,
        StoreSource::Config,
        StoreSource::GitCommonDir,
        StoreSource::Folder,
    ]
    .map(|s| spelled(serde_json::to_value(s).expect("encoded")));
    let outcomes = Outcome::ALL.map(|o| spelled(serde_json::to_value(o).expect("encoded")));
    let points = DecisionPoint::ALL.map(|p| spelled(serde_json::to_value(p).expect("encoded")));

    assert_eq!(
        health,
        ["ok", "degraded", "off"].map(|s| Some(s.to_owned())),
        "health"
    );
    assert_eq!(
        sources,
        ["flag", "env", "config", "git_common_dir", "folder"].map(|s| Some(s.to_owned())),
        "store_source"
    );
    assert_eq!(
        outcomes,
        Outcome::ALL.map(|o| Some(o.as_str().to_owned())),
        "outcome: serde and as_str agree"
    );
    assert_eq!(
        points,
        DecisionPoint::ALL.map(|p| Some(p.as_str().to_owned())),
        "point: serde and as_str agree"
    );
}

#[test]
fn plan_status_and_clef_mode_spell_as_their_as_str() {
    let spelled = |v: Value| v.as_str().map(str::to_owned);
    let statuses = PlanStatus::ALL.map(|s| spelled(serde_json::to_value(s).expect("encoded")));
    let modes = ClefMode::ALL.map(|m| spelled(serde_json::to_value(m).expect("encoded")));

    assert_eq!(
        statuses,
        PlanStatus::ALL.map(|s| Some(s.as_str().to_owned())),
        "plan status"
    );
    assert_eq!(
        modes,
        ClefMode::ALL.map(|m| Some(m.as_str().to_owned())),
        "clef mode"
    );
}

#[test]
fn another_shape_is_refused() {
    let refused = serde_json::from_value::<Status>(json!({"plugin": "rk-bench"}));

    assert!(
        refused.is_err_and(|e| e.to_string().contains("missing field")),
        "a missing key is refused"
    );
}
