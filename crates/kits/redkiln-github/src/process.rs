//! Running a child process to completion within a deadline and with capped output (H-13), for
//! `git` ([`crate::git`], [`crate::diff`]) and `gh` (`tracker::github::GhCli`).
//!
//! A copy of the waiting half of Bench's `process` module (as `tools/skillcheck` has one): the
//! plan duplicates it until `redkiln-dispatch` (Move 4) absorbs the copies. Only what this kit
//! calls is kept, so nothing here is dead code.

use std::convert::Infallible;
use std::io::{self, Read};
use std::process::{Child, ExitStatus, Output};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// How often a running child is checked for completion.
const POLL_INTERVAL: Duration = Duration::from_millis(5);

/// Why waiting for a child failed.
#[derive(Debug, thiserror::Error)]
pub(crate) enum WaitError {
    /// Its pipes could not be read, or it could not be waited on or killed.
    #[error("waiting for the process")]
    Io(#[source] io::Error),
    /// It, or a process still holding its pipes, did not finish within the timeout. The child
    /// itself was killed if still running, and reaped.
    #[error("the process did not finish in time")]
    Timeout,
}

/// Waits for `child`, draining its piped stdout and stderr on named threads (`<label>-stdout`,
/// `<label>-stderr`) so a chatty child cannot block on a full pipe. Each stream keeps at most
/// `cap` bytes; the rest is read and dropped. Past `timeout` the child is killed and reaped,
/// and the error is [`WaitError::Timeout`].
///
/// The output is complete only once every holder of the pipes' write ends has closed them, so
/// a child that exits while a grandchild it started still holds its stdout or stderr is a
/// timeout too, at the deadline (a child that exits just before it still gets about 100 ms for
/// its pipes to close). Either way this returns within `timeout` plus about 100 ms: after a
/// kill the threads get that long to see the pipes close, then their handles are dropped, so
/// one a grandchild still blocks is left to end on its own, holding only its pipe end and at
/// most `cap` bytes, until that grandchild closes the pipe or exits, or this process exits.
/// Only the child is killed, not its descendants: std cannot kill a process group without
/// `unsafe`. Nothing caps these threads across calls: a caller that loops, such as Bench's
/// `watch` polling `gh` through [`crate::tracker::github::GhCli`] (16 MiB cap) for up to
/// 1800 s, can leave one set per timed-out call, each bounded as above, until its
/// grandchildren exit or the process does.
pub(crate) fn wait_with_deadline(
    child: &mut Child,
    timeout: Duration,
    cap: u64,
    label: &str,
) -> Result<Output, WaitError> {
    let started = Instant::now();
    let readers = match Readers::spawn(child, cap, label) {
        Ok(readers) => readers,
        Err(error) => return Err(kill_after(child, error)),
    };
    let status = match wait_or_kill(child, timeout) {
        Ok(status) => status,
        Err(error) => {
            // Killed and reaped: its pipes close now, unless a grandchild holds them too. Which
            // threads end within the grace does not change the answer: both are given up on.
            readers.finished_within(ABANDON_GRACE, Instant::now());
            readers.abandon();
            return Err(error);
        }
    };
    // The child exited, but a grandchild may still hold its output pipes: wait no longer than
    // the deadline for them to close, and at least the grace, so a child that exits just before
    // the deadline is not a timeout while its readers drain what it wrote last.
    let left = timeout.saturating_sub(started.elapsed()).max(ABANDON_GRACE);
    if !readers.finished_within(left, Instant::now()) {
        readers.abandon();
        return Err(WaitError::Timeout);
    }
    readers.join(status)
}

/// How long the pipe threads of a killed child get to see its pipes close before the wait
/// returns without them.
const ABANDON_GRACE: Duration = Duration::from_millis(100);

/// The error when an output reader panicked.
const READER_PANICKED: &str = "an output reader thread panicked";

/// The threads that drain the child's stdout and stderr.
struct Readers {
    out: Worker<Vec<u8>>,
    err: Worker<Vec<u8>>,
}

impl Readers {
    /// Takes the child's piped output streams and starts a named thread for each. A thread
    /// already started when the other fails ends once the caller kills the child.
    fn spawn(child: &mut Child, cap: u64, label: &str) -> io::Result<Self> {
        let read_out = move |pipe| read_capped(pipe, cap);
        let stdout = child.stdout.take();
        let out = Worker::spawn(format!("{label}-stdout"), stdout, read_out, READER_PANICKED)?;
        let read_err = move |pipe| read_capped(pipe, cap);
        let stderr = child.stderr.take();
        let err = Worker::spawn(format!("{label}-stderr"), stderr, read_err, READER_PANICKED)?;
        Ok(Self { out, err })
    }

