//! `run`: every scenario, each on a fresh store, then the report.

use std::fs::File;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context as _, bail};

use crate::args::Args;
use crate::events::unix_millis;
use crate::report::{Report, ScenarioResult};
use crate::verify::{Ops, Writer, verify};

/// How long after spawning the writers they all start, so that process start-up does not
/// stagger them.
const BARRIER: Duration = Duration::from_millis(500);

/// A mixed-load scenario: who writes, how hard, for how long.
#[derive(Debug, Clone, Copy)]
struct Mix {
    name: &'static str,
    hooks: u32,
    commands: u32,
    racers: u32,
    hook_pause_ms: u64,
    command_pause_ms: u64,
    racer_pause_ms: u64,
    batch: u64,
}

const MIXES: [Mix; 6] = [
    Mix {
        name: "busy: 4 hook lanes, 2 commands (batch 5) every 10 ms, 2 racers every 5 ms",
        hooks: 4,
        commands: 2,
        racers: 2,
        hook_pause_ms: 0,
        command_pause_ms: 10,
        racer_pause_ms: 5,
        batch: 5,
    },
    Mix {
        name: "import: 6 hook lanes every 100 ms, 2 commands appending 256-event chunks back to back",
        hooks: 6,
        commands: 2,
        racers: 0,
        hook_pause_ms: 100,
        command_pause_ms: 0,
        racer_pause_ms: 0,
        batch: 256,
    },
    Mix {
        name: "import, yielding: as import, with 25 ms between chunks",
        hooks: 6,
        commands: 2,
        racers: 0,
        hook_pause_ms: 100,
        command_pause_ms: 25,
        racer_pause_ms: 0,
        batch: 256,
    },
    Mix {
        name: "stress: 4 hook lanes, 2 commands (batch 5), 2 racers, no pauses",
        hooks: 4,
        commands: 2,
        racers: 2,
        hook_pause_ms: 0,
        command_pause_ms: 0,
        racer_pause_ms: 0,
        batch: 5,
    },
    Mix {
        name: "hooks only: 8 hook lanes, no pauses",
        hooks: 8,
        commands: 0,
        racers: 0,
        hook_pause_ms: 0,
        command_pause_ms: 0,
        racer_pause_ms: 0,
        batch: 1,
    },
    Mix {
        name: "paced: 4 hook lanes every 100 ms, 2 commands every 50 ms, 2 racers every 20 ms",
        hooks: 4,
        commands: 2,
        racers: 2,
        hook_pause_ms: 100,
        command_pause_ms: 50,
        racer_pause_ms: 20,
        batch: 5,
    },
];

