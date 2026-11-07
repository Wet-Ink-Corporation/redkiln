//! The repository class (ST-12, ST-13, #17 part 5b): read from `.redkiln/suite.toml`, else from
//! weigh-in 1.x's `.kb/_ledger/config.json` with a warning (M1-F), else `client`; a `client`
//! repository exports no journal ref and no board for claude.ai (D8 A); `init` writes
//! the class to suite.toml, never to config.json.

use crate::scratch::{Repo, TestResult};
use crate::support::state_of;
use serde_json::{Value, json};
use std::path::Path;

const SUITE: &str = ".redkiln/suite.toml";
const CONFIG: &str = ".kb/_ledger/config.json";

/// What weighin's notice for Core's `redkiln_core::suite::Warning::LegacyRepoClass` starts with:
/// `env::location::suite_notice`'s wording, which mirrors Core's `Display` with a shorter path.
const LEGACY_WARNING: &str = "repo_class `own` comes from";

/// Writes `text` to `<root>/<relative>`, making its directory.
fn write(root: &Path, relative: &str, text: &str) -> std::io::Result<()> {
    let path = root.join(relative);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, text)
}

/// The `systemMessage` of a `SessionStart` in `repo`, read through its JSON; empty when it
/// printed plain context (no message).
fn session_start_message(repo: &Repo) -> TestResult<String> {
    let out = repo.hook(
        &json!({"hook_event_name": "SessionStart", "session_id": "s",
        "cwd": repo.root, "source": "startup"}),
    )?;
    assert!(out.status.success(), "the hook exits 0");
    let text = String::from_utf8(out.stdout)?;
    Ok(match serde_json::from_str::<Value>(&text) {
        Ok(json) => json
            .get("systemMessage")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        Err(_) => String::new(),
    })
}

