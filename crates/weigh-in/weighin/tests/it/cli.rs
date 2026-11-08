//! End to end: a scratch repository, hooks fed the JSON Claude Code sends, a linked worktree, a
//! sitting done through `batch`, and the board round trip.

use crate::support::{
    LAUNCHER_LIMIT, MESSAGE, Run, annotate_scanned_and_rescan, bin, capture_both_markers,
    context_pack_for, folder_store, hook, in_a_folder_with_no_ledger, isolated_launcher,
    items_seen_from_a_worktree, launcher_target, reopen_default_from_the_board, run_within,
    scan_one, scan_written_plan, settle_crlf_question, setup, sit_and_decide, state_of, weighin,
    weighin_env, wrote_in,
};
use serde_json::{Value, json};
use std::io::Write as _;
use std::process::Command;

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_session_from_capture_to_record() {
    let (_tmp, repo) = setup();
    capture_both_markers(&repo);
    scan_written_plan(&repo);
    let (one_way, scanned) = items_seen_from_a_worktree(&repo);
    context_pack_for(&repo, &one_way);
    sit_and_decide(&repo, &one_way);
    annotate_scanned_and_rescan(&repo, &scanned);
    reopen_default_from_the_board(&repo);
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn hooks_never_fail_a_session() {
    let tmp = tempfile::tempdir().unwrap();
    let out = hook(
        tmp.path(),
        &json!({"hook_event_name": "Stop", "cwd": tmp.path(), "last_assistant_message": "no markers here"}),
    );
    assert_eq!(out, "");
    let (ok, _) = weighin(tmp.path(), "x", &["hook"], Some("not json"));
    assert!(ok);
}

/// Exec-form hooks start the binary directly on Windows (`libexec/weighin.exe`), so nothing drops
/// the event name `hooks.json` passes after `hook` (#21, B-16). The binary takes it and reads the
/// event from stdin as before.
#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn hook_takes_the_event_name_hooks_json_passes() {
    let (_tmp, repo) = setup();
    let start = json!({"hook_event_name": "SessionStart", "session_id": "s1", "cwd": repo, "source": "startup"});

    let (ok, out) = weighin(
        &repo,
        "s1",
        &["hook", "SessionStart"],
        Some(&start.to_string()),
    );

    assert!(ok, "{out}");
    assert!(out.starts_with("Weigh-In (startup): session s1"), "{out}");
}

/// The plugin's hook rules (`docs/weigh-in/hooks.md`; SPEC CLI-09): every `hooks.json` entry is
/// `hook <event>` and runs through the binary, and no Weigh-In hook blocks or stops the session,
/// so the Stop hook spends none of the shared Stop budget. The Stop payload carries a one-way
/// marker, so it takes the nudge path, the one Stop that prints anything.
#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn every_hooks_json_entry_is_hook_event_runs_and_never_blocks() {
    let (_tmp, repo) = setup();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../plugins/weigh-in/hooks/hooks.json");
    let manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("hooks.json is readable"))
            .expect("hooks.json is JSON");
    let events = manifest["hooks"].as_object().expect("hooks is an object");
    assert!(!events.is_empty());
    for (event, groups) in events {
        let entries = groups.as_array().expect("each event holds groups");
        for entry in entries
            .iter()
            .flat_map(|g| g["hooks"].as_array().into_iter().flatten())
        {
            let args: Vec<&str> = entry["args"]
                .as_array()
                .expect("exec form has args")
                .iter()
                .map(|a| a.as_str().expect("args are strings"))
                .collect();
            assert_eq!(args, ["hook", event.as_str()], "{event}");
            let payload = json!({
                "hook_event_name": event, "session_id": "s1", "cwd": repo, "source": "startup",
                "last_assistant_message": MESSAGE,
                "tool_name": "Write", "tool_input": {"file_path": repo.join("docs/plan.md"), "content": "# Plan\n"},
            });
            let (ok, out) = weighin(&repo, "s1", &args, Some(&payload.to_string()));
            assert!(ok, "{event}: {out}");
            if event == "Stop" {
                assert!(
                    out.contains("systemMessage") && out.contains("one-way"),
                    "{out}"
                );
            }
            for line in out.lines().filter(|l| l.starts_with('{')) {
                let response: Value = serde_json::from_str(line).expect("a JSON response line");
                assert_eq!(response.get("decision"), None, "{event}: {line}");
                assert_eq!(response.get("continue"), None, "{event}: {line}");
            }
        }
    }
}

/// The board state is one db document, capped at 256 KiB, and every default used to go into it:
/// about 700 overflowed it (#19). The export now carries the newest unacknowledged defaults up
/// to the cap, with the total, and the digest covers the rest (wi-c34fb3).
#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn board_export_caps_defaults_at_the_newest_unacknowledged() {
    let (_tmp, repo) = setup();
    let (ok, out) = weighin(&repo, "s1", &["init", "--class", "own"], None);
    assert!(ok, "{out}");
    let raises: Vec<Value> = (0..205)
        .map(|i| json!({"op": "raise", "q": format!("Should cache entry {i} expire after its TTL?"), "kind": "call",
            "door": "two-way", "authority": "judgment", "did": format!("expire entry {i} after the TTL"), "channel": "marker"}))
        .collect();
    let (ok, out) = weighin(
        &repo,
        "s1",
        &["batch"],
        Some(&Value::Array(raises).to_string()),
    );
    assert!(ok, "{out}");
    let ids: Vec<String> = out
        .lines()
        .filter_map(|l| l.strip_prefix("✓ raised "))
        .filter_map(|l| l.split_whitespace().next())
        .map(str::to_owned)
        .collect();
    assert_eq!(ids.len(), 205, "{out}");
    let ack = json!([{"op": "ack", "id": ids[204]}]).to_string();
    let (ok, out) = weighin(&repo, "s1", &["batch"], Some(&ack));
    assert!(ok, "{out}");

    let (ok, out) = weighin(
        &repo,
        "s1",
        &["board", "export", "--out", "board.json"],
        None,
    );

    assert!(ok, "{out}");
    let state: Value =
        serde_json::from_str(&std::fs::read_to_string(repo.join("board.json")).unwrap()).unwrap();
    let shown: Vec<&str> = state["defaults"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        (shown.len(), state["defaults_total"].as_u64()),
        (200, Some(204)),
        "{out}"
    );
    assert!(
        !shown.contains(&ids[204].as_str()),
        "an acknowledged default stays off the board"
    );
    assert!(
        shown.contains(&ids[203].as_str()),
        "the newest unacknowledged default is shown"
    );
    assert!(
        !shown.contains(&ids[0].as_str()),
        "the oldest beyond the cap is left to the digest"
    );
}

