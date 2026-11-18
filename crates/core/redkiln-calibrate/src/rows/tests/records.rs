//! Records (CA-02) and v1 rows read alike (CA-04): the upcaster, the mixed golden, what is
//! skipped, and the round trip from a v1 row through a record.

use std::fs;
use std::path::{Path, PathBuf};

use proptest::prelude::{Strategy, any, prop_assert_eq};
use proptest::test_runner::{Config, RngSeed, TestCaseError, TestRunner};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::super::line::{Entry, Line, parse};
use super::super::{LogRows, read};
use crate::log::DecisionLog;
use crate::record::{
    Answer, JUDGEMENT_LABELLED, JUDGEMENT_RECORDED, JudgementRecorded, LabelBy, Mode,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
enum Point {
    D1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Question {
    Noul,
}

/// Bench's hand-computed v1 log (`crates/bench/bench/tests/fixtures/calibrate/decisions/`).
fn bench_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../bench/bench/tests/fixtures/calibrate/decisions")
}

/// `spec/fixtures/calibrate/<name>`.
fn spec_calibrate(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../spec/fixtures/calibrate")
        .join(name)
}

/// The log of producer `bench` under `root`, holding `text` as its live file.
fn bench_log(root: &Path, text: &str) -> std::io::Result<DecisionLog> {
    let log = DecisionLog::new(root.join("calibrate").join("bench"));
    fs::create_dir_all(log.dir())?;
    fs::write(log.path(), text)?;
    Ok(log)
}

/// One record line: an envelope of `event_type` at `version` around `data`.
fn record_line(event_type: &str, version: u32, data: &Value) -> String {
    json!({
        "type": event_type, "version": version, "ts": "2026-11-16T10:00:00Z", "actor": "hook",
        "trust": {"level": "asserted"},
        "writer": {"binary": "bench", "version": "0.1.0", "spec": "1.0.0"},
        "data": data,
    })
    .to_string()
        + "\n"
}

fn answered_data(producer: &str, id: &str) -> Value {
    json!({
        "producer": producer, "id": id, "point": "D1", "question": "noul", "model": "m",
        "mode": "shadow", "probability": 0.7, "asked": false,
    })
}

/// The v1 keys the record's typed fields take (CA-04's table); every other key lands in
/// `detail`.
const TYPED: [&str; 12] = [
    "kind",
    "ts_ms",
    "id",
    "point",
    "question",
    "model",
    "mode",
    "probability",
    "probabilities",
    "top",
    "action",
    "error",
];

#[test]
fn every_v1_fixture_row_upcasts_and_reads_as_before() -> TestResult {
    let mut decisions = 0_u32;
    for name in ["decisions.1.jsonl", "decisions.jsonl"] {
        let text = fs::read_to_string(bench_fixture().join(name))?;
        for raw in text.lines() {
            let Ok(Value::Object(row)) = serde_json::from_str::<Value>(raw) else {
                continue; // the torn line, which `read` counts as skipped
            };
            let line = parse(raw.as_bytes(), Some("bench")).ok_or("the row upcasts")?;
            assert_eq!(line.ms, row.get("ts_ms").and_then(Value::as_u64), "{raw}");
            if let Entry::Judgement(record) = line.entry {
                decisions += 1;
                assert_no_key_lost(&row, &record);
            }
        }
    }
    assert_eq!(decisions, 7, "every whole decision row of the fixture");

    let dir = tempfile::tempdir()?;
    let log = DecisionLog::new(dir.path().join("calibrate").join("bench"));
    fs::create_dir_all(log.dir())?;
    for name in ["decisions.1.jsonl", "decisions.jsonl"] {
        fs::copy(bench_fixture().join(name), log.dir().join(name))?;
    }
    let rows: LogRows<Point, Question> = read([&log])?;
    let noul = rows.unanswered.get(&(Point::D1, Question::Noul)).copied();
    assert_eq!(
        (
            rows.answered.len(),
            noul,
            rows.skipped_rows,
            rows.labels.len()
        ),
        (6, Some(1), 1, 7),
        "the fixture README's counts: the torn line skipped, seven ids labelled"
    );
    Ok(())
}

#[test]
fn a_v1_row_that_does_not_fit_ca_02_is_skipped_and_counted() -> TestResult {
    let decision = |id: &str, point: &str, options: Value| {
        json!({
            "kind": "decision", "id": id, "ts_ms": 1, "point": point, "question": "noul",
            "model": "m", "probability": 0.6, "probabilities": options, "action": "deny",
        })
    };
    let lines = [
        // anneal's real shapes, which fit: a pid-stamped id and a snake_case outcome.
        decision("d1760000000000-123-0", "D1", json!({"yes": 0.6, "no": 0.4})),
        json!({"kind": "label", "id": "d1760000000000-123-0", "outcome": "not_destructive"}),
        decision("has space", "D1", Value::Null),
        decision("d2", &"D".repeat(65), Value::Null),
        json!({"kind": "label", "id": "d2", "outcome": "two words"}),
        // CA-04: an option outside 0..=1 skips the row.
        decision("d3", "D1", json!({"yes": 1.5})),
    ];
    let text = lines
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let dir = tempfile::tempdir()?;
    let log = bench_log(dir.path(), &text)?;

    let rows: LogRows<Point, Question> = read([&log])?;

    let ids: Vec<&str> = rows.answered.iter().map(|a| a.id.as_str()).collect();
    let outcome = rows
        .labels
        .get("d1760000000000-123-0")
        .map(|label| label.outcome.as_str());
    assert_eq!(
        (ids, outcome, rows.labels.len(), rows.skipped_rows),
        (vec!["d1760000000000-123-0"], Some("not_destructive"), 1, 4)
    );
    Ok(())
}

#[test]
fn a_v1_label_with_an_actor_this_build_does_not_know_reads_without_it() -> TestResult {
    let row = json!({
        "kind": "label", "id": "d1", "outcome": "no", "ts_ms": 2,
        "actor": "robot", "via": "carrier-pigeon",
    });

    let line = parse(row.to_string().as_bytes(), Some("bench"));

    let Some(Line {
        entry: Entry::Label(label),
        ..
    }) = line
    else {
        return Err(format!("not a label: {line:?}").into());
    };
    assert_eq!(
        (label.outcome.as_str(), label.by),
        (
            "no",
            Some(LabelBy {
                actor: None,
                via: Some("carrier-pigeon".to_owned())
            })
        )
    );
    Ok(())
}

#[test]
fn a_v1_decision_without_a_text_model_has_the_empty_model_and_keeps_the_value() -> TestResult {
    let row = |model: Value| {
        json!({
            "kind": "decision", "id": "d1", "ts_ms": 1, "point": "D1", "question": "noul",
            "model": model, "probability": 0.5,
        })
        .to_string()
    };
    let judgement = |text: &str| match parse(text.as_bytes(), Some("bench")) {
        Some(Line {
            entry: Entry::Judgement(record),
            ..
        }) => Ok((record.model, record.detail)),
        other => Err(format!("not a judgement: {other:?}")),
    };

    let absent = judgement(&row(Value::Null))?;
    let number = judgement(&row(json!(7)))?;

    let kept: Map<String, Value> = [("model".to_owned(), json!(7))].into_iter().collect();
    assert_eq!(
        (absent, number),
        ((String::new(), None), (String::new(), Some(kept)))
    );
    Ok(())
}

/// Asserts each non-null key of the v1 `row` is in `record`: a typed field or `detail`.
fn assert_no_key_lost(row: &Map<String, Value>, record: &JudgementRecorded) {
    for (key, value) in row.iter().filter(|(_, value)| !value.is_null()) {
        if !TYPED.contains(&key.as_str()) && key != "error_reason" {
            let kept = record.detail.as_ref().and_then(|detail| detail.get(key));
            assert_eq!(kept, Some(value), "{key} kept in detail");
        }
    }
    let text = |key: &str| row.get(key).and_then(Value::as_str);
    assert_eq!(
        (Some(record.id.as_str()), Some(record.point.as_str())),
        (text("id"), text("point"))
    );
    assert_eq!(Some(record.question.as_str()), text("question"));
    assert_eq!(Some(record.model.as_str()), text("model"));
    let mode = if text("mode") == Some("enforce") {
        Mode::Enforce
    } else {
        Mode::Shadow
    };
    assert_eq!(record.mode, mode);
    match &record.answer {
        Answer::Given {
            probability,
            options,
            top,
            action,
            ..
        } => {
            assert_eq!(Some(probability.get()), row["probability"].as_f64());
            let names: Vec<&str> = options.keys().map(String::as_str).collect();
            let v1: Vec<&str> = row["probabilities"]
                .as_object()
                .map(|options| options.keys().map(String::as_str).collect())
                .unwrap_or_default();
            assert_eq!(names, v1);
            assert_eq!(
                (top.as_deref(), action.as_deref()),
                (text("top"), text("action"))
            );
        }
        Answer::Failed { kind, reason } => {
            assert_eq!(
                (Some(kind.as_str()), reason.as_deref()),
                (text("error"), text("error_reason"))
            );
        }
    }
}

#[test]
fn a_mixed_log_reads_records_and_v1_rows_alike() -> TestResult {
    let dir = tempfile::tempdir()?;
    let log = bench_log(
        dir.path(),
        &fs::read_to_string(spec_calibrate("mixed.jsonl"))?,
    )?;

    let rows: LogRows<Point, Question> = read([&log])?;

    let answered: Vec<(&str, f64, bool)> = rows
        .answered
        .iter()
        .map(|a| (a.id.as_str(), a.probability.get(), a.asked))
        .collect();
    assert_eq!(
        answered,
        [
            ("d1763287200000-41-0", 0.8, true),
            ("d1763287201000-41-1", 0.2, false)
        ]
    );
    assert_eq!(
        rows.answered_ms,
        [Some(1_763_287_200_000), Some(1_763_287_201_000)]
    );
    assert_eq!(
        rows.unanswered.get(&(Point::D1, Question::Noul)).copied(),
        Some(2)
    );
    let label = |id: &str| rows.labels.get(id).map(|l| (l.outcome.as_str(), l.via_mod));
    assert_eq!(label("d1763287200000-41-0"), Some(("no", true)), "latest");
    assert_eq!(label("d1763287201000-41-1"), Some(("yes", false)));
    assert_eq!(
        rows.skipped_rows, 4,
        "version 2, an unknown type, another producer, the torn line"
    );
    Ok(())
}

#[test]
fn a_record_of_a_newer_version_or_unknown_type_is_skipped_and_counted() -> TestResult {
    let dir = tempfile::tempdir()?;
    let data = answered_data("bench", "d1");
    let text = record_line(JUDGEMENT_RECORDED, 2, &data)
        + &record_line(JUDGEMENT_RECORDED, 0, &data)
        + &record_line("core.judgement_scored", 1, &data)
        + &record_line(JUDGEMENT_RECORDED, 1, &data);
    let log = bench_log(dir.path(), &text)?;

    let rows: LogRows<Point, Question> = read([&log])?;

    assert_eq!((rows.answered.len(), rows.skipped_rows), (1, 3));
    Ok(())
}

#[test]
fn a_record_of_another_producer_is_skipped_and_counted() -> TestResult {
    let dir = tempfile::tempdir()?;
    let label = json!({"producer": "review", "judgement": "d1", "outcome": "yes"});
    let text = record_line(JUDGEMENT_RECORDED, 1, &answered_data("review", "d1"))
        + &record_line(JUDGEMENT_LABELLED, 1, &label);
    let log = bench_log(dir.path(), &text)?;

    let rows: LogRows<Point, Question> = read([&log])?;

    assert_eq!(
        (rows.answered.len(), rows.labels.len(), rows.skipped_rows),
        (0, 0, 2)
    );
    Ok(())
}

#[test]
fn a_record_line_past_the_row_cap_or_with_a_bad_ts_is_skipped_and_counted() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut padded = answered_data("bench", "d1");
    padded["detail"] = json!({"pad": "x".repeat(8 * 1024)});
    let bad_ts = record_line(JUDGEMENT_RECORDED, 1, &answered_data("bench", "d2"))
        .replace("2026-11-16T10:00:00Z", "2026-11-16 10:00:00");
    let text = record_line(JUDGEMENT_RECORDED, 1, &padded) + &bad_ts;
    let log = bench_log(dir.path(), &text)?;

    let rows: LogRows<Point, Question> = read([&log])?;

    assert_eq!((rows.answered.len(), rows.skipped_rows), (0, 2));
    Ok(())
}

