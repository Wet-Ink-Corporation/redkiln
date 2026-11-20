//! `orchestrate park` and `orchestrate rule`: a one-way call raised as a Weigh-In decision
//! request, or a call ruled on and logged as a two-way one.
//!
//! Every argument is checked before anything runs or a ledger is opened: the ticket as
//! `bench round` takes it, each text one non-blank line within its cap, 2 to 4 options with a
//! `--holds` and a `--cost` each, `--recommend` one of them, `--resolve-by` a `YYYY-MM-DD` date
//! after today (UTC), `--decision` a decision id and `--session` a ledger tag.
//!
//! A park then asks the `weighin` it finds whether it folds requests into the store Bench
//! writes (decision D3), before that store is opened. Only then does it open the store and
//! append the request through weighin-contract (DR-01), once per ticket and question (DR-03);
//! otherwise it creates and writes nothing and exits 4, and the orchestrator rules instead.
//!
//! A ruling asks nobody (T11): it appends a two-way request defaulted to what was ruled, with
//! no deadline, which Weigh-In folds as already defaulted (call F, D4 B), once per ticket and
//! ruling; with no `weighin` found, a finding says it waits in the store unseen.

use std::ffi::OsString;
use std::io::Write;

use crate::store::{EventEnvelope, LedgerEvent, Timestamp};
use bench_contract::event::TaskDone;
use clap::{ArgAction, Args};
use serde::Serialize;
use weighin_contract::request::{OptionId, RequestError};

use crate::brief::ticket::Ticket;
use crate::command::CommandName;
use crate::decide::id::DecisionId;
use crate::envelope::{Envelope, Finding};
use crate::error::{CliError, RequestCommand};
use crate::error_chain;
use crate::exit_codes::{self, Code};
use crate::hook::injection_scan::detect;
use crate::legacy_import;
use crate::paths;
use crate::weighin::{self, RaiseError, Raised, Runner};

use super::Ports;

mod request;

use request::{Park, ParkOption, Ruling};

/// Fewest options a park offers (DR-04).
const MIN_OPTIONS: usize = 2;

/// Most options a park offers (DR-04).
const MAX_OPTIONS: usize = 4;

/// Most characters of a question.
const MAX_QUESTION_CHARS: usize = 1024;

/// Most characters of every other text.
const MAX_TEXT_CHARS: usize = 512;

/// `bench orchestrate park`.
#[derive(Debug, Args)]
pub struct ParkArgs {
    /// The ticket the call blocks: an issue number or a dag ticket id
    #[arg(long, value_name = "ID")]
    ticket: String,
    /// The question for the human, one line
    #[arg(long = "q", value_name = "QUESTION")]
    question: String,
    /// An option, 2 to 4 times: ids a, b, c, d in order
    #[arg(long = "option", value_name = "LABEL", action = ArgAction::Append)]
    options: Vec<String>,
    /// When each option holds, once per --option, in order
    #[arg(long, value_name = "WHEN", action = ArgAction::Append)]
    holds: Vec<String>,
    /// What each option costs if it is wrong, once per --option, in order
    #[arg(long, value_name = "IF-WRONG", action = ArgAction::Append)]
    cost: Vec<String>,
    /// The option you recommend: a, b, c or d
    #[arg(long, value_name = "ID", default_value = "a")]
    recommend: String,
    /// Where the call arises, as path:line
    #[arg(long, value_name = "PATH:LINE")]
    at: String,
    /// The last day to decide, YYYY-MM-DD (UTC); a one-way call is never defaulted
    #[arg(long, value_name = "DATE")]
    resolve_by: String,
    /// The Clef D2 decision id that said park, from its `data.id`
    #[arg(long, value_name = "ID")]
    decision: Option<String>,
    /// The session id to record
    #[arg(long, value_name = "ID")]
    session: Option<String>,
}

