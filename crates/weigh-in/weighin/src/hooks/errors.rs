//! `hook-errors.log`: where a hook appends its failures, and what the next `SessionStart`
//! reports of it, the lines added since the last report, each reported once. Both files are
//! used only when they are regular files (HK-6), and at most the log's last
//! [`MAX_REPORTED_LOG`] bytes are read (H-13). [`report_since`] is that report for any such log
//! and its record: `late` reads `hook-late.log` through it.

use anyhow::{Context as _, Result};
use std::fs::File;
use std::io::{ErrorKind, Read, Seek as _, SeekFrom, Write as _};
use std::path::{Path, PathBuf};

use crate::env::{self, Env};
use crate::state_file::{Presence, real_dir, regular_or_absent};
use crate::store;
use crate::text::clip;

/// Where a failure was appended.
#[derive(Debug)]
pub(super) struct Appended {
    /// The log.
    pub(super) path: PathBuf,
    /// Whether the log held nothing unreported before it (decision D2 B).
    pub(super) fresh: bool,
}

/// Appends `e` to the error log as a line for `event`, and says where.
///
/// # Errors
/// There is no place to log it (no store located, or a refused store outside git), the state
/// dir or the log is not what it should be (HK-6), or the append fails.
pub(super) fn log_error(env: &Env, event: &str, e: &anyhow::Error) -> Result<Appended> {
    let dir = log_dir(env, e)?;
    let path = dir.join(ERRORS_LOG);
    regular_or_absent(&path)?;
    let fresh = nothing_unreported(&dir);
    let mut log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("opening {}", path.display()))?;
    writeln!(log, "{} {event}: {e:#}", env::now())
        .with_context(|| format!("appending to {}", path.display()))?;
    Ok(Appended { path, fresh })
}

/// Whether `e` is Core refusing a store path the user gave (an old ledger, another program's
/// database), beside which nothing is written, not even the state dir (D4, review F2). A refused
/// file at the default place is in redkiln's own directory, and is logged as any failure.
pub(super) fn refused_given_store(env: &Env, e: &anyhow::Error) -> bool {
    env.store_was_given() && store::is_refused(e)
}

/// Where `e` is logged, the directory made: the state dir, except for
/// [`refused_given_store`], logged in the default store's state dir, and outside git nowhere
/// (`SessionStart` reports it).
fn log_dir(env: &Env, e: &anyhow::Error) -> Result<PathBuf> {
    let dir = if refused_given_store(env, e) {
        env.default_state_dir().context(
            "nothing is written beside a store weighin refuses, and outside git there is no \
             other place for the log",
        )?
    } else {
        env.state_dir()
            .context("there is no state dir for the log without a store")?
    };
    real_dir(&dir)?;
    Ok(dir)
}

/// Whether the log in `dir` holds no line the next `SessionStart` would report. When that
/// cannot be told, `true`: the hook then says so itself.
fn nothing_unreported(dir: &Path) -> bool {
    match Log::open(&dir.join(ERRORS_LOG)) {
        Ok(Some(log)) => match read_seen(&dir.join(ERRORS_SEEN)) {
            Ok(Some(recorded)) => log.seen(&recorded) == log.len,
            Ok(None) => log.len == 0,
            Err(_) => true,
        },
        Ok(None) | Err(_) => true,
    }
}

/// The error log, in the state dir.
const ERRORS_LOG: &str = "hook-errors.log";

/// How far `hook-errors.log` had been reported, as a byte offset and the log's first line then,
/// beside the log.
const ERRORS_SEEN: &str = "hook-errors.seen";

/// The most of the log a report reads, in KiB: past it, the lines before are counted as "at
/// least".
const MAX_REPORTED_LOG_KIB: u64 = 256;

/// [`MAX_REPORTED_LOG_KIB`] in bytes.
const MAX_REPORTED_LOG: u64 = MAX_REPORTED_LOG_KIB * 1024;

/// How much of the log's first line `hook-errors.seen` keeps: the timestamp and event, and more.
const SEEN_HEAD_BYTES: u64 = 80;

/// The lines added to `hook-errors.log` since the last `SessionStart` reported it, as a message;
/// records the log's length so each line is reported once. Writes nothing when there is no log,
/// and overwrites rather than deletes (B-01). A log that is not a regular file is reported as
/// such; a record that cannot be read is neither trusted nor written, so the report repeats.
pub(super) fn new_errors(env: &Env) -> Option<String> {
    // No store located is no state dir, so no log to report.
    let dir = env.state_dir().ok()?;
    let log_path = dir.join(ERRORS_LOG);
    match report_since(&dir, ERRORS_LOG, ERRORS_SEEN) {
        Since::Nothing => None,
        Since::Unopened(e) => Some(format!(
            "Weigh-In: hook errors can be neither logged nor reported: {}.",
            clip(&format!("{e:#}"), 300)
        )),
        Since::Unread(e) => Some(format!(
            "Weigh-In: hooks logged errors since the last session, so capture may be off, but \
             the log cannot be read: {} (full log: {}).",
            clip(&format!("{e:#}"), 300),
            log_path.display()
        )),
        Since::Found { found, marked } => Some(format!(
            "Weigh-In: {} hook error(s) since the last session, so capture may be off. Last: {} (full log: {}){marked}",
            found.count(),
            found.last,
            log_path.display()
        )),
    }
}

