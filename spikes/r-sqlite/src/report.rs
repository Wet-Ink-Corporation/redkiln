//! The run's Markdown report: each scenario's checks, then its latencies.

use std::fmt::Write as _;

use crate::verify::Ops;

/// What a failed check means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A lost, duplicated or phantom event, a deleted or WAL file, a corrupt store: the run fails.
    Correctness,
    /// A write refused cleanly on a busy lock past its timeout: reported, never a failed run.
    Load,
}

/// One check's verdict.
#[derive(Debug)]
pub struct Check {
    pub name: &'static str,
    pub kind: Kind,
    pub passed: bool,
    pub detail: String,
}

impl Check {
    pub const fn pass(name: &'static str, detail: String) -> Self {
        Self {
            name,
            kind: Kind::Correctness,
            passed: true,
            detail,
        }
    }

    pub const fn fail(name: &'static str, detail: String) -> Self {
        Self {
            name,
            kind: Kind::Correctness,
            passed: false,
            detail,
        }
    }

    /// The same check, as a load check.
    pub const fn load(mut self) -> Self {
        self.kind = Kind::Load;
        self
    }

    fn fails_run(&self) -> bool {
        !self.passed && self.kind == Kind::Correctness
    }
}

/// A scenario's operations and checks.
#[derive(Debug)]
pub struct ScenarioResult {
    pub name: String,
    pub ops: Ops,
    pub checks: Vec<Check>,
}

impl ScenarioResult {
    fn passed(&self) -> bool {
        !self.checks.iter().any(Check::fails_run)
    }

    /// Every check passed, load included.
    fn clean(&self) -> bool {
        self.checks.iter().all(|check| check.passed)
    }
}

/// The whole run.
#[derive(Debug)]
pub struct Report {
    seconds: u64,
    rounds: u32,
    pub cold: Vec<ScenarioResult>,
    pub mixes: Vec<ScenarioResult>,
}

impl Report {
    pub const fn new(seconds: u64, rounds: u32) -> Self {
        Self {
            seconds,
            rounds,
            cold: Vec::new(),
            mixes: Vec::new(),
        }
    }

    /// No correctness check of any scenario failed.
    pub fn passed(&self) -> bool {
        self.cold
            .iter()
            .chain(&self.mixes)
            .all(ScenarioResult::passed)
    }

    pub fn markdown(&self) -> String {
        let mut out = String::new();
        // Writing to a String cannot fail; the results are collected rather than discarded.
        let written = self.write_markdown(&mut out);
        if written.is_err() {
            out.push_str("\n(report formatting failed)\n");
        }
        out
    }

    fn write_markdown(&self, out: &mut String) -> std::fmt::Result {
        writeln!(
            out,
            "# R-SQLITE run: {} {}\n",
            std::env::consts::OS,
            std::env::consts::ARCH
        )?;
        writeln!(
            out,
            "Correctness: **{}** (no event lost, duplicated or invented; no file deleted beside the store; no WAL; integrity ok). Busy refusals are load findings, reported per scenario.\n",
            if self.passed() { "PASS" } else { "FAIL" }
        )?;
        self.write_cold(out)?;
        for mix in &self.mixes {
            write_mix(out, mix, self.seconds)?;
        }
        Ok(())
    }

    fn write_cold(&self, out: &mut String) -> std::fmt::Result {
        let clean = self.cold.iter().filter(|r| r.clean()).count();
        writeln!(out, "## Cold start: 8 hooks open a missing store at once\n")?;
        writeln!(
            out,
            "{clean} of {} rounds passed every check.\n",
            self.rounds
        )?;
        for failed in self.cold.iter().filter(|r| !r.clean()) {
            write_checks(out, failed)?;
        }
        let hooks: Vec<_> = self.cold.iter().flat_map(|r| r.ops.hooks.iter()).collect();
        let open: Vec<u64> = hooks.iter().map(|h| h.open_us).collect();
        let append: Vec<u64> = hooks.iter().map(|h| h.append_us).collect();
        writeln!(out, "| Step (ms) | n | p50 | p95 | p99 | max |")?;
        writeln!(out, "|---|---|---|---|---|---|")?;
        write_row(out, "open (creates or waits)", &open)?;
        write_row(out, "append", &append)?;
        writeln!(out)
    }
}