/// `bench orchestrate rule`.
#[derive(Debug, Args)]
pub struct RuleArgs {
    /// The ticket the call was made in
    #[arg(long, value_name = "ID")]
    ticket: String,
    /// What was ruled
    #[arg(long, value_name = "TEXT")]
    what: String,
    /// Why
    #[arg(long, value_name = "TEXT")]
    why: String,
    /// What undoing it costs if the ruling is wrong
    #[arg(long, value_name = "TEXT")]
    cost: String,
    /// The Clef D2 decision id behind the ruling, from its `data.id`
    #[arg(long, value_name = "ID")]
    decision: Option<String>,
    /// The session id to record
    #[arg(long, value_name = "ID")]
    session: Option<String>,
}

/// `data` of `orchestrate park`.
#[derive(Debug, Serialize)]
struct ParkData<'a> {
    ticket: &'a str,
    parked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    item_id: Option<&'a str>,
    /// False when the request of this ticket and question was already in the store.
    #[serde(skip_serializing_if = "Option::is_none")]
    recorded: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'a str>,
    /// Where the request sits: written now, or earlier; absent when nothing was written.
    #[serde(skip_serializing_if = "Option::is_none")]
    position: Option<u64>,
}

/// `data` of `orchestrate rule` (`bench.orchestrate-rule/2`).
#[derive(Debug, Serialize)]
struct RuleData<'a> {
    ticket: &'a str,
    item_id: &'a str,
    /// False when the request of this ticket and ruling was already in the store.
    recorded: bool,
    /// Where the request sits: written now, or earlier.
    position: u64,
}

/// `orchestrate park`: see the module documentation.
pub(super) fn park<O, L, R>(
    args: &ParkArgs,
    ports: &Ports<'_, L, R>,
    out: &mut O,
) -> Result<Code, CliError>
where
    O: Write,
    L: Fn(&str) -> Option<OsString>,
    R: Runner,
{
    let ticket = args.ticket.parse::<Ticket>()?;
    let park = Park {
        ticket: &ticket,
        question: one_line("q", &args.question, MAX_QUESTION_CHARS)?,
        options: park_options(args)?,
        recommend: recommendation(&args.recommend)?,
        at: one_line("at", &args.at, MAX_TEXT_CHARS)?,
        resolve_by: date("resolve-by", &args.resolve_by)?,
        decision: decision_id(args.decision.as_deref())?,
        session: args.session.as_deref(),
    };
    let probe = probe(&ticket, park.session)?;
    after_today(park.resolve_by, &probe.ts)?;
    let request = park.request();
    request
        .check(&probe.ts.to_core())
        .map_err(|error| refused(RequestCommand::Park, &error))?;
    let location = paths::ledger_store(None, &ports.lookup, ports.cwd)?;
    // Weigh-In is asked before the store is opened, so the exit-4 path creates no store and
    // runs no import (D3).
    let able = weighin::locate(&ports.lookup)
        .map_err(weighin::Unable::from)
        .and_then(|found| weighin::able(ports.runner, &found.path, ports.cwd, &location.path));
    if let Err(unable) = able {
        return unavailable(&ticket, &error_chain::reason(&unable), out);
    }
    let store = legacy_import::open_for_command(&location)?;
    let envelope = store.envelope_for(&probe, request)?;
    let (item, position, recorded) =
        raised(RequestCommand::Park, weighin::raise(&store, &envelope))?;
    let data = ParkData {
        ticket: ticket.as_str(),
        parked: true,
        item_id: Some(item.as_str()),
        recorded: Some(recorded),
        reason: None,
        position: Some(position),
    };
    let code = exit_codes::ORCHESTRATE_OK;
    emit(
        &Envelope::new(CommandName::Orchestrate, Some("park"), code, data),
        out,
    )
}

/// The item, position and whether it was written now, of `command`'s raise
/// `outcome`; a refusal is a usage error, all three id lengths taken
/// [`CliError::RequestIdTaken`].
fn raised(
    command: RequestCommand,
    outcome: Result<Raised, RaiseError>,
) -> Result<(weighin_contract::ItemId, u64, bool), CliError> {
    match outcome {
        Ok(Raised::New { item, position }) => Ok((item, position, true)),
        Ok(Raised::Already { item, position }) => Ok((item, position, false)),
        Err(RaiseError::Refused(error)) => Err(refused(command, &error)),
        Err(RaiseError::IdTaken { item }) => Err(CliError::RequestIdTaken {
            command,
            item: item.as_str().to_owned(),
        }),
        Err(RaiseError::Store(error)) => Err(error.into()),
    }
}

