//! Bench's own sections of [`super::Status`], each read by the `bench` module that owns its
//! data (`docs/bench/status.md`). Positions (`event`) are ledger positions; times (`ts`) are the
//! events' RFC 3339 timestamps.

use serde::{Deserialize, Serialize};

use crate::event::{Mode, Verdict, VerdictSource};

/// `phase`: the worktree's bench mode and the playbook it runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Phase {
    /// `build` or `diagnose`.
    pub mode: Mode,
    /// The playbook of the mode: the events its stop rules require.
    pub playbook: Playbook,
    /// The `mode_set` event's position.
    pub event: u64,
    /// Build mode's plan, relative to the worktree root.
    pub plan: Option<String>,
    /// Build mode's design tree, relative to the worktree root.
    pub tree: Option<String>,
    /// Whether the working tree was clean when build mode was entered.
    pub clean: Option<bool>,
}

/// The active playbook: its name and the events its rules require since the mode was entered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Playbook {
    /// The playbook's name, the mode's.
    pub name: String,
    /// The event types its rules require, by stored name (`bench.test_run`).
    pub requires: Vec<String>,
}

/// `plan`: the build plan's approval at its current bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    /// The plan id: its path relative to the worktree root.
    pub id: String,
    /// Whether the plan is approved at its current bytes.
    pub status: PlanStatus,
    /// The `sha256:` hash of the plan's current bytes.
    pub hash: String,
    /// The latest approval, when there is one.
    pub approved: Option<Approved>,
}

/// Whether the plan is approved at its current bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    /// The latest approval has the plan's hash.
    Approved,
    /// The latest approval has another hash: the plan changed since.
    Stale,
    /// No approval.
    #[serde(rename = "none")]
    NotApproved,
}

impl PlanStatus {
    /// Every status, in order.
    pub const ALL: [Self; 3] = [Self::Approved, Self::Stale, Self::NotApproved];

    /// The name as written: `approved`, `stale` or `none`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Approved => "approved",
            Self::Stale => "stale",
            Self::NotApproved => "none",
        }
    }
}

/// The latest approval: the hash it approved, its ledger position and when.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Approved {
    /// The `sha256:` hash approved.
    pub hash: String,
    /// The `plan_approved` event's position.
    pub event: u64,
    /// When it was approved.
    pub ts: String,
}

/// `tasks`: the build plan's tasks done since build mode was entered, and in all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tasks {
    /// Tasks done.
    pub done: usize,
    /// Tasks in the plan.
    pub total: usize,
    /// The first task not done, when there is one.
    pub next: Option<NextTask>,
}

/// The first task of the plan not done.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NextTask {
    /// Its id (`T2`).
    pub id: String,
    /// Its title, cut to the handoff note's length.
    pub title: String,
}

/// One gate that blocked: a `bench.gate_blocked` or a `bench.stop_blocked` event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gate {
    /// The gate, as its event names it (`guard.rm-rf`, `completeness.claim`).
    pub id: String,
    /// `bench.gate_blocked` or `bench.stop_blocked`.
    pub kind: String,
    /// Why it blocked, in one line.
    pub reason: String,
    /// The event's position.
    pub event: u64,
    /// When it blocked.
    pub ts: String,
}

/// `verdict`: the review verdict for `HEAD`, judged as `bench verdict valid` judges it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeadVerdict {
    /// The pull request judged.
    pub pr: u64,
    /// `HEAD`'s full commit id.
    pub head: String,
    /// The base the patch id is taken from: the latest verdict's.
    pub base: Option<String>,
    /// The verdict's outcome, as `verdict valid` names it.
    pub outcome: Outcome,
    /// True exactly when `outcome` is `valid`.
    pub valid: bool,
    /// The latest verdict still fresh for `HEAD`: by commit, or by patch id after a rebase.
    pub verdict: Option<FreshVerdict>,
    /// How many of the pull request's verdicts are for other code.
    pub stale: usize,
}

/// A verdict's outcome, as `verdict valid` names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// The latest fresh verdict is a level.
    Valid,
    /// The latest fresh verdict says every finding was noise.
    NoiseOnly,
    /// No verdict was ever set for the pull request.
    NoVerdict,
    /// Every verdict is for other code.
    Stale,
    /// The latest fresh verdict requests changes.
    ChangesRequested,
}

