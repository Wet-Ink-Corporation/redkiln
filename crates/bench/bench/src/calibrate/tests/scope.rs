//! D1: the rows in anneal's old folder (`<home>/decisions/`) are read alongside Bench's new one
//! (`<home>/calibrate/bench/`, the scope these tests give both variables the same home for),
//! with a warning; only the new folder is written.

use std::fs;

use super::{TestResult, calibrate, decision, hand_log, label, point};

#[test]
fn rows_only_in_the_old_folder_are_calibrated_and_the_run_warns_once() -> TestResult {
    let home = tempfile::tempdir()?;
    hand_log(home.path())?;
    let old = home.path().join("decisions");
    fs::rename(home.path().join("calibrate").join("bench"), &old)?;
    fs::remove_dir(home.path().join("calibrate"))?;
    let before = fs::read(old.join("decisions.jsonl"))?;

    let ran = calibrate(home.path(), &["--point", "D1"]);

    assert_eq!(ran.code.as_ref().ok(), Some(&0), "{:?}", ran.code);
    let noul = &point(&ran.envelope, "D1")["questions"][0];
    assert_eq!((&noul["n"], &noul["labelled"]), (&5.into(), &5.into()));
    let warning = format!(
        "bench: warning: reading the old decision log at {} too; new rows go to {}, and the old \
         folder is never written or deleted\n",
        old.display(),
        home.path().join("calibrate").join("bench").display()
    );
    assert!(ran.stderr.starts_with(&warning), "{}", ran.stderr);
    assert_eq!(ran.stderr.matches("bench: warning:").count(), 1);
    assert_eq!(
        (
            fs::read(old.join("decisions.jsonl"))?,
            old.join("proposals.json").exists(),
            home.path()
                .join("calibrate")
                .join("bench")
                .join("proposals.json")
                .exists(),
        ),
        (before, false, true),
        "the old folder is never written; the proposals go to the new one"
    );
    Ok(())
}

#[test]
fn a_label_in_the_new_folder_matches_a_decision_in_the_old_one() -> TestResult {
    let home = tempfile::tempdir()?;
    let old = home.path().join("decisions");
    let new = home.path().join("calibrate").join("bench");
    fs::create_dir_all(&old)?;
    fs::create_dir_all(&new)?;
    fs::write(
        old.join("decisions.jsonl"),
        decision("d1-1-0", "D1", 0.9, "deny").to_string() + "\n",
    )?;
    fs::write(
        new.join("decisions.jsonl"),
        label("d1-1-0", "yes").to_string() + "\n",
    )?;

    let ran = calibrate(home.path(), &["--point", "D1"]);

    let noul = &point(&ran.envelope, "D1")["questions"][0];
    assert_eq!(
        (
            &noul["n"],
            &noul["labelled"],
            &ran.envelope["data"]["unmatched_labels"]
        ),
        (&1.into(), &1.into(), &0.into())
    );
    Ok(())
}

#[test]
fn without_the_old_folder_there_is_no_warning() -> TestResult {
    let home = tempfile::tempdir()?;
    hand_log(home.path())?;

    let ran = calibrate(home.path(), &[]);

    assert!(!ran.stderr.contains("bench: warning:"), "{}", ran.stderr);
    Ok(())
}
