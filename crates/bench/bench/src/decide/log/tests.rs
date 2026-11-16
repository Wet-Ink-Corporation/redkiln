use std::fs;
use std::path::Path;

use redkiln_calibrate::scope::DecisionDirs;
use serde_json::{Value, json};

use super::{DecisionLog, LogError};
use crate::decide::point::Point;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn answered_counts_start_at_zero_and_persist_per_point() -> TestResult {
    let home = tempfile::tempdir()?;
    let log = DecisionLog::in_anneal_layout(home.path());
    assert_eq!(log.answered(Point::D1)?, 0);

    log.count_answered(Point::D1)?;
    log.count_answered(Point::D1)?;
    log.count_answered(Point::D3)?;

    let reopened = DecisionLog::in_anneal_layout(home.path());
    assert_eq!(reopened.answered(Point::D1)?, 2);
    assert_eq!(reopened.answered(Point::D3)?, 1);
    assert_eq!(reopened.answered(Point::D2)?, 0);
    Ok(())
}

#[test]
fn a_counts_file_of_another_kind_is_refused() {
    let home = tempfile::tempdir().expect("tempdir");
    let log = DecisionLog::in_anneal_layout(home.path());
    fs::create_dir_all(log.dir()).expect("mkdir");
    fs::write(
        log.counts_path(),
        r#"{"kind":"other","version":1,"answered":{}}"#,
    )
    .expect("write");

    assert!(matches!(
        log.answered(Point::D1),
        Err(LogError::CountsForeign { .. })
    ));
}

#[test]
fn a_directory_at_the_counts_path_is_refused() {
    let home = tempfile::tempdir().expect("tempdir");
    let log = DecisionLog::in_anneal_layout(home.path());
    fs::create_dir_all(log.counts_path()).expect("a directory where counts go");

    assert!(matches!(log.answered(Point::D1), Err(LogError::Counts(_))));
}

/// A user scope with anneal's old folder and Bench's new one, both under `home`.
fn moved(home: &Path) -> DecisionLog {
    DecisionLog::from_dirs(DecisionDirs {
        current: home.join("redkiln").join("calibrate").join("bench"),
        legacy: Some(home.join("anneal").join("decisions")),
    })
}

/// Every file under `dir` with its bytes, sorted by path.
fn snapshot(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut files = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        for entry in fs::read_dir(&next).expect("readable") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let bytes = fs::read(&path).expect("readable");
                files.push((path.display().to_string(), bytes));
            }
        }
    }
    files.sort();
    files
}

fn lines(path: &Path) -> Vec<Value> {
    fs::read_to_string(path)
        .expect("the log is readable")
        .lines()
        .map(|line| serde_json::from_str(line).expect("each line is JSON"))
        .collect()
}

#[test]
fn rows_and_counts_go_only_to_the_new_folder_and_the_old_one_is_untouched() -> TestResult {
    let home = tempfile::tempdir()?;
    let log = moved(home.path());
    let old = home.path().join("anneal").join("decisions");
    fs::create_dir_all(&old)?;
    fs::write(old.join("decisions.jsonl"), "{\"n\":0}\n")?;
    fs::write(
        old.join("counts.json"),
        r#"{"kind":"anneal-decision-counts","version":1,"answered":{"D1":49}}"#,
    )?;
    let before = snapshot(&old);

    log.append(&json!({"n": 1}))?;
    let counted = log.count_answered(Point::D1)?;

    assert_eq!(counted, 50, "the old count carries over");
    assert_eq!(lines(&log.path()), [json!({"n": 1})]);
    assert_eq!(
        log.answered(Point::D1)?,
        50,
        "read back from the new folder"
    );
    assert_eq!(snapshot(&old), before, "the old folder is never written");
    Ok(())
}

#[test]
fn counts_are_read_from_the_old_folder_until_the_new_one_has_its_own() -> TestResult {
    let home = tempfile::tempdir()?;
    let log = moved(home.path());
    let old = log.legacy_dir().ok_or("an old folder")?.to_path_buf();
    fs::create_dir_all(&old)?;
    fs::write(
        old.join("counts.json"),
        r#"{"kind":"anneal-decision-counts","version":1,"answered":{"D2":3}}"#,
    )?;
    let from_old = log.answered(Point::D2)?;
    fs::create_dir_all(log.dir())?;
    fs::write(
        log.counts_path(),
        r#"{"kind":"anneal-decision-counts","version":1,"answered":{"D2":1}}"#,
    )?;

    assert_eq!((from_old, log.answered(Point::D2)?), (3, 1));
    Ok(())
}

#[test]
fn a_state_file_is_read_from_the_new_folder_else_the_old_one() -> TestResult {
    let home = tempfile::tempdir()?;
    let log = moved(home.path());
    let (new, old) = (
        log.dir().join("proposals.json"),
        log.legacy_dir()
            .ok_or("an old folder")?
            .join("proposals.json"),
    );
    let neither = log.file_to_read("proposals.json");
    fs::create_dir_all(log.legacy_dir().ok_or("an old folder")?)?;
    fs::write(&old, "{}")?;
    let only_old = log.file_to_read("proposals.json");
    fs::create_dir_all(log.dir())?;
    fs::write(&new, "{}")?;

    assert_eq!(
        (neither, only_old, log.file_to_read("proposals.json")),
        (new.clone(), old, new)
    );
    Ok(())
}

#[test]
fn the_old_folder_is_read_first_then_the_new_one() {
    let home = Path::new("/h");
    let log = moved(home);

    let dirs: Vec<&Path> = log
        .logs()
        .map(redkiln_calibrate::log::DecisionLog::dir)
        .collect();

    assert_eq!(
        dirs,
        [
            home.join("anneal").join("decisions"),
            home.join("redkiln").join("calibrate").join("bench")
        ]
    );
}

#[test]
fn the_warning_names_both_folders_only_when_the_old_one_exists() -> TestResult {
    let home = tempfile::tempdir()?;
    let log = moved(home.path());
    let absent = log.legacy_warning();
    let old = log.legacy_dir().ok_or("an old folder")?;
    fs::create_dir_all(old)?;

    assert_eq!(absent, None);
    assert_eq!(
        log.legacy_warning(),
        Some(format!(
            "reading the old decision log at {} too; new rows go to {}, and the old folder is \
             never written or deleted",
            old.display(),
            log.dir().display()
        ))
    );
    Ok(())
}

#[test]
fn open_puts_benchs_rows_under_calibrate_bench_and_reads_anneal_home_alongside() -> TestResult {
    let home = tempfile::tempdir()?;
    let (redkiln, anneal) = (home.path().join("r"), home.path().join("a"));
    let vars = [("REDKILN_HOME", &redkiln), ("ANNEAL_HOME", &anneal)];

    // Each call hands out its own value, as `std::env::var_os` does.
    let log = super::open(|var: &str| {
        vars.iter()
            .find(|(name, _)| *name == var)
            .map(|(_, value)| value.as_os_str().to_owned())
    })?;

    assert_eq!(
        (log.dir(), log.legacy_dir()),
        (
            redkiln.join("calibrate").join("bench").as_path(),
            Some(anneal.join("decisions").as_path())
        )
    );
    Ok(())
}
