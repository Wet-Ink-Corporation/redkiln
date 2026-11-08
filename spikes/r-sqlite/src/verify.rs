//! What the writers reported, what the store holds afterwards, and whether the two agree.

use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::Context as _;
use happenstance_sqlite::rusqlite::{Connection, OpenFlags};
use redkiln_core::store::{Filter, Store, Stored, Wait};

use crate::events::{CLAIM_KEY, SEQ_KEY, WRITER_KEY};
use crate::report::{Check, ScenarioResult};

/// One hook process's report.
#[derive(Debug, Clone)]
pub struct HookOp {
    pub writer: String,
    pub seq: u64,
    pub outcome: String,
    pub open_us: u64,
    pub append_us: u64,
    pub telemetry_us: u64,
    pub telemetry: String,
    /// Spawn to exit, measured by the lane; absent in cold start.
    pub wall_us: Option<u64>,
}

/// One command append of `count` events.
#[derive(Debug, Clone)]
pub struct CmdOp {
    pub writer: String,
    pub first: u64,
    pub count: u64,
    pub outcome: String,
    pub us: u64,
}

/// One conditioned claim.
#[derive(Debug, Clone)]
pub struct RaceOp {
    pub outcome: String,
    pub us: u64,
}

/// A long-lived writer's open.
#[derive(Debug, Clone)]
pub struct OpenOp {
    pub outcome: String,
    pub us: u64,
}

/// Every operation a scenario's writers reported.
#[derive(Debug, Default)]
pub struct Ops {
    pub hooks: Vec<HookOp>,
    pub cmds: Vec<CmdOp>,
    pub races: Vec<RaceOp>,
    pub opens: Vec<OpenOp>,
    /// Lines that did not parse, and hook processes that printed nothing.
    pub garbled: Vec<String>,
}

/// A numeric field, or `None` when it is not one: the line is then counted as garbled.
fn num(field: &str) -> Option<u64> {
    field.parse::<u64>().ok()
}

impl Ops {
    /// Reads a writer's stdout. `wall` is a hook lane's spawn-to-exit time for the one hook line
    /// it expects; a hook that printed nothing is counted as garbled.
    pub fn parse(&mut self, stdout: &[u8], wall: Option<u64>) -> anyhow::Result<()> {
        let text = std::str::from_utf8(stdout).context("a writer's output is not UTF-8")?;
        let before = self.hooks.len();
        for line in text.lines() {
            if !self.parse_line(line, wall) {
                self.garbled.push(line.to_owned());
            }
        }
        if wall.is_some() && self.hooks.len() == before {
            self.garbled
                .push("a hook process printed no report".to_owned());
        }
        Ok(())
    }

    fn parse_line(&mut self, line: &str, wall: Option<u64>) -> bool {
        let fields: Vec<&str> = line.split_whitespace().collect();
        match fields.as_slice() {
            ["hook", writer, seq, outcome, open, append, tele_us, tele] => {
                let (Some(seq), Some(open_us), Some(append_us), Some(telemetry_us)) =
                    (num(seq), num(open), num(append), num(tele_us))
                else {
                    return false;
                };
                self.hooks.push(HookOp {
                    writer: (*writer).to_owned(),
                    seq,
                    outcome: (*outcome).to_owned(),
                    open_us,
                    append_us,
                    telemetry_us,
                    telemetry: (*tele).to_owned(),
                    wall_us: wall,
                });
                true
            }
            ["cmd", writer, first, count, outcome, us] => {
                let (Some(first), Some(count), Some(us)) = (num(first), num(count), num(us)) else {
                    return false;
                };
                self.cmds.push(CmdOp {
                    writer: (*writer).to_owned(),
                    first,
                    count,
                    outcome: (*outcome).to_owned(),
                    us,
                });
                true
            }
            ["race", _writer, _claim, outcome, us] => num(us).is_some_and(|us| {
                self.races.push(RaceOp {
                    outcome: (*outcome).to_owned(),
                    us,
                });
                true
            }),
            ["open", _writer, outcome, us] => num(us).is_some_and(|us| {
                self.opens.push(OpenOp {
                    outcome: (*outcome).to_owned(),
                    us,
                });
                true
            }),
            _ => false,
        }
    }

    /// Moves `other`'s operations into these.
    pub fn merge(&mut self, other: Self) {
        self.hooks.extend(other.hooks);
        self.cmds.extend(other.cmds);
        self.races.extend(other.races);
        self.opens.extend(other.opens);
        self.garbled.extend(other.garbled);
    }
}

/// What the watcher saw beside the store while the writers ran.
#[derive(Debug, Default, Clone, Copy)]
pub struct Watch {
    pub polls: u64,
    /// Polls that found the `-journal`.
    pub journal_present: u64,
    /// Polls that found the `-journal` missing after it had been seen: a delete, which TRUNCATE
    /// never does.
    pub journal_vanished: u64,
    /// Polls that found a `-wal`, or a `-shm`.
    pub wal_or_shm: u64,
}