#[test]
fn a_record_line_whose_ts_names_a_day_its_month_lacks_is_skipped_and_counted() -> TestResult {
    let dir = tempfile::tempdir()?;
    let dated = |id: &str, date: &str| {
        record_line(JUDGEMENT_RECORDED, 1, &answered_data("bench", id))
            .replace("2026-11-16T", &format!("{date}T"))
    };
    let text = dated("feb-31", "2026-02-31")
        + &dated("feb-29-common", "2026-02-29")
        + &dated("apr-31", "2026-04-31")
        + &dated("feb-29-century", "2100-02-29")
        + &dated("feb-29-leap", "2028-02-29")
        + &dated("feb-29-400", "2000-02-29")
        + &dated("dec-31", "2026-12-31");
    let log = bench_log(dir.path(), &text)?;

    let rows: LogRows<Point, Question> = read([&log])?;

    let mut read_ids: Vec<&str> = rows.answered.iter().map(|a| a.id.as_str()).collect();
    read_ids.sort_unstable();
    assert_eq!(
        (read_ids, rows.skipped_rows),
        (vec!["dec-31", "feb-29-400", "feb-29-leap"], 4)
    );
    Ok(())
}

#[test]
fn a_v1_row_or_record_line_with_a_repeated_key_is_skipped_and_counted() -> TestResult {
    let dir = tempfile::tempdir()?;
    let label = r#"{"kind": "label", "id": "v1-0", "outcome": "yes", "outcome": "no"}"#;
    let record = record_line(JUDGEMENT_RECORDED, 1, &answered_data("bench", "twice")).replacen(
        "\"version\":1",
        "\"version\":1,\"version\":1",
        1,
    );
    assert!(record.contains("\"version\":1,\"version\":1"), "{record}");
    let text = format!("{label}\n")
        + &record
        + &record_line(JUDGEMENT_RECORDED, 1, &answered_data("bench", "once"));
    let log = bench_log(dir.path(), &text)?;

    let rows: LogRows<Point, Question> = read([&log])?;

    let read_ids: Vec<&str> = rows.answered.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(
        (read_ids, rows.labels.len(), rows.skipped_rows),
        (vec!["once"], 0, 2)
    );
    Ok(())
}