/// Raises one question in session `s`; its stderr.
fn raise(repo: &Repo) -> TestResult<String> {
    let out = repo.run(
        "s",
        &[
            "raise",
            "--q",
            "Keep retention per series?",
            "--kind",
            "question",
        ],
        None,
    )?;
    let stderr = String::from_utf8(out.stderr)?;
    assert!(out.status.success(), "{stderr}");
    Ok(stderr)
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn config_jsons_class_below_no_suite_toml_is_used_and_said_on_stderr_and_at_session_start()
-> TestResult {
    let repo = Repo::new(false)?;
    write(&repo.root, CONFIG, r#"{"repo_class":"own"}"#)?;

    let stderr = raise(&repo)?;
    let message = session_start_message(&repo)?;
    let stop = repo.hook(&repo.stop("s"))?;

    assert!(stop.status.success(), "the hook exits 0");
    let warnings: Vec<&str> = stderr
        .lines()
        .filter(|l| l.contains(LEGACY_WARNING))
        .collect();
    assert_eq!(warnings.len(), 1, "{stderr}");
    assert!(
        warnings.iter().all(|w| w.starts_with("weighin: warning: ")
            && w.contains(CONFIG)
            && w.contains(SUITE)),
        "{stderr}"
    );
    assert!(message.contains(LEGACY_WARNING), "{message}");
    assert!(
        repo.has_ref("refs/redkiln/journal/s"),
        "an own repository exports its journal ref"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_client_repository_exports_no_journal_ref_and_no_board_and_an_own_one_does_both() -> TestResult
{
    for class in ["client", "own"] {
        let repo = Repo::new(false)?;
        repo.ok(
            "s",
            &["init", "--class", class, "--board", "artifact"],
            None,
        )?;

        raise(&repo)?;
        let stop = repo.hook(&repo.stop("s"))?;
        let export = repo.run("s", &["board", "export"], None)?;

        assert!(stop.status.success(), "{class}: the hook exits 0");
        let stderr = String::from_utf8(export.stderr)?;
        let board = state_of(&repo.root).join("board-state.json");
        if class == "own" {
            assert!(repo.has_ref("refs/redkiln/journal/s"), "own: a journal ref");
            assert!(export.status.success() && board.exists(), "own: {stderr}");
        } else {
            assert!(
                !repo.has_ref("refs/redkiln/journal/s"),
                "client: no journal ref"
            );
            assert!(!export.status.success(), "client: export refused");
            assert!(
                stderr.contains("`client`") && stderr.contains(SUITE),
                "client: {stderr}"
            );
            assert!(!board.exists(), "client: no board state written");
        }
    }
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_suite_toml_that_does_not_parse_is_client_whatever_config_json_says() -> TestResult {
    let repo = Repo::new(false)?;
    write(&repo.root, CONFIG, r#"{"repo_class":"own"}"#)?;
    write(&repo.root, SUITE, "[suite\nrepo_class = \"own\"\n")?;

    let stderr = raise(&repo)?;
    let stop = repo.hook(&repo.stop("s"))?;

    assert!(stop.status.success(), "the hook exits 0");
    assert!(
        stderr.contains("suite.toml is not used") && stderr.contains("the repository is client"),
        "{stderr}"
    );
    assert!(!stderr.contains(LEGACY_WARNING), "no fallback: {stderr}");
    assert!(
        !repo.has_ref("refs/redkiln/journal/s"),
        "client: no journal ref"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn init_writes_the_class_to_suite_toml_never_to_config_json_and_refuses_another() -> TestResult {
    let repo = Repo::new(false)?;

    let receipt = repo.ok("s", &["init", "--class", "own"], None)?;

    let suite = std::fs::read_to_string(repo.root.join(SUITE))?;
    assert_eq!(suite, "[suite]\nrepo_class = \"own\"\n");
    let config: Value = serde_json::from_str(&std::fs::read_to_string(repo.root.join(CONFIG))?)?;
    assert_eq!(config.get("repo_class"), None, "{config}");
    assert!(
        receipt.contains("class own") && receipt.contains(&format!("wrote {SUITE}")),
        "{receipt}"
    );
    let config_bytes = std::fs::read(repo.root.join(CONFIG))?;

    let refused = repo.run("s", &["init", "--class", "client"], None)?;
    let same = repo.ok("s", &["init", "--class", "own"], None)?;

    let stderr = String::from_utf8(refused.stderr)?;
    assert!(
        !refused.status.success() && stderr.contains(SUITE) && stderr.contains("`own`"),
        "{stderr}"
    );
    assert!(
        same.contains("class own") && same.contains(&format!("from {SUITE}")),
        "{same}"
    );
    assert_eq!(std::fs::read_to_string(repo.root.join(SUITE))?, suite);
    assert_eq!(
        std::fs::read(repo.root.join(CONFIG))?,
        config_bytes,
        "the refusal wrote nothing, and the same class rewrote the same config"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn init_without_a_class_moves_config_jsons_into_suite_toml_and_keeps_the_old_key() -> TestResult {
    let repo = Repo::new(false)?;
    write(
        &repo.root,
        CONFIG,
        r#"{"repo_class":"client","board":{"mode":"local"}}"#,
    )?;

    let receipt = repo.ok("s", &["init", "--ledger", "../l.sqlite3"], None)?;

    assert_eq!(
        std::fs::read_to_string(repo.root.join(SUITE))?,
        "[suite]\nrepo_class = \"client\"\n"
    );
    let config: Value = serde_json::from_str(&std::fs::read_to_string(repo.root.join(CONFIG))?)?;
    // Kept for a 1.x binary, which reads only config.json; suite.toml wins from now on.
    assert_eq!(config.get("repo_class"), Some(&json!("client")), "{config}");
    assert!(receipt.contains("class client · board local"), "{receipt}");
    let stderr = raise(&repo)?;
    assert!(!stderr.contains("comes from"), "no more fallback: {stderr}");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_first_init_without_a_class_anywhere_is_refused_before_any_write() -> TestResult {
    let repo = Repo::new(false)?;

    let out = repo.run("s", &["init", "--owner", "human:ann"], None)?;

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success() && stderr.contains("pass `--class own` or `--class client`"),
        "{stderr}"
    );
    assert!(!repo.root.join(SUITE).exists(), "no suite.toml");
    assert!(!repo.root.join(CONFIG).exists(), "no config.json");
    Ok(())
}
