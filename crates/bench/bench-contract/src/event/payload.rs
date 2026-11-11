//! The payloads of Bench's 17 event types, and the enums they hold.
//!
//! Every payload field name is a persisted JSON key, and every optional field is
//! `#[serde(default)]`, so an event written before the field existed still reads.

use serde::{Deserialize, Serialize};

use super::By;

/// Whether a test run passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestOutcome {
    /// Every test passed.
    Green,
    /// At least one test failed.
    Red,
}

/// `test_run`: a test command ran. Subject tag `task:` when `task` is set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestRun {
    /// The task the run was for, if any.
    #[serde(default)]
    pub task: Option<String>,
    /// The command line that ran.
    pub command: String,
    /// Whether it passed.
    pub outcome: TestOutcome,
    /// The command's exit code; `None` when it has none: it was killed (on a timeout, or by a
    /// signal) (registry v3).
    #[serde(default)]
    pub exit: Option<i32>,
    /// How long the command ran, in milliseconds, from a monotonic clock (registry v3).
    #[serde(default)]
    pub duration_ms: Option<u64>,
    /// The deadline in seconds, set only when the command ran past it and was killed
    /// (registry v3).
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

/// `task_done`: a task was marked done. Subject tag `task:`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskDone {
    /// The task id.
    pub task: String,
}

/// `plan_approved`: a plan was approved at a content hash. Subject tag `plan:`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanApproved {
    /// The plan id (its path relative to the repository root).
    pub plan: String,
    /// The hash of the plan's content that was approved.
    pub hash: String,
    /// Who approved it and through what, as far as the approval said (#62); written only when
    /// known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<By>,
}

/// `grill_confirmed`: the human confirmed a plan's grilling. Subject tag `plan:`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrillConfirmed {
    /// The plan id: for `bench grill confirm`, the design tree's path as given.
    pub plan: String,
    /// The hash of the design tree that was confirmed, `sha256:<64 hex digits>` (registry v2).
    #[serde(default)]
    pub hash: Option<String>,
    /// The human's confirmation, word for word (registry v2).
    #[serde(default)]
    pub quote: Option<String>,
}

/// `repro_red`: a reproduction test was seen failing before its fix. Subject tag `task:`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReproRed {
    /// The task id.
    pub task: String,
    /// The test that failed.
    pub test: String,
}

/// A review verdict: a level a change reached (`tests_verified`, `reviewed`, `approved`, from
/// lowest to highest; the first two in registry v6), or a review's outcome
/// (`changes_requested`, `noise_only`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Level: the change's tests were verified green (registry v6).
    TestsVerified,
    /// Level: the change was reviewed (registry v6).
    Reviewed,
    /// Level: the change may merge.
    Approved,
    /// The change needs work.
    ChangesRequested,
    /// Every finding was noise.
    NoiseOnly,
}

/// Who set a verdict (registry v6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerdictSource {
    /// anneal's own checks in the session, such as `verify`.
    Local,
    /// The review plugin.
    ReviewPlugin,
    /// A human.
    Human,
}

/// `verdict_set`: a review verdict was set on a pull request. Subject tag `pr:`.
///
/// `sha`, `patch_id` and `base` say what the verdict is for: it is fresh for a head whose commit
/// is `sha`, or whose `git patch-id --verbatim` of the diff from `base` is `patch_id` (a
/// rebase that changed nothing else).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerdictSet {
    /// The pull request number.
    pub pr: u64,
    /// The verdict.
    pub verdict: Verdict,
    /// Who set it (registry v6).
    #[serde(default)]
    pub source: Option<VerdictSource>,
    /// The full commit id of the head it was set for (registry v6).
    #[serde(default)]
    pub sha: Option<String>,
    /// `git patch-id --verbatim` of `git diff <base>...<sha>`; `None` for an empty diff
    /// (registry v6).
    #[serde(default)]
    pub patch_id: Option<String>,
    /// The base ref the patch id was taken from, as given (registry v6).
    #[serde(default)]
    pub base: Option<String>,
    /// Who set it and through what, as far as the verdict said (#62); written only when known.
    /// `source` is what the verdict claims to come from; `by` is who gave it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<By>,
}