/// What [`report_since`] found in a log past its seen record.
#[derive(Debug)]
pub(super) enum Since {
    /// No log, nothing added since the last report, or only blank lines.
    Nothing,
    /// Lines added since the last report, now marked seen. `marked` is the sentence that says
    /// they could not be marked, so the report will repeat; empty when they were.
    Found { found: Unreported, marked: String },
    /// The log is not a regular file (HK-6), or it cannot be opened.
    Unopened(anyhow::Error),
    /// The log cannot be read past its record.
    Unread(anyhow::Error),
}

/// The lines added to the log named `log` in `dir` since the record named `seen` beside it says
/// it was last reported, recording the log's length so each line is reported once. Writes
/// nothing when there is no log, and overwrites rather than deletes (B-01). A record that cannot
/// be read is neither trusted nor written, so the report repeats. Reads at most the log's last
/// [`MAX_REPORTED_LOG`] bytes (H-13).
pub(super) fn report_since(dir: &Path, log: &str, seen: &str) -> Since {
    let mut opened = match Log::open(&dir.join(log)) {
        Ok(Some(opened)) => opened,
        Ok(None) => return Since::Nothing,
        Err(e) => return Since::Unopened(e),
    };
    let seen_path = dir.join(seen);
    let recorded = read_seen(&seen_path);
    // No record, or one that cannot be read, reports the whole log.
    let from = match &recorded {
        Ok(Some(recorded)) => opened.seen(recorded),
        Ok(None) | Err(_) => 0,
    };
    if from == opened.len {
        return Since::Nothing;
    }
    let tail = match opened.tail(from) {
        Ok(tail) => tail,
        Err(e) => return Since::Unread(e),
    };
    // Worked out before the record is written, so nothing is marked seen unreported.
    let found = tail.reported();
    let marked = match recorded {
        Ok(_) => match std::fs::write(&seen_path, format!("{}\n{}", opened.len, opened.head)) {
            Ok(()) => String::new(),
            Err(e) => format!(" Could not mark this report as seen, so it will repeat: {e}."),
        },
        Err(e) => format!(
            " Could not read what was reported before, so this report will repeat: {}.",
            clip(&format!("{e:#}"), 300)
        ),
    };
    found.map_or(Since::Nothing, |found| Since::Found { found, marked })
}

/// What the next `SessionStart` would report of the log ([`new_errors`]), read only: nothing is
/// marked seen, so that report still comes. `status` reads it (CLI-12's `hook_errors`).
///
/// # Errors
/// The log is not a regular file (HK-6), or it cannot be read.
pub(crate) fn unreported(env: &Env) -> Result<Option<Unreported>> {
    // No store located is no state dir, so no log to report, as for `new_errors`.
    let Ok(dir) = env.state_dir() else {
        return Ok(None);
    };
    let Some(mut log) = Log::open(&dir.join(ERRORS_LOG))? else {
        return Ok(None);
    };
    // A record that cannot be read reports the whole log, as it does in `new_errors`.
    let seen = match read_seen(&dir.join(ERRORS_SEEN)) {
        Ok(Some(recorded)) => log.seen(&recorded),
        Ok(None) | Err(_) => 0,
    };
    if seen == log.len {
        return Ok(None);
    }
    Ok(log.tail(seen)?.reported())
}

/// The lines added to the log since it was last reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Unreported {
    /// The whole lines read; past the read cap, the lines before were not counted.
    pub(crate) lines: usize,
    /// Whether lines before those read were left unread, so `lines` is "at least".
    pub(crate) cut: bool,
    /// The last line, cut to 200 characters.
    pub(crate) last: String,
}

impl Unreported {
    /// The count as a report says it: `3`, or `at least 3` when cut.
    pub(crate) fn count(&self) -> String {
        if self.cut {
            format!("at least {}", self.lines)
        } else {
            self.lines.to_string()
        }
    }
}

/// A log as a report reads it.
struct Log {
    file: File,
    /// Its length when opened: what a report reads up to and records.
    len: u64,
    /// The start of its first line, at most [`SEEN_HEAD_BYTES`].
    head: String,
}

