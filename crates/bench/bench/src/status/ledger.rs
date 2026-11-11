//! The sections `bench status` reads from the ledger: `phase`, `plan` and `tasks` of the
//! worktree, and the ledger-wide `gates` and `ledger_tail`. Each reads through the module that
//! owns the data and builds its `bench_contract::status` section; a failure is the
//! [`Unreadable`] the caller turns into a finding.

use std::path::Path;

use crate::store::{EventKind, Filter, LedgerEvent, Recorded, Store};
use bench_contract::event::{GateBlocked, Mode, StopBlocked};
use bench_contract::status::{
    Approved, Gate, NextTask, Phase, Plan, PlanStatus, Playbook, TailEvent, Tasks,
};

use super::Unreadable;
use crate::hook::completeness::playbook::RULES;
use crate::hook::handoff::facts::{self, MAX_TITLE_CHARS};
use crate::lint;
use crate::mode::{self, Active, BuildMode, Entered, scope};
use crate::plan::{self, Approval, ApprovalRecord, Status};

/// How many of the newest blocking events `gates` lists (H-13).
pub(super) const GATES_SHOWN: usize = 10;

/// The worktree the status is about: the ledger and the root git printed.
#[derive(Debug, Clone, Copy)]
pub(super) struct Worktree<'a> {
    /// The ledger.
    pub(super) store: &'a Store,
    /// The worktree's top-level directory, as git prints it.
    pub(super) root: &'a str,
}

/// What build mode names: the plan, relative to the worktree root, and the position build
/// mode was entered at (tasks count only when done after it).
#[derive(Debug, Clone, Copy)]
pub(super) struct BuildPlan<'a> {
    plan: &'a str,
    since: u64,
}

impl<'a> BuildPlan<'a> {
    /// The plan of `entered` when it is build mode with a plan.
    pub(super) fn of(entered: &'a Entered) -> Option<Self> {
        match &entered.mode {
            Active::Build(BuildMode { plan, .. }) => Some(Self {
                plan,
                since: entered.position,
            }),
            Active::IncompleteBuild | Active::Diagnose => None,
        }
    }
}

/// `phase` of `entered`, which it consumes: the mode, and its playbook from the Stop hook's
/// rules (`hook::completeness::playbook::RULES`).
pub(super) fn phase(entered: Entered) -> Phase {
    let (mode, plan, tree, clean) = match entered.mode {
        Active::Build(BuildMode { plan, tree, clean }) => {
            (Mode::Build, Some(plan), Some(tree), Some(clean))
        }
        Active::IncompleteBuild => (Mode::Build, None, None, None),
        Active::Diagnose => (Mode::Diagnose, None, None, None),
    };
    let requires = RULES
        .iter()
        .filter(|rule| rule.mode == mode)
        .map(|rule| rule.event.as_str().to_owned())
        .collect();
    Phase {
        mode,
        playbook: Playbook {
            name: facts::mode_name(mode).to_owned(),
            requires,
        },
        event: entered.position,
        plan,
        tree,
        clean,
    }
}

/// The plan's approval and its bytes, read once for `plan` and `tasks`.
#[derive(Debug)]
pub(super) struct PlanRead {
    /// The `plan` section.
    pub(super) approval: Plan,
    /// The plan's bytes.
    pub(super) bytes: Vec<u8>,
}

impl Worktree<'_> {
    /// The worktree's active mode (`mode::active`).
    pub(super) fn mode(&self) -> Result<Option<Entered>, Unreadable> {
        Ok(mode::active(self.store, self.root)?)
    }

    /// The build plan's bytes and its approval (`plan::approval_record`).
    pub(super) fn plan(&self, build: &BuildPlan<'_>) -> Result<PlanRead, Unreadable> {
        let path = Path::new(self.root).join(build.plan);
        let bytes = lint::read_whole_bytes(&path).map_err(|source| Unreadable::Plan {
            // The error outlives this borrow of build mode's record, so it owns the path.
            plan: build.plan.to_owned(),
            source,
        })?;
        let Approval {
            status,
            hash,
            latest,
        } = plan::approval_record(self.store, build.plan, &bytes)?;
        let status = match status {
            Status::Current => PlanStatus::Approved,
            Status::Stale => PlanStatus::Stale,
            Status::NotApproved => PlanStatus::NotApproved,
        };
        let approved = latest.map(|ApprovalRecord { hash, position, ts }| Approved {
            hash,
            event: position,
            ts,
        });
        let approval = Plan {
            id: build.plan.to_owned(),
            status,
            hash,
            approved,
        };
        Ok(PlanRead { approval, bytes })
    }

    /// The build plan's tasks in `bytes`, counted against the `task_done` events of this
    /// worktree since build mode was entered (`hook::handoff::facts`).
    pub(super) fn tasks(&self, build: &BuildPlan<'_>, bytes: &[u8]) -> Result<Tasks, Unreadable> {
        let tasks = facts::plan_tasks(build.plan, bytes);
        let key = scope::worktree_id(self.root);
        let done = facts::done_since(self.store, build.since, Some(&key))
            .map_err(Unreadable::DoneTasks)?;
        let finished = facts::finished(&tasks, &done);
        let total = tasks.len();
        let next = facts::next_task(&tasks, &done).map(|(index, _)| index);
        // The plan's tasks are this function's own: the next one moves into the section.
        let next = next
            .and_then(|index| tasks.into_iter().nth(index))
            .map(|task| NextTask {
                title: task.title.chars().take(MAX_TITLE_CHARS).collect(),
                id: task.id,
            });
        Ok(Tasks {
            done: finished,
            total,
            next,
        })
    }
}

/// `gates`: the newest [`GATES_SHOWN`] blocking events, oldest first: `gate_blocked` (a guard
/// or precondition denied a tool call) and `stop_blocked` (the Stop hook refused a stop).
pub(super) fn gates(store: &Store) -> Result<Vec<Gate>, Unreadable> {
    let filter = Filter {
        types: vec![EventKind::GateBlocked, EventKind::StopBlocked],
        tags: Vec::new(),
    };
    let recorded = store.tail(&filter, GATES_SHOWN)?;
    Ok(recorded.into_iter().filter_map(gate).collect())
}

/// The gate a blocking event records; `None` for any other event.
fn gate(recorded: Recorded) -> Option<Gate> {
    let kind = recorded.event.event.kind().as_str();
    let (LedgerEvent::GateBlocked(GateBlocked { point, reason })
    | LedgerEvent::StopBlocked(StopBlocked { point, reason })) = recorded.event.event
    else {
        return None;
    };
    Some(Gate {
        id: point,
        kind: kind.to_owned(),
        reason,
        event: recorded.position,
        ts: recorded.event.ts.as_str().to_owned(),
    })
}

/// `ledger_tail`: the newest `n` events, oldest first.
pub(super) fn tail(store: &Store, n: usize) -> Result<Vec<TailEvent>, Unreadable> {
    let recorded = store.tail(&Filter::default(), n)?;
    Ok(recorded
        .into_iter()
        .map(|recorded| TailEvent {
            event: recorded.position,
            kind: recorded.event.event.kind().as_str().to_owned(),
            summary: facts::event_word(&recorded),
            ts: recorded.event.ts.as_str().to_owned(),
            actor: recorded.event.actor,
        })
        .collect())
}