/// When the report cannot be marked as seen, the message says it will repeat rather than
/// dropping that failure (H-10).
#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn session_start_says_when_the_error_report_cannot_be_marked_seen() {
    let (_tmp, repo) = setup();
    let start = json!({"hook_event_name": "SessionStart", "session_id": "s1", "cwd": repo, "source": "startup"});
    let state = state_of(&repo);
    // A directory where the marker file goes, so writing it fails.
    std::fs::create_dir_all(state.join("hook-errors.seen")).unwrap();
    std::fs::write(
        state.join("hook-errors.log"),
        "2026-10-02T09:00:00Z Stop: locked\n",
    )
    .unwrap();

    let out: Value = serde_json::from_str(&hook(&repo, &start)).unwrap();
    let message = out["systemMessage"].as_str().unwrap();
    assert!(
        message.contains("1 hook error(s)") && message.contains("will repeat"),
        "{message}"
    );
    assert!(!message.contains('\n'), "one line: {message}");
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn session_start_asks_for_one_block_per_call_in_a_list_for_the_user() {
    // Agents list calls for the user in prose ("Decisions for you:") with no block, so capture
    // misses them (eval seed recall 1 of 7, wi-357345); the convention names that case.
    let (_tmp, repo) = setup();
    let start = json!({"hook_event_name": "SessionStart", "session_id": "s1", "cwd": repo, "source": "startup"});

    let prime = hook(&repo, &start);

    assert!(
        prime.contains("Decisions for you") && prime.contains("one block per call"),
        "{prime}"
    );
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn session_start_reports_hook_errors_since_the_last_session() {
    let (_tmp, repo) = setup();
    let start = json!({"hook_event_name": "SessionStart", "session_id": "s1", "cwd": repo, "source": "startup"});
    let state = state_of(&repo);
    std::fs::create_dir_all(&state).unwrap();
    let log = state.join("hook-errors.log");
    std::fs::write(
        &log,
        "2026-10-02T09:00:00Z Stop: opening ledger: disk full\n2026-10-02T09:01:00Z PostToolUse: \"quoted\" failure\n",
    )
    .unwrap();

    // Two new lines: one message for the user, the prime still in Claude's context.
    let out: Value = serde_json::from_str(&hook(&repo, &start)).unwrap();
    let message = out["systemMessage"].as_str().unwrap();
    assert!(
        message.contains("2 hook error(s)")
            && message.contains("\"quoted\" failure")
            && message.contains("hook-errors.log"),
        "{message}"
    );
    assert!(!message.contains('\n'), "one line: {message}");
    assert_eq!(out["hookSpecificOutput"]["hookEventName"], "SessionStart");
    assert!(
        out["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap()
            .contains("```weigh-in")
    );

    // Reported once: the next session is the plain prime.
    let again = hook(&repo, &start);
    assert!(
        again.starts_with("Weigh-In (startup)") && !again.contains("systemMessage"),
        "{again}"
    );

    // A line added later is the only one counted.
    let mut f = std::fs::OpenOptions::new().append(true).open(&log).unwrap();
    writeln!(f, "2026-10-02T10:00:00Z Stop: locked").unwrap();
    let out: Value = serde_json::from_str(&hook(&repo, &start)).unwrap();
    let message = out["systemMessage"].as_str().unwrap();
    assert!(
        message.contains("1 hook error(s)") && message.contains("Stop: locked"),
        "{message}"
    );

    // A ledger SessionStart cannot read is reported in the same session, and Claude still gets
    // the convention.
    let folder = repo.parent().unwrap().join("broken");
    std::fs::create_dir_all(folder.join(".redkiln")).unwrap();
    std::fs::write(folder_store(&folder), b"not a database").unwrap();
    let out: Value = serde_json::from_str(&hook(
        &folder,
        &json!({"hook_event_name": "SessionStart", "session_id": "s1", "cwd": folder}),
    ))
    .unwrap();
    let message = out["systemMessage"].as_str().unwrap();
    assert!(
        message.contains("1 hook error(s)") && message.contains("SessionStart"),
        "{message}"
    );
    assert!(
        out["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap()
            .contains("```weigh-in")
    );
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn batch_refs_reach_items_raised_in_the_same_call() {
    let (_tmp, repo) = setup();
    let batch = json!([
        {"op": "raise", "q": "Keep the retry at three attempts?", "door": "two-way", "authority": "judgment", "channel": "sweep"},
        {"op": "triage", "id": "@1", "door": "two-way", "blast": "file", "route": "dismiss", "reason": "the config already says three"},
        {"op": "dismiss", "id": "@1", "why": "recoverable", "reason": "appsettings.json sets RetryCount=3"},
        {"op": "dismiss", "id": "@2", "why": "moot", "reason": "no second raise"}
    ]);
    let (ok, out) = weighin(&repo, "s9", &["batch"], Some(&batch.to_string()));
    assert!(!ok, "the bad ref fails the batch exit code");
    assert!(
        out.contains("✓ dismissed")
            && out.contains("recoverable")
            && out.contains("only 1 raise(s) so far"),
        "{out}"
    );
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_ref_to_an_unparsable_raise_names_no_item() {
    let (_tmp, repo) = setup();
    let batch = json!([
        {"op": "raise", "q": "Retention per record?", "source_line": "twelve"},
        {"op": "raise", "q": "Which queue, A or B?"},
        {"op": "dismiss", "id": "@1", "why": "moot", "reason": "r"},
        {"op": "present", "id": "@2"}
    ]);

    let (ok, out) = weighin(&repo, "s9", &["batch"], Some(&batch.to_string()));

    let lines: Vec<&str> = out.lines().collect();
    assert!(!ok, "{out}");
    assert_eq!(
        lines.get(2),
        Some(&"✗ op 2: `@1`: raise 1 in this batch failed, so it names no item"),
        "{out}"
    );
    assert!(
        lines
            .get(3)
            .is_some_and(|l| l.starts_with("✓ presented wi-")),
        "{out}"
    );
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn authority_items_are_not_papered_over() {
    let (_tmp, repo) = setup();
    let batch = json!([
        {"op": "raise", "q": "Must per-tenant isolation meet a compliance boundary?", "door": "unsure", "channel": "sweep"},
        {"op": "dismiss", "id": "@1", "why": "already-decided:task-directive", "reason": "user said assume and keep going"},
        {"op": "dismiss", "id": "@1", "why": "already-decided:chat:\"key-level isolation is fine, it's not regulated\"", "reason": "user's words"}
    ]);
    let (ok, out) = weighin(&repo, "s7", &["batch"], Some(&batch.to_string()));
    assert!(!ok, "{out}");
    assert!(
        out.contains("is authority-bound (`compliance`)") && out.contains("✓ dismissed"),
        "{out}"
    );
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn session_start_in_a_plain_folder_leaves_no_store_behind() {
    let (_tmp, repo) = setup();
    // #8: SessionStart in a plain folder leaves nothing behind.
    let plain = repo.parent().unwrap().join("plain");
    std::fs::create_dir_all(&plain).unwrap();
    hook(
        &plain,
        &json!({"hook_event_name": "SessionStart", "session_id": "s1", "cwd": plain, "source": "startup"}),
    );
    assert!(!wrote_in(&plain), "prime must not create a store");
}

/// #127: in the Windows desktop app the skill's shell cannot see the plugin folder, so
/// `SessionStart` copies the binary into the repository's git dir and names the copy, which runs.
#[test]
#[cfg(windows)]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn session_start_on_windows_names_a_copy_the_shell_can_run() {
    let (_tmp, repo) = setup();

    let out = hook(
        &repo,
        &json!({"hook_event_name": "SessionStart", "session_id": "s1", "cwd": repo, "source": "startup"}),
    );

    let copy = out
        .lines()
        .find_map(|l| l.strip_prefix("Weigh-In binary for the shell: "))
        .unwrap_or_else(|| panic!("no copy named in: {out}"));
    let version = Command::new(copy).arg("--version").output().unwrap();
    assert!(version.status.success(), "{copy} did not run");
    assert!(
        String::from_utf8_lossy(&version.stdout).contains(env!("CARGO_PKG_VERSION")),
        "{copy}"
    );
}

/// What `SessionStart` gave Claude: its whole output, or `additionalContext` when it also has a
/// `systemMessage` for the user and so prints JSON; `None` for JSON without it.
fn session_context(out: &str) -> Option<String> {
    match serde_json::from_str::<Value>(out) {
        Ok(json) => json
            .pointer("/hookSpecificOutput/additionalContext")
            .and_then(Value::as_str)
            .map(str::to_owned),
        Err(_) => Some(out.to_owned()),
    }
}

/// A config's `store` (a 2.x fallback) is a warning for the user, so `SessionStart` prints JSON
/// and its context, the line naming the shell's copy on Windows among it, moves into
/// `additionalContext`: the copy's tests read it there (#127).
#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn session_start_with_a_store_fallback_warns_and_keeps_its_context_for_claude() {
    let (_tmp, repo) = setup();
    std::fs::create_dir_all(repo.join(".kb/_ledger")).unwrap();
    std::fs::write(
        repo.join(".kb/_ledger/config.json"),
        r#"{"store":"../elsewhere/ledger.sqlite3"}"#,
    )
    .unwrap();

    let out = hook(
        &repo,
        &json!({"hook_event_name": "SessionStart", "session_id": "s1", "cwd": repo, "source": "startup"}),
    );

    let json: Value = serde_json::from_str(&out).unwrap();
    let message = json["systemMessage"].as_str().unwrap();
    assert!(message.contains(".kb/_ledger/config.json"), "{message}");
    let context = session_context(&out).expect("SessionStart's JSON has additionalContext");
    assert!(
        context.starts_with("Weigh-In (startup)") && context.contains("```weigh-in"),
        "{context}"
    );
    assert_eq!(
        context.contains("Weigh-In binary for the shell: "),
        cfg!(windows),
        "{context}"
    );
}

/// The copy stays in the repository's git dir even when the config keeps the ledger elsewhere,
/// a folder the shell may not see (#127).
#[test]
#[cfg(windows)]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn session_start_on_windows_copies_into_the_git_dir_whatever_the_store() {
    let (_tmp, repo) = setup();
    std::fs::create_dir_all(repo.join(".kb/_ledger")).unwrap();
    std::fs::write(
        repo.join(".kb/_ledger/config.json"),
        r#"{"store":"../elsewhere/ledger.sqlite3"}"#,
    )
    .unwrap();

    let out = hook(
        &repo,
        &json!({"hook_event_name": "SessionStart", "session_id": "s1", "cwd": repo, "source": "startup"}),
    );

    // The config's `store` is a 2.x fallback, so SessionStart also has a warning for the user
    // and Claude's context is in `additionalContext`.
    let context = session_context(&out).expect("SessionStart's JSON has additionalContext");
    let copy = context
        .lines()
        .find_map(|l| l.strip_prefix("Weigh-In binary for the shell: "))
        .unwrap_or_else(|| panic!("no copy named in: {out}"));
    let git_dir = repo.join(".git").canonicalize().unwrap();
    assert!(
        std::path::Path::new(copy)
            .canonicalize()
            .unwrap()
            .starts_with(&git_dir),
        "{copy} is outside {}",
        git_dir.display()
    );
    let elsewhere = repo.parent().unwrap().join("elsewhere");
    assert!(!elsewhere.join("bin").exists() && !elsewhere.join("weighin/bin").exists());
}

/// Off Windows the plugin folder is reachable, so `SessionStart` copies nothing (#127).
#[test]
#[cfg(not(windows))]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn session_start_off_windows_copies_no_binary() {
    let (_tmp, repo) = setup();

    let out = hook(
        &repo,
        &json!({"hook_event_name": "SessionStart", "session_id": "s1", "cwd": repo, "source": "startup"}),
    );

    assert!(!out.contains("Weigh-In binary for the shell"), "{out}");
    assert!(!repo.join(".git/weighin/bin").exists() && !state_of(&repo).join("bin").exists());
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_write_outside_the_repository_is_not_scanned() {
    let (_tmp, repo) = setup();
    // #6: a Write outside the repository is not scanned.
    let outside = repo.parent().unwrap().join("outside.md");
    let text = "## Open questions\n- Should the outside file be touched?\n";
    std::fs::write(&outside, text).unwrap();
    hook(
        &repo,
        &json!({"hook_event_name": "PostToolUse", "session_id": "s1", "cwd": repo, "tool_name": "Write", "tool_input": {"file_path": outside, "content": text}}),
    );
    let (_, open) = weighin(&repo, "s1", &["open", "--repo"], None);
    assert!(open.contains("0 open in repo"), "{open}");
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn annotating_a_crlf_file_keeps_it_crlf() {
    let (_tmp, repo) = setup();
    // #7: annotating a CRLF file keeps it CRLF.
    settle_crlf_question(&repo);
    let after = std::fs::read_to_string(repo.join("docs/spec.md")).unwrap();
    assert!(
        after.contains("decided yes")
            && after.matches("\r\n").count() == 4
            && !after.replace("\r\n", "").contains('\n'),
        "{after:?}"
    );
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_settled_question_raised_again_is_not_re_asked() {
    let (_tmp, repo) = setup();
    let id = settle_crlf_question(&repo);
    // #5: a settled question raised again (another session) is not re-asked.
    let (_, out) = weighin(
        &repo,
        "s2",
        &["raise", "--q", "Keep the queue durable?"],
        None,
    );
    assert!(out.contains(&format!("{id} already raised")), "{out}");
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_bare_defer_next_to_a_decide_in_one_batch_applies_both() {
    let (_tmp, repo) = setup();
    // #3: a bare defer next to a decide in one batch: both apply.
    let batch = json!([
        {"op": "raise", "q": "Pick A or B for the cache?", "door": "two-way", "channel": "sweep"},
        {"op": "raise", "q": "Which region hosts the backup?", "door": "one-way", "channel": "sweep"},
        {"op": "decide", "id": "@1", "option": "A"},
        {"op": "defer", "id": "@2", "unqualified": true, "trigger": "next sitting", "resolve_by": "2026-10-04", "default": "west"},
        {"op": "no-such-op", "id": "@1"}
    ]);
    let (_, out) = weighin(&repo, "s3", &["batch"], Some(&batch.to_string()));
    assert!(
        out.contains("✓ decided") && out.contains("✓ deferred") && out.contains("✗ op 4"),
        "{out}"
    );
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn open_json_is_weighin_contract_s_type_byte_for_byte() {
    let (_tmp, repo) = setup();
    let batch = json!([
        {"op": "raise", "q": "Which store for sessions?", "door": "two-way", "channel": "sweep"},
        {"op": "triage", "id": "@1", "door": "two-way", "blast": "module", "route": "present", "reason": "r"},
        {"op": "ground", "id": "@1", "card": {"options": [{"id": "A", "title": "Postgres"}, {"id": "B", "title": "SQLite"}], "recommendation": {"option": "A"}}},
        {"op": "raise", "q": "Keep three retries?", "door": "one-way"}
    ]);
    let (ok, out) = weighin(&repo, "s4", &["batch"], Some(&batch.to_string()));
    assert!(ok, "{out}");

    let (ok, json) = weighin(&repo, "s4", &["open", "--json"], None);
    assert!(ok, "{json}");
    let items: Vec<weighin_contract::open::OpenItem> =
        serde_json::from_str(json.trim()).expect("open --json reads as the contract's type");

    assert_eq!(items.len(), 2, "{json}");
    assert_eq!(serde_json::to_string(&items).unwrap(), json.trim());
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn an_answer_to_an_older_card_is_refused() {
    let (_tmp, repo) = setup();
    // #2: an answer to an older card is refused.
    let batch = json!([
        {"op": "raise", "q": "Which store for sessions?", "door": "two-way", "channel": "sweep"},
        {"op": "ground", "id": "@1", "card": {"options": [{"id": "A", "title": "Postgres"}, {"id": "B", "title": "SQLite"}], "recommendation": {"option": "A"}}}
    ]);
    let (ok, out) = weighin(&repo, "s4", &["batch"], Some(&batch.to_string()));
    assert!(ok, "{out}");
    let (_, j) = weighin(&repo, "s4", &["open", "--json"], None);
    let items: Vec<Value> = serde_json::from_str(j.trim()).unwrap();
    let sid = items[0]["id"].as_str().unwrap().to_owned();
    let old_rev = items[0]["card_rev"].as_u64().unwrap();
    let regrounded = json!([{"op": "ground", "id": sid, "card": {"options": [{"id": "A", "title": "SQLite"}, {"id": "B", "title": "Postgres"}], "recommendation": {"option": "A"}}}]);
    weighin(&repo, "s4", &["batch"], Some(&regrounded.to_string()));
    let answers = repo.join("stale.json");
    std::fs::write(
        &answers,
        json!({"item_id": sid, "action": "choose", "option": "B", "card_rev": old_rev}).to_string(),
    )
    .unwrap();
    let (_, out) = weighin(
        &repo,
        "s4",
        &["board", "apply", answers.to_str().unwrap()],
        None,
    );
    assert!(
        out.contains("re-grounded after this answer") && out.contains("applied: none"),
        "{out}"
    );
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn launcher_does_not_loop_when_no_binary_exists() {
    let (_tmp, launcher, path) = isolated_launcher();
    let limit = LAUNCHER_LIMIT;
    let start = std::time::Instant::now();
    let out = run_within(
        Command::new("sh")
            .arg(&launcher)
            .arg("open")
            .env("PATH", &path),
        limit,
    );
    assert_eq!(
        out.status.code(),
        Some(127),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(start.elapsed() < limit);
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn launcher_hook_without_a_binary_never_fails_and_is_silent() {
    let (_tmp, launcher, path) = isolated_launcher();
    let hook = run_within(
        Command::new("sh")
            .arg(&launcher)
            .arg("hook")
            .env("PATH", &path),
        LAUNCHER_LIMIT,
    );
    assert_eq!(
        hook.status.code(),
        Some(0),
        "a missing binary never fails a hook"
    );
    assert!(hook.stdout.is_empty(), "only SessionStart speaks");
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn launcher_session_start_without_a_binary_says_weighin_is_off() {
    let (_tmp, launcher, path) = isolated_launcher();
    // SessionStart says weighin is off, in a message shown to the user, and still exits 0.
    let start = run_within(
        Command::new("sh")
            .arg(&launcher)
            .args(["hook", "SessionStart"])
            .env("PATH", &path)
            .env_remove("CLAUDE_CODE_ENTRYPOINT"),
        LAUNCHER_LIMIT,
    );
    assert_eq!(start.status.code(), Some(0));
    let out: Value = serde_json::from_slice(&start.stdout).unwrap();
    assert!(
        out["systemMessage"]
            .as_str()
            .unwrap()
            .contains("weighin is off"),
        "{out}"
    );
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn launcher_command_names_a_copy_with_no_binaries() {
    // No build for any platform: the development tree, synced or copied as is (B-18). The suite's
    // shared launcher names the missing build the same way whatever else the copy holds.
    let (_tmp, launcher, path) = isolated_launcher();

    let run = run_within(
        Command::new("sh")
            .arg(&launcher)
            .arg("open")
            .env("PATH", &path),
        LAUNCHER_LIMIT,
    );

    assert_eq!(run.status.code(), Some(127));
    let err = String::from_utf8_lossy(&run.stderr);
    assert!(err.starts_with("weighin: no weighin build for "), "{err}");
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn launcher_session_start_names_a_copy_with_no_binaries() -> Result<(), Box<dyn std::error::Error>>
{
    let (_tmp, launcher, path) = isolated_launcher();

    let start = run_within(
        Command::new("sh")
            .arg(&launcher)
            .args(["hook", "SessionStart"])
            .env("PATH", &path)
            .env_remove("CLAUDE_CODE_ENTRYPOINT"),
        LAUNCHER_LIMIT,
    );

    assert_eq!(start.status.code(), Some(0));
    let out: Value = serde_json::from_slice(&start.stdout)?;
    let message = out["systemMessage"].as_str().ok_or("no systemMessage")?;
    assert!(
        message.starts_with("weighin is off: no weighin build for "),
        "{out}"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn launcher_with_only_another_platforms_build_names_the_platform()
-> Result<(), Box<dyn std::error::Error>> {
    // A build for some other platform only: the copy has binaries, just not this one's.
    let (tmp, launcher, path) = isolated_launcher();
    let other = tmp.path().join("bin/riscv64gc-unknown-linux-gnu");
    std::fs::create_dir_all(&other)?;
    std::fs::write(other.join("weighin"), "not run")?;

    let run = run_within(
        Command::new("sh")
            .arg(&launcher)
            .arg("open")
            .env("PATH", &path),
        LAUNCHER_LIMIT,
    );

    assert_eq!(run.status.code(), Some(127));
    let err = String::from_utf8_lossy(&run.stderr);
    assert!(err.starts_with("weighin: no weighin build for "), "{err}");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn launcher_session_start_in_cowork_without_a_binary_is_silent() {
    let (_tmp, launcher, path) = isolated_launcher();
    // Cowork captures through the sitting, not hooks: nothing is said there (B-04).
    let cowork = run_within(
        Command::new("sh")
            .arg(&launcher)
            .args(["hook", "SessionStart"])
            .env("PATH", &path)
            .env("CLAUDE_CODE_ENTRYPOINT", "remote_cowork"),
        LAUNCHER_LIMIT,
    );
    assert_eq!(cowork.status.code(), Some(0));
    assert!(cowork.stdout.is_empty());
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn launcher_passes_session_start_to_the_binary() {
    let Some((target, ext)) = launcher_target() else {
        return;
    };
    let (_tmp, repo) = setup();
    let plugin = repo.parent().unwrap().join("plugin");
    std::fs::create_dir_all(plugin.join(target)).unwrap();
    let launcher = plugin.join("weighin");
    std::fs::copy(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../plugins/weigh-in/libexec/weighin"
        ),
        &launcher,
    )
    .unwrap();
    std::fs::copy(bin(), plugin.join(target).join(format!("weighin{ext}"))).unwrap();
    let payload = repo.parent().unwrap().join("start.json");
    std::fs::write(
        &payload,
        json!({"hook_event_name": "SessionStart", "session_id": "s1", "cwd": repo, "source": "startup"})
            .to_string(),
    )
    .unwrap();
    let out = run_within(
        Command::new("sh")
            .arg(&launcher)
            .args(["hook", "SessionStart"])
            .env_remove("CLAUDE_CODE_ENTRYPOINT")
            .stdin(std::fs::File::open(&payload).unwrap()),
        std::time::Duration::from_secs(30),
    );
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.starts_with("Weigh-In (startup): session s1"),
        "{text}{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// `weighin.cmd` runs the msvc build unless the gnu build is newer, as the `sh` launcher does
/// (B-05), and passes the exit code back. One target holds the real binary and the other a copy
/// of `cmd.exe`; given `/d /c exit 7`, `weighin` refuses the arguments (exit 2, its usage on
/// stderr) and `cmd.exe` exits 7, so the result says which one ran.
#[cfg(windows)]
#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn cmd_launcher_runs_the_newer_build() {
    use std::path::{Path, PathBuf};
    use std::time::{Duration, SystemTime};
    let tmp = tempfile::tempdir().unwrap();
    let launcher = tmp.path().join("weighin.cmd");
    std::fs::copy(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../plugins/weigh-in/libexec/weighin.cmd"
        ),
        &launcher,
    )
    .unwrap();
    let msvc = tmp.path().join("x86_64-pc-windows-msvc/weighin.exe");
    let gnu = tmp.path().join("x86_64-pc-windows-gnu/weighin.exe");
    let stand_in = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/cmd.exe");
    let set_age = |path: &Path, hours_ago: u64| {
        let t = SystemTime::now() - Duration::from_secs(hours_ago * 3600);
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(t)
            .unwrap();
    };
    let ran = || {
        let out = run_within(
            Command::new(&launcher).args(["/d", "/c", "exit", "7"]),
            Duration::from_secs(30),
        );
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        match out.status.code() {
            Some(2) if stderr.contains("weighin") => "msvc",
            Some(7) => "gnu",
            code => panic!("neither build ran: {code:?} {stderr}"),
        }
    };
    for (path, from) in [(&msvc, Path::new(bin())), (&gnu, stand_in.as_path())] {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::copy(from, path).unwrap();
    }

    // One build only: it runs.
    std::fs::rename(&gnu, tmp.path().join("gnu-aside.exe")).unwrap();
    assert_eq!(ran(), "msvc", "msvc alone runs");
    std::fs::rename(tmp.path().join("gnu-aside.exe"), &gnu).unwrap();
    std::fs::rename(&msvc, tmp.path().join("msvc-aside.exe")).unwrap();
    assert_eq!(ran(), "gnu", "gnu alone runs");
    std::fs::rename(tmp.path().join("msvc-aside.exe"), &msvc).unwrap();

    // Both: the newer one runs.
    set_age(&gnu, 2);
    set_age(&msvc, 1);
    assert_eq!(ran(), "msvc", "the newer msvc build runs");
    set_age(&msvc, 3);
    assert_eq!(
        ran(),
        "gnu",
        "a stale msvc build does not shadow a newer gnu build"
    );
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn cowork_hooks_write_nothing_and_still_nudge() {
    let (_tmp, repo) = setup();
    let cowork = [("CLAUDE_CODE_ENTRYPOINT", "remote_cowork")];
    let run = |payload: &Value| {
        let (ok, out) = weighin_env(
            &repo,
            &Run {
                session: "unused",
                args: &["hook"],
                stdin: Some(&payload.to_string()),
                vars: &cowork,
            },
        );
        assert!(ok, "{out}");
        out
    };
    let prime = run(&json!({"hook_event_name": "SessionStart", "session_id": "s1", "cwd": repo}));
    assert!(
        prime.contains("(cowork)") && prime.contains("```weigh-in"),
        "{prime}"
    );
    let nudge = run(
        &json!({"hook_event_name": "Stop", "session_id": "s1", "cwd": repo, "last_assistant_message": MESSAGE}),
    );
    assert!(nudge.contains("1 one-way question"), "{nudge}");
    let edit = run(
        &json!({"hook_event_name": "PostToolUse", "session_id": "s1", "cwd": repo, "tool_name": "Write",
        "tool_input": {"file_path": repo.join("docs/plan.md"), "content": "## Open questions\n- Which tenant first?\n"}}),
    );
    assert!(edit.is_empty(), "{edit}");
    assert!(
        !repo.join(".git/weighin").exists() && !repo.join(".git/redkiln").exists(),
        "no ledger in the Cowork scratch folder"
    );
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn the_ledger_goes_where_the_folder_says() {
    // A non-git folder, as a Cowork connected folder usually is.
    let tmp = tempfile::tempdir().unwrap();
    let folder = tmp.path().join("Rulings");
    std::fs::create_dir_all(&folder).unwrap();
    let raise = r#"[{"op":"raise","q":"Which rule wins when two rulings conflict?","kind":"question","door":"two-way","authority":"authority","channel":"sweep"}]"#;

    // Default: <folder>/.redkiln/, under a TRUNCATE journal.
    let (ok, out) = weighin(&folder, "s1", &["batch"], Some(raise));
    assert!(ok, "{out}");
    let default = folder_store(&folder);
    assert!(default.exists());
    assert_eq!(
        std::fs::metadata(folder.join(".redkiln/events.sqlite3-journal"))
            .unwrap()
            .len(),
        0
    );
    assert!(!folder.join(".redkiln/events.sqlite3-wal").exists());

    // `init --ledger` moves it outside the folder, relative to the folder root.
    let (ok, out) = weighin(
        &folder,
        "s2",
        &[
            "init",
            "--class",
            "own",
            "--ledger",
            "../ledgers/rulings.sqlite3",
        ],
        None,
    );
    assert!(ok, "{out}");
    assert!(out.contains("ledgers"), "{out}");
    let (ok, out) = weighin(&folder, "s2", &["batch"], Some(raise));
    assert!(ok, "{out}");
    // D2 B: the key names a 1.x ledger, and the store goes beside it.
    assert!(tmp.path().join("ledgers/rulings.events.sqlite3").exists());
    let (ok, out) = weighin(&folder, "s2", &["open", "--repo"], None);
    assert!(ok && out.contains("Which rule wins"), "{out}");

    // `--store` wins over the config.
    let flag = tmp.path().join("flag.sqlite3");
    let (ok, out) = weighin(
        &folder,
        "s3",
        &["--store", flag.to_str().unwrap(), "open", "--repo"],
        None,
    );
    assert!(ok, "{out}");
    assert!(!out.contains("Which rule wins"), "{out}");
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn scan_reads_a_relative_path_under_dir_not_the_process_cwd() {
    let (tmp, repo) = setup();
    std::fs::write(
        repo.join("docs/a.md"),
        "# A\n\n## Open questions\n- Should the ledger use Postgres?\n",
    )
    .unwrap();
    let other = tmp.path().join("other");
    std::fs::create_dir_all(other.join("docs")).unwrap();
    std::fs::write(
        other.join("docs/a.md"),
        "# A\n\n## Open questions\n- Should the OTHER file decide on tabs?\n",
    )
    .unwrap();
    let dir = repo.to_str().unwrap();

    let (ok, out) = weighin(&other, "s1", &["--dir", dir, "scan", "docs/a.md"], None);

    assert!(ok && out.starts_with("raised "), "{out}");
    let (_, open) = weighin(&repo, "s1", &["open", "--repo"], None);
    assert!(
        open.contains("Should the ledger use Postgres?") && !open.contains("OTHER"),
        "{open}"
    );
    let (_, json) = weighin(&repo, "s1", &["open", "--repo", "--json"], None);
    let items: Vec<Value> = serde_json::from_str(json.trim()).unwrap();
    assert_eq!(items[0]["at"], json!("docs/a.md:4"), "{json}");
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn init_refuses_an_unknown_class_and_writes_no_config() {
    let (_tmp, repo) = setup();

    // A typo for `client`: before #169 it was stored with the `artifact` board.
    let (ok, out) = weighin(&repo, "s1", &["init", "--class", "clinet"], None);

    assert!(!ok && out.contains("clinet"), "{out}");
    assert!(!repo.join(".kb/_ledger/config.json").exists(), "{out}");
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn init_refuses_an_unknown_board_and_keeps_the_config() {
    let (_tmp, repo) = setup();
    let (ok, out) = weighin(&repo, "s1", &["init", "--class", "client"], None);
    assert!(ok, "{out}");

    let (ok, out) = weighin(&repo, "s1", &["init", "--board", "locl"], None);

    assert!(!ok && out.contains("locl"), "{out}");
    let config: Value = serde_json::from_str(
        &std::fs::read_to_string(repo.join(".kb/_ledger/config.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(config["board"]["mode"], json!("local"), "{config}");
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn init_without_class_or_board_keeps_the_existing_ones() {
    let (_tmp, repo) = setup();
    let (ok, out) = weighin(&repo, "s1", &["init", "--class", "client"], None);
    assert!(ok, "{out}");

    let (ok, out) = weighin(&repo, "s1", &["init", "--ledger", "../l.sqlite3"], None);

    assert!(ok && out.contains("class client · board local"), "{out}");
    let config: Value = serde_json::from_str(
        &std::fs::read_to_string(repo.join(".kb/_ledger/config.json")).unwrap(),
    )
    .unwrap();
    // The class is suite.toml's since #17 part 5b, and config.json carries none.
    assert_eq!(
        (
            std::fs::read_to_string(repo.join(".redkiln/suite.toml")).unwrap(),
            &config["repo_class"],
            &config["board"]["mode"],
            &config["store"]
        ),
        (
            "[suite]\nrepo_class = \"client\"\n".to_owned(),
            &Value::Null,
            &json!("local"),
            &json!("../l.sqlite3")
        )
    );
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_first_init_without_flags_asks_for_a_class_and_writes_nothing() {
    let (_tmp, repo) = setup();

    let (ok, out) = weighin(&repo, "s1", &["init"], None);

    // Ryan's F1 decision (#36, option A): no class anywhere is not guessed as `own`.
    assert!(
        !ok && out.contains("--class own") && out.contains("--class client"),
        "{out}"
    );
    assert!(
        !repo.join(".kb/_ledger/config.json").exists() && !repo.join(".redkiln").exists(),
        "{out}"
    );
}

/// Every command that only reads, run where no ledger exists yet.
const READ_ONLY: &[&[&str]] = &[
    &["stats"],
    &["stats", "--json"],
    &["open"],
    &["open", "--all", "--json"],
    &["digest"],
    &["digest", "--json"],
    &["prime"],
    &["board", "export"],
    &["journal"],
];

/// Read-only commands that name an item; with no ledger there is none to find.
const READ_ONLY_BY_ID: &[&[&str]] = &[
    &["show", "wi-000000"],
    &["context", "wi-000000"],
    &["trailer", "wi-000000"],
];

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn the_cowork_sweep_raises_every_block_into_the_folder_ledger() {
    // Cowork's sweep pipes a conversation to `raise --markers` with `--dir` on a connected
    // folder, which is not a git repository (D-0002).
    let tmp = tempfile::tempdir().unwrap();
    let folder = tmp.path().join("Rulings");
    std::fs::create_dir_all(&folder).unwrap();
    let dir = folder.to_str().unwrap();

    let (ok, out) = weighin(
        tmp.path(),
        "s1",
        &["--dir", dir, "raise", "--markers"],
        Some(MESSAGE),
    );

    assert!(ok, "{out}");
    assert_eq!(
        out.lines().filter(|l| l.starts_with("raised ")).count(),
        2,
        "{out}"
    );
    assert!(folder_store(&folder).exists(), "{out}");
    assert!(
        !wrote_in(tmp.path()),
        "the ledger follows --dir, not the process's cwd"
    );
    let (ok, open) = weighin(
        tmp.path(),
        "s1",
        &["--dir", dir, "open", "--all", "--json"],
        None,
    );
    assert!(ok, "{open}");
    let items: Vec<Value> = serde_json::from_str(&open).unwrap();
    let channels: Vec<&str> = items
        .iter()
        .map(|i| i["channel"].as_str().unwrap())
        .collect();
    assert_eq!(channels, ["sweep", "sweep"], "{open}");
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn raise_with_no_blocks_leaves_no_ledger_behind() -> Result<(), Box<dyn std::error::Error>> {
    // Cowork's sweep pipes a conversation to `raise --markers`; with no blocks in it, a folder
    // that refuses deletes must not be left an empty ledger (B-01).
    let tmp = tempfile::tempdir()?;
    let folder = tmp.path().join("empty");
    std::fs::create_dir_all(&folder)?;
    let dir = folder.to_str().ok_or("temp path is not UTF-8")?;

    let run = weighin(
        tmp.path(),
        "s1",
        &["--dir", dir, "raise", "--markers"],
        Some("No decisions in this conversation.\n"),
    );

    assert_eq!(
        run,
        (
            false,
            "weighin: no weigh-in blocks found on stdin\n".to_owned()
        )
    );
    assert!(!wrote_in(&folder), "raise created .weighin/");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_decide_that_finds_no_item_leaves_no_ledger_behind() {
    let (ok, out, created) = in_a_folder_with_no_ledger(&["decide", "wi-000000", "A"], None);

    assert!(!ok && out.contains("no single item matches"), "{out}");
    assert!(!created, "decide created .weighin/");
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn an_empty_batch_leaves_no_ledger_behind() {
    let (ok, out, created) = in_a_folder_with_no_ledger(&["batch"], Some("[]"));

    assert!(ok, "{out}");
    assert!(!created, "batch created .weighin/");
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_board_apply_with_no_answers_leaves_no_ledger_behind() {
    let (ok, out, created) = in_a_folder_with_no_ledger(&["board", "apply", "<answers>"], None);

    assert_eq!((ok, out.as_str()), (true, "applied: none\n"));
    assert!(!created, "board apply created .weighin/");
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_context_pack_lists_a_decided_atom_on_the_same_file() {
    let (_tmp, repo) = setup();
    let raise = |q: &str, at: &str| {
        let (ok, out) = weighin(&repo, "s1", &["raise", "--q", q, "--at", at], None);
        assert!(ok, "{out}");
        out.split_whitespace().nth(1).unwrap().to_owned()
    };
    let decided = raise("Keep overrides per series?", "src/retention.rs:6");
    let (ok, out) = weighin(&repo, "s1", &["decide", &decided, "yes"], None);
    assert!(ok, "{out}");
    // Written straight to `.kb/decision/`, outside the `_intake` context packs skip.
    assert!(
        repo.join(".kb/decision").is_dir(),
        "the decision wrote its atom"
    );
    let open = raise("Cap overrides per tenant?", "src/retention.rs:13");

    let (ok, ctx) = weighin(&repo, "s1", &["context", &open], None);

    assert!(ok, "{ctx}");
    let atom = knowledge_contract::AtomId::hashed(
        knowledge_contract::Kind::Decision,
        &decided,
        knowledge_contract::HashLen::Ten,
    );
    let listed = format!(".kb: {atom} [accepted] Keep overrides per series: yes");
    assert!(ctx.lines().any(|l| l == listed), "{ctx}");
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn an_anchor_line_past_usize_max_does_not_crash_a_context_pack() {
    let (_tmp, repo) = setup();
    for (q, at) in [
        (
            "Is the huge anchor fine?",
            "src/retention.rs:18446744073709551615",
        ),
        (
            "Is a missing file fine?",
            "src/nope.rs:18446744073709551615",
        ),
    ] {
        let (ok, out) = weighin(&repo, "s1", &["raise", "--q", q, "--at", at], None);
        assert!(ok, "{out}");
        let id = out.split_whitespace().nth(1).unwrap().to_owned();

        let (ok, out) = weighin(&repo, "s1", &["context", &id], None);

        assert!(ok && out.contains(&format!("## {id} context")), "{out}");
    }
    let (ok, out) = weighin(&repo, "s1", &["open", "--with-context"], None);
    assert!(ok && out.contains("2 open in this session"), "{out}");
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn read_only_commands_leave_no_ledger_behind() {
    // A non-git folder, reached with `--dir` as Cowork does, and a git repository.
    let tmp = tempfile::tempdir().unwrap();
    let folder = tmp.path().join("empty");
    std::fs::create_dir_all(&folder).unwrap();
    let (_repo_tmp, repo) = setup();
    let dir = folder.to_str().unwrap();

    for args in READ_ONLY {
        let (ok, out) = weighin(tmp.path(), "s1", &[&["--dir", dir], *args].concat(), None);
        assert!(ok, "{args:?}: {out}");
        let (ok, out) = weighin(&repo, "s1", args, None);
        assert!(ok, "{args:?}: {out}");
    }
    for args in READ_ONLY_BY_ID {
        let (ok, out) = weighin(tmp.path(), "s1", &[&["--dir", dir], *args].concat(), None);
        assert!(
            !ok && out.contains("no single item matches"),
            "{args:?}: {out}"
        );
        let (ok, out) = weighin(&repo, "s1", args, None);
        assert!(
            !ok && out.contains("no single item matches"),
            "{args:?}: {out}"
        );
    }

    let (ok, out) = weighin(tmp.path(), "s1", &["--dir", dir, "stats"], None);
    assert!(ok && out.contains("presented 0"), "{out}");
    // An explicit `--out` still gets the (empty) board, and only that file is written.
    let board = tmp.path().join("board.json");
    let out_arg = board.to_str().unwrap();
    let (ok, out) = weighin(
        tmp.path(),
        "s1",
        &["--dir", dir, "board", "export", "--out", out_arg],
        None,
    );
    assert!(ok, "{out}");
    let state: Value = serde_json::from_str(&std::fs::read_to_string(&board).unwrap()).unwrap();
    for key in ["items", "decided", "defaults"] {
        assert_eq!(state[key], json!([]), "{key} in {state}");
    }
    let (ok, out) = weighin(tmp.path(), "s1", &["--dir", dir, "board", "export"], None);
    assert!(ok && out.contains("nothing to export"), "{out}");
    assert!(!wrote_in(&folder), "a read created .weighin/");
    assert!(
        !repo.join(".git/weighin").exists() && !repo.join(".git/redkiln").exists(),
        "a read created .git/weighin/"
    );
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn an_option_spanning_lines_is_decided_but_not_annotated() {
    let (_tmp, repo) = setup();
    let source = "# Plan\n\n## Open questions\n- Should the cap be fixed?\n";
    std::fs::write(repo.join("docs/cap.md"), source).unwrap();
    let id = scan_one(&repo, "docs/cap.md");

    let (ok, out) = weighin(&repo, "s1", &["decide", &id, "10\n- Ship it?"], None);

    assert!(
        ok && out.contains("not annotated docs/cap.md (the option does not fit on one line)"),
        "{out}"
    );
    assert_eq!(
        std::fs::read_to_string(repo.join("docs/cap.md")).unwrap(),
        source
    );
}
