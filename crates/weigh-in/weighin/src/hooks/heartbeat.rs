//! The hook heartbeat (HK-5): how the next hook, and later `status`, tell a hook its timeout
//! killed from one that was interrupted or finished.
//!
//! A hook of a registered event keeps one slot, `hook-heartbeat-<Event>` in the default state
//! dir `<git-common-dir>/redkiln/weighin/`, as six text lines ([`Slot`]): `running` before its
//! dispatch, a fresh `beat` every [`BEAT`] from one scoped, named thread while it runs, and
//! `finished` once it is done. Slots are overwritten in place, never deleted (B-01), and used
//! only when they are regular files (HK-6). Nothing is written in Cowork, outside git, or where
//! `<git-common-dir>/redkiln/` does not exist yet, so no hook creates it; the event is one of the
//! four weighin is registered for, never payload text.
//!
//! Before its dispatch every such hook reads all four slots. A `running` slot past its deadline
//! is dead whatever its beat's age: it timed out when its last beat came within [`MARGIN`]
//! seconds of the deadline, and was interrupted otherwise (cancelled, the session ended, a
//! crash). Before its deadline a slot whose beat is more than [`STALE`] seconds old is dead,
//! interrupted. Times are whole seconds, and `started` is floored and read after the harness's
//! timer starts, so a hook its timeout killed less than about a second before the next hook of
//! its event starts, or one interrupted less than [`STALE`] seconds before, is overwritten
//! unseen.
//! The hook logs one line for it under the dead hook's event, which the next
//! `SessionStart` reports with the other hook errors, and rewrites the slot to that state so it
//! is logged once. Two hooks finding the same dead slot may both log it, and two `PostToolUse`
//! hooks share a slot, so one finishing can hide the other's death for a second; neither makes
//! a live hook look dead (decision D8).

use anyhow::{Context as _, Result};
use std::fs::File;
use std::io::{ErrorKind, Read as _};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::errors::log_error;
use super::failures::{Logged, one_line};
use super::{Failures, Hook, HookEvent};
use crate::state_file::{Presence, real_dir, regular_or_absent};

/// How often a running hook rewrites its beat.
const BEAT: Duration = Duration::from_secs(1);

/// Seconds after its last beat past which a `running` slot is dead: five beats.
const STALE: u64 = 5;

/// Seconds before its deadline within which a dead hook's last beat means its timeout killed it.
const MARGIN: u64 = 3;

/// A slot's first line: the format and its version.
const FORMAT: &str = "weighin-heartbeat 1";

/// The most of a slot that is read; a whole slot is under 120 bytes.
const MAX_SLOT_BYTES: u64 = 512;

/// The state a slot records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum State {
    /// Dispatched and not yet done.
    Running,
    /// Done, whatever it found.
    Finished,
    /// Killed by its timeout, as a later hook found it.
    TimedOut,
    /// Ended before its deadline without finishing, as a later hook found it.
    Interrupted,
}

impl State {
    fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Finished => "finished",
            Self::TimedOut => "timed-out",
            Self::Interrupted => "interrupted",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        match text {
            "running" => Some(Self::Running),
            "finished" => Some(Self::Finished),
            "timed-out" => Some(Self::TimedOut),
            "interrupted" => Some(Self::Interrupted),
            _ => None,
        }
    }
}

/// How a hook that never finished ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lost {
    TimedOut,
    Interrupted,
}

impl Lost {
    fn state(self) -> State {
        match self {
            Self::TimedOut => State::TimedOut,
            Self::Interrupted => State::Interrupted,
        }
    }
}

/// One hook's slot; times are seconds since 1970.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Slot {
    pub(crate) state: State,
    /// When the hook started.
    pub(crate) started: u64,
    /// Its last beat.
    pub(crate) beat: u64,
    /// Its harness timeout, in seconds.
    pub(crate) timeout: u64,
    /// The process that wrote it.
    pub(crate) pid: u32,
}