/// Polls the store's siblings every millisecond until `stop`.
pub fn watch(store: &Path, stop: &AtomicBool) -> Watch {
    let journal = sibling(store, "-journal");
    let wal = sibling(store, "-wal");
    let shm = sibling(store, "-shm");
    let mut seen = Watch::default();
    // Relaxed: the flag carries no data, and the scope joins this thread right after setting it.
    while !stop.load(Ordering::Relaxed) {
        seen.polls = seen.polls.saturating_add(1);
        if journal.exists() {
            seen.journal_present = seen.journal_present.saturating_add(1);
        } else if seen.journal_present > 0 {
            seen.journal_vanished = seen.journal_vanished.saturating_add(1);
        }
        if wal.exists() || shm.exists() {
            seen.wal_or_shm = seen.wal_or_shm.saturating_add(1);
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    seen
}

fn sibling(store: &Path, suffix: &str) -> PathBuf {
    let mut name = store.as_os_str().to_owned();
    name.push(suffix);
    name.into()
}

/// Checks the store against the writers' reports.
pub fn verify(store: &Path, name: &str, ops: Ops, watch: Option<Watch>) -> ScenarioResult {
    let reported = if ops.garbled.is_empty() {
        Check::pass(REPORTED, String::new())
    } else {
        Check::fail(REPORTED, ops.garbled.join("; "))
    };
    let mut checks = vec![reported, no_failed_writes(&ops)];
    match read_all(store) {
        Ok(events) => {
            checks.push(contiguous(&events));
            checks.push(appends_match(&events, &ops));
            checks.push(claims_once(&events, &ops));
        }
        Err(error) => checks.push(Check::fail("store reads back", format!("{error:#}"))),
    }
    checks.push(layout(store));
    if let Some(seen) = watch {
        checks.push(watched(seen));
    }
    checks.push(integrity(store));
    ScenarioResult {
        name: name.to_owned(),
        ops,
        checks,
    }
}

fn bump<K: Ord>(map: &mut BTreeMap<K, u64>, key: K) {
    let slot = map.entry(key).or_insert(0);
    *slot = slot.saturating_add(1);
}

const REPORTED: &str = "every writer process reported every operation (none crashed)";

const WRITES: &str = "every open and write succeeded, or lost a claim cleanly";

fn no_failed_writes(ops: &Ops) -> Check {
    let mut failures: BTreeMap<&str, u64> = BTreeMap::new();
    let outcomes = ops
        .hooks
        .iter()
        .map(|h| h.outcome.as_str())
        .chain(ops.cmds.iter().map(|c| c.outcome.as_str()))
        .chain(ops.races.iter().map(|r| r.outcome.as_str()))
        .chain(ops.opens.iter().map(|o| o.outcome.as_str()))
        .chain(ops.hooks.iter().map(|h| h.telemetry.as_str()));
    for outcome in outcomes.filter(|o| o.starts_with("err:")) {
        bump(&mut failures, outcome);
    }
    if failures.is_empty() {
        Check::pass(WRITES, String::new()).load()
    } else {
        let detail = failures
            .iter()
            .map(|(message, count)| format!("{count} x {message}"))
            .collect::<Vec<_>>()
            .join("; ");
        Check::fail(WRITES, detail).load()
    }
}

fn read_all(store: &Path) -> anyhow::Result<Vec<Stored>> {
    let store = Store::open(store, Wait::Command).context("reopening the store")?;
    let page = NonZeroUsize::new(1000).context("page size")?;
    let mut events = Vec::new();
    let mut after = 0;
    loop {
        let batch = store.read(&Filter::all(), after, page).context("reading")?;
        let Some(last) = batch.last() else {
            return Ok(events);
        };
        after = last.position;
        events.extend(batch);
    }
}

const CONTIGUOUS: &str = "positions run 1..N with no gap";

fn contiguous(events: &[Stored]) -> Check {
    let gap = events
        .iter()
        .zip(1_u64..)
        .find(|(event, expected)| event.position != *expected);
    match gap {
        None => Check::pass(CONTIGUOUS, format!("N = {}", events.len())),
        Some((event, expected)) => Check::fail(
            CONTIGUOUS,
            format!("expected {expected}, found {}", event.position),
        ),
    }
}

fn tag_value<'a>(event: &'a Stored, key: &str) -> Option<&'a str> {
    event
        .tags
        .iter()
        .find(|tag| tag.key() == Some(key))
        .and_then(|tag| tag.value())
}

const APPENDS: &str = "every reported append is stored exactly once, and nothing else";

