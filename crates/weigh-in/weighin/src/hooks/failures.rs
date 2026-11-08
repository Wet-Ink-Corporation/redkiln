//! A hook's failures and the note it adds to its output (CLI-09, decision D2 B).

use std::path::PathBuf;

use super::EventKind;
use crate::text::clip;

/// Where a hook's failure went.
#[derive(Debug)]
pub(crate) enum Logged {
    /// Appended to the log at `path`; `fresh` when the log held nothing unreported before.
    At { path: PathBuf, fresh: bool },
    /// Not logged, and why.
    Not(String),
    /// In Cowork, where hooks keep no log.
    Cowork,
}

/// One failure of a hook: its one line, and where it went.
#[derive(Debug)]
struct Failure {
    line: String,
    logged: Logged,
}

impl Failure {
    /// Whether the failure is told only by the hook's note: it is in no log a report reads.
    fn unlogged(&self) -> bool {
        matches!(self.logged, Logged::Not(_) | Logged::Cowork)
    }
}

/// When a hook's failures are noted in its output, as decision D2 B reads CLI-09.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Rule {
    /// When the log held nothing unreported before the first, when one could not be logged, or
    /// in Cowork: a failing session says so once, and the next `SessionStart` counts the rest.
    /// `Stop`, `SubagentStop`, `PostToolUse`, and a `SessionStart` that dispatched nothing.
    Fresh,
    /// Only those that could not be logged: a dispatched `SessionStart`, whose report reads the
    /// others back.
    Unlogged,
    /// Never: any other event (weighin is registered for none), and a `SessionStart` that
    /// cannot locate its store, which it tells already.
    Never,
}

/// How far a hook got: whether it dispatched its event (a payload over its cap, or not read,
/// names none), and whether its store was located.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reach {
    Undispatched,
    Unlocated,
    Located,
}

impl Rule {
    /// The rule for a hook of `kind` that got as far as `reach`.
    pub(crate) fn of(kind: EventKind, reach: Reach) -> Self {
        match (kind, reach) {
            // An undispatched SessionStart ran nothing, so no report reads its failure back.
            (EventKind::Turn | EventKind::PostToolUse, _)
            | (EventKind::SessionStart, Reach::Undispatched) => Self::Fresh,
            (EventKind::SessionStart, Reach::Located) => Self::Unlogged,
            (EventKind::SessionStart, Reach::Unlocated) | (EventKind::Other, _) => Self::Never,
        }
    }
}

/// The failures one hook had, in order.
#[derive(Debug, Default)]
pub(crate) struct Failures(Vec<Failure>);

impl Failures {
    /// Records `e`, which went where `logged` says, as one line of at most 200 characters.
    pub(crate) fn push(&mut self, e: &anyhow::Error, logged: Logged) {
        self.0.push(Failure {
            line: one_line(e, 200),
            logged,
        });
    }

    /// The note for the person of the hook labelled `label`, by `rule` ([`Rule`]).
    pub(crate) fn note(&self, label: &str, rule: Rule) -> Option<String> {
        let noted: Vec<&Failure> = match rule {
            Rule::Fresh => {
                let due = self
                    .0
                    .iter()
                    .any(|f| f.unlogged() || matches!(f.logged, Logged::At { fresh: true, .. }));
                if due {
                    self.0.iter().collect()
                } else {
                    Vec::new()
                }
            }
            Rule::Unlogged => self.0.iter().filter(|f| f.unlogged()).collect(),
            Rule::Never => Vec::new(),
        };
        let (first, rest) = noted.split_first()?;
        let more = match rest.len() {
            0 => String::new(),
            n => format!(" (and {n} more)"),
        };
        let place = match &first.logged {
            Logged::At { path, .. } => format!("Logged in {}.", path.display()),
            Logged::Not(why) => format!("It could not be logged: {}.", why.trim_end_matches('.')),
            Logged::Cowork => "Hooks keep no log in Cowork.".to_owned(),
        };
        Some(format!(
            "Weigh-In: the {label} hook failed, so capture may be off: {}{more}. {place}",
            first.line.trim_end_matches('.')
        ))
    }
}