/// What a report reads of the log.
struct Tail {
    /// The unreported bytes; when cut, from the byte before the cut, so up to the first newline
    /// they are the end of a line begun before it.
    bytes: Vec<u8>,
    /// Whether unreported lines before them were left unread.
    cut: bool,
}

impl Tail {
    /// The count and last line to report; `None` when only blank lines were added.
    fn reported(&self) -> Option<Unreported> {
        let read = String::from_utf8_lossy(&self.bytes);
        let whole = if self.cut {
            // Up to the first newline is the end of a line begun before the cut.
            read.split_once('\n').map_or("", |(_, whole)| whole)
        } else {
            &read
        };
        let lines: Vec<&str> = whole.lines().filter(|l| !l.trim().is_empty()).collect();
        match (lines.last(), self.cut) {
            (Some(last), cut) => Some(Unreported {
                lines: lines.len(),
                cut,
                last: clip(last, 200),
            }),
            // Cut with no whole line: one line longer than all the report reads, or one cut
            // line and blank ones after it.
            (None, true) => Some(Unreported {
                lines: 1,
                cut: true,
                last: format!(
                    "no whole line in the last {MAX_REPORTED_LOG_KIB} KiB the report reads"
                ),
            }),
            (None, false) => None,
        }
    }
}

impl Log {
    /// The log at `path`, `None` when there is none.
    fn open(path: &Path) -> Result<Option<Self>> {
        if regular_or_absent(path)? == Presence::Absent {
            return Ok(None);
        }
        let mut file = match File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
            Err(e) => {
                return Err(anyhow::Error::new(e).context(format!("opening {}", path.display())));
            }
        };
        let len = file
            .metadata()
            .with_context(|| format!("reading {}", path.display()))?
            .len();
        let mut start = Vec::new();
        Read::take(&mut file, SEEN_HEAD_BYTES)
            .read_to_end(&mut start)
            .with_context(|| format!("reading {}", path.display()))?;
        // `split` yields at least one piece, so the default is never taken.
        let line = start.split(|&b| b == b'\n').next().unwrap_or_default();
        let head = String::from_utf8_lossy(line).into_owned();
        Ok(Some(Self { file, len, head }))
    }

    /// The offset up to which `recorded` says the log was reported: the length reported, then the
    /// start of the log's first line at that time. A different start means the log was truncated
    /// since, so all of it is new. Lines begin with a timestamp, so this assumes a truncation
    /// removes the first line; a log trimmed only at its end is not detected. A record with no
    /// first line, from an older build, is taken as it stands; an older build reading this
    /// record reports the whole log once, then records a bare length again.
    fn seen(&self, recorded: &str) -> u64 {
        let (offset, recorded_head) = match recorded.split_once('\n') {
            Some((offset, recorded_head)) => (offset, Some(recorded_head)),
            None => (recorded, None),
        };
        offset
            .trim()
            .parse::<u64>()
            .ok()
            .filter(|&n| n <= self.len && recorded_head.is_none_or(|h| h == self.head))
            .unwrap_or(0)
    }

    /// The bytes after `seen`, at most [`MAX_REPORTED_LOG`] of them (one more when cut).
    fn tail(&mut self, seen: u64) -> Result<Tail> {
        let start = seen.max(self.len.saturating_sub(MAX_REPORTED_LOG));
        let cut = start > seen;
        // From the byte before a cut start, to tell whether that start begins a line.
        let from = if cut { start.saturating_sub(1) } else { start };
        self.file
            .seek(SeekFrom::Start(from))
            .context("seeking in the log")?;
        let mut bytes = Vec::new();
        Read::take(&mut self.file, self.len.saturating_sub(from))
            .read_to_end(&mut bytes)
            .context("reading the log")?;
        Ok(Tail { bytes, cut })
    }
}

/// The text of the record at `path`; `None` when there is none.
///
/// # Errors
/// Something other than a regular file is there, or it cannot be read.
fn read_seen(path: &Path) -> Result<Option<String>> {
    if regular_or_absent(path)? == Presence::Absent {
        return Ok(None);
    }
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(anyhow::Error::new(e).context(format!("reading {}", path.display()))),
    }
}

#[cfg(test)]
mod tests {
    use super::{new_errors, unreported};
    use crate::env::{Env, Vars};
    use crate::text::clip;
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
    fn unreported_reads_what_the_next_session_start_reports_and_marks_nothing() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let state = tmp.path().join(".redkiln/weighin");
        std::fs::create_dir_all(&state).expect("state dir");
        let env = env_in(tmp.path());
        assert_eq!(unreported(&env).expect("no log reads"), None);
        let lines = "2026-10-01T00:00:00Z Stop: one\n2026-10-02T00:00:00Z Stop: two\n";
        std::fs::write(state.join("hook-errors.log"), lines).expect("log");