/// Every append a writer reported as written is in the store once, and nothing else is.
fn appends_match(events: &[Stored], ops: &Ops) -> Check {
    let mut stored: BTreeMap<(&str, u64), u64> = BTreeMap::new();
    for event in events
        .iter()
        .filter(|e| e.event_type == "loop.spike_appended")
    {
        let writer = tag_value(event, WRITER_KEY);
        let seq = tag_value(event, SEQ_KEY).and_then(num);
        if let Some(key) = writer.zip(seq) {
            bump(&mut stored, key);
        }
    }
    let mut reported: Vec<(&str, u64)> = ops
        .hooks
        .iter()
        .filter(|h| h.outcome == "ok")
        .map(|h| (h.writer.as_str(), h.seq))
        .collect();
    for cmd in ops.cmds.iter().filter(|c| c.outcome == "ok") {
        let end = cmd.first.saturating_add(cmd.count);
        reported.extend((cmd.first..end).map(|seq| (cmd.writer.as_str(), seq)));
    }
    let missing = reported.iter().filter(|k| !stored.contains_key(*k)).count();
    let duplicated = stored.values().filter(|count| **count > 1).count();
    let found = reported.len().saturating_sub(missing);
    let extra = stored.len().saturating_sub(found);
    let detail = format!(
        "{} reported, {} stored, {missing} missing, {duplicated} duplicated, {extra} stored but reported failed",
        reported.len(),
        stored.len()
    );
    if missing == 0 && duplicated == 0 && extra == 0 {
        Check::pass(APPENDS, detail)
    } else {
        Check::fail(APPENDS, detail)
    }
}

const CLAIMS: &str = "conditioned appends: one winner per subject";

/// Each subject has at most one claim, and the store holds exactly the claims reported won.
fn claims_once(events: &[Stored], ops: &Ops) -> Check {
    let mut per_claim: BTreeMap<&str, u64> = BTreeMap::new();
    for event in events
        .iter()
        .filter(|e| e.event_type == "loop.spike_claimed")
    {
        if let Some(claim) = tag_value(event, CLAIM_KEY) {
            bump(&mut per_claim, claim);
        }
    }
    let doubled = per_claim.values().filter(|count| **count > 1).count();
    let won = ops.races.iter().filter(|r| r.outcome == "won").count();
    let lost = ops.races.iter().filter(|r| r.outcome == "lost").count();
    let claims: u64 = per_claim.values().sum();
    let detail =
        format!("{won} won, {lost} lost, {claims} claims stored, {doubled} subjects claimed twice");
    if doubled == 0 && u64::try_from(won).is_ok_and(|won| won == claims) {
        Check::pass(CLAIMS, detail)
    } else {
        Check::fail(CLAIMS, detail)
    }
}

const LAYOUT: &str = "TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header";

fn layout(store: &Path) -> Check {
    let journal = std::fs::metadata(sibling(store, "-journal")).map(|m| m.len());
    let wal = sibling(store, "-wal").exists();
    let shm = sibling(store, "-shm").exists();
    let header = match std::fs::read(store) {
        Ok(bytes) => bytes.get(18..20).map(<[u8]>::to_vec),
        Err(error) => return Check::fail(LAYOUT, format!("reading the store: {error}")),
    };
    let journal_text = match &journal {
        Ok(len) => format!("{len} bytes"),
        Err(error) => format!("missing ({error})"),
    };
    let detail = format!(
        "-journal {journal_text}, -wal {wal}, -shm {shm}, header versions {header:?} (1 = rollback, 2 = WAL)"
    );
    let ok = journal.is_ok_and(|len| len == 0) && !wal && !shm && header == Some(vec![1, 1]);
    if ok {
        Check::pass(LAYOUT, detail)
    } else {
        Check::fail(LAYOUT, detail)
    }
}

const WATCHED: &str = "nothing deleted, and no -wal or -shm created, while the writers ran";

fn watched(seen: Watch) -> Check {
    let detail = format!(
        "{} polls: -journal present on {}, missing after first seen on {}, -wal or -shm on {}",
        seen.polls, seen.journal_present, seen.journal_vanished, seen.wal_or_shm
    );
    if seen.journal_vanished == 0 && seen.wal_or_shm == 0 {
        Check::pass(WATCHED, detail)
    } else {
        Check::fail(WATCHED, detail)
    }
}

fn integrity(store: &Path) -> Check {
    let result =
        Connection::open_with_flags(store, OpenFlags::SQLITE_OPEN_READ_ONLY).and_then(|conn| {
            conn.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
        });
    match result {
        Ok(text) if text == "ok" => Check::pass("PRAGMA integrity_check", text),
        Ok(text) => Check::fail("PRAGMA integrity_check", text),
        Err(error) => Check::fail("PRAGMA integrity_check", error.to_string()),
    }
}