/// `round_started`: a review or fix-loop round started, on a pull request or a ticket.
///
/// Subject tag `pr:` when `pr` is set; a ticket round has no subject tag and is found by its
/// type and `ticket` (`bench round`). It names a pull request, a ticket or both, never
/// neither: it is built with [`RoundStarted::for_pr`] or [`RoundStarted::for_ticket`], and an
/// event naming neither does not read. A missing one is not written, so a pull request round
/// keeps registry v6's shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RoundStartedFields")]
pub struct RoundStarted {
    /// The pull request number; optional since registry v8, required before.
    #[serde(skip_serializing_if = "Option::is_none")]
    pr: Option<u64>,
    /// The ticket the round is for, as given to `bench round start --ticket` (registry v8).
    #[serde(skip_serializing_if = "Option::is_none")]
    ticket: Option<String>,
    /// The round number, from 1.
    round: u32,
}

impl RoundStarted {
    /// Round `round` of the review of pull request `pr`.
    #[must_use]
    pub const fn for_pr(pr: u64, round: u32) -> Self {
        Self {
            pr: Some(pr),
            ticket: None,
            round,
        }
    }

    /// Round `round` of the fix loop of `ticket`. The ticket is stored, so it is taken owned.
    #[must_use]
    pub fn for_ticket(ticket: impl Into<String>, round: u32) -> Self {
        Self {
            pr: None,
            ticket: Some(ticket.into()),
            round,
        }
    }

    /// The pull request, if the round is a review round.
    #[must_use]
    pub const fn pr(&self) -> Option<u64> {
        self.pr
    }

    /// The ticket, if the round is a fix-loop round.
    #[must_use]
    pub fn ticket(&self) -> Option<&str> {
        self.ticket.as_deref()
    }

    /// The round number, from 1.
    #[must_use]
    pub const fn round(&self) -> u32 {
        self.round
    }
}

/// `round_started` as stored, before it is checked to name a pull request or a ticket.
#[derive(Deserialize)]
struct RoundStartedFields {
    #[serde(default)]
    pr: Option<u64>,
    #[serde(default)]
    ticket: Option<String>,
    round: u32,
}

/// A `round_started` that names neither a pull request nor a (non-empty) ticket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("round_started names neither a pull request (pr) nor a ticket")]
pub struct RoundOfNothing;

impl TryFrom<RoundStartedFields> for RoundStarted {
    type Error = RoundOfNothing;

    fn try_from(fields: RoundStartedFields) -> Result<Self, Self::Error> {
        let RoundStartedFields { pr, ticket, round } = fields;
        let ticket = ticket.filter(|ticket| !ticket.is_empty());
        if pr.is_none() && ticket.is_none() {
            return Err(RoundOfNothing);
        }
        Ok(Self { pr, ticket, round })
    }
}

/// `gate_blocked`: a gate refused to let work proceed. Subject tag `point:`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateBlocked {
    /// The gate (decision point) that blocked.
    pub point: String,
    /// Why, in one line.
    pub reason: String,
}

/// `stop_blocked`: the Stop hook refused to let the session stop. Subject tag `point:`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StopBlocked {
    /// The check that blocked the stop.
    pub point: String,
    /// Why, in one line.
    pub reason: String,
}

/// A bench mode: what the preconditions hook enforces in a worktree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Building from an approved plan: source edits need the define work.
    Build,
    /// Diagnosing a failure: source edits need a red reproduction first.
    Diagnose,
    /// No mode: the hook enforces nothing.
    Off,
}

/// `mode_set`: a worktree entered or left a bench mode (registry v4). Its subject is the
/// envelope's `worktree` (tagged `worktree:`); the latest one for a worktree is its mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModeSet {
    /// The mode entered.
    pub mode: Mode,
    /// Whether the working tree had no changes (untracked files included) at entry.
    pub clean: bool,
    /// For build mode, the plan's path relative to the worktree root, `/`-separated.
    #[serde(default)]
    pub plan: Option<String>,
    /// For build mode, the design tree's path relative to the worktree root, `/`-separated.
    #[serde(default)]
    pub tree: Option<String>,
}