#[test]
fn a_label_record_labels_a_v1_decision_and_the_reverse() -> TestResult {
    let dir = tempfile::tempdir()?;
    let v1 = json!({
        "kind": "decision", "id": "v1-0", "ts_ms": 1, "point": "D1", "question": "noul",
        "probability": 0.4, "action": "defer",
    });
    let label = json!({"producer": "bench", "judgement": "v1-0", "outcome": "no"});
    let text = format!("{v1}\n")
        + &record_line(JUDGEMENT_RECORDED, 1, &answered_data("bench", "rec-0"))
        + &record_line(JUDGEMENT_LABELLED, 1, &label)
        + &format!(
            "{}\n",
            json!({"kind": "label", "id": "rec-0", "outcome": "yes"})
        );
    let log = bench_log(dir.path(), &text)?;

    let rows: LogRows<Point, Question> = read([&log])?;

    let outcome = |id: &str| rows.labels.get(id).map(|l| l.outcome.as_str());
    assert_eq!(
        (outcome("v1-0"), outcome("rec-0"), rows.answered.len()),
        (Some("no"), Some("yes"), 2)
    );
    Ok(())
}

/// `ms` since the epoch as an envelope `ts` (`YYYY-MM-DDTHH:MM:SS.mmmZ`), by Howard Hinnant's
/// civil-from-days.
fn ts_of(ms: u64) -> String {
    let days = i64::try_from(ms / 86_400_000).expect("days fit") + 719_468;
    let of_day = ms % 86_400_000;
    let era = days / 146_097;
    let doe = days - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        of_day / 3_600_000,
        of_day / 60_000 % 60,
        of_day / 1000 % 60,
        of_day % 1000
    )
}