/// The exit-4 envelope: weigh-in cannot take the call for `reason`, and nothing was written.
fn unavailable<O: Write>(ticket: &Ticket, reason: &str, out: &mut O) -> Result<Code, CliError> {
    let findings = [Finding::new(
        format!("orchestrate/weigh-in-unavailable:{ticket}"),
        format!("weigh-in cannot take the call ({reason}): rule on it instead"),
    )];
    let data = ParkData {
        ticket: ticket.as_str(),
        parked: false,
        item_id: None,
        recorded: None,
        reason: Some(reason),
        position: None,
    };
    let code = exit_codes::ORCHESTRATE_UNAVAILABLE;
    emit(
        &Envelope::new(CommandName::Orchestrate, Some("park"), code, data).with_findings(&findings),
        out,
    )
}

/// The park's options, `--option`, `--holds` and `--cost` zipped in order; unequal counts, or
/// fewer than 2 or more than 4, are a usage error naming the counts.
fn park_options(args: &ParkArgs) -> Result<Vec<ParkOption<'_>>, CliError> {
    let (n, holds, cost) = (args.options.len(), args.holds.len(), args.cost.len());
    if n != holds || n != cost || !(MIN_OPTIONS..=MAX_OPTIONS).contains(&n) {
        return Err(CliError::Usage {
            reason: format!(
                "a park takes {MIN_OPTIONS} to {MAX_OPTIONS} options, each with one --holds and \
                 one --cost: got {n} --option, {holds} --holds and {cost} --cost"
            ),
        });
    }
    args.options
        .iter()
        .zip(&args.holds)
        .zip(&args.cost)
        .map(|((label, holds_when), cost_if_wrong)| {
            Ok(ParkOption {
                label: one_line("option", label, MAX_TEXT_CHARS)?,
                holds_when: one_line("holds", holds_when, MAX_TEXT_CHARS)?,
                cost_if_wrong: one_line("cost", cost_if_wrong, MAX_TEXT_CHARS)?,
            })
        })
        .collect()
}

/// A `command`'s request weigh-in's contract refused (a DR-04 guard: a
/// deadline already past, a recommendation that is no option), as a usage error.
fn refused(command: RequestCommand, error: &RequestError) -> CliError {
    CliError::Usage {
        reason: format!(
            "weigh-in's contract refused the {command}: {}",
            error_chain::reason(error)
        ),
    }
}

/// The `--recommend` option id, `a` to `d`; any other word is a usage error.
fn recommendation(value: &str) -> Result<OptionId, CliError> {
    OptionId::ALL
        .into_iter()
        .find(|id| id.as_str() == value)
        .ok_or_else(|| CliError::Usage {
            reason: format!("--recommend must be a, b, c or d, not {value:?}"),
        })
}

/// `orchestrate rule`: raises the ruling as a two-way Weigh-In request, defaulted to what was
/// ruled and with no deadline (call F, D4 B), once per ticket and ruling (DR-03).
///
/// A ruling is a record, not an ask, so Weigh-In is not asked first (T11): the request waits in
/// the store for a `weighin` that folds it, and with none found a finding says so.
pub(super) fn rule<O, L, R>(
    args: &RuleArgs,
    ports: &Ports<'_, L, R>,
    out: &mut O,
) -> Result<Code, CliError>
where
    O: Write,
    L: Fn(&str) -> Option<OsString>,
{
    let ticket = args.ticket.parse::<Ticket>()?;
    let ruling = Ruling {
        ticket: &ticket,
        what: one_line("what", &args.what, MAX_TEXT_CHARS)?,
        why: one_line("why", &args.why, MAX_TEXT_CHARS)?,
        cost: one_line("cost", &args.cost, MAX_TEXT_CHARS)?,
        decision: decision_id(args.decision.as_deref())?,
        session: args.session.as_deref(),
    };
    let probe = probe(&ticket, ruling.session)?;
    let request = ruling.request();
    request
        .check(&probe.ts.to_core())
        .map_err(|error| refused(RequestCommand::Rule, &error))?;
    let location = paths::ledger_store(None, &ports.lookup, ports.cwd)?;
    let findings: Vec<Finding> = weighin::locate(&ports.lookup)
        .err()
        .map(|absent| absent_finding(&ticket, &absent))
        .into_iter()
        .collect();
    let store = legacy_import::open_for_command(&location)?;
    let envelope = store.envelope_for(&probe, request)?;
    let (item, position, recorded) =
        raised(RequestCommand::Rule, weighin::raise(&store, &envelope))?;
    let data = RuleData {
        ticket: ticket.as_str(),
        item_id: item.as_str(),
        recorded,
        position,
    };
    let code = exit_codes::ORCHESTRATE_OK;
    emit(
        &Envelope::new(CommandName::Orchestrate, Some("rule"), code, data).with_findings(&findings),
        out,
    )
}