impl Slot {
    /// The slot as its file holds it.
    fn render(&self) -> String {
        format!(
            "{FORMAT}\nstate {}\nstarted {}\nbeat {}\ntimeout {}\npid {}\n",
            self.state.as_str(),
            self.started,
            self.beat,
            self.timeout,
            self.pid
        )
    }

    /// The slot `text` holds; `None` when it is torn (a write under way), of another format, or
    /// not a slot at all.
    fn parse(text: &str) -> Option<Self> {
        let mut lines = text.strip_suffix('\n')?.split('\n');
        if lines.next()? != FORMAT {
            return None;
        }
        let state = State::parse(field(lines.next(), "state")?)?;
        let started = number(lines.next(), "started")?;
        let beat = number(lines.next(), "beat")?;
        let timeout = number(lines.next(), "timeout")?;
        let pid = field(lines.next(), "pid")?.parse().ok()?;
        lines.next().is_none().then_some(Self {
            state,
            started,
            beat,
            timeout,
            pid,
        })
    }

    /// The slot's state as a reader at `now` sees it: a dead `running` slot is timed out or
    /// interrupted ([`Slot::lost`]); a beat in the future
    /// (a clock that went back) is left as it is. `status` reports this, and writes nothing.
    pub(crate) fn state_at(&self, now: u64) -> State {
        self.lost(now).map_or(self.state, Lost::state)
    }

    /// How the hook ended when its slot is `running` and dead at `now`. Past the deadline it is
    /// dead whatever its beat's age (the harness kills a hook at its timeout): timed out when its
    /// last beat came within [`MARGIN`] seconds of the deadline, else interrupted. Before the
    /// deadline it is dead, interrupted, only once its beat is more than [`STALE`] seconds old.
    /// The answer for a slot never changes once given, as `now` grows.
    fn lost(&self, now: u64) -> Option<Lost> {
        if self.state != State::Running || self.beat > now {
            return None;
        }
        if now > self.deadline() {
            return Some(if self.beat.saturating_add(MARGIN) >= self.deadline() {
                Lost::TimedOut
            } else {
                Lost::Interrupted
            });
        }
        (now.saturating_sub(self.beat) > STALE).then_some(Lost::Interrupted)
    }

    fn deadline(&self) -> u64 {
        self.started.saturating_add(self.timeout)
    }

    /// The log line for the hook this slot records, which ended as `lost` says.
    fn lost_line(&self, lost: Lost) -> String {
        let started = rfc3339(self.started);
        match lost {
            Lost::TimedOut => format!(
                "the hook started at {started} was killed by its {} s timeout",
                self.timeout
            ),
            Lost::Interrupted => format!(
                "the hook started at {started} ended {} s before its {} s timeout without \
                 finishing (interrupted)",
                self.deadline().saturating_sub(self.beat),
                self.timeout
            ),
        }
    }
}

/// The value of `line` after `key` and one space.
fn field<'t>(line: Option<&'t str>, key: &str) -> Option<&'t str> {
    line?.strip_prefix(key)?.strip_prefix(' ')
}

/// The number on `line` after `key`; `None` when it is not one, as in a torn slot.
fn number(line: Option<&str>, key: &str) -> Option<u64> {
    field(line, key)?.parse().ok()
}

/// `secs` since 1970 as RFC 3339, or as a count when it is out of range.
fn rfc3339(secs: u64) -> String {
    i64::try_from(secs)
        .ok()
        .and_then(|s| time::OffsetDateTime::from_unix_timestamp(s).ok())
        .and_then(|t| {
            t.format(&time::format_description::well_known::Rfc3339)
                .ok()
        })
        .unwrap_or_else(|| format!("{secs} s after 1970"))
}

/// The seconds since 1970; `None` when the clock reads before it.
fn unix_now() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|since| since.as_secs())
}

/// The events weighin is registered for, each with its slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SlotEvent {
    SessionStart,
    Stop,
    SubagentStop,
    PostToolUse,
}

impl SlotEvent {
    const ALL: [Self; 4] = [
        Self::SessionStart,
        Self::Stop,
        Self::SubagentStop,
        Self::PostToolUse,
    ];

