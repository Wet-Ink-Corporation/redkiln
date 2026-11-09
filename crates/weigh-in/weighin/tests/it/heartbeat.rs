//! The hook heartbeat (HK-5): a hook of a registered event keeps a slot in
//! `<git-common-dir>/redkiln/weighin/` while it runs, only where that `redkiln/` exists, never
//! in Cowork or outside git; the next hook logs a slot left `running` by a hook that died, once,
//! and the next `SessionStart` reports it. Slots are used only when they are regular files
//! (HK-6).

use crate::support::{MESSAGE, Run, hook, setup, state_of, weighin_env};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// The slot of `event` in the repository's state dir.
fn slot_of(repo: &Path, event: &str) -> PathBuf {
    state_of(repo).join(format!("hook-heartbeat-{event}"))
}

/// The value after `key` in the slot `text`.
fn field<'t>(text: &'t str, key: &str) -> Option<&'t str> {
    text.lines()
        .find_map(|line| line.strip_prefix(key)?.strip_prefix(' '))
}

/// `<git-common-dir>/redkiln/`, which a hook keeps a heartbeat in only when it exists.
fn make_redkiln(repo: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(repo.join(".git").join("redkiln"))
}

fn stop(dir: &Path, message: &str) -> Value {
    json!({"hook_event_name": "Stop", "session_id": "s1", "cwd": dir, "last_assistant_message": message})
}

fn now() -> Result<u64, Box<dyn std::error::Error>> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}

/// A slot as a hook writes it, `running` since `started` with its last beat at `beat`.
fn running(started: u64, beat: u64, timeout: u64) -> String {
    format!(
        "weighin-heartbeat 1\nstate running\nstarted {started}\nbeat {beat}\ntimeout {timeout}\npid 1\n"
    )
}

