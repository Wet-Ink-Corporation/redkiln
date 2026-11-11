//! `bench ledger`: append to, query, tail and journal Bench's events in the suite store.
//!
//! The store is [`crate::store`] over Core; this module parses the command line, resolves the
//! store ([`paths::ledger_store`]) and writes the result in the 0.x flat form, short type names
//! and `registry` included, until step 4. A bad event exits 1 with an envelope and nothing is
//! written; reads create no store, and skip and count a stored event this build cannot use
//! (EV-10). `journal` prints JR-04 journal lines (D7). A `by` on
//! `plan_approved` or `verdict_set` is a usage error (2, nothing on stdout, nothing written):
//! `plan approve` and `verdict set` write it.

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::store::{
    EventEnvelope, EventKind, Filter, JournalError, LedgerEvent, LedgerTag, REGISTRY_VERSION, Read,
    Recorded, Store, StoreError, TagError, TagKey, Timestamp, Wait,
};
use bench_contract::event::{PlanApproved, VerdictSet};
use clap::{Args, Subcommand};
use serde::Serialize;
use serde_json::{Map, Value};

use crate::command::CommandName;
use crate::envelope::{Envelope, Finding};
use crate::error::CliError;
use crate::error_chain;
use crate::exit_codes::{self, Code};
use crate::legacy_import;
use crate::paths::{self, StoreLocation, StoreSource};

/// `bench ledger`.
#[derive(Debug, Args)]
pub struct LedgerArgs {
    /// Use this ledger file instead of the resolved one
    #[arg(long, global = true, value_name = "FILE")]
    store: Option<PathBuf>,
    #[command(subcommand)]
    action: LedgerAction,
}

/// The `bench ledger` subcommands.
#[derive(Debug, Subcommand)]
pub enum LedgerAction {
    /// Append one event, given as a JSON object with a registered `type`
    Append(AppendArgs),
    /// Print the events matching the filters, oldest first
    Query(FilterArgs),
    /// Print the last events matching the filters, oldest first
    Tail(TailArgs),
    /// Print every bench event as one journal line (the importable form)
    Journal,
}

/// `bench ledger append`.
#[derive(Debug, Args)]
pub struct AppendArgs {
    #[arg(
        value_name = "EVENT_JSON",
        help = r#"The event, e.g. '{"type":"task_done","task":"T1"}'"#
    )]
    event: String,
    /// The writer, when the event names none
    #[arg(long, default_value = "cli")]
    actor: String,
    /// The session id, when the event names none
    #[arg(long)]
    session: Option<String>,
}

/// Filters shared by `query` and `tail`: any of the types, all of the tags.
#[derive(Debug, Args)]
pub struct FilterArgs {
    /// Only events of this type (repeatable: any of them)
    #[arg(long = "type", value_name = "TYPE")]
    types: Vec<EventKind>,
    /// Only events carrying this key:value tag (repeatable: all of them)
    #[arg(long = "tag", value_name = "KEY:VALUE")]
    tags: Vec<LedgerTag>,
    /// Only events of this session (the tag session:ID)
    #[arg(long, value_name = "ID", value_parser = session_tag)]
    session: Option<LedgerTag>,
}

/// `bench ledger tail`.
#[derive(Debug, Args)]
pub struct TailArgs {
    /// How many events
    #[arg(short = 'n', long = "lines", value_name = "N", default_value_t = 10)]
    lines: usize,
    #[command(flatten)]
    filter: FilterArgs,
}

fn session_tag(id: &str) -> Result<LedgerTag, TagError> {
    LedgerTag::new(TagKey::Session, id)
}

impl FilterArgs {
    fn to_filter(&self) -> Filter {
        // The filter owns its tags; the parsed arguments stay with clap's tree.
        let tags = self.tags.iter().chain(&self.session).cloned().collect();
        Filter {
            types: self.types.clone(),
            tags,
        }
    }
}