        let found = unreported(&env).expect("read").expect("two lines");

        assert_eq!((found.lines, found.cut), (2, false));
        assert_eq!(found.last, "2026-10-02T00:00:00Z Stop: two");
        assert!(
            !state.join("hook-errors.seen").exists(),
            "nothing marked seen"
        );
        let report = new_errors(&env).expect("still reported");
        assert!(report.starts_with("Weigh-In: 2 hook error(s)"), "{report}");
        assert_eq!(unreported(&env).expect("read"), None, "once reported");
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

    #[test]
    #[cfg_attr(
        miri,
        ignore = "reads and writes the error log through the file system"
    )]
    fn a_log_past_the_tail_cap_is_counted_at_least() {
        use std::fmt::Write as _;
        let tmp = tempfile::tempdir().expect("temp dir");
        let state = tmp.path().join(".redkiln/weighin");
        std::fs::create_dir_all(&state).expect("state dir");
        let env = env_in(tmp.path());
        // 3,000 lines of 100 bytes: the last 256 KiB start 37,856 bytes in, inside line 379, so
        // the 2,621 whole lines after it are counted.
        let log = (1..=3000).fold(String::new(), |mut acc, n| {
            let line = format!("2026-10-01T00:00:00Z Stop: failure number {n:04}");
            writeln!(acc, "{line:<99}").expect("writing to a String");
            acc
        });
        assert_eq!(log.len(), 300_000);
        std::fs::write(state.join("hook-errors.log"), &log).expect("log");

        let report = new_errors(&env).expect("the errors are reported");

        assert!(
            report.starts_with("Weigh-In: at least 2621 hook error(s) since the last session")
                && report.contains("Last: 2026-10-01T00:00:00Z Stop: failure number 3000"),
            "{report}"
        );
        assert_eq!(new_errors(&env), None, "reported once");
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "reads and writes the error log through the file system"
    )]
    fn one_line_longer_than_the_tail_cap_is_still_reported() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let state = tmp.path().join(".redkiln/weighin");
        std::fs::create_dir_all(&state).expect("state dir");
        let env = env_in(tmp.path());
        let line = format!("2026-10-01T00:00:00Z Stop: {}\n", "x".repeat(300_000));
        std::fs::write(state.join("hook-errors.log"), &line).expect("log");

        let report = new_errors(&env).expect("the long line is reported");

        assert!(
            report.starts_with("Weigh-In: at least 1 hook error(s) since the last session")
                && report.contains("(full log: "),
            "{}",
            clip(&report, 300)
        );
        assert_eq!(new_errors(&env), None, "reported once");
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "reads and writes the error log through the file system"
    )]
    fn a_cut_tail_of_blank_lines_says_it_read_no_whole_line() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let state = tmp.path().join(".redkiln/weighin");
        std::fs::create_dir_all(&state).expect("state dir");
        let env = env_in(tmp.path());
        // One line past the cap, then only blank lines: the tail holds no whole line to show.
        let log = format!("2026-10-01T00:00:00Z Stop: {}\n\n\n", "x".repeat(300_000));
        std::fs::write(state.join("hook-errors.log"), &log).expect("log");

        let report = new_errors(&env).expect("the cut line is reported");

        assert!(
            report.starts_with("Weigh-In: at least 1 hook error(s) since the last session")
                && report.contains("Last: no whole line in the last 256 KiB the report reads"),
            "{}",
            clip(&report, 300)
        );
    }

    #[cfg(unix)]
    #[test]
    #[cfg_attr(miri, ignore = "makes a symlink through the file system")]
    fn a_symlinked_seen_record_is_neither_read_nor_written() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let state = tmp.path().join(".redkiln/weighin");
        std::fs::create_dir_all(&state).expect("state dir");
        let env = env_in(tmp.path());
        let line = "2026-10-02T00:00:00Z Stop: logged\n";
        std::fs::write(state.join("hook-errors.log"), line).expect("log");
        // A record saying all of it was reported, reached only through a link.
        let elsewhere = tmp.path().join("elsewhere");
        let record = format!("{}\n{}", line.len(), line.trim_end());
        std::fs::write(&elsewhere, &record).expect("record");
        std::os::unix::fs::symlink(&elsewhere, state.join("hook-errors.seen")).expect("symlink");

        let first = new_errors(&env).expect("the record is not read, so the line is new");
        let again = new_errors(&env).expect("and not marked, so it repeats");

        assert!(
            first.starts_with("Weigh-In: 1 hook error(s) since the last session")
                && first.contains("so this report will repeat")
                && first.contains("is not a regular file"),
            "{first}"
        );
        assert_eq!(again, first);
        assert_eq!(
            std::fs::read_to_string(&elsewhere).expect("record readable"),
            record,
            "the link's target is left alone"
        );
    }
}