/// The parts of an answered v1 decision row of producer `bench`, drawn by proptest.
type RowParts = (u32, u32, usize, u64, u32, u64, u8, bool, u64, u8);

/// Answered v1 decision rows: probabilities of at most three decimals, any of Bench's actions
/// or none, and any moment up to 2100.
fn v1_rows() -> impl Strategy<Value = Value> {
    (
        0..=1000_u32,
        0..=1000_u32,
        0..6_usize,
        0..(1_u64 << 40),
        0..100_000_u32,
        0..4_102_444_800_000_u64,
        0..3_u8,
        any::<bool>(),
        0..5000_u64,
        any::<u8>(),
    )
        .prop_map(|parts: RowParts| {
            let (yes, no, action, ms, pid, ts_ms, model, enforce, latency, state) = parts;
            let yes = f64::from(yes) / 1000.0;
            let action = ["ask", "ask_human", "defer", "deny", "allow"]
                .get(action)
                .copied();
            json!({
                "kind": "decision",
                "id": format!("d{ms}-{pid}-{}", state % 9),
                "ts_ms": ts_ms,
                "point": "D1", "question": "noul",
                "model": format!("model-{model}"),
                "mode": if enforce { "enforce" } else { "shadow" },
                "probabilities": {"yes": yes, "no": f64::from(no) / 1000.0},
                "top": null, "probability": yes, "action": action,
                "latency_ms": latency, "state_sha256": format!("sha256:{state:02x}"),
            })
        })
}

