//! ST-09's give-way: a `Store` handle that wrote more than one event, or held the write lock
//! longer than [`LONG_HOLD`], waits until [`PAUSE`] has passed since that write ended before it
//! writes again. SQLite's rollback-journal lock has no queue, so a writer that commits and begins
//! again at once keeps every other process out past its busy timeout, hooks first (R-SQLITE,
//! `spikes/r-sqlite/FINDINGS.md`). A hook makes one append, so it never waits here.
//!
//! The lock hold is measured as the whole write call, busy wait included: happenstance runs the
//! transaction, so this is the closest bound Core has, and it only ever errs towards a pause.

use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

/// How long a write may take before the next write from the same handle gives way (ST-09).
pub(super) const LONG_HOLD: Duration = Duration::from_millis(5);

/// How long after such a write the handle's next write waits to start (ST-09).
pub(crate) const PAUSE: Duration = Duration::from_millis(25);

/// Where the give-way reads the time and waits: [`SystemClock`], or in tests a clock that only
/// moves when told to, so no test sleeps (TST-10).
pub(super) trait Clock: Send + Sync {
    /// The current instant.
    fn now(&self) -> Instant;
    /// Waits `pause`.
    fn sleep(&self, pause: Duration);
}

/// The system's monotonic clock: `Instant::now` and `thread::sleep`.
struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn sleep(&self, pause: Duration) {
        std::thread::sleep(pause);
    }
}

/// One handle's give-way: when its next write may start.
pub(super) struct GiveWay {
    clock: Box<dyn Clock>,
    /// The earliest start of the next write; `None` when it may start at once. A `Mutex`, not a
    /// `Cell`, so that `Store` stays `Sync`; it is held only to read or set the instant, never
    /// across a wait or a write.
    next: Mutex<Option<Instant>>,
}

impl GiveWay {
    /// A give-way on the system clock, with no write behind it yet.
    pub(super) fn new() -> Self {
        Self::with_clock(Box::new(SystemClock))
    }

    /// A give-way on `clock`, with no write behind it yet.
    pub(super) fn with_clock(clock: Box<dyn Clock>) -> Self {
        Self {
            clock,
            next: Mutex::new(None),
        }
    }

    /// Runs `write`, a write transaction of `events` events, after giving way to the previous
    /// write if it asked for it, and sets the next write's start from how this one went. The
    /// count is what the transaction was given, whatever its outcome: a refused or conflicted
    /// write held the lock too.
    pub(super) fn write<T>(&self, events: usize, write: impl FnOnce() -> T) -> T {
        let next = *self.slot();
        if let Some(pause) = next.and_then(|at| at.checked_duration_since(self.clock.now()))
            && !pause.is_zero()
        {
            self.clock.sleep(pause);
        }
        let started = self.clock.now();
        let done = write();
        let ended = self.clock.now();
        let held = ended.saturating_duration_since(started);
        *self.slot() = if events > 1 || held > LONG_HOLD {
            ended.checked_add(PAUSE)
        } else {
            None
        };
        done
    }

    /// The next write's start. A panic elsewhere while it was held left a plain instant, which
    /// is still meaningful, so a poisoned lock is read through.
    fn slot(&self) -> std::sync::MutexGuard<'_, Option<Instant>> {
        self.next.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A clock for tests: it starts at a fixed instant, moves only when advanced or slept on, and
/// keeps every sleep asked of it.
#[cfg(test)]
pub(crate) struct FakeClock {
    now: Mutex<Instant>,
    slept: Mutex<Vec<Duration>>,
}

#[cfg(test)]
impl Default for FakeClock {
    fn default() -> Self {
        Self {
            now: Mutex::new(Instant::now()),
            slept: Mutex::new(Vec::new()),
        }
    }
}

#[cfg(test)]
impl Clock for std::sync::Arc<FakeClock> {
    fn now(&self) -> Instant {
        *self.now.lock().expect("the clock")
    }

    fn sleep(&self, pause: Duration) {
        self.slept.lock().expect("the sleeps").push(pause);
        self.advance(pause);
    }
}

#[cfg(test)]
impl FakeClock {
    /// Moves the clock on by `by`.
    pub(crate) fn advance(&self, by: Duration) {
        let mut now = self.now.lock().expect("the clock");
        *now = now.checked_add(by).expect("a test instant");
    }