fn write_checks(out: &mut String, result: &ScenarioResult) -> std::fmt::Result {
    writeln!(out, "| Check | Result | Detail |")?;
    writeln!(out, "|---|---|---|")?;
    for check in &result.checks {
        let mark = match (check.passed, check.kind) {
            (true, _) => "pass",
            (false, Kind::Load) => "**busy**",
            (false, Kind::Correctness) => "**FAIL**",
        };
        let detail = check.detail.replace('|', "/");
        writeln!(out, "| {} | {mark} | {detail} |", check.name)?;
    }
    writeln!(out)
}

fn write_mix(out: &mut String, mix: &ScenarioResult, seconds: u64) -> std::fmt::Result {
    writeln!(out, "## {} ({seconds} s)\n", mix.name)?;
    write_checks(out, mix)?;
    let ops = &mix.ops;
    let hooks = &ops.hooks;
    let durable: Vec<u64> = hooks
        .iter()
        .map(|h| h.open_us.saturating_add(h.append_us))
        .collect();
    let col = |f: fn(&crate::verify::HookOp) -> u64| hooks.iter().map(f).collect::<Vec<_>>();
    let wall: Vec<u64> = hooks.iter().filter_map(|h| h.wall_us).collect();
    let cmd: Vec<u64> = ops.cmds.iter().map(|c| c.us).collect();
    let race: Vec<u64> = ops.races.iter().map(|r| r.us).collect();
    let opens: Vec<u64> = ops.opens.iter().map(|o| o.us).collect();
    let written = hooks.iter().filter(|h| h.telemetry == "written").count();
    let dropped = hooks.iter().filter(|h| h.telemetry == "dropped").count();
    let events: u64 = ops
        .cmds
        .iter()
        .filter(|c| c.outcome == "ok")
        .map(|c| c.count)
        .sum::<u64>()
        .saturating_add(
            u64::try_from(hooks.iter().filter(|h| h.outcome == "ok").count()).unwrap_or(0),
        )
        .saturating_add(
            u64::try_from(ops.races.iter().filter(|r| r.outcome == "won").count()).unwrap_or(0),
        );
    let per_second = events.checked_div(seconds).unwrap_or(0);
    writeln!(
        out,
        "{} hook processes, {} command appends, {} claims; {events} durable events (~{per_second}/s). Telemetry: {written} written, {dropped} dropped on a busy lock.\n",
        hooks.len(),
        ops.cmds.len(),
        ops.races.len()
    )?;
    let over = |limit_us: u64| durable.iter().filter(|us| **us > limit_us).count();
    writeln!(
        out,
        "Hook durable path (open + append) over 250 ms: {}, over 1 s: {}, over 2 s: {}.\n",
        over(250_000),
        over(1_000_000),
        over(2_000_000)
    )?;
    writeln!(out, "| Step (ms) | n | p50 | p95 | p99 | max |")?;
    writeln!(out, "|---|---|---|---|---|---|")?;
    write_row(out, "hook open", &col(|h| h.open_us))?;
    write_row(out, "hook append", &col(|h| h.append_us))?;
    write_row(out, "hook open + append", &durable)?;
    write_row(out, "hook telemetry", &col(|h| h.telemetry_us))?;
    write_row(out, "hook process, spawn to exit", &wall)?;
    write_row(out, "command open", &opens)?;
    write_row(out, "command append (batch)", &cmd)?;
    write_row(out, "conditioned claim", &race)?;
    writeln!(out)
}

fn write_row(out: &mut String, name: &str, micros: &[u64]) -> std::fmt::Result {
    let mut sorted = micros.to_vec();
    sorted.sort_unstable();
    let ms = |p: u64| percentile(&sorted, p).map_or_else(|| "-".to_owned(), format_ms);
    writeln!(
        out,
        "| {name} | {} | {} | {} | {} | {} |",
        sorted.len(),
        ms(50),
        ms(95),
        ms(99),
        ms(100)
    )
}

/// The `p`th percentile of `sorted` by nearest rank, `None` when empty.
fn percentile(sorted: &[u64], p: u64) -> Option<u64> {
    let len = u64::try_from(sorted.len()).ok()?;
    let rank = len.checked_mul(p)?.div_ceil(100).max(1);
    let index = usize::try_from(rank.checked_sub(1)?).ok()?;
    sorted.get(index).copied()
}

fn format_ms(micros: u64) -> String {
    format!("{}.{:01}", micros / 1000, (micros % 1000) / 100)
}