impl Outcome {
    /// Every outcome, in order.
    pub const ALL: [Self; 5] = [
        Self::Valid,
        Self::NoiseOnly,
        Self::NoVerdict,
        Self::Stale,
        Self::ChangesRequested,
    ];

    /// The name as written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Valid => "valid",
            Self::NoiseOnly => "noise_only",
            Self::NoVerdict => "no_verdict",
            Self::Stale => "stale",
            Self::ChangesRequested => "changes_requested",
        }
    }
}

/// A fresh verdict: what it says, who set it, for which commit, and why it is fresh.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FreshVerdict {
    /// The `verdict_set` event's position.
    pub event: u64,
    /// What it says.
    pub verdict: Verdict,
    /// Who set it, when recorded.
    pub source: Option<VerdictSource>,
    /// The commit it was set for, when recorded.
    pub sha: Option<String>,
    /// Why it is fresh.
    pub fresh: Fresh,
}

/// Why a verdict is fresh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fresh {
    /// It was set for the head's commit.
    BySha,
    /// Its patch id is the head's, from the same base.
    ByPatchId,
}

/// `clef`: each decision point's answered decisions and its mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clef {
    /// Whether an account is configured; without one every point is off.
    pub configured: bool,
    /// D1 to D7, in order.
    pub points: Vec<PointStatus>,
}

/// One decision point.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PointStatus {
    /// The point.
    pub point: DecisionPoint,
    /// Answered decisions logged for it.
    pub answered: u64,
    /// The mode configured.
    pub configured_mode: ClefMode,
    /// The mode in effect.
    pub mode: ClefMode,
}

/// A Clef decision point, D1 to D7.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecisionPoint {
    /// Guard.
    D1,
    /// Orchestrate.
    D2,
    /// Route.
    D3,
    /// Stop completeness.
    D4,
    /// Verify.
    D5,
    /// Ruled-before.
    D6,
    /// Grill.
    D7,
}

impl DecisionPoint {
    /// Every point, in order.
    pub const ALL: [Self; 7] = [
        Self::D1,
        Self::D2,
        Self::D3,
        Self::D4,
        Self::D5,
        Self::D6,
        Self::D7,
    ];

    /// The name as written: `D1` to `D7`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::D1 => "D1",
            Self::D2 => "D2",
            Self::D3 => "D3",
            Self::D4 => "D4",
            Self::D5 => "D5",
            Self::D6 => "D6",
            Self::D7 => "D7",
        }
    }
}

/// How a decision point's answer is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClefMode {
    /// Clef is not asked.
    Off,
    /// Clef is asked and the would-be action logged.
    Shadow,
    /// Clef is asked and callers act on the action.
    Enforce,
}

impl ClefMode {
    /// Every mode, in order.
    pub const ALL: [Self; 3] = [Self::Off, Self::Shadow, Self::Enforce];

    /// The name as written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Shadow => "shadow",
            Self::Enforce => "enforce",
        }
    }
}

/// `context`: the newest session's context usage, as the status line or the mod saved it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Context {
    /// Tokens in the context window; `None` before Claude Code reports usage.
    pub used_tokens: Option<f64>,
    /// The auto-compact window, in tokens.
    pub window: u32,
    /// Where the window came from.
    pub window_source: WindowSource,
    /// `used_tokens` as a percentage of `window`, to one decimal.
    pub percent: Option<f64>,
    /// The context monitor's threshold reached, if any.
    pub level: Option<Level>,
}

/// Where a context window came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowSource {
    /// `CLAUDE_CODE_AUTO_COMPACT_WINDOW`.
    Env,
    /// The model's context window.
    Model,
    /// Bench's default window.
    Default,
}

/// A context threshold reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    /// 60% of the window or more.
    Warn,
    /// 75% of the window or more.
    Stop,
}

/// One ledger event of `ledger_tail`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TailEvent {
    /// Its position.
    pub event: u64,
    /// Its type, by stored name (`bench.task_done`).
    #[serde(rename = "type")]
    pub kind: String,
    /// Its short type name and what it is about, one line, as the handoff note writes it
    /// (`task_done task:T1`).
    pub summary: String,
    /// Who wrote it.
    pub actor: String,
    /// When.
    pub ts: String,
}
