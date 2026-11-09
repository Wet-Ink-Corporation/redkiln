//! `weighin status [--json]` (SPEC CLI-12): the envelope `weighin.status/1`, its `data` against
//! the SPEC's `status-1` and the contract, exit 0 whenever it reported (CLI-06), and a read that
//! writes nothing: no store, no import (decision D3), no hook state.

use crate::scratch::{Repo, TestResult};
use crate::support::{bin, folder_store, state_of, store_of, wrote_in};
use redkiln_core::store::{NewEvent, Store, Tag, Wait};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};
use weighin_contract::status::{self, Health, HookState, RepoClass, Status, StoreSource};

/// The SPEC schema `spec/schemas/<stem>.schema.json`.
fn schema(stem: &str) -> TestResult<Value> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("spec")
        .join("schemas")
        .join(format!("{stem}.schema.json"));
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}

/// Where `doc` breaks the SPEC schema `stem`.
fn breaks(stem: &str, doc: &Value) -> TestResult<Vec<String>> {
    let validator = jsonschema::draft202012::new(&schema(stem)?)?;
    Ok(validator
        .iter_errors(doc)
        .map(|e| format!("{}: {e}", e.instance_path().as_str()))
        .collect())
}

/// `out`'s one envelope line, checked against `cli-envelope-1` and its `data` against
/// `status-1`, exit 0; the envelope and its `data` read through the contract.
fn report(out: &Output) -> TestResult<(Value, Status)> {
    let text = std::str::from_utf8(&out.stdout)?;
    assert_eq!(
        out.status.code(),
        Some(0),
        "CLI-06: {text} {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(text.matches('\n').count(), 1, "CLI-01, one line: {text}");
    let envelope: Value = serde_json::from_str(text)?;
    let data = envelope.get("data").ok_or("a data key")?;
    let none = Vec::<String>::new();
    assert_eq!(breaks("cli-envelope-1", &envelope)?, none, "{text}");
    assert_eq!(breaks("status-1", data)?, none, "{text}");
    let field = |key: &str| envelope.get(key).cloned();
    assert_eq!(field("schema"), Some(status::SCHEMA.into()), "{text}");
    assert_eq!(field("command"), Some("status".into()), "{text}");
    assert_eq!(field("ok"), Some(true.into()), "CLI-02: {text}");
    let data = Status::from_envelope_data(data)?;
    Ok((envelope, data))
}

/// The keys of the envelope's findings, in order.
fn keys(envelope: &Value) -> Vec<String> {
    let findings = envelope.get("findings").and_then(Value::as_array);
    findings
        .into_iter()
        .flatten()
        .filter_map(|f| f.get("key").and_then(Value::as_str).map(str::to_owned))
        .collect()
}

/// `path` canonicalised, so `/var` and `/private/var` (macOS) or `\\?\` (Windows) compare equal.
fn real(path: impl AsRef<Path>) -> TestResult<PathBuf> {
    Ok(std::fs::canonicalize(path)?)
}

fn now() -> TestResult<u64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn status_json_conforms() -> TestResult {
    let repo = Repo::new(true)?;
    repo.raise("s1", "Keep retention per series?")?;

    let (envelope, data) = report(&repo.run("s1", &["status", "--json"], None)?)?;

    assert_eq!(keys(&envelope), Vec::<String>::new(), "{envelope}");
    assert_eq!(data.plugin, "rk-weigh-in");
    assert_eq!(data.version, env!("CARGO_PKG_VERSION"));
    assert_eq!(data.spec, redkiln_core::envelope::SPEC_VERSION);
    let store = data.store.as_deref().ok_or("a located store")?;
    assert_eq!(real(store)?, real(store_of(&repo.root))?);
    assert_eq!(data.store_source, Some(StoreSource::GitCommonDir));
    assert_eq!(data.store_schema.as_deref(), Some("1.0"));
    assert_eq!(data.repo_class, RepoClass::Own);
    assert!(data.unreadable.is_empty(), "{envelope}");
    assert_eq!((data.dropped, data.hook_errors), (0, 0));
    assert_eq!(data.health, Health::Ok);
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn status_without_json_says_its_health_and_store_in_lines() -> TestResult {
    let repo = Repo::new(true)?;
    repo.raise("s1", "Keep retention per series?")?;

    let out = repo.run("s1", &["status"], None)?;

    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8(out.stdout)?;
    let mut lines = text.lines();
    let head = format!(
        "Weigh-In {} (rk-weigh-in, SPEC {}): ok",
        env!("CARGO_PKG_VERSION"),
        redkiln_core::envelope::SPEC_VERSION
    );
    assert_eq!(lines.next(), Some(head.as_str()), "{text}");
    let store = lines.next().ok_or("a store line")?;
    assert!(store.starts_with("store: "), "{text}");
    assert!(
        store.ends_with(" (git_common_dir, schema 1.0), own"),
        "{text}"
    );
    assert_eq!(lines.next(), None, "no finding: {text}");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn an_unlocated_store_is_off_with_nulls() -> TestResult {
    let repo = Repo::new(false)?;
    // A store path from home with no home directory: Core cannot locate it (ST-01).
    let out = Command::new(bin())
        .current_dir(&repo.root)
        .args(["status", "--json"])
        .env("REDKILN_STORE", "~/events.sqlite3")
        .env_remove("WEIGHIN_STORE")
        .env_remove("HOME")
        .env_remove("USERPROFILE")
        .env_remove("WEIGHIN_SESSION")
        .env_remove("CLAUDE_SESSION_ID")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .stdin(Stdio::null())
        .output()?;

    let (envelope, data) = report(&out)?;

    assert_eq!(keys(&envelope), ["store/unlocated"], "{envelope}");
    assert_eq!(envelope["findings"][0]["severity"], "error");
    assert_eq!(
        (data.store, data.store_source, data.store_schema),
        (None, None, None)
    );
    assert_eq!(data.repo_class, RepoClass::Client);
    assert_eq!(data.health, Health::Off);
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_refused_store_is_off_and_left_unchanged() -> TestResult {
    let repo = Repo::new(true)?;
    let dir = repo.sibling("bogus")?;
    let bogus = dir.join("events.sqlite3");
    std::fs::write(&bogus, "not a database")?;
    let store = bogus.to_string_lossy().into_owned();

    let (envelope, data) =
        report(&repo.run("s1", &["--store", &store, "status", "--json"], None)?)?;

    assert_eq!(keys(&envelope), ["store/refused"], "{envelope}");
    assert_eq!(data.health, Health::Off);
    assert_eq!(data.store_source, Some(StoreSource::Flag));
    assert_eq!(data.store_schema, None);
    assert_eq!(std::fs::read(&bogus)?, b"not a database");
    let beside: Vec<_> = std::fs::read_dir(&dir)?
        .map(|entry| entry.map(|e| e.file_name()))
        .collect::<Result<_, _>>()?;
    assert_eq!(beside, ["events.sqlite3"], "nothing written beside it");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_folder_with_no_store_creates_none() -> TestResult {
    let repo = Repo::new(false)?;

    let (envelope, data) = report(&repo.run("s1", &["status", "--json"], None)?)?;

    assert!(
        !repo.root.join(".git").join("redkiln").exists(),
        "{envelope}"
    );
    assert_eq!(data.store_schema, None, "no store yet");
    assert_eq!(data.store_source, Some(StoreSource::GitCommonDir));
    assert_eq!(data.health, Health::Ok, "{envelope}");

    let folder = repo.sibling("plain")?;
    let out = repo.run_in(&folder, &["status", "--json"])?;
    let (_, data) = report(&out)?;
    assert!(!wrote_in(&folder), "nothing in the folder");
    assert_eq!(data.store_source, Some(StoreSource::Folder));
    let store = PathBuf::from(data.store.ok_or("a store path")?);
    assert_eq!(
        real(store.parent().and_then(Path::parent).ok_or("two parents")?)?,
        real(
            folder_store(&folder)
                .parent()
                .and_then(Path::parent)
                .ok_or("two parents")?
        )?
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn unusable_events_are_listed_by_type_and_degrade() -> TestResult {
    let repo = Repo::new(true)?;
    let store = Store::open(&store_of(&repo.root), Wait::Command)?;
    let newer = NewEvent::new(
        "weighin.item_raised",
        br#"{"type":"weighin.item_raised","version":2}"#.to_vec(),
    )?
    .with_tag(Tag::new("item", "wi-0123456789ab")?);
    store.append(vec![newer], None)?;
    drop(store);

    let (envelope, data) = report(&repo.run("s1", &["status", "--json"], None)?)?;

    assert_eq!(keys(&envelope), ["events/unreadable"], "{envelope}");
    assert_eq!(
        data.unreadable.into_iter().collect::<Vec<_>>(),
        [("weighin.item_raised".to_owned(), 1)]
    );
    assert_eq!(data.health, Health::Degraded);
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn hook_errors_and_lost_hooks_degrade_and_status_marks_nothing_seen() -> TestResult {
    let repo = Repo::new(true)?;
    let state = state_of(&repo.root);
    std::fs::create_dir_all(&state)?;
    let log = state.join("hook-errors.log");
    std::fs::write(&log, "2026-11-08T09:00:00Z Stop: the store is busy\n")?;
    // A Stop that stopped beating long before its deadline: interrupted.
    let started = now()? - 100;
    let slot = state.join("hook-heartbeat-Stop");
    let running = format!(
        "weighin-heartbeat 1\nstate running\nstarted {started}\nbeat {started}\ntimeout 20\npid 1\n"
    );
    std::fs::write(&slot, &running)?;

    let (envelope, data) = report(&repo.run("s1", &["status", "--json"], None)?)?;

    assert_eq!(
        keys(&envelope),
        ["hooks/errors", "hooks/lost:Stop"],
        "{envelope}"
    );
    assert_eq!(data.hook_errors, 1);
    let stop = data.hooks.get("Stop").ok_or("the Stop slot")?;
    assert_eq!(stop.state, HookState::Interrupted);
    assert_eq!(data.health, Health::Degraded);
    assert!(
        !state.join("hook-errors.seen").exists(),
        "nothing marked seen"
    );
    assert_eq!(
        std::fs::read_to_string(&slot)?,
        running,
        "the slot untouched"
    );

    let start = serde_json::json!({"hook_event_name": "SessionStart", "session_id": "s1", "cwd": repo.root});
    let printed = String::from_utf8(repo.hook(&start)?.stdout)?;
    assert!(
        printed.contains("hook error(s) since the last session"),
        "the next SessionStart still reports them: {printed}"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn status_runs_no_legacy_import_and_reports_it_due() -> TestResult {
    let repo = Repo::new(false)?;
    let journals = repo.root.join(".kb").join("_ledger").join("journal");
    std::fs::create_dir_all(&journals)?;
    std::fs::write(journals.join("s1.jsonl"), "{}\n")?;

    let (envelope, data) = report(&repo.run("s1", &["status", "--json"], None)?)?;

    assert_eq!(keys(&envelope), ["legacy/due"], "{envelope}");
    assert_eq!(envelope["findings"][0]["severity"], "note");
    assert_eq!(data.health, Health::Ok, "a note does not degrade");
    assert!(
        !repo.root.join(".git").join("redkiln").exists(),
        "no store created, nothing imported"
    );
    Ok(())
}

#[test]
#[cfg(unix)]
#[cfg_attr(miri, ignore = "spawns a shell and the weighin binary")]
fn a_removed_working_directory_exits_70_with_an_internal_envelope() -> TestResult {
    let repo = Repo::new(false)?;
    let parent = repo.sibling("gone-parent")?;
    std::fs::create_dir_all(parent.join("d"))?;

    // The shell enters `d`, removes it, then runs weighin there: no working directory.
    let out = Command::new("sh")
        .current_dir(&parent)
        .args([
            "-c",
            "cd d && rmdir ../d && exec \"$0\" status --json",
            bin(),
        ])
        .env_remove("REDKILN_STORE")
        .env_remove("WEIGHIN_STORE")
        .stdin(Stdio::null())
        .output()?;

    let text = String::from_utf8(out.stdout)?;
    assert_eq!(out.status.code(), Some(70), "CLI-05: {text}");
    assert!(!parent.join("d").exists(), "the directory was removed");
    let envelope: Value = serde_json::from_str(&text)?;
    assert_eq!(breaks("cli-envelope-1", &envelope)?, Vec::<String>::new());
    assert_eq!(envelope["schema"], status::SCHEMA);
    assert_eq!(envelope["ok"], false);
    assert_eq!(envelope["data"], serde_json::json!({}));
    assert_eq!(keys(&envelope), ["internal"]);
    let stderr = String::from_utf8(out.stderr)?;
    assert!(stderr.starts_with("weighin: error: "), "CLI-08: {stderr}");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_closed_stdout_exits_0() -> TestResult {
    let repo = Repo::new(true)?;
    for args in [&["status", "--json"][..], &["status"]] {
        let out = repo.run_with_stdout_closed(args)?;
        assert_eq!(out.status.code(), Some(0), "CLI-07: {args:?}");
        assert!(
            out.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(())
}
