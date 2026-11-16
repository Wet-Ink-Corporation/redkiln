use std::fs;
use std::path::Path;

use serde::Deserialize;
use serde_json::{Value, json};

use super::{HEADROOM_ROWS, LogRows, MAX_LOG_FILE_BYTES, ROW_BYTES, ReadError, VIA_MOD, read};
use crate::log::{DecisionLog, MAX_FILE_BYTES, MAX_ROW_BYTES};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A caller's closed set of points: a row naming another is skipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
enum Point {
    D1,
}

fn decision(id: &str, point: &str, probability: Option<f64>) -> Value {
    json!({
        "kind": "decision", "id": id, "ts_ms": 7, "point": point, "question": "noul",
        "probabilities": probability.map(|p| json!({"yes": p, "no": 1.0 - p})),
        "top": null, "probability": probability, "action": "defer",
    })
}

fn write_rows(path: &Path, rows: &[Value]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(
        path,
        rows.iter()
            .map(|row| row.to_string() + "\n")
            .collect::<String>(),
    )
}

#[test]
fn the_old_log_is_read_first_so_a_newer_label_matches_an_older_decision() -> TestResult {
    let dir = tempfile::tempdir()?;
    let old = DecisionLog::in_home(&dir.path().join("anneal"));
    let new = DecisionLog::new(dir.path().join("calibrate").join("bench"));
    write_rows(
        &old.path(),
        &[
            decision("d1-1-0", "D1", Some(0.25)),
            json!({"kind": "label", "id": "d1-1-0", "outcome": "no"}),
        ],
    )?;
    write_rows(
        &new.path(),
        &[json!({"kind": "label", "id": "d1-1-0", "outcome": "yes", "via": VIA_MOD})],
    )?;

    let rows: LogRows<Point, String> = read([&old, &new])?;

    assert_eq!(rows.files, [old.path(), new.path()]);
    assert_eq!(
        (
            rows.answered
                .iter()
                .map(|a| a.id.as_str())
                .collect::<Vec<_>>(),
            rows.answered_ms.as_slice(),
        ),
        (vec!["d1-1-0"], [Some(7)].as_slice())
    );
    let latest = rows.labels.get("d1-1-0").expect("labelled");
    assert_eq!(
        (latest.outcome.as_str(), latest.via_mod),
        ("yes", true),
        "the new log's label is the latest"
    );
    Ok(())
}

#[test]
fn a_row_whose_point_the_caller_does_not_know_is_skipped() -> TestResult {
    let dir = tempfile::tempdir()?;
    let log = DecisionLog::new(dir.path().to_path_buf());
    write_rows(
        &log.path(),
        &[
            decision("d9-1-0", "D9", Some(0.5)),
            decision("d1-1-1", "D1", None),
        ],
    )?;

    let rows: LogRows<Point, String> = read([&log])?;

    assert_eq!(
        (
            rows.answered.len(),
            rows.unanswered.get(&(Point::D1, "noul".to_owned())),
            rows.skipped_rows
        ),
        (0, Some(&1), 1)
    );
    Ok(())
}

#[test]
fn a_file_past_the_headroom_is_refused_naming_it() -> TestResult {
    let dir = tempfile::tempdir()?;
    let log = DecisionLog::new(dir.path().to_path_buf());
    fs::write(
        log.path(),
        vec![b'\n'; usize::try_from(MAX_LOG_FILE_BYTES + 1)?],
    )?;

    let result: Result<LogRows<Point, String>, _> = read([&log]);

    assert!(
        matches!(&result, Err(ReadError::TooLarge { path, cap })
            if *path == log.path() && *cap == MAX_LOG_FILE_BYTES),
        "{result:?}"
    );
    Ok(())
}

#[test]
fn a_directory_at_a_log_file_is_refused() -> TestResult {
    let dir = tempfile::tempdir()?;
    let log = DecisionLog::new(dir.path().to_path_buf());
    fs::create_dir_all(log.path())?;

    let result: Result<LogRows<Point, String>, _> = read([&log]);

    assert!(
        matches!(&result, Err(ReadError::NotAFile { path }) if *path == log.path()),
        "{result:?}"
    );
    Ok(())
}

#[test]
fn the_headroom_is_64_rows_of_the_logs_row_cap() {
    assert_eq!(u64::try_from(MAX_ROW_BYTES), Ok(ROW_BYTES));
    assert_eq!(
        MAX_LOG_FILE_BYTES,
        MAX_FILE_BYTES + HEADROOM_ROWS * ROW_BYTES
    );
}