    /// The slot event of a dispatched `event`; none for any other.
    fn of(event: &HookEvent) -> Option<Self> {
        match event {
            HookEvent::SessionStart { .. } => Some(Self::SessionStart),
            HookEvent::Stop(_) => Some(Self::Stop),
            HookEvent::SubagentStop(_) => Some(Self::SubagentStop),
            HookEvent::PostToolUse(_) => Some(Self::PostToolUse),
            HookEvent::Other(_) | HookEvent::Unnamed => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::SessionStart => "SessionStart",
            Self::Stop => "Stop",
            Self::SubagentStop => "SubagentStop",
            Self::PostToolUse => "PostToolUse",
        }
    }

    /// The slot's path in the state dir `dir`.
    fn slot_in(self, dir: &Path) -> PathBuf {
        dir.join(format!("hook-heartbeat-{}", self.name()))
    }
}

/// The slot at `path`; `None` when there is none, or it is torn or foreign.
///
/// # Errors
/// Something other than a regular file is there (HK-6), or it cannot be read.
fn read_slot(path: &Path) -> Result<Option<Slot>> {
    if regular_or_absent(path)? == Presence::Absent {
        return Ok(None);
    }
    let file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(anyhow::Error::new(e).context(format!("opening {}", path.display()))),
    };
    let mut bytes = Vec::new();
    file.take(MAX_SLOT_BYTES)
        .read_to_end(&mut bytes)
        .with_context(|| format!("reading {}", path.display()))?;
    Ok(std::str::from_utf8(&bytes).ok().and_then(Slot::parse))
}

/// Overwrites the slot at `path` with `slot`.
///
/// # Errors
/// Something other than a regular file is there (HK-6), or the write fails.
fn write_slot(path: &Path, slot: &Slot) -> Result<()> {
    regular_or_absent(path)?;
    std::fs::write(path, slot.render()).with_context(|| format!("writing {}", path.display()))
}

/// Runs `work` while a scoped thread named `weighin-heartbeat` calls `beat` every `period`, and
/// returns what `work` returned with the first failure of the beats: a failed beat (which stops
/// them), a thread that could not start (`work` then runs without beats) or one that panicked.
/// The thread ends when `work` does, since dropping the channel's sender wakes it at once, and
/// it is joined before this returns (H-13).
fn with_beats<T>(
    period: Duration,
    beat: impl Fn() -> Result<()> + Sync,
    work: impl FnOnce() -> T,
) -> (T, Option<anyhow::Error>) {
    thread::scope(|scope| {
        // Nothing is ever sent: the sender's drop is the stop signal.
        let (stop, stopped) = mpsc::sync_channel::<()>(1);
        let beat = &beat;
        let beater = thread::Builder::new()
            .name("weighin-heartbeat".to_owned())
            .spawn_scoped(scope, move || {
                loop {
                    match stopped.recv_timeout(period) {
                        Err(RecvTimeoutError::Timeout) => {
                            if let Err(e) = beat() {
                                return Some(e);
                            }
                        }
                        Ok(()) | Err(RecvTimeoutError::Disconnected) => return None,
                    }
                }
            });
        let out = work();
        drop(stop);
        let failed = match beater {
            Ok(beater) => beater
                .join()
                .unwrap_or_else(|_| Some(anyhow::anyhow!("the heartbeat thread panicked"))),
            Err(e) => Some(anyhow::Error::new(e).context("starting the heartbeat thread")),
        };
        (out, failed)
    })
}