    /// Whether both readers end within `limit` of `started`.
    fn finished_within(&self, limit: Duration, started: Instant) -> bool {
        self.out.finished_within(limit, started) && self.err.finished_within(limit, started)
    }

    /// Joins the finished readers into the child's [`Output`].
    fn join(self, status: ExitStatus) -> Result<Output, WaitError> {
        Ok(Output {
            status,
            stdout: self.out.join()?,
            stderr: self.err.join()?,
        })
    }

    /// Gives up on the threads, dropping their handles; their results are not wanted. One
    /// still running is blocked on a pipe whose other end a grandchild of the child inherited:
    /// std can neither interrupt a blocked read nor kill a process group without `unsafe`. It
    /// holds only that pipe end and at most `cap` bytes of output, and ends when the grandchild
    /// closes the pipe or exits, or when this process exits, whichever comes first (H-13).
    fn abandon(self) {
        drop(self);
    }
}

/// A named thread that owns one of the child's pipes; none when that stream is not piped.
struct Worker<T> {
    running: Option<Running<T>>,
    /// The error's message when the thread panicked.
    panicked: &'static str,
}

/// A started worker thread and the signal that its work has ended.
struct Running<T> {
    handle: JoinHandle<io::Result<T>>,
    /// Disconnected once the thread's work has ended, returned or unwound: the thread owns the
    /// one sender and drops it then. Nothing is ever sent (`Infallible`) and the channel has
    /// no buffer, so it holds nothing (H-13).
    done: Receiver<Infallible>,
}

impl<T: Default + Send + 'static> Worker<T> {
    fn spawn<P: Send + 'static>(
        name: String,
        pipe: Option<P>,
        work: impl FnOnce(P) -> io::Result<T> + Send + 'static,
        panicked: &'static str,
    ) -> io::Result<Self> {
        let Some(pipe) = pipe else {
            return Ok(Self {
                running: None,
                panicked,
            });
        };
        let (signal, done) = mpsc::sync_channel(0);
        let handle = thread::Builder::new().name(name).spawn(move || {
            let result = work(pipe);
            // The signal: the waiter's `recv_timeout` sees the channel disconnect. A panic in
            // `work` drops it while unwinding, so the waiter wakes then too.
            drop::<SyncSender<Infallible>>(signal);
            result
        })?;
        Ok(Self {
            running: Some(Running { handle, done }),
            panicked,
        })
    }

    /// Waits, without polling, until `limit` past `started` for the thread's work to end;
    /// whether it has. One with nothing piped has nothing to wait for.
    fn finished_within(&self, limit: Duration, started: Instant) -> bool {
        let Some(running) = &self.running else {
            return true;
        };
        let left = limit.saturating_sub(started.elapsed());
        match running.done.recv_timeout(left) {
            Ok(never) => match never {},
            Err(RecvTimeoutError::Disconnected) => true,
            Err(RecvTimeoutError::Timeout) => false,
        }
    }

    /// The thread's result, or the default when nothing was piped.
    fn join(self) -> Result<T, WaitError> {
        let Some(running) = self.running else {
            return Ok(T::default());
        };
        let panicked = self.panicked;
        running
            .handle
            .join()
            .map_err(|_payload| WaitError::Io(io::Error::other(panicked)))?
            .map_err(WaitError::Io)
    }
}

/// Reads `pipe` to its end, keeping the first `cap` bytes and discarding the rest, so the
/// writer never blocks on a full pipe and memory stays bounded.
fn read_capped<R: Read>(mut pipe: R, cap: u64) -> io::Result<Vec<u8>> {
    let mut kept = Vec::new();
    pipe.by_ref().take(cap).read_to_end(&mut kept)?;
    io::copy(&mut pipe, &mut io::sink())?;
    Ok(kept)
}

/// Polls `child` until it exits; past `timeout` kills and reaps it. The deadline is checked
/// before each poll, so an exit first seen after the deadline is a timeout, and a zero
/// timeout always is.
fn wait_or_kill(child: &mut Child, timeout: Duration) -> Result<ExitStatus, WaitError> {
    let started = Instant::now();
    loop {
        if started.elapsed() >= timeout {
            child.kill().map_err(WaitError::Io)?;
            child.wait().map_err(WaitError::Io)?;
            return Err(WaitError::Timeout);
        }
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {}
            // Kill it: the reader threads would otherwise wait on its pipes forever.
            Err(error) => return Err(kill_after(child, error)),
        }
        thread::sleep(POLL_INTERVAL);
    }
}

/// `duration` with its sub-millisecond part dropped, for naming a timeout: debugged, it reads
/// `500ms`, `1s` or `1.987s`, where whole seconds would show a sub-second budget as `0s` and
/// what is left of a deadline would show every nanosecond.
pub(crate) fn whole_millis(duration: Duration) -> Duration {
    Duration::new(
        duration.as_secs(),
        duration.subsec_millis().saturating_mul(1_000_000),
    )
}