/// The status a subagent reports on its `Status:` line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    /// `DONE`: the work is finished and verified.
    Done,
    /// `DONE_WITH_CONCERNS`: finished, with concerns the caller should read.
    DoneWithConcerns,
    /// `NEEDS_CONTEXT`: it cannot go on without more context.
    NeedsContext,
    /// `BLOCKED`: it needs a decision it cannot take.
    Blocked,
    /// `WIP`: temper's pause, the work unfinished and the reason given (registry version 10).
    Wip,
}

/// Tokens a subagent used, summed over its transcript's assistant messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TokenUsage {
    /// Input tokens not read from or written to the cache.
    pub input: u64,
    /// Output tokens.
    pub output: u64,
    /// Input tokens read from the prompt cache.
    pub cache_read: u64,
    /// Input tokens written to the prompt cache.
    pub cache_creation: u64,
}

/// `subagent_status`: a subagent stopped (registry v5). Subject tag `agent:` (its type).
///
/// `status` is `None` when the last message had no valid `Status:` line; `violation` then says
/// why, as it does when required sections are `missing`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubagentStatus {
    /// The subagent's type, such as `Explore` or `temper:rust-reviewer`.
    pub agent: String,
    /// The subagent's id in its session.
    #[serde(default)]
    pub agent_id: Option<String>,
    /// The status it reported, when it reported a known one.
    #[serde(default)]
    pub status: Option<AgentStatus>,
    /// What is wrong with its status block, in one line, when anything is.
    #[serde(default)]
    pub violation: Option<String>,
    /// The required sections its status block lacks.
    #[serde(default)]
    pub missing: Vec<String>,
    /// From its transcript's first to last timestamp, in milliseconds.
    #[serde(default)]
    pub duration_ms: Option<u64>,
    /// The tokens it used, from its transcript.
    #[serde(default)]
    pub tokens: Option<TokenUsage>,
}

/// `migrate.wave.started` and `migrate.wave.done`: a wave of an expand, migrate, contract
/// migration started or finished (registry v7). Subject tag `migration:`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrateWave {
    /// The migration's id.
    pub migration: String,
    /// The wave's number within the migration.
    pub wave: u32,
}

/// `migrate.contract`: a migration's contract step ran, removing the old shape (registry v7).
/// Subject tag `migration:`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrateContract {
    /// The migration's id.
    pub migration: String,
}

/// `weighin_parked`: a one-way call was parked in weigh-in with a default (registry v9).
///
/// `bench orchestrate park` raised it and deferred it. The decision itself is weigh-in's; this
/// records only that the ticket waits on `item_id`. Subject tag `task:` (the ticket).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeighinParked {
    /// The weigh-in item the call was raised as, `wi-` and its id.
    pub item_id: String,
    /// The ticket the call blocks.
    pub ticket: String,
    /// The Clef D2 decision that parked it, so its outcome can be labelled later; written only
    /// when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<String>,
}

/// `bridge.unavailable`: the weigh-in bridge could not park a call (registry v9).
///
/// There was no `weighin`, or it failed, so the orchestrator rules instead. Subject tag `task:`
/// (the ticket).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeUnavailable {
    /// The ticket whose call could not be parked.
    pub ticket: String,
    /// Why, in one line.
    pub reason: String,
}

/// `orchestrate.ruled`: the orchestrator ruled on a call itself (registry v9).
///
/// It rules below D2's one-way threshold, in shadow, or with the bridge unavailable. Subject
/// tag `task:` (the ticket).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchestrateRuled {
    /// The ticket the call was made in.
    pub ticket: String,
    /// What was ruled.
    pub what: String,
    /// Why.
    pub why: String,
    /// What it costs to undo if the ruling is wrong.
    pub cost_if_wrong: String,
    /// The Clef D2 decision behind the ruling, if one was asked; written only when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<String>,
}
