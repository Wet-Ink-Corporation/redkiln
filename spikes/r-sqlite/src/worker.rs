//! The writers `run` starts, each its own process. Every operation prints one line on stdout:
//!
//! - `hook <writer> <seq> <outcome> <open_us> <append_us> <telemetry_us> <telemetry>`
//! - `cmd <writer> <first_seq> <count> <outcome> <append_us>`
//! - `race <writer> <claim> <won|lost|err:..> <append_us>`
//! - `open <writer> <outcome> <open_us>`
//!
//! An outcome is `ok` or `err:<message with spaces as underscores>`.

use std::io::Write as _;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

use anyhow::Context as _;
use redkiln_core::envelope::Actor;
use redkiln_core::store::{Appended, Recorded, Store, Wait};

use crate::args::Args;
use crate::events::{appended, claimed, timed, unix_millis};

/// One hook process: open under the hook's 2 s wait, append one durable event, record one
/// telemetry event, exit. `--at` holds it until that Unix millisecond, so a round of hooks
/// starts together.
pub fn hook_once(args: &Args) -> anyhow::Result<()> {
    let store = args.path("store")?;
    let writer: String = args.get("writer")?;
    let seq: u64 = args.get("seq")?;
    wait_until(args.get_or("at", 0)?)?;
    let line = match hook_body(&store, &writer, seq) {
        Ok(line) => line,
        Err(error) => format!("hook {writer} {seq} {} 0 0 0 none", outcome_err(&error)),
    };
    emit(&line)
}

fn hook_body(path: &Path, writer: &str, seq: u64) -> anyhow::Result<String> {
    let start = Instant::now();
    let opened = Store::open(path, Wait::Hook);
    let open_us = micros(start.elapsed());
    let mut store = match opened {
        Ok(store) => store,
        Err(error) => {
            let message = outcome_err(&anyhow::Error::new(error).context("open"));
            return Ok(format!("hook {writer} {seq} {message} {open_us} 0 0 none"));
        }
    };
    let event = appended(Actor::Hook, writer, seq)?;
    let start = Instant::now();
    let written = store.append(vec![event], None);
    let append_us = micros(start.elapsed());
    let outcome = match written {
        Ok(Appended::At(_)) => "ok".to_owned(),
        Ok(Appended::Conflict) => "err:unconditioned_conflict".to_owned(),
        Err(error) => outcome_err(&anyhow::Error::new(error).context("append")),
    };
    let start = Instant::now();
    let recorded = store.record_telemetry(timed(append_us)?, SystemTime::now());
    let telemetry_us = micros(start.elapsed());
    let telemetry = match recorded {
        Ok(Recorded::At(_)) => "written".to_owned(),
        Ok(Recorded::Dropped) => "dropped".to_owned(),
        Err(error) => outcome_err(&anyhow::Error::new(error).context("telemetry")),
    };
    Ok(format!(
        "hook {writer} {seq} {outcome} {open_us} {append_us} {telemetry_us} {telemetry}"
    ))
}

/// A long-lived command writer, as Loop runs one: open once under the command's 10 s wait,
/// then append batches of `--batch` events until `--deadline`, pausing `--pause-ms` between.
pub fn command(args: &Args) -> anyhow::Result<()> {
    let path = args.path("store")?;
    let writer: String = args.get("writer")?;
    let deadline: u64 = args.get("deadline")?;
    let pause = Duration::from_millis(args.get_or("pause-ms", 0)?);
    let batch: u64 = args.get_or("batch", 1)?;
    wait_until(args.get_or("at", 0)?)?;
    let Some(store) = open_reported(&path, &writer)? else {
        return Ok(());
    };
    let mut seq: u64 = 0;
    while unix_millis()? < deadline {
        let events = (seq..seq.saturating_add(batch))
            .map(|n| appended(Actor::Cli, &writer, n))
            .collect::<anyhow::Result<Vec<_>>>()?;
        let start = Instant::now();
        let written = store.append(events, None);
        let us = micros(start.elapsed());
        let outcome = match written {
            Ok(Appended::At(_)) => "ok".to_owned(),
            Ok(Appended::Conflict) => "err:unconditioned_conflict".to_owned(),
            Err(error) => outcome_err(&anyhow::Error::new(error)),
        };
        emit(&format!("cmd {writer} {seq} {batch} {outcome} {us}"))?;
        seq = seq.saturating_add(batch);
        std::thread::sleep(pause);
    }
    Ok(())
}

/// A conditioned writer (ST-10): claims subjects 0, 1, 2, ... in order, each only if no claim
/// of it exists yet. Two racers run the same sequence; each subject must end with one claim.
pub fn racer(args: &Args) -> anyhow::Result<()> {
    let path = args.path("store")?;
    let writer: String = args.get("writer")?;
    let deadline: u64 = args.get("deadline")?;
    let pause = Duration::from_millis(args.get_or("pause-ms", 0)?);
    wait_until(args.get_or("at", 0)?)?;
    let Some(store) = open_reported(&path, &writer)? else {
        return Ok(());
    };
    let mut claim: u64 = 0;
    while unix_millis()? < deadline {
        let (event, condition) = claimed(&writer, claim)?;
        let start = Instant::now();
        let written = store.append(vec![event], Some(&condition));
        let us = micros(start.elapsed());
        let outcome = match written {
            Ok(Appended::At(_)) => "won".to_owned(),
            Ok(Appended::Conflict) => "lost".to_owned(),
            Err(error) => outcome_err(&anyhow::Error::new(error)),
        };
        emit(&format!("race {writer} {claim} {outcome} {us}"))?;
        claim = claim.saturating_add(1);
        std::thread::sleep(pause);
    }
    Ok(())
}

fn open_reported(path: &Path, writer: &str) -> anyhow::Result<Option<Store>> {
    let start = Instant::now();
    let opened = Store::open(path, Wait::Command);
    let us = micros(start.elapsed());
    match opened {
        Ok(store) => {
            emit(&format!("open {writer} ok {us}"))?;
            Ok(Some(store))
        }
        Err(error) => {
            let message = outcome_err(&anyhow::Error::new(error));
            emit(&format!("open {writer} {message} {us}"))?;
            Ok(None)
        }
    }
}

/// Sleeps until Unix millisecond `at`; 0 or a past time returns at once.
fn wait_until(at: u64) -> anyhow::Result<()> {
    let now = unix_millis()?;
    if let Some(ahead) = at.checked_sub(now) {
        std::thread::sleep(Duration::from_millis(ahead));
    }
    Ok(())
}

fn micros(elapsed: Duration) -> u64 {
    u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX)
}

/// The error chain as one token: `err:` and the messages, spaces and colons made underscores.
fn outcome_err(error: &anyhow::Error) -> String {
    let text = format!("{error:#}");
    let token: String = text
        .chars()
        .map(|c| {
            if c.is_whitespace() || c == ':' {
                '_'
            } else {
                c
            }
        })
        .collect();
    format!("err:{token}")
}

fn emit(line: &str) -> anyhow::Result<()> {
    let mut out = std::io::stdout().lock();
    writeln!(out, "{line}").context("writing to stdout")
}