/// A fixed seed and no regression files, so the property runs the same cases everywhere
/// (TST-10).
fn fixed_runner() -> TestRunner {
    TestRunner::new(Config {
        cases: 256,
        rng_seed: RngSeed::Fixed(0x5EED_CA11_B4A7_E000),
        failure_persistence: None,
        ..Config::default()
    })
}

#[test]
fn upcast_then_serialise_then_read_is_identity_on_answered() -> TestResult {
    fixed_runner().run(&v1_rows(), |row| {
        let upcast = parse(row.to_string().as_bytes(), Some("bench"));
        let Some(Line {
            ms: Some(ms),
            entry: Entry::Judgement(ref record),
        }) = upcast
        else {
            return Err(TestCaseError::fail(format!(
                "not an answered judgement with its time: {row}"
            )));
        };
        let line = json!({
            "type": JUDGEMENT_RECORDED, "version": 1, "ts": ts_of(ms), "actor": "hook",
            "trust": {"level": "asserted"},
            "writer": {"binary": "bench", "version": "0.1.0", "spec": "1.0.0"},
            "data": serde_json::to_value(record).map_err(|e| TestCaseError::fail(e.to_string()))?,
        });

        let read = parse(line.to_string().as_bytes(), Some("bench"));

        prop_assert_eq!(read, upcast, "{}", row);
        Ok(())
    })?;
    Ok(())
}

#[test]
fn ts_of_writes_the_epoch_and_a_known_moment() {
    assert_eq!(ts_of(0), "1970-01-01T00:00:00.000Z");
    assert_eq!(ts_of(1_763_287_200_003), "2025-11-16T10:00:00.003Z");
}
