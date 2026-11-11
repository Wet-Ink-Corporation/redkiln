use std::path::Path;
use std::process::Command;
use std::time::Duration;

use serde_json::json;

use super::{approval, owner};
use redkiln_core::suite::Owner;

const TIMEOUT: Duration = Duration::from_secs(30);

/// Runs `git <args>` in `dir`, isolated from the caller's repository.
fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

#[test]
#[cfg_attr(miri, ignore = "runs git in a subprocess")]
fn the_suite_owner_comes_first() {
    let dir = tempfile::tempdir().expect("temp dir");
    git(dir.path(), &["init", "-q"]);
    git(dir.path(), &["config", "user.name", "Someone Else"]);
    std::fs::create_dir(dir.path().join(".redkiln")).expect("mkdir");
    std::fs::write(
        dir.path().join(".redkiln/suite.toml"),
        "[suite]\nowner = \"ryan\"\n",
    )
    .expect("write");

    let found = owner(dir.path(), TIMEOUT).expect("git answers");

    assert_eq!(found.as_ref().map(Owner::as_str), Some("ryan"), "ST-12");
}

#[test]
#[cfg_attr(miri, ignore = "runs git in a subprocess")]
fn without_a_suite_owner_gits_user_name_is_the_owner() {
    let dir = tempfile::tempdir().expect("temp dir");
    git(dir.path(), &["init", "-q"]);
    git(dir.path(), &["config", "user.name", "Ryan Britton"]);
    let nested = dir.path().join("src");
    std::fs::create_dir(&nested).expect("mkdir");

    let found = owner(&nested, TIMEOUT).expect("git answers");

    assert_eq!(
        found.as_ref().map(Owner::as_str),
        Some("ryan-britton"),
        "the repository's last user.name, slugged"
    );
}

#[test]
fn a_plan_approval_is_recorded_for_the_repository_with_no_gates() {
    let owner = Owner::from_git_config("user.name ryan\n").expect("an owner");

    let record = approval(&owner, b"plan\n", "docs/plan.md");

    assert_eq!(
        serde_json::to_value(&record).expect("serialises"),
        json!({
            "subject": {
                "kind": "plan",
                "method": "sha256",
                "hash": "sha256:1b4025dc7b8d27cf38df85e77b20ed44a00851a2c28b338560560d85deded8e3",
                "locator": "docs/plan.md",
            },
            "verdict": "approved",
            "scope": "repository",
            "decider": "human:ryan",
        }),
        "AP-01's data, D3's shape"
    );
}
