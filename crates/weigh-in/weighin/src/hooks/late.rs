//! `hook-late.log`: where a hook records journal work it left for a later run because its budget
//! ran out (the Stop's export, HK-5), and what the next `SessionStart` reports of it, once
//! (wi-6ab291e7fb, superseding D12's silence for a late journal). It is not `hook-errors.log`: a
//! late run is no failure, says nothing of capture, and adds no note to the hook that ran late
//! (CLI-09). Both files are used only when they are regular files (HK-6), the report reads at
//! most the log's last 256 KiB (H-13), and the seen record is overwritten, never deleted (B-01).

use anyhow::{Context as _, Result};
use std::io::Write as _;

use super::errors::{Since, report_since};
use crate::env::{self, Env};
use crate::state_file::{real_dir, regular_or_absent};
use crate::text::clip;

/// The record of late runs, in the state dir.
const LATE_LOG: &str = "hook-late.log";

/// How far `hook-late.log` had been reported, as in `hook-errors.seen`.
const LATE_SEEN: &str = "hook-late.seen";

/// Appends a line saying that the `event` hook ran out of time and left `what` for later.
///
/// # Errors
/// There is no state dir (no store located), it or the log is not what it should be (HK-6), or
/// the append fails.
pub(super) fn record(env: &Env, event: &str, what: &str) -> Result<()> {
    let dir = env
        .state_dir()
        .context("there is no state dir for the late record without a store")?;
    real_dir(&dir)?;
    let path = dir.join(LATE_LOG);
    regular_or_absent(&path)?;
    let mut log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("opening {}", path.display()))?;
    writeln!(log, "{} {event}: {what}", env::now())
        .with_context(|| format!("appending to {}", path.display()))
}

/// The runs recorded late since the last `SessionStart` reported them, as one sentence for the
/// person, each reported once; `None` when there are none, or no state dir.
pub(super) fn new_late(env: &Env) -> Option<String> {
    let dir = env.state_dir().ok()?;
    match report_since(&dir, LATE_LOG, LATE_SEEN) {
        Since::Nothing => None,
        Since::Found { found, marked } => Some(format!(
            "Weigh-In: hooks ran out of time {} time(s) since the last session and left journal \
             work for a later run. Last: {}. The next Stop exports what is left; `weighin journal \
             checkpoint` does it now.{marked}",
            found.count(),
            found.last
        )),
        Since::Unopened(e) | Since::Unread(e) => Some(format!(
            "Weigh-In: hooks may have run out of time and left journal work for a later run, but \
             their record cannot be read: {}.",
            clip(&format!("{e:#}"), 300)
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{new_late, record};
    use crate::env::{Env, Vars};
    use redkiln_core::store::Wait;
    use std::path::Path;

    /// The environment of a hook in `dir` with its store at `dir/.redkiln/events.sqlite3`.
    fn env_in(dir: &Path) -> Env {
        let store = dir.join(".redkiln/events.sqlite3");
        Env::discover(dir, None, Some(store), Vars::default()).with_wait(Wait::Hook)
    }

    #[test]
    #[cfg_attr(miri, ignore = "writes the late record through the file system")]
    fn nothing_recorded_reports_nothing() {
        let tmp = tempfile::tempdir().expect("temp dir");

        assert_eq!(new_late(&env_in(tmp.path())), None);
    }

    #[test]
    #[cfg_attr(miri, ignore = "writes the late record through the file system")]
    fn late_runs_are_reported_once_with_the_last() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let env = env_in(tmp.path());
        record(&env, "Stop", "1 journal(s) left for the next export").expect("recorded");
        record(&env, "Stop", "2 journal(s) left for the next export").expect("recorded");

        let report = new_late(&env).expect("reported");

        assert!(
            report.starts_with("Weigh-In: hooks ran out of time 2 time(s) since the last session")
                && report.contains("Stop: 2 journal(s) left for the next export. The next Stop")
                && report.ends_with("`weighin journal checkpoint` does it now."),
            "{report}"
        );
        assert_eq!(new_late(&env), None, "reported once");
        assert!(
            !tmp.path().join(".redkiln/weighin/hook-errors.log").exists(),
            "a late run is not a hook error"
        );
    }

    #[test]
    #[cfg_attr(miri, ignore = "writes the late record through the file system")]
    fn a_run_recorded_after_a_report_is_the_only_one_counted() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let env = env_in(tmp.path());
        record(&env, "Stop", "first").expect("recorded");
        assert!(new_late(&env).is_some(), "the first is reported");
        record(&env, "Stop", "second").expect("recorded");

        let report = new_late(&env).expect("the second is reported");

        assert!(
            report.contains("ran out of time 1 time(s)") && report.contains("Stop: second."),
            "{report}"
        );
    }

    #[cfg(unix)]
    #[test]
    #[cfg_attr(miri, ignore = "makes a symlink through the file system")]
    fn a_symlinked_late_log_is_neither_written_nor_read() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let state = tmp.path().join(".redkiln/weighin");
        std::fs::create_dir_all(&state).expect("state dir");
        let env = env_in(tmp.path());
        let elsewhere = tmp.path().join("elsewhere");
        std::fs::write(&elsewhere, "").expect("target");
        std::os::unix::fs::symlink(&elsewhere, state.join("hook-late.log")).expect("symlink");

        let refused = record(&env, "Stop", "left").expect_err("a link is not written through");
        let report = new_late(&env).expect("told that the record cannot be read");

        assert!(
            format!("{refused:#}").contains("is not a regular file"),
            "{refused:#}"
        );
        assert!(
            report.contains("their record cannot be read") && report.contains("not a regular file"),
            "{report}"
        );
        assert_eq!(
            std::fs::read_to_string(&elsewhere).expect("target readable"),
            "",
            "the link's target is left alone"
        );
    }
}