/// The lines of the error log that tell a dead hook.
fn lost_lines(repo: &Path) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let log = match std::fs::read_to_string(state_of(repo).join("hook-errors.log")) {
        Ok(log) => log,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.into()),
    };
    Ok(log
        .lines()
        .filter(|line| line.contains(": the hook started at "))
        .map(str::to_owned)
        .collect())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_finished_hook_leaves_a_finished_slot() -> TestResult {
    let (_tmp, repo) = setup();
    make_redkiln(&repo)?;
    let before = now()?;

    hook(&repo, &stop(&repo, MESSAGE));

    let text = std::fs::read_to_string(slot_of(&repo, "Stop"))?;
    assert_eq!(text.lines().next(), Some("weighin-heartbeat 1"), "{text}");
    assert_eq!(field(&text, "state"), Some("finished"), "{text}");
    assert_eq!(field(&text, "timeout"), Some("20"), "{text}");
    let started: u64 = field(&text, "started").ok_or("no start")?.parse()?;
    let beat: u64 = field(&text, "beat").ok_or("no beat")?.parse()?;
    assert!(
        before <= started && started <= beat && beat <= now()?,
        "{text}"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn no_heartbeat_without_the_redkiln_folder_in_cowork_or_outside_git() -> TestResult {
    // No `redkiln/` yet: a Stop that captures nothing creates none.
    let (_tmp, repo) = setup();
    hook(&repo, &stop(&repo, "Nothing to capture."));
    assert!(
        !repo.join(".git").join("redkiln").exists(),
        "no hook creates <git-common-dir>/redkiln/"
    );

    // Cowork writes nothing, even where `redkiln/` exists.
    let (_tmp2, cowork) = setup();
    make_redkiln(&cowork)?;
    let payload = stop(&cowork, MESSAGE).to_string();
    let (ok, out) = weighin_env(
        &cowork,
        &Run {
            session: "unused",
            args: &["hook"],
            stdin: Some(&payload),
            vars: &[("CLAUDE_CODE_ENTRYPOINT", "remote_cowork")],
        },
    );
    assert!(ok, "{out}");
    assert!(!slot_of(&cowork, "Stop").exists(), "no slot in Cowork");

    // Outside git there is no default state dir, whatever `.redkiln/` holds.
    let tmp3 = tempfile::tempdir()?;
    let folder = tmp3.path().join("folder");
    std::fs::create_dir_all(folder.join(".redkiln").join("weighin"))?;
    hook(&folder, &stop(&folder, MESSAGE));
    assert!(
        !folder
            .join(".redkiln")
            .join("weighin")
            .join("hook-heartbeat-Stop")
            .exists(),
        "no slot outside git"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_dead_running_slot_is_logged_once_by_the_next_hook() -> TestResult {
    let (_tmp, repo) = setup();
    std::fs::create_dir_all(state_of(&repo))?;
    let now = now()?;
    let started = now.saturating_sub(100);
    // A 10 s PostToolUse whose last beat came 2 s before its deadline: its timeout killed it.
    std::fs::write(
        slot_of(&repo, "PostToolUse"),
        running(started, started + 8, 10),
    )?;
    // A 20 s Stop whose last beat came 1 s after it started: it was interrupted.
    std::fs::write(slot_of(&repo, "Stop"), running(started, started + 1, 20))?;
    let start = json!({"hook_event_name": "SessionStart", "session_id": "s1", "cwd": repo, "source": "startup"});

    let printed = hook(&repo, &start);

    let lost = lost_lines(&repo)?;
    assert_eq!(lost.len(), 2, "{lost:?}");
    assert!(
        lost.iter()
            .any(|line| line.contains(" PostToolUse: the hook started at ")
                && line.ends_with("was killed by its 10 s timeout")),
        "{lost:?}"
    );
    assert!(
        lost.iter()
            .any(|line| line.contains(" Stop: the hook started at ")
                && line.ends_with(
                    "ended 19 s before its 20 s timeout without finishing (interrupted)"
                )),
        "{lost:?}"
    );
    let out: Value = serde_json::from_str(&printed)?;
    let message = out
        .get("systemMessage")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("no systemMessage in {printed}"))?;
    assert!(
        message.contains("hook error(s) since the last session")
            && message.contains(": the hook started at "),
        "{message}"
    );
    let posted = std::fs::read_to_string(slot_of(&repo, "PostToolUse"))?;
    assert_eq!(field(&posted, "state"), Some("timed-out"), "{posted}");
    let stopped = std::fs::read_to_string(slot_of(&repo, "Stop"))?;
    assert_eq!(field(&stopped, "state"), Some("interrupted"), "{stopped}");

    hook(&repo, &start);

    assert_eq!(lost_lines(&repo)?.len(), 2, "logged once");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_same_event_slot_past_its_deadline_is_logged_even_with_a_fresh_beat() -> TestResult {
    let (_tmp, repo) = setup();
    std::fs::create_dir_all(state_of(&repo))?;
    let now = now()?;
    // A 20 s Stop that started 25 s ago and beat 2 s ago: its timeout killed it moments ago,
    // and the next Stop starts before its last beat is stale.
    std::fs::write(
        slot_of(&repo, "Stop"),
        running(now.saturating_sub(25), now.saturating_sub(2), 20),
    )?;

    hook(&repo, &stop(&repo, "Nothing to capture."));

    let lost = lost_lines(&repo)?;
    assert_eq!(lost.len(), 1, "{lost:?}");
    assert!(
        lost.iter()
            .all(|line| line.contains(" Stop: the hook started at ")
                && line.ends_with("was killed by its 20 s timeout")),
        "{lost:?}"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_lost_line_that_cannot_be_logged_is_noted_with_the_dead_event() -> TestResult {
    let (_tmp, repo) = setup();
    // A directory where the log goes: nothing can be appended.
    std::fs::create_dir_all(state_of(&repo).join("hook-errors.log"))?;
    let now = now()?;
    let started = now.saturating_sub(100);
    std::fs::write(
        slot_of(&repo, "PostToolUse"),
        running(started, started + 8, 10),
    )?;

    let printed = hook(&repo, &stop(&repo, "Nothing to capture."));

    let out: Value = serde_json::from_str(&printed)?;
    let message = out
        .get("systemMessage")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("no systemMessage in {printed}"))?;
    assert!(
        message.contains("PostToolUse: the hook started at ")
            && message.contains("was killed by its 10 s timeout")
            && message.contains("It could not be logged: "),
        "{message}"
    );
    let posted = std::fs::read_to_string(slot_of(&repo, "PostToolUse"))?;
    assert_eq!(field(&posted, "state"), Some("timed-out"), "{posted}");
    Ok(())
}

#[cfg(unix)]
#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_symlinked_slot_is_not_written() -> TestResult {
    let (tmp, repo) = setup();
    std::fs::create_dir_all(state_of(&repo))?;
    let elsewhere = tmp.path().join("elsewhere");
    std::fs::write(&elsewhere, "kept\n")?;
    std::os::unix::fs::symlink(&elsewhere, slot_of(&repo, "Stop"))?;

    hook(&repo, &stop(&repo, MESSAGE));

    assert_eq!(std::fs::read_to_string(&elsewhere)?, "kept\n");
    let log = std::fs::read_to_string(state_of(&repo).join("hook-errors.log"))?;
    assert!(
        log.contains(" Stop: ")
            && log.contains("is not a regular file, so weighin leaves it alone"),
        "{log}"
    );
    Ok(())
}