    /// Every sleep asked of the clock, in order.
    pub(crate) fn slept(&self) -> Vec<Duration> {
        self.slept.lock().expect("the sleeps").clone()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    const MS: Duration = Duration::from_millis(1);

    fn fake() -> (GiveWay, Arc<FakeClock>) {
        let clock = Arc::new(FakeClock::default());
        (GiveWay::with_clock(Box::new(Arc::clone(&clock))), clock)
    }

    #[test]
    fn single_quick_writes_never_wait() {
        let (give_way, clock) = fake();
        for _ in 0..3 {
            give_way.write(1, || ());
        }
        assert!(clock.slept().is_empty(), "{:?}", clock.slept());
    }

    #[test]
    fn a_write_of_several_events_makes_the_next_wait_the_pause() {
        let (give_way, clock) = fake();
        give_way.write(2, || ());
        assert!(clock.slept().is_empty(), "the write itself does not wait");
        give_way.write(1, || ());
        assert_eq!(clock.slept(), [PAUSE]);
        give_way.write(1, || ());
        assert_eq!(
            clock.slept(),
            [PAUSE],
            "a quick single write leaves no pause"
        );
    }

    #[test]
    fn a_write_holding_the_lock_past_the_long_hold_makes_the_next_wait() {
        let (give_way, clock) = fake();
        give_way.write(1, || clock.advance(LONG_HOLD));
        give_way.write(1, || ());
        assert!(
            clock.slept().is_empty(),
            "exactly the long hold is not past it"
        );
        give_way.write(1, || clock.advance(LONG_HOLD.saturating_add(MS)));
        give_way.write(1, || ());
        assert_eq!(clock.slept(), [PAUSE]);
    }

    #[test]
    fn time_since_the_last_write_counts_toward_the_pause() {
        let (give_way, clock) = fake();
        give_way.write(128, || ());
        clock.advance(10 * MS);
        give_way.write(1, || ());
        assert_eq!(clock.slept(), [PAUSE.saturating_sub(10 * MS)]);
        give_way.write(128, || ());
        clock.advance(PAUSE);
        give_way.write(1, || ());
        assert_eq!(clock.slept().len(), 1, "the pause has passed already");
    }

    #[test]
    fn the_writes_result_is_returned() {
        let (give_way, _clock) = fake();
        assert_eq!(give_way.write(1, || 7), 7);
    }

    /// A new store at `dir`, giving way on a fake clock.
    fn store_on_fake_clock(dir: &std::path::Path) -> (super::super::Store, Arc<FakeClock>) {
        let clock = Arc::new(FakeClock::default());
        let store =
            super::super::Store::open(&dir.join("events.sqlite3"), super::super::Wait::Hook)
                .expect("opens")
                .giving_way_on(Arc::clone(&clock));
        (store, clock)
    }

    fn event() -> super::super::NewEvent {
        super::super::NewEvent::new("weighin.item_raised", b"{}".to_vec()).expect("an event")
    }

    #[test]
    fn a_hooks_single_append_is_never_delayed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (store, clock) = store_on_fake_clock(dir.path());
        store.append(vec![event()], None).expect("appended");
        assert!(clock.slept().is_empty(), "{:?}", clock.slept());
    }

    #[test]
    fn an_append_after_a_two_event_append_gives_way() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (store, clock) = store_on_fake_clock(dir.path());
        store
            .append(vec![event(), event()], None)
            .expect("appended");
        store.append(vec![event()], None).expect("appended");
        assert_eq!(clock.slept(), [PAUSE]);
    }

    /// Runs `f` inside a tokio runtime context, where the store refuses to run.
    fn inside_a_runtime<T>(f: impl FnOnce() -> T) -> T {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("a runtime");
        let entered = runtime.enter();
        let result = f();
        drop(entered);
        result
    }

    #[test]
    fn an_append_refused_inside_a_runtime_neither_waits_nor_changes_the_pause() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (store, clock) = store_on_fake_clock(dir.path());
        store
            .append(vec![event(), event()], None)
            .expect("appended");
        let refused = inside_a_runtime(|| store.append(vec![event()], None));
        assert!(
            matches!(refused, Err(super::super::StoreError::InsideRuntime)),
            "{refused:?}"
        );
        assert!(clock.slept().is_empty(), "{:?}", clock.slept());
        store.append(vec![event()], None).expect("appended");
        assert_eq!(clock.slept(), [PAUSE], "the pending pause is still owed");
    }

    #[test]
    fn an_ingest_refused_inside_a_runtime_neither_waits_nor_changes_the_pause() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (store, clock) = store_on_fake_clock(dir.path());
        store
            .append(vec![event(), event()], None)
            .expect("appended");
        let refused = inside_a_runtime(|| store.ingest(&[]));
        assert!(
            matches!(refused, Err(super::super::StoreError::InsideRuntime)),
            "{refused:?}"
        );
        assert!(clock.slept().is_empty(), "{:?}", clock.slept());
        store.append(vec![event()], None).expect("appended");
        assert_eq!(clock.slept(), [PAUSE], "the pending pause is still owed");
    }
}
