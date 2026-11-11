//! The `data` of `bench status --json`, schema `bench.status/1` (SPEC CLI-12).
//!
//! CLI-12's common keys come first, then Bench's sections (`phase` … `ledger_tail`, [`Phase`]
//! to [`TailEvent`]); field order is wire order. Every `Option` is written as `null`, never left
//! out, since CLI-12 lists every key as present and a section with no data, or not asked for
//! with `--fields`, is `null`. Unknown keys are accepted when read, at the top and in a
//! section: CLI-04 lets a version grow additively, so a reader takes what a later Bench adds.
//! A reader deserialises the envelope's `data` into [`Status`] after checking the envelope's
//! `schema` is [`SCHEMA`]. The cockpit mod and the suite's status view read it.

mod sections;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub use sections::{
    Approved, Clef, ClefMode, Context, DecisionPoint, Fresh, FreshVerdict, Gate, HeadVerdict,
    Level, NextTask, Outcome, Phase, Plan, PlanStatus, Playbook, PointStatus, TailEvent, Tasks,
    WindowSource,
};

/// The output id the envelope carries (CLI-01, CLI-12).
pub const SCHEMA: &str = "bench.status/1";

/// Bench's status: where its store is and how it is doing (CLI-12), then its read model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Status {
    /// The install id, [`crate::plugin::NAME`].
    pub plugin: String,
    /// The binary's version.
    pub version: String,
    /// The SPEC version the binary conforms to.
    pub spec: String,
    /// The store's path; `None` when it cannot be located.
    pub store: Option<String>,
    /// The step that chose the store (ST-03); `None` when it cannot be located.
    pub store_source: Option<StoreSource>,
    /// The store's schema as `<major>.<minor>` (ST-06); `None` when there is no store yet, or
    /// it was refused or cannot be located.
    pub store_schema: Option<String>,
    /// The repository class (ST-13).
    pub repo_class: RepoClass,
    /// Bench events this build skipped, by type (EV-10).
    pub unreadable: BTreeMap<String, u64>,
    /// Telemetry events dropped on a busy lock (ST-09): always 0, since Bench records none.
    pub dropped: u64,
    /// `ok`, `degraded` or `off`, from the envelope's findings alone.
    pub health: Health,
    /// The worktree's bench mode and its playbook.
    pub phase: Option<Phase>,
    /// Build mode's plan and its approval.
    pub plan: Option<Plan>,
    /// Build mode's plan's tasks done and in all.
    pub tasks: Option<Tasks>,
    /// The newest gates that blocked, oldest first.
    pub gates: Option<Vec<Gate>>,
    /// The review verdict for `HEAD`.
    pub verdict: Option<HeadVerdict>,
    /// Clef's decision points.
    pub clef: Option<Clef>,
    /// The newest session's context usage.
    pub context: Option<Context>,
    /// The newest ledger events, oldest first.
    pub ledger_tail: Option<Vec<TailEvent>>,
}

/// The step of ST-03 that chose the store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoreSource {
    /// `--store`.
    Flag,
    /// `$REDKILN_STORE`, or the store beside the anneal ledger `$ANNEAL_STORE` names.
    Env,
    /// `store` in `.redkiln/suite.toml`.
    Config,
    /// `<git-common-dir>/redkiln/`.
    GitCommonDir,
    /// `<folder>/.redkiln/`, outside git.
    Folder,
}

/// The repository class (ST-13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepoClass {
    /// The owner's own repository.
    Own,
    /// A client's repository.
    Client,
}

/// How the plugin is doing: `off` with an error finding, `degraded` with a warning, else `ok`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Health {
    /// No error or warning.
    Ok,
    /// A warning: it works, with something to look at.
    Degraded,
    /// An error: it cannot do its work.
    Off,
}

#[cfg(test)]
mod tests;