/// The finding of a ruling recorded while no `weighin` is found, for `reason`.
fn absent_finding(ticket: &Ticket, reason: &weighin::BridgeError) -> Finding {
    Finding::new(
        format!("orchestrate/weigh-in-absent:{ticket}"),
        format!(
            "recorded for Weigh-In, which is not installed to show it ({})",
            error_chain::reason(reason)
        ),
    )
}

/// Writes `envelope`.
fn emit<O: Write, D: Serialize>(envelope: &Envelope<'_, D>, out: &mut O) -> Result<Code, CliError> {
    envelope.emit(out).map_err(CliError::Output)
}

/// A Bench event of `ticket` made now by the CLI for `session`, its tags checked, and never
/// written: the context a request is written in (the same ts, actor and session as a Bench
/// event), and a `--session` the ledger cannot tag refused as a usage error. Any kind whose
/// subject is the `task:` ticket serves; `task_done` is the one with nothing else to fill.
fn probe(ticket: &Ticket, session: Option<&str>) -> Result<EventEnvelope, CliError> {
    let event = LedgerEvent::TaskDone(TaskDone {
        task: ticket.as_str().to_owned(),
    });
    let mut envelope = EventEnvelope::new(event, "cli", Timestamp::now()?);
    envelope.session_id = session.map(str::to_owned);
    envelope.tags().map_err(|error| CliError::Usage {
        reason: format!("the --session is recorded as a ledger tag: {error}"),
    })?;
    Ok(envelope)
}

/// `value` trimmed, when it is one non-blank line of at most `max` characters with no control,
/// default-ignorable, format, line or paragraph separator character (Cc, and what
/// `injection_scan::detect::is_unprintable` names: a bidi override or a U+2028 would hide or
/// break the line on weigh-in's board); else a usage error naming `--flag`.
fn one_line<'a>(flag: &str, value: &'a str, max: usize) -> Result<&'a str, CliError> {
    let text = value.trim();
    let hidden = |c: char| c.is_control() || detect::is_unprintable(c);
    if text.is_empty() || text.chars().any(hidden) || text.chars().count() > max {
        return Err(CliError::Usage {
            reason: format!("--{flag} must be one non-blank line of at most {max} characters"),
        });
    }
    Ok(text)
}

/// `value` when it is a `YYYY-MM-DD` date that exists (month 01 to 12, day 01 to the month's
/// length, February 29 only in a leap year); else a usage error naming `--flag`.
fn date<'a>(flag: &str, value: &'a str) -> Result<&'a str, CliError> {
    let number = |part: &str, digits: usize| {
        (part.len() == digits && part.bytes().all(|b| b.is_ascii_digit()))
            .then(|| part.parse::<u32>().ok())
            .flatten()
    };
    let valid = match value.split('-').collect::<Vec<_>>().as_slice() {
        [year, month, day] => match (number(year, 4), number(month, 2), number(day, 2)) {
            (Some(year), Some(month), Some(day)) => {
                days_in_month(year, month).is_some_and(|days| (1..=days).contains(&day))
            }
            _ => false,
        },
        _ => false,
    };
    if valid {
        Ok(value)
    } else {
        Err(CliError::Usage {
            reason: format!("--{flag} must be a date, YYYY-MM-DD"),
        })
    }
}