/// Runs `bench ledger <action>` in `cwd`, reading environment variables through `lookup`.
pub(crate) fn run<O: Write>(
    args: &LedgerArgs,
    cwd: &Path,
    lookup: impl Fn(&str) -> Option<OsString>,
    out: &mut O,
) -> Result<Code, CliError> {
    let locate = || paths::ledger_store(args.store.as_deref(), &lookup, cwd);
    match &args.action {
        LedgerAction::Append(append_args) => append(append_args, locate, out),
        LedgerAction::Query(filter) => {
            let filter = filter.to_filter();
            read(&locate()?, "query", |store| store.query_read(&filter), out)
        }
        LedgerAction::Tail(tail) => {
            let filter = tail.filter.to_filter();
            read(
                &locate()?,
                "tail",
                |store| store.tail_read(&filter, tail.lines),
                out,
            )
        }
        LedgerAction::Journal => journal(&locate()?, out),
    }
}

#[derive(Serialize)]
struct Appended<'a> {
    store: &'a Path,
    source: StoreSource,
    position: u64,
    event: &'a EventEnvelope,
}

#[derive(Serialize)]
struct Listing<'a> {
    store: &'a Path,
    source: StoreSource,
    registry: u32,
    events: &'a [Recorded],
}

fn append<O: Write>(
    args: &AppendArgs,
    locate: impl FnOnce() -> Result<StoreLocation, paths::PathError>,
    out: &mut O,
) -> Result<Code, CliError> {
    const SUBCOMMAND: &str = "append";
    let event = match parse_event(args)? {
        Ok(event) => event,
        Err(schema) => return schema_error(SUBCOMMAND, &schema, out),
    };
    refuse_a_claimed_giver(&event.event)?;
    let location = locate()?;
    // Core's own checks (the actor, a `wt-` worktree, the branch and head, EV-05) are the
    // schema's too: nothing is written.
    let position = match legacy_import::open_for_command(&location)?.append(&event) {
        Ok(position) => position,
        Err(refused) if refused.is_schema() => return schema_error(SUBCOMMAND, &refused, out),
        Err(other) => return Err(other.into()),
    };
    let data = Appended {
        store: &location.path,
        source: location.source,
        position,
        event: &event,
    };
    emit(
        &Envelope::new(CommandName::Ledger, Some(SUBCOMMAND), exit_codes::OK, data),
        out,
    )
}

/// The event in `args`, with `actor`, `session_id` and `ts` filled in where it leaves them
/// out. The outer `Result` is a failure to run (the clock); the inner one a schema error.
fn parse_event(
    args: &AppendArgs,
) -> Result<Result<EventEnvelope, crate::store::SchemaError>, CliError> {
    let mut value: Value = match serde_json::from_str(&args.event) {
        Ok(value) => value,
        Err(error) => return Ok(Err(crate::store::SchemaError::Json(error))),
    };
    if let Value::Object(fields) = &mut value {
        fill(fields, "actor", || Ok(Value::from(args.actor.as_str())))?;
        if let Some(session) = &args.session {
            fill(fields, "session_id", || Ok(Value::from(session.as_str())))?;
        }
        fill(fields, "ts", || Ok(Value::from(Timestamp::now()?.as_str())))?;
    }
    Ok(EventEnvelope::from_value(&value))
}

/// Refuses a `by` on `plan_approved` or `verdict_set` (#62): who gave an approval or a verdict
/// is recorded only by their writers, `plan approve` and `verdict set`. For `verdict_set` that
/// is where the guard's `human-only` rule sees the actor, so a raw append cannot claim a human
/// verdict; `plan approve` is allowed with any actor (wi-720468), so for `plan_approved` the
/// refusal only keeps one writer of `by`.
fn refuse_a_claimed_giver(event: &LedgerEvent) -> Result<(), CliError> {
    let (kind, writer) = match event {
        LedgerEvent::PlanApproved(PlanApproved { by: Some(_), .. }) => {
            (EventKind::PlanApproved, "plan approve")
        }
        LedgerEvent::VerdictSet(VerdictSet { by: Some(_), .. }) => {
            (EventKind::VerdictSet, "verdict set")
        }
        _ => return Ok(()),
    };
    Err(CliError::Usage {
        reason: format!(
            "ledger append does not write a `by` on {kind}: use `bench {writer} --actor/--via`"
        ),
    })
}