/// Where `hook` keeps its heartbeat: its slot event, the default state dir and its timeout in
/// seconds; `None` when it keeps none (module doc).
///
/// # Errors
/// What is at `<git-common-dir>/redkiln/` cannot be told: its metadata fails other than as
/// not found.
fn place(hook: &Hook) -> Result<Option<(SlotEvent, PathBuf, u64)>> {
    let Some((event, timeout, dir)) = (!hook.cowork)
        .then(|| SlotEvent::of(&hook.event))
        .flatten()
        .zip(hook.timeout)
        .and_then(|(event, timeout)| Some((event, timeout, hook.env.default_state_dir()?)))
    else {
        return Ok(None);
    };
    // `redkiln/` must be there already, and be a directory rather than a link to one.
    let Some(redkiln) = dir.parent() else {
        return Ok(None);
    };
    match std::fs::symlink_metadata(redkiln) {
        Ok(meta) if meta.file_type().is_dir() => Ok(Some((event, dir, timeout.as_secs()))),
        Ok(_) => Ok(None),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(anyhow::Error::new(e).context(format!("reading {}", redkiln.display()))),
    }
}

/// Runs `work` under `hook`'s heartbeat, when it keeps one: logs and rewrites each dead slot,
/// writes its own `running`, beats while `work` runs, then writes `finished`. Each failure is
/// logged in `failures`, and `work` runs whatever happens.
pub(super) fn around<T>(
    hook: &Hook,
    failures: &mut Failures,
    work: impl FnOnce(&mut Failures) -> T,
) -> T {
    let (event, dir, timeout) = match place(hook) {
        Ok(Some(place)) => place,
        Ok(None) => return work(failures),
        Err(e) => {
            hook.log(
                failures,
                &e.context("finding where the hook keeps its heartbeat"),
            );
            return work(failures);
        }
    };
    let Some(now) = unix_now() else {
        let e = anyhow::anyhow!("the clock reads before 1970, so the hook keeps no heartbeat");
        hook.log(failures, &e);
        return work(failures);
    };
    if let Err(e) = real_dir(&dir) {
        hook.log(failures, &e);
        return work(failures);
    }
    scan_lost(hook, &dir, now, failures);
    let path = event.slot_in(&dir);
    let running = Slot {
        state: State::Running,
        started: now,
        beat: now,
        timeout,
        pid: std::process::id(),
    };
    if let Err(e) = write_slot(&path, &running) {
        hook.log(failures, &e);
        return work(failures);
    }
    let beat = || {
        let beat = unix_now().context("the clock reads before 1970")?;
        write_slot(&path, &Slot { beat, ..running })
    };
    let (out, failed) = with_beats(BEAT, beat, || work(failures));
    if let Some(e) = failed {
        hook.log(failures, &e.context("keeping the hook's heartbeat"));
    }
    // A clock gone before 1970 since the start keeps the start as the last beat.
    let finished = Slot {
        state: State::Finished,
        beat: unix_now().unwrap_or(now),
        ..running
    };
    if let Err(e) = write_slot(&path, &finished) {
        hook.log(failures, &e);
    }
    out
}

/// Logs each slot in `dir` that is dead at `now`, under its own event, and rewrites it to how it
/// ended, so it is logged once. A line that cannot be logged goes into `failures` as such, so
/// the hook's note tells it (CLI-09).
fn scan_lost(hook: &Hook, dir: &Path, now: u64, failures: &mut Failures) {
    for event in SlotEvent::ALL {
        let path = event.slot_in(dir);
        // No slot, a torn one, or one that cannot be read: the same check refuses the latter's
        // write by its own event's hook, which logs it, so logging it here too would repeat it
        // in every hook.
        let Ok(Some(slot)) = read_slot(&path) else {
            continue;
        };
        let Some(lost) = slot.lost(now) else {
            continue;
        };
        let line = slot.lost_line(lost);
        if let Err(why) = log_error(&hook.env, event.name(), &anyhow::anyhow!("{line}")) {
            // The note names the hook that runs, so the line names the one that died.
            let noted = anyhow::anyhow!("{}: {line}", event.name());
            failures.push(&noted, Logged::Not(one_line(&why, 300)));
        }
        // The state any reader now sees (`lost`'s), so the slot is logged once.
        let rewritten = Slot {
            state: slot.state_at(now),
            ..slot
        };
        if let Err(e) = write_slot(&path, &rewritten) {
            hook.log(failures, &e);
        }
    }
}

#[cfg(test)]
mod tests;