/// Runs the cold-start rounds and every mix under `--dir`, writes the report to stdout and to
/// `--report` when given, and exits 1 when any correctness check failed.
pub fn run(args: &Args) -> anyhow::Result<ExitCode> {
    let dir = args.path("dir")?;
    let seconds: u64 = args.get_or("seconds", 30)?;
    let rounds: u32 = args.get_or("rounds", 20)?;
    let exe = std::env::current_exe().context("finding this binary")?;
    std::fs::create_dir_all(&dir).context("creating --dir")?;
    let mut report = Report::new(seconds, rounds);
    for round in 0..rounds {
        let store = fresh_store(&dir, &format!("cold-{round}"))?;
        let result = cold_start(&exe, &store, round)?;
        report.cold.push(result);
    }
    for (index, mix) in MIXES.iter().enumerate() {
        let store = fresh_store(&dir, &format!("mix-{index}"))?;
        let result = mixed(&exe, &store, mix, seconds)?;
        report.mixes.push(result);
    }
    let text = report.markdown();
    if let Some(path) = args.optional_path("report") {
        std::fs::write(&path, &text).context("writing --report")?;
    }
    let mut out = std::io::stdout().lock();
    out.write_all(text.as_bytes())
        .context("writing the report")?;
    Ok(if report.passed() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

fn fresh_store(dir: &Path, name: &str) -> anyhow::Result<PathBuf> {
    let folder = dir.join(name);
    if folder.exists() {
        bail!("{} exists; run against an empty --dir", folder.display());
    }
    std::fs::create_dir_all(&folder).context("creating a scenario folder")?;
    Ok(folder.join("events.sqlite3"))
}

/// Eight hook processes open a store that does not exist yet, at the same millisecond.
fn cold_start(exe: &Path, store: &Path, round: u32) -> anyhow::Result<ScenarioResult> {
    let at = unix_millis()?.saturating_add(millis(BARRIER));
    let mut fleet = Fleet::default();
    for n in 0..8 {
        let child = hook_command(exe, store, &format!("h{n}"), u64::from(round))
            .arg("--at")
            .arg(at.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("spawning a hook")?;
        fleet.push(child, Writer::Hook { wall: None });
    }
    let mut ops = Ops::default();
    fleet.finish(&mut ops)?;
    Ok(verify(
        store,
        &format!("cold start round {round}"),
        ops,
        None,
    ))
}

fn hook_command(exe: &Path, store: &Path, writer: &str, seq: u64) -> Command {
    let mut command = Command::new(exe);
    command
        .arg("hook-once")
        .arg("--store")
        .arg(store)
        .arg("--writer")
        .arg(writer)
        .arg("--seq")
        .arg(seq.to_string());
    command
}

/// One mix for `seconds`: long-lived writers as processes, and hook lanes that each start one
/// short-lived hook process after another, the way a session's hooks arrive.
fn mixed(exe: &Path, store: &Path, mix: &Mix, seconds: u64) -> anyhow::Result<ScenarioResult> {
    // Created first, so every writer below opens an existing store; creation is cold start's.
    redkiln_core::store::Store::open(store, redkiln_core::store::Wait::Command)
        .context("creating the mix's store")?;
    let at = unix_millis()?.saturating_add(millis(BARRIER));
    let deadline = at.saturating_add(seconds.saturating_mul(1000));
    let stop = AtomicBool::new(false);
    let (ops, watch) = std::thread::scope(|scope| -> anyhow::Result<_> {
        let watcher = std::thread::Builder::new()
            .name("watch".to_owned())
            .spawn_scoped(scope, || crate::verify::watch(store, &stop))
            .context("starting the watcher")?;
        // Stops the watcher on every way out of this closure, so the scope's join of it ends.
        let stopping = Stop(&stop);
        let shared = Shared {
            exe,
            store,
            at,
            deadline,
        };
        // Kills and reaps every writer still running if anything below returns early.
        let mut long = Fleet::default();
        for n in 0..mix.commands {
            long.push_logged(
                long_lived(shared, "command", &format!("c{n}"), mix)?,
                Writer::LongLived,
            );
        }
        for n in 0..mix.racers {
            long.push_logged(
                long_lived(shared, "racer", &format!("r{n}"), mix)?,
                Writer::LongLived,
            );
        }
        let lanes = (0..mix.hooks)
            .map(|n| {
                std::thread::Builder::new()
                    .name(format!("lane-h{n}"))
                    .spawn_scoped(scope, move || {
                        hook_lane(shared, &format!("h{n}"), mix.hook_pause_ms)
                    })
                    .context("starting a hook lane")
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let mut ops = Ops::default();
        long.finish(&mut ops)?;
        for lane in lanes {
            match lane.join() {
                Ok(lane_ops) => ops.merge(lane_ops?),
                Err(_panic) => ops.lost("a hook lane panicked".to_owned()),
            }
        }
        drop(stopping);
        let watch = watcher
            .join()
            .map_err(|_panic| anyhow::anyhow!("the watcher panicked"))?;
        Ok((ops, watch))
    })?;
    Ok(verify(store, mix.name, ops, Some(watch)))
}

/// What every writer of one mix shares: this binary, the store, the start and the deadline.
#[derive(Debug, Clone, Copy)]
struct Shared<'a> {
    exe: &'a Path,
    store: &'a Path,
    at: u64,
    deadline: u64,
}

fn long_lived(
    shared: Shared<'_>,
    kind: &str,
    writer: &str,
    mix: &Mix,
) -> anyhow::Result<(Child, Logs)> {
    let pause = if kind == "racer" {
        mix.racer_pause_ms
    } else {
        mix.command_pause_ms
    };
    // To files, not pipes: a pipe nobody reads fills at 64 KiB and stalls its writer, and the
    // fleet waits for one writer at a time.
    let folder = shared.store.parent().unwrap_or(Path::new("."));
    let logs = Logs {
        out: folder.join(format!("{writer}.out")),
        err: folder.join(format!("{writer}.err")),
    };
    let stdout = File::create(&logs.out).context("creating a writer's output file")?;
    let stderr = File::create(&logs.err).context("creating a writer's error file")?;
    let child = Command::new(shared.exe)
        .arg(kind)
        .arg("--store")
        .arg(shared.store)
        .arg("--writer")
        .arg(writer)
        .arg("--at")
        .arg(shared.at.to_string())
        .arg("--deadline")
        .arg(shared.deadline.to_string())
        .arg("--pause-ms")
        .arg(pause.to_string())
        .arg("--batch")
        .arg(mix.batch.to_string())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .with_context(|| format!("spawning {kind} {writer}"))?;
    Ok((child, logs))
}

/// Where a long-lived writer's stdout and stderr go.
#[derive(Debug)]
struct Logs {
    out: PathBuf,
    err: PathBuf,
}

/// One hook process after another until the deadline, each timed from spawn to exit.
fn hook_lane(shared: Shared<'_>, writer: &str, pause_ms: u64) -> anyhow::Result<Ops> {
    let mut ops = Ops::default();
    if let Some(ahead) = shared.at.checked_sub(unix_millis()?) {
        std::thread::sleep(Duration::from_millis(ahead));
    }
    let mut seq: u64 = 0;
    while unix_millis()? < shared.deadline {
        let start = Instant::now();
        let output = hook_command(shared.exe, shared.store, writer, seq)
            .output()
            .context("running a hook")?;
        let wall = u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX);
        ops.parse(&output, Writer::Hook { wall: Some(wall) })?;
        seq = seq.saturating_add(1);
        std::thread::sleep(Duration::from_millis(pause_ms));
    }
    Ok(ops)
}

/// Sets the watcher's stop flag when dropped.
struct Stop<'a>(&'a AtomicBool);

impl Drop for Stop<'_> {
    fn drop(&mut self) {
        // The watcher only reads; Relaxed is enough for a stop flag the scope joins right after.
        self.0.store(true, Ordering::Relaxed);
    }
}

/// Writer processes this run owns: `finish` waits for each and reads its output; any still
/// held when the fleet is dropped (an early return) are killed and reaped.
#[derive(Default)]
struct Fleet(Vec<(Child, Writer, Option<Logs>)>);

impl Fleet {
    /// A child whose stdout and stderr are pipes; only for one that prints a line or two.
    fn push(&mut self, child: Child, writer: Writer) {
        self.0.push((child, writer, None));
    }

    /// A child writing to `logs`, read once it has exited.
    fn push_logged(&mut self, (child, logs): (Child, Logs), writer: Writer) {
        self.0.push((child, writer, Some(logs)));
    }

    fn finish(mut self, ops: &mut Ops) -> anyhow::Result<()> {
        while let Some((child, writer, logs)) = self.0.pop() {
            match collect(child, logs) {
                Ok(output) => ops.parse(&output, writer)?,
                Err(error) => ops.lost(format!("waiting for a writer: {error:#}")),
            }
        }
        Ok(())
    }
}

/// Waits for `child` and gathers its output, from its pipes or its log files.
fn collect(mut child: Child, logs: Option<Logs>) -> anyhow::Result<Output> {
    let Some(logs) = logs else {
        return child.wait_with_output().context("waiting");
    };
    let status = child.wait().context("waiting")?;
    Ok(Output {
        status,
        stdout: std::fs::read(&logs.out).context("reading a writer's output")?,
        stderr: std::fs::read(&logs.err).context("reading a writer's errors")?,
    })
}

impl Drop for Fleet {
    fn drop(&mut self) {
        for (child, _, _) in &mut self.0 {
            let reaped = child.kill().and_then(|()| child.wait().map(drop));
            if let Err(error) = reaped {
                let mut err = std::io::stderr().lock();
                if writeln!(err, "r-sqlite: could not stop a writer: {error}").is_err() {
                    // stderr is gone too: there is nowhere left to say it.
                }
            }
        }
    }
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn sh(script: &str, logs: Option<&Logs>) -> std::io::Result<Child> {
        let mut command = Command::new("sh");
        command.arg("-c").arg(script);
        if let Some(logs) = logs {
            command
                .stdout(File::create(&logs.out)?)
                .stderr(File::create(&logs.err)?);
        }
        command.spawn()
    }

    #[test]
    fn logged_writers_never_stall_on_a_full_pipe() {
        let dir = tempfile_dir().expect("temp dir");
        let mut fleet = Fleet::default();
        let start = Instant::now();
        for n in 0..2 {
            let logs = Logs {
                out: dir.join(format!("w{n}.out")),
                err: dir.join(format!("w{n}.err")),
            };
            // Far past a pipe's 64 KiB: through a pipe nobody read, the second would block.
            let child =
                sh("head -c 1000000 /dev/zero | tr '\\0' 'y'", Some(&logs)).expect("sh runs");
            fleet.push_logged((child, logs), Writer::LongLived);
        }
        let mut sizes = Vec::new();
        while let Some((child, _, logs)) = fleet.0.pop() {
            sizes.push(collect(child, logs).expect("collects").stdout.len());
        }
        assert_eq!(sizes, vec![1_000_000, 1_000_000]);
        assert!(start.elapsed() < Duration::from_secs(30));
    }

    #[test]
    fn dropping_a_fleet_kills_and_reaps_its_writers() {
        let mut fleet = Fleet::default();
        fleet.push(sh("sleep 60", None).expect("sh runs"), Writer::LongLived);
        let id = fleet
            .0
            .first()
            .map(|(child, _, _)| child.id())
            .expect("one child");
        let start = Instant::now();
        drop(fleet);
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "killed, not waited out"
        );
        // Reaped: the process id no longer names a live child of ours.
        let alive = Command::new("kill")
            .arg("-0")
            .arg(id.to_string())
            .status()
            .expect("kill runs");
        assert!(!alive.success());
    }

    #[test]
    fn dropping_stop_sets_the_flag() {
        let flag = AtomicBool::new(false);
        drop(Stop(&flag));
        assert!(flag.load(Ordering::Relaxed));
    }

    fn tempfile_dir() -> std::io::Result<PathBuf> {
        let dir = std::env::temp_dir().join(format!("r-sqlite-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir)?;
        Ok(dir)
    }
}
