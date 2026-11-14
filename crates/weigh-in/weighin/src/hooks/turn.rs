//! The turn hooks and the edit hook: `Stop` and `SubagentStop` capture the turn's `weigh-in`
//! blocks (Stop then exports the journals), `PostToolUse` scans what a tool wrote and recalls
//! the decisions anchored at that file, and Cowork's nudge.

use anyhow::Result;
use weighin_core::Ledger;
use weighin_core::event::Channel;
use weighin_core::marker::{Door, parse_markers};

use super::{Failures, Hook, HookOutput, Tool, ToolWrite, Turn, late, scan_text};
use crate::env::Env;
use crate::journal::refs::{self, Checkpointed, Marked, Scope};
use crate::ops::{self, RaiseInput, RaiseOutcome, Session};
use crate::recall;

impl Hook {
    /// `PostToolUse`: scan what the tool wrote, then, when the tool edited a file, read the
    /// ledger once, as it stands after the scan, for the decisions anchored at that file (#116),
    /// and give them as context. The scan's result stands whatever follows: the recall raises
    /// nothing, and each failure is logged once (and noted, [`Hook::note`]). Nothing is exported
    /// here (D11): the session's Stop exports it.
    pub(super) fn post_tool_use(
        &self,
        failures: &mut Failures,
        write: Option<&ToolWrite>,
    ) -> HookOutput {
        let env = &self.env;
        if let Err(e) = scan(env, write) {
            self.log(failures, &e);
        }
        let Some(edit) = write.and_then(|w| recall_edit(env, w)) else {
            return HookOutput::default();
        };
        let events = match ops::stored_events(env) {
            Ok(events) => events,
            Err(e) => {
                self.log(failures, &e.context("reading the ledger after the edit"));
                return HookOutput::default();
            }
        };
        let ledger = Ledger::fold(events.iter().map(|(p, e)| (*p, e)));
        HookOutput {
            // `None` is no decision anchored at the file, not a failure: nothing to add.
            context: recall::edit_context(env, &ledger, &edit),
            message: None,
        }
    }

    /// `result`, after exporting the store to the journal refs inside the Stop budget, whatever
    /// `result` is: a capture that failed part way may have raised items first, and capture
    /// runs first so this turn's items leave with it (JR-08). An export failure, and each
    /// outcome a person must act on (`needs_a_person`, D12), is logged; journals left because the
    /// budget ran out are recorded in `hook-late.log` for the next `SessionStart` to tell
    /// (wi-6ab291e7fb), not logged as errors. The result is unchanged, and the export says
    /// nothing. The session's own journal goes first, then only the journals with an event after
    /// the watermark, so the Stop's work grows with what is new, not with the store's history.
    pub(super) fn checkpointed(
        &self,
        failures: &mut Failures,
        result: Result<Option<String>>,
    ) -> Result<Option<String>> {
        let budget = self.budget;
        match refs::checkpoint(&self.env, budget, self.started, Scope::SinceWatermark) {
            Ok(Checkpointed::Ran(report, marked)) => {
                for line in refs::needs_a_person(&report) {
                    self.log(failures, &anyhow::anyhow!("{line}"));
                }
                let late = refs::late_journals(&report);
                if late > 0
                    && let Err(e) = late::record(
                        &self.env,
                        self.label(),
                        &format!("{late} journal(s) left for the next export"),
                    )
                {
                    self.log(failures, &e.context("recording the late journal export"));
                }
                if let Marked::Failed(e) = marked {
                    self.log(failures, &e);
                }
            }
            // Not git, a client repository, opted out or no store: nothing to say.
            Ok(Checkpointed::Skipped(_)) => {}
            Err(e) => self.log(failures, &e),
        }
        let took = self.started.elapsed();
        if let Some(worst) = budget.worst_case()
            && took > worst
        {
            // The budget's bound assumes Core's call pattern: running past it means that grew.
            self.log(
                failures,
                &anyhow::anyhow!(
                    "the Stop hook ran {took:?} to the end of its journal export, past its \
                     budget's worst case of {worst:?}"
                ),
            );
        }
        result
    }
}

/// Cowork: no ledger here to write, so nudge straight from the markers in the last message.
pub(super) fn cowork_nudge(text: &str) -> Option<String> {
    let one_way = parse_markers(text)
        .iter()
        .filter(|m| m.door == Door::OneWay)
        .count();
    (one_way > 0).then(|| {
        format!(
            "Weigh-In: {one_way} one-way question(s) in that message. Run /weigh-in while the context is fresh."
        )
    })
}

/// Raise the markers in `turn`'s message; the message for the person when this session gains a
/// one-way question.
pub(super) fn capture(env: &Env, turn: &Turn) -> Result<Option<String>> {
    let markers = parse_markers(&turn.message);
    if markers.is_empty() {
        return Ok(None);
    }
    let mut s = Session::open(env, "hook")?;
    let one_way_before = one_way_here(&s, env);
    let mut raised = 0;
    for m in &markers {
        let mut input = RaiseInput::from_marker(m, Channel::Marker);
        if let Some(agent) = &turn.agent {
            input.quote = Some(format!("[subagent {agent}]\n{}", m.raw));
        }
        if matches!(s.raise(&input)?, RaiseOutcome::New(_)) {
            raised += 1;
        }
    }
    let one_way_after = one_way_here(&s, env);
    // One user-visible nudge when this session gains a one-way question; never an automatic sitting.
    if raised > 0 && one_way_after > one_way_before {
        return Ok(Some(format!(
            "Weigh-In: {one_way_after} one-way question(s) from this session are waiting. Run /weigh-in while the context is fresh."
        )));
    }
    Ok(None)
}

fn one_way_here(s: &Session<'_>, env: &Env) -> usize {
    env.session.as_deref().map_or(0, |id| {
        s.ledger
            .open_in_session(id)
            .iter()
            .filter(|i| i.door() == Door::OneWay)
            .count()
    })
}

/// Raise what a `PostToolUse` tool wrote. It says nothing.
fn scan(env: &Env, write: Option<&ToolWrite>) -> Result<()> {
    let Some(write) = write else {
        return Ok(());
    };
    if write.texts.is_empty() {
        return Ok(());
    }
    // Borrowed from the input: a large Write is not copied to be scanned.
    let written: Vec<&str> = write.texts.iter().map(String::as_str).collect();
    scan_text(env, &write.file, &written, "hook")?;
    Ok(())
}

#[cfg(test)]
mod tests;

/// A `Write`, `Edit` or `MultiEdit` to a file in this repository, for `recall`; `None` for any
/// other tool or file. The line a `Write` touched is not known, so it carries no text.
fn recall_edit<'w>(env: &Env, write: &'w ToolWrite) -> Option<recall::Edit<'w>> {
    let edited = match write.tool {
        Tool::Edit | Tool::MultiEdit => write.texts.iter().map(String::as_str).collect(),
        Tool::Write => Vec::new(),
        Tool::Other => return None,
    };
    recall::Edit::of(env, &write.file, edited)
}