/// `e` and its causes on one line, at most `n` characters.
pub(crate) fn one_line(e: &anyhow::Error, n: usize) -> String {
    clip(&format!("{e:#}").replace(['\r', '\n'], " "), n)
}

#[cfg(test)]
mod tests {
    use super::{Failures, Logged, Rule};
    use std::path::PathBuf;

    fn at(fresh: bool) -> Logged {
        Logged::At {
            path: PathBuf::from("/r/.git/redkiln/weighin/hook-errors.log"),
            fresh,
        }
    }

    #[test]
    fn the_rule_follows_the_event_kind_and_whether_it_dispatched() {
        use super::Reach::{Located, Undispatched, Unlocated};
        use crate::hooks::EventKind::{Other, PostToolUse, SessionStart, Turn};
        let rules = [
            ((Turn, Located), Rule::Fresh),
            ((Turn, Undispatched), Rule::Fresh),
            ((PostToolUse, Unlocated), Rule::Fresh),
            ((SessionStart, Located), Rule::Unlogged),
            ((SessionStart, Unlocated), Rule::Never),
            ((SessionStart, Undispatched), Rule::Fresh),
            ((Other, Located), Rule::Never),
            ((Other, Undispatched), Rule::Never),
        ];
        for ((kind, reach), rule) in rules {
            assert_eq!(Rule::of(kind, reach), rule, "{kind:?} {reach:?}");
        }
    }

    #[test]
    fn a_turn_notes_its_first_failure_and_counts_the_rest() {
        let mut failures = Failures::default();
        failures.push(&anyhow::anyhow!("opening the store."), at(true));
        failures.push(&anyhow::anyhow!("a second\nline"), at(false));

        let note = failures.note("Stop", Rule::Fresh);

        assert_eq!(
            note.as_deref(),
            Some(
                "Weigh-In: the Stop hook failed, so capture may be off: opening the store (and 1 \
                 more). Logged in /r/.git/redkiln/weighin/hook-errors.log."
            )
        );
    }

    #[test]
    fn a_turn_whose_log_held_unreported_lines_notes_nothing() {
        let mut failures = Failures::default();
        failures.push(&anyhow::anyhow!("opening the store"), at(false));

        assert_eq!(failures.note("PostToolUse", Rule::Fresh), None);
    }

    #[test]
    fn a_failure_that_could_not_be_logged_is_always_noted() {
        let mut failures = Failures::default();
        failures.push(&anyhow::anyhow!("first"), at(false));
        failures.push(
            &anyhow::anyhow!("second"),
            Logged::Not("no log.".to_owned()),
        );

        let note = failures.note("SubagentStop", Rule::Fresh);

        assert_eq!(
            note.as_deref(),
            Some(
                "Weigh-In: the SubagentStop hook failed, so capture may be off: first (and 1 \
                 more). Logged in /r/.git/redkiln/weighin/hook-errors.log."
            )
        );
    }

    #[test]
    fn a_session_start_notes_only_what_it_could_not_log() {
        let mut failures = Failures::default();
        failures.push(&anyhow::anyhow!("logged"), at(true));
        failures.push(&anyhow::anyhow!("lost"), Logged::Not("no log".to_owned()));

        let located = failures.note("SessionStart", Rule::Unlogged);
        let unlocated = failures.note("SessionStart", Rule::Never);

        assert_eq!(
            located.as_deref(),
            Some(
                "Weigh-In: the SessionStart hook failed, so capture may be off: lost. It could \
                 not be logged: no log."
            )
        );
        assert_eq!(unlocated, None);
    }

    #[test]
    fn cowork_failures_are_noted_and_never_means_never() {
        let mut failures = Failures::default();
        failures.push(&anyhow::anyhow!("parsing"), Logged::Cowork);

        assert_eq!(
            failures.note("Stop", Rule::Fresh).as_deref(),
            Some(
                "Weigh-In: the Stop hook failed, so capture may be off: parsing. Hooks keep no \
                 log in Cowork."
            )
        );
        assert_eq!(failures.note("Notification", Rule::Never), None);
    }
}