/// Sets `key` to `make()` when it is absent or `null`.
fn fill(
    fields: &mut Map<String, Value>,
    key: &str,
    make: impl FnOnce() -> Result<Value, CliError>,
) -> Result<(), CliError> {
    if fields.get(key).is_none_or(Value::is_null) {
        fields.insert(key.to_owned(), make()?);
    }
    Ok(())
}

/// Runs `select` on the store at `location` and writes what it found. A stored Bench event
/// this build cannot use is skipped and counted (EV-10): a `ledger/unusable` finding says how
/// many. With no store, nothing is created and nothing found.
fn read<O: Write>(
    location: &StoreLocation,
    subcommand: &str,
    select: impl FnOnce(&Store) -> Result<Read, StoreError>,
    out: &mut O,
) -> Result<Code, CliError> {
    let read = match Store::open_existing(&location.path, Wait::Command)? {
        Some(store) => select(&store)?,
        None => Read::default(),
    };
    let data = Listing {
        store: &location.path,
        source: location.source,
        registry: REGISTRY_VERSION,
        events: &read.events,
    };
    let findings: Vec<Finding> = (read.unusable > 0)
        .then(|| {
            Finding::new(
                "ledger/unusable",
                format!(
                    "{} stored bench event(s) could not be read by this build (a newer version, \
                     or data it cannot decode) and were skipped",
                    read.unusable
                ),
            )
        })
        .into_iter()
        .collect();
    let envelope = Envelope::new(CommandName::Ledger, Some(subcommand), exit_codes::OK, data);
    emit(&envelope.with_findings(&findings), out)
}

/// Every Bench event as its journal line (JR-04, D7), in position order: the importable form.
/// Not wrapped in the result envelope: the output is the journal file itself. With no store,
/// nothing is created and nothing printed.
fn journal<O: Write>(location: &StoreLocation, out: &mut O) -> Result<Code, CliError> {
    let Some(store) = Store::open_existing(&location.path, Wait::Command)? else {
        return Ok(exit_codes::OK);
    };
    // Each line already ends in its `\n` (JR-01).
    let written = store.journal(|line| out.write_all(line));
    match written {
        Ok(()) => Ok(exit_codes::OK),
        Err(JournalError::Store(error)) => Err(error.into()),
        Err(JournalError::Write(error)) => Err(CliError::Output(error)),
        Err(line @ JournalError::Line { .. }) => Err(CliError::Journal(line)),
    }
}

/// The envelope for an event that does not match the schema: exit 1, one finding.
fn schema_error<O: Write>(
    subcommand: &str,
    error: &dyn std::error::Error,
    out: &mut O,
) -> Result<Code, CliError> {
    let findings = [Finding::new("ledger/schema", error_chain::reason(error))];
    let types: Vec<&str> = EventKind::ALL.iter().map(|kind| kind.as_str()).collect();
    let data = serde_json::json!({"registry": REGISTRY_VERSION, "types": types});
    let envelope = Envelope::new(
        CommandName::Ledger,
        Some(subcommand),
        exit_codes::LEDGER_SCHEMA_ERROR,
        data,
    );
    emit(&envelope.with_findings(&findings), out)
}

fn emit<D: Serialize, O: Write>(envelope: &Envelope<'_, D>, out: &mut O) -> Result<Code, CliError> {
    envelope.emit(out).map_err(CliError::Output)
}

#[cfg(test)]
mod tests {
    use super::{AppendArgs, parse_event};

    fn args(event: &str, session: Option<&str>) -> AppendArgs {
        AppendArgs {
            event: event.to_owned(),
            actor: "cli".to_owned(),
            session: session.map(str::to_owned),
        }
    }

    #[test]
    fn append_fills_actor_session_and_ts_only_where_the_event_leaves_them_out() {
        let given = args(
            r#"{"type":"task_done","task":"T1","actor":"hook","session_id":null}"#,
            Some("s9"),
        );

        let event = parse_event(&given)
            .expect("the clock works")
            .expect("a valid event");

        assert_eq!(event.actor, "hook", "the event's own actor wins");
        assert_eq!(event.session_id.as_deref(), Some("s9"), "null is filled");
        assert!(event.ts.as_str().ends_with('Z'), "ts filled with now");
    }
}
