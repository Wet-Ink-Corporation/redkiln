//! The owner of a folder with no config comes from git, not a fixed name (#27, B-31). Each test
//! gives git its own global config and none from the system, so the machine's never leaks in.

use crate::support::{Run, setup, weighin_env};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A repository from `setup`, and a global git config holding `global` beside it.
fn repo_with_global(global: &str) -> std::io::Result<(tempfile::TempDir, PathBuf, PathBuf)> {
    let (tmp, repo) = setup();
    let config = tmp.path().join("gitconfig");
    std::fs::write(&config, global)?;
    Ok((tmp, repo, config))
}

/// `weighin <args>` in `repo` with git reading only `global` and the repository's own config.
fn run(repo: &Path, global: &Path, args: &[&str]) -> Result<String, String> {
    let global = global.to_string_lossy();
    let (ok, out) = weighin_env(
        repo,
        &Run {
            session: "s1",
            args,
            stdin: None,
            vars: &[("GIT_CONFIG_GLOBAL", &global), ("GIT_CONFIG_NOSYSTEM", "1")],
        },
    );
    if ok { Ok(out) } else { Err(out) }
}

/// The `owner` an `init` with no `--owner` writes.
fn owner_after_init(repo: &Path, global: &Path) -> Result<Value, Box<dyn std::error::Error>> {
    run(repo, global, &["init"])?;
    let config: Value = serde_json::from_str(&std::fs::read_to_string(
        repo.join(".kb/_ledger/config.json"),
    )?)?;
    Ok(config.get("owner").cloned().unwrap_or(Value::Null))
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_first_init_takes_the_owner_from_the_git_user_name() -> TestResult {
    let (_tmp, repo, global) = repo_with_global("[user]\n\tname = Ana Lima\n")?;

    assert_eq!(owner_after_init(&repo, &global)?, json!("ana-lima"));
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn without_a_user_name_the_owner_is_the_email_local_part() -> TestResult {
    let (_tmp, repo, global) = repo_with_global("[user]\n\temail = j.doe@example.com\n")?;

    assert_eq!(owner_after_init(&repo, &global)?, json!("j-doe"));
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn without_a_git_user_the_owner_is_ryan_as_before() -> TestResult {
    let (_tmp, repo, global) = repo_with_global("")?;

    assert_eq!(owner_after_init(&repo, &global)?, json!("ryan"));
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn init_keeps_the_owner_an_existing_config_names() -> TestResult {
    let (_tmp, repo, global) = repo_with_global("[user]\n\tname = Ana Lima\n")?;
    run(&repo, &global, &["init", "--owner", "bea"])?;

    assert_eq!(owner_after_init(&repo, &global)?, json!("bea"));
    Ok(())
}

/// The decider of a decision taken now in `repo`.
fn decider(repo: &Path, global: &Path, q: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let raised = run(repo, global, &["raise", "--q", q, "--kind", "question"])?;
    let id = raised.split_whitespace().nth(1).ok_or("no id")?;
    run(repo, global, &["decide", id, "yes"])?;
    let shown: Value = serde_json::from_str(&run(repo, global, &["show", id, "--json"])?)?;
    Ok(shown
        .pointer("/decision/decider")
        .cloned()
        .unwrap_or(Value::Null))
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn suite_tomls_owner_wins_over_config_jsons_which_wins_over_gits() -> TestResult {
    let (_tmp, repo, global) = repo_with_global("[user]\n\tname = Ana Lima\n")?;
    run(
        &repo,
        &global,
        &["init", "--class", "own", "--owner", "bea"],
    )?;
    let suite = repo.join(".redkiln/suite.toml");
    std::fs::write(&suite, "[suite]\nrepo_class = \"own\"\nowner = \"cy\"\n")?;

    let from_suite = decider(&repo, &global, "Per series?")?;
    let refused = run(&repo, &global, &["init", "--owner", "dee"]);
    let kept = run(&repo, &global, &["init"]);
    std::fs::write(&suite, "[suite]\nrepo_class = \"own\"\n")?;
    let from_config = decider(&repo, &global, "Per record?")?;
    std::fs::remove_file(repo.join(".kb/_ledger/config.json"))?;
    let from_git = decider(&repo, &global, "Per tenant?")?;

    assert_eq!(from_suite, json!("human:cy"));
    let refused = refused.err().ok_or("init --owner dee is refused")?;
    assert!(
        refused.contains("suite.toml") && refused.contains("`cy`"),
        "{refused}"
    );
    kept?;
    assert_eq!(
        from_config,
        json!("human:bea"),
        "an init with no --owner kept config.json's owner"
    );
    assert_eq!(from_git, json!("human:ana-lima"));
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_decision_in_a_folder_with_no_config_is_the_git_users() -> TestResult {
    let (_tmp, repo, global) = repo_with_global("[user]\n\tname = Ana Lima\n")?;
    let batch = json!([
        {"op": "raise", "q": "Per series or per record?", "door": "two-way"},
        {"op": "decide", "id": "@1", "option": "per series"}
    ])
    .to_string();
    let (ok, out) = weighin_env(
        &repo,
        &Run {
            session: "s1",
            args: &["batch"],
            stdin: Some(&batch),
            vars: &[
                ("GIT_CONFIG_GLOBAL", &global.to_string_lossy()),
                ("GIT_CONFIG_NOSYSTEM", "1"),
            ],
        },
    );
    assert!(ok, "{out}");
    let id = out
        .split_whitespace()
        .find(|w| w.starts_with("wi-"))
        .ok_or("no id")?;

    let shown: Value = serde_json::from_str(&run(&repo, &global, &["show", id, "--json"])?)?;

    assert_eq!(
        shown.pointer("/decision/decider"),
        Some(&json!("human:ana-lima"))
    );
    Ok(())
}
