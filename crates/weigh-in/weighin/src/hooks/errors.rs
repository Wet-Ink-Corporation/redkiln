//! What the next `SessionStart` reports of `hook-errors.log`: the lines added since the last
//! report, each reported once.

use crate::env::Env;
use crate::text::clip;

/// How far `hook-errors.log` had been reported, as a byte offset and the log's first line then,
/// beside the log.
const ERRORS_SEEN: &str = "hook-errors.seen";

/// The lines added to `hook-errors.log` since the last `SessionStart` reported it, as a message;
/// records the log's length so each line is reported once. Writes nothing when there is no log,
/// and overwrites rather than deletes (B-01).
pub(super) fn new_errors(env: &Env) -> Option<String> {
    // No store located is no state dir, so no log to report.
    let dir = env.state_dir().ok()?;
    let log_path = dir.join("hook-errors.log");
    let log = std::fs::read(&log_path).ok()?;
    // The length reported so far, then the start of the log's first line at that time. A
    // different start means the log was truncated since, so all of it is new. Lines begin with a
    // timestamp, so this assumes a truncation removes the first line; a log trimmed only at its
    // end is not detected. A record with no first line, from an older build, is taken as it
    // stands; an older build reading this record reports the whole log once, then records a
    // bare length again.
    let head = String::from_utf8_lossy(first_line(&log));
    let recorded = std::fs::read_to_string(dir.join(ERRORS_SEEN)).unwrap_or_default();
    let (offset, recorded_head) = match recorded.split_once('\n') {
        Some((offset, recorded_head)) => (offset, Some(recorded_head)),
        None => (recorded.as_str(), None),
    };
    let seen = offset
        .trim()
        .parse::<usize>()
        .ok()
        .filter(|&n| n <= log.len() && recorded_head.is_none_or(|h| h == head))
        .unwrap_or(0);
    if seen == log.len() {
        return None;
    }
    let record = format!("{}\n{head}", log.len());
    let marked = match std::fs::write(dir.join(ERRORS_SEEN), record) {
        Ok(()) => String::new(),
        Err(e) => format!(" Could not mark this report as seen, so it will repeat: {e}."),
    };
    let added = String::from_utf8_lossy(log.get(seen..)?);
    let lines: Vec<&str> = added.lines().filter(|l| !l.trim().is_empty()).collect();
    let last = clip(lines.last()?, 200);
    Some(format!(
        "Weigh-In: {} hook error(s) since the last session, so capture may be off. Last: {last} (full log: {}){marked}",
        lines.len(),
        log_path.display()
    ))
}

/// How much of the log's first line `hook-errors.seen` keeps: the timestamp and event, and more.
const SEEN_HEAD_BYTES: usize = 80;

/// The bytes of `log` before its first newline, at most [`SEEN_HEAD_BYTES`] of them.
fn first_line(log: &[u8]) -> &[u8] {
    let line = log.split(|&b| b == b'\n').next().unwrap_or_default();
    line.get(..SEEN_HEAD_BYTES).unwrap_or(line)
}

#[cfg(test)]
mod tests {
    use super::new_errors;
    use crate::env::{Env, Vars};
    use redkiln_core::store::Wait;
    use std::path::Path;

    /// The environment of a hook in `dir` with its store at `dir/.redkiln/events.sqlite3`,
    /// whatever the process environment says (`REDKILN_STORE`, `WEIGHIN_STORE`).
    fn env_in(dir: &Path) -> Env {
        let store = dir.join(".redkiln/events.sqlite3");
        Env::discover(dir, None, Some(store), Vars::default()).with_wait(Wait::Hook)
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "reads and writes the error log through the file system"
    )]
    fn a_seen_record_from_an_older_build_still_counts_from_its_length() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let state = tmp.path().join(".redkiln/weighin");
        std::fs::create_dir_all(&state).expect("state dir");
        let env = env_in(tmp.path());
        let old = "2026-10-01T00:00:00Z Stop: reported before\n";
        let added = "2026-10-02T00:00:00Z Stop: logged since\n";
        std::fs::write(state.join("hook-errors.log"), format!("{old}{added}")).expect("log");
        // Older builds recorded the length alone.
        std::fs::write(state.join("hook-errors.seen"), old.len().to_string()).expect("seen");

        let report = new_errors(&env).expect("the added error is reported");

        assert!(
            report.starts_with("Weigh-In: 1 hook error(s) since the last session")
                && report.contains("Last: 2026-10-02T00:00:00Z Stop: logged since"),
            "{report}"
        );
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "reads and writes the error log through the file system"
    )]
    fn errors_logged_after_the_log_was_truncated_are_all_reported() {
        use std::fmt::Write as _;
        let tmp = tempfile::tempdir().expect("temp dir");
        let state = tmp.path().join(".redkiln/weighin");
        std::fs::create_dir_all(&state).expect("state dir");
        let log = state.join("hook-errors.log");
        let env = env_in(tmp.path());
        let old = "2026-10-01T00:00:00Z Stop: an old failure that was reported\n".repeat(3);
        std::fs::write(&log, &old).expect("old log");
        assert!(
            new_errors(&env).is_some(),
            "the old errors are reported once"
        );
        // The owner truncates the log once read; new errors grow it past the old length.
        let new = (1..=4).fold(String::new(), |mut acc, n| {
            writeln!(
                acc,
                "2026-10-02T00:00:0{n}Z Stop: new failure number {n} after the log was truncated"
            )
            .expect("writing to a String");
            acc
        });
        assert!(new.len() > old.len(), "the new log outgrows the old one");
        std::fs::write(&log, &new).expect("new log");

        let report = new_errors(&env).expect("the new errors are reported");

        assert!(
            report.starts_with("Weigh-In: 4 hook error(s) since the last session")
                && report.contains("Last: 2026-10-02T00:00:04Z Stop: new failure number 4"),
            "{report}"
        );
        assert_eq!(new_errors(&env), None, "reported once");
    }
}