/// Kills and reaps `child` after a setup failure, so no reader waits on a live pipe. A failure
/// to kill is reported alongside the original error rather than dropped.
fn kill_after(child: &mut Child, error: io::Error) -> WaitError {
    match child.kill().and_then(|()| child.wait().map(drop)) {
        Ok(()) => WaitError::Io(error),
        Err(kill_error) => WaitError::Io(io::Error::new(
            error.kind(),
            format!("{error}; killing the process also failed: {kill_error}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use super::wait_with_deadline;

    /// Thirty seconds: ample for `git` on any runner.
    const TIMEOUT: Duration = Duration::from_secs(30);

    #[test]
    fn a_childs_output_is_kept_up_to_the_cap() {
        let mut child = Command::new("git")
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("git starts");

        let output = wait_with_deadline(&mut child, TIMEOUT, 3, "git").expect("git finishes");

        assert!(output.status.success(), "{output:?}");
        assert_eq!(output.stdout, b"git", "only the first 3 bytes are kept");
    }

    #[test]
    fn a_worker_reads_as_finished_once_its_work_ends_and_stays_so_with_no_time_left() {
        let (release, blocked) = mpsc::sync_channel::<()>(0);
        let worker = super::Worker::spawn(
            "test-worker".to_owned(),
            Some(blocked),
            |blocked| Ok(blocked.recv().is_err()),
            "the test worker panicked",
        )
        .expect("the worker starts");

        let while_blocked = worker.finished_within(Duration::ZERO, Instant::now());
        drop(release);
        let once_released = worker.finished_within(TIMEOUT, Instant::now());
        let afterwards = worker.finished_within(Duration::ZERO, Instant::now());

        assert!(!while_blocked, "its work had not ended");
        assert!(once_released, "its work ended");
        assert!(afterwards, "an ended worker needs no time to say so");
        assert!(worker.join().expect("it joins"), "it saw the release");
    }

    #[test]
    fn a_worker_whose_work_panics_reads_as_finished_and_joins_as_an_error() {
        let worker = super::Worker::<()>::spawn(
            "test-worker".to_owned(),
            Some(()),
            |()| panic!("the work fails"),
            "the test worker panicked",
        )
        .expect("the worker starts");

        let finished = worker.finished_within(TIMEOUT, Instant::now());
        let joined = worker.join();

        assert!(finished, "unwinding drops the signal too");
        assert!(
            matches!(&joined, Err(super::WaitError::Io(error))
                if error.to_string() == "the test worker panicked"),
            "{joined:?}"
        );
    }

    /// The timeout the grandchild tests give their child.
    #[cfg(unix)]
    const SHORT: Duration = Duration::from_millis(200);

    /// Well past [`SHORT`] and its margin, well under the grandchild's 10 s `sleep`.
    #[cfg(unix)]
    const BOUND: Duration = Duration::from_secs(5);

    /// `sh -c script` with its output piped. A script whose last command is not a simple
    /// command keeps `sh` from exec'ing it, so its `sleep` is a grandchild holding the pipes.
    #[cfg(unix)]
    fn piped_sh(script: &str) -> std::process::Child {
        Command::new("sh")
            .args(["-c", script])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("sh starts")
    }

    #[test]
    #[cfg(unix)]
    #[cfg_attr(miri, ignore = "spawns sh")]
    fn a_grandchild_holding_stdout_does_not_hold_the_wait_past_the_timeout() {
        let mut child = piped_sh("sleep 10; exit 0");

        // The one clock read: the bound is wall time by definition.
        let started = Instant::now();
        let result = wait_with_deadline(&mut child, SHORT, 1024, "sh");
        let elapsed = started.elapsed();

        assert!(
            matches!(result, Err(super::WaitError::Timeout)),
            "{result:?}"
        );
        assert!(elapsed < BOUND, "returned at the deadline: {elapsed:?}");
        assert!(
            child.try_wait().expect("waitable").is_some(),
            "the child was killed and reaped"
        );
    }

    #[test]
    #[cfg(unix)]
    #[cfg_attr(miri, ignore = "spawns sh")]
    fn a_child_that_exits_leaving_a_grandchild_on_its_pipes_times_out_at_the_deadline() {
        let mut child = piped_sh("sleep 10 & exit 0");

        // The one clock read: the bound is wall time by definition.
        let started = Instant::now();
        let result = wait_with_deadline(&mut child, SHORT, 1024, "sh");
        let elapsed = started.elapsed();

        assert!(
            matches!(result, Err(super::WaitError::Timeout)),
            "{result:?}"
        );
        assert!(elapsed < BOUND, "returned at the deadline: {elapsed:?}");
    }
}
