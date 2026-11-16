//! `bench ship skip-rule` through the built binary, on scratch pull-request repositories.

use serde_json::json;

use crate::pr_repo::{PrRepo, TestResult};

#[test]
#[cfg_attr(miri, ignore = "runs git and the binary")]
fn a_small_change_to_ordinary_files_may_skip_full_review() -> TestResult {
    let repo = PrRepo::new()?;

    let envelope = repo.envelope(&["ship", "skip-rule"], 0)?;
    crate::support::golden("bench.ship-skip-rule/1", &envelope, &[repo.path()], &[])?;

    assert_eq!(
        (
            &envelope["data"]["skip"],
            &envelope["data"]["files"],
            &envelope["data"]["lines"],
            &envelope["data"]["base"],
        ),
        (&json!(true), &json!(1), &json!(3), &json!("main")),
        "{envelope}"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "runs git and the binary")]
fn three_files_need_a_full_review() -> TestResult {
    let repo = PrRepo::new()?;
    repo.write("src/a.rs", "a\n")?;
    repo.write("src/b.rs", "b\n")?;
    repo.commit("two more files")?;

    let envelope = repo.envelope(&["ship", "skip-rule"], 1)?;

    assert_eq!(
        &envelope["data"]["reasons"],
        &json!([{"reason": "too_many_files", "files": 3}]),
        "{envelope}"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "runs git and the binary")]
fn a_sensitive_path_needs_a_full_review() -> TestResult {
    let repo = PrRepo::new()?;
    repo.write("db/migrations/0001.sql", "create table t (id int);\n")?;
    repo.commit("a migration")?;

    let envelope = repo.envelope(&["ship", "skip-rule"], 1)?;

    assert_eq!(
        &envelope["data"]["reasons"],
        &json!([{
            "reason": "sensitive", "path": "db/migrations/0001.sql",
            "glob": "**/migrations/**", "category": "migrations",
        }]),
        "{envelope}"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "runs git and the binary")]
fn a_glob_configured_in_the_repository_adds_a_sensitive_path() -> TestResult {
    let repo = PrRepo::new()?;
    repo.write(
        ".anneal/config.json",
        r#"{"ship": {"sensitive_globs": ["src/lib.rs"]}}"#,
    )?;

    let output = repo.anneal(&["ship", "skip-rule"])?;

    let envelope = crate::support::envelope(&output)?;
    assert_eq!(
        (
            output.status.code(),
            std::str::from_utf8(&output.stderr)?,
            &envelope["data"]["reasons"][0]["category"],
        ),
        (
            Some(1),
            "bench: warning: .anneal/config.json is read because .redkiln/bench/config.json \
             does not exist; bench reads anneal's .anneal/ only as a 0.x fallback, so move the \
             file there\n",
            &json!("configured"),
        ),
        "{envelope}"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "runs git and the binary")]
fn a_glob_configured_in_bench_s_folder_adds_a_sensitive_path_without_a_warning() -> TestResult {
    let repo = PrRepo::new()?;
    repo.write(
        ".redkiln/bench/config.json",
        r#"{"ship": {"sensitive_globs": ["src/lib.rs"]}}"#,
    )?;

    let envelope = repo.envelope(&["ship", "skip-rule"], 1)?;

    assert_eq!(
        envelope.pointer("/data/reasons/0/category"),
        Some(&json!("configured")),
        "{envelope}"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "runs git and the binary")]
fn a_config_only_in_anneal_s_folder_is_read_with_one_warning_and_left_alone() -> TestResult {
    let repo = PrRepo::new()?;
    let config = r#"{"ship": {"sensitive_globs": ["src/lib.rs"]}}"#;
    repo.write(".anneal/config.json", config)?;

    let output = repo.anneal(&["ship", "skip-rule"])?;

    let envelope = crate::support::envelope(&output)?;
    assert_eq!(
        (
            output.status.code(),
            std::str::from_utf8(&output.stderr)?,
            envelope.pointer("/data/reasons/0/category"),
            std::fs::read_to_string(repo.path().join(".anneal").join("config.json"))?,
            repo.path().join(".redkiln").exists(),
        ),
        (
            Some(1),
            "bench: warning: .anneal/config.json is read because .redkiln/bench/config.json \
             does not exist; bench reads anneal's .anneal/ only as a 0.x fallback, so move the \
             file there\n",
            Some(&json!("configured")),
            config.to_owned(),
            false,
        )
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "runs git and the binary")]
fn with_both_configs_bench_s_is_read_and_anneal_s_named_as_ignored() -> TestResult {
    let repo = PrRepo::new()?;
    repo.write(
        ".anneal/config.json",
        r#"{"ship": {"sensitive_globs": ["src/lib.rs"]}}"#,
    )?;
    repo.write(".redkiln/bench/config.json", "{}")?;

    let output = repo.anneal(&["ship", "skip-rule"])?;

    let envelope = crate::support::envelope(&output)?;
    assert_eq!(
        (
            std::str::from_utf8(&output.stderr)?,
            envelope.pointer("/data/reasons/0/category"),
        ),
        (
            "bench: warning: .anneal/config.json is ignored: bench reads \
             .redkiln/bench/config.json\n",
            None,
        )
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "runs git and the binary")]
fn a_base_that_names_no_commit_is_a_usage_error() -> TestResult {
    let repo = PrRepo::new()?;

    let output = repo.anneal(&["ship", "skip-rule", "--base", "no-such-base"])?;

    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(output.stdout.is_empty(), "nothing on stdout: {output:?}");
    assert!(
        String::from_utf8(output.stderr)?.starts_with("bench: error: base no-such-base"),
        "one `bench: error:` line naming the base"
    );
    Ok(())
}