/// Refuses a `--resolve-by` `day` (checked `YYYY-MM-DD`) that is not after the UTC day of
/// `now`: a one-way request due today leaves the human no sitting to answer it. Both are
/// zero-padded, four-digit-year dates, so they compare as text.
fn after_today(day: &str, now: &Timestamp) -> Result<(), CliError> {
    match now.as_str().get(..10) {
        Some(today) if day > today => Ok(()),
        Some(today) => Err(CliError::Usage {
            reason: format!(
                "--resolve-by must be after today ({today}, UTC): the deadline is the end of that day"
            ),
        }),
        None => Err(CliError::Usage {
            reason: format!("the clock reads no date ({})", now.as_str()),
        }),
    }
}

/// The days of `month` (1 to 12) in `year` of the Gregorian calendar; `None` for no month.
const fn days_in_month(year: u32, month: u32) -> Option<u32> {
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => Some(31),
        4 | 6 | 9 | 11 => Some(30),
        2 if leap => Some(29),
        2 => Some(28),
        _ => None,
    }
}

/// The `--decision` id, when given; a malformed one is a usage error.
fn decision_id(value: Option<&str>) -> Result<Option<DecisionId>, CliError> {
    value
        .map(|id| {
            id.parse::<DecisionId>().map_err(|error| CliError::Usage {
                reason: format!("--decision: {error}"),
            })
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::{date, one_line};
    use crate::error::CliError;

    #[test]
    fn a_date_is_four_two_and_two_digits_in_range() {
        for good in ["2026-10-19", "2026-01-01", "2026-12-31"] {
            assert_eq!(date("resolve-by", good).ok(), Some(good), "{good}");
        }
        for bad in [
            "2026-13-01",
            "2026-00-10",
            "2026-10-32",
            "26-10-19",
            "2026-1-19",
            "+026-10-19",
            "",
        ] {
            assert!(
                matches!(date("resolve-by", bad), Err(CliError::Usage { reason })
                    if reason == "--resolve-by must be a date, YYYY-MM-DD"),
                "{bad}"
            );
        }
    }

    #[test]
    fn a_text_is_trimmed_and_capped_in_characters() {
        let refused = |value: &str, max| {
            matches!(one_line("q", value, max), Err(CliError::Usage { reason })
                if reason == format!("--q must be one non-blank line of at most {max} characters"))
        };

        assert_eq!(one_line("q", "  Shared?  ", 8).ok(), Some("Shared?"));
        assert_eq!(one_line("q", &"é".repeat(8), 8).ok(), Some("éééééééé"));
        assert!(refused(&"é".repeat(9), 8), "nine characters");
        assert!(refused("tab\there", 64), "a control character");
    }

    #[test]
    fn a_day_past_the_end_of_its_month_is_refused_with_leap_years_counted() {
        for good in [
            "2024-02-29",
            "2000-02-29",
            "2026-02-28",
            "2026-04-30",
            "2026-01-31",
        ] {
            assert_eq!(date("resolve-by", good).ok(), Some(good), "{good}");
        }
        for bad in [
            "2026-02-29",
            "2100-02-29",
            "2026-02-31",
            "2026-04-31",
            "2026-06-31",
            "2026-09-31",
            "2026-11-31",
        ] {
            assert!(
                matches!(date("resolve-by", bad), Err(CliError::Usage { reason })
                    if reason == "--resolve-by must be a date, YYYY-MM-DD"),
                "{bad}"
            );
        }
    }

    #[test]
    fn invisible_format_and_separator_characters_are_refused_like_control_characters() {
        for character in [
            '\u{2028}', '\u{2029}', '\u{202E}', '\u{200B}', '\u{FEFF}', '\u{00AD}',
        ] {
            let text = format!("Shared{character}cache?");

            let result = one_line("q", &text, 64);

            assert!(
                matches!(&result, Err(CliError::Usage { reason })
                    if reason == "--q must be one non-blank line of at most 64 characters"),
                "U+{:04X}: {result:?}",
                u32::from(character)
            );
        }
    }
}
