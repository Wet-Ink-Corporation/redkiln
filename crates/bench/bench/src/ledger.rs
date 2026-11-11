//! `bench ledger`: append to, query, tail and journal Bench's events in the suite store.
//!
//! The store is [`crate::store`] over Core; this module parses the command line, resolves the
//! store ([`paths::ledger_store`]) and reads and writes Core's EV-01 envelopes (`type`
//! `bench.<name>`, the payload under `data`; since Move 2 step 4, PR 4.3). `append` takes the
//! envelope's writable keys and fills `actor`, `session` and `ts` when they are left out; Bench
//! writes `version`, `trust` and `writer`. A bad event exits 1 with an envelope and nothing is
//! written; reads create no store, and skip and count a stored event this build cannot use
//! (EV-10). `--type` takes the stored name, and a 0.x short name with a `bench: warning:` line.
//! `journal` prints JR-04 journal lines (D7). A `by` on `plan_approved` or `verdict_set` is a
//! usage error (2, nothing on stdout, nothing written): `plan approve` and `verdict set` write
//! it.

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::store::{
    EventEnvelope, EventKind, Filter, JournalError, LedgerEvent, LedgerTag, Read, Store,
    StoreError, StoredEvent, TagError, TagKey, Timestamp, Wait,
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
    /// Append one event, given as an EV-01 envelope with a `bench.*` `type` and its `data`
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
        help = r#"The event, e.g. '{"type":"bench.task_done","data":{"task":"T1"}}'"#
    )]
    event: String,
    /// The actor, when the event names none
    #[arg(long, default_value = "cli")]
    actor: String,
    /// The session id, when the event names none
    #[arg(long)]
    session: Option<String>,
}

/// Filters shared by `query` and `tail`: any of the types, all of the tags.
#[derive(Debug, Args)]
pub struct FilterArgs {
    #[arg(
        long = "type",
        value_name = "TYPE",
        value_parser = type_name,
        help = "Only events of this type, e.g. bench.task_done (repeatable: any of them)"
    )]
    types: Vec<TypeName>,
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

/// A `--type` value: the stored name (`bench.task_done`), or the 0.x short name (`task_done`),
/// still read for scripts written before step 4 and warned about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeName {
    kind: EventKind,
    short: bool,
}

/// A `--type` value that names none of Bench's 17 types.
#[derive(Debug, thiserror::Error)]
#[error("unknown event type {name:?}: a bench.* type, such as bench.task_done")]
pub struct UnknownType {
    name: String,
}

fn type_name(name: &str) -> Result<TypeName, UnknownType> {
    if let Some(kind) = EventKind::from_stored(name) {
        return Ok(TypeName { kind, short: false });
    }
    name.parse::<EventKind>()
        .map(|kind| TypeName { kind, short: true })
        .map_err(|_short_unknown_too| UnknownType {
            name: name.to_owned(),
        })
}

impl FilterArgs {
    /// The store's filter, after one `bench: warning:` line on `err` for each 0.x short name.
    fn to_filter<E: Write>(&self, err: &mut E) -> Result<Filter, CliError> {
        for name in self.types.iter().filter(|name| name.short) {
            crate::diagnostic::warning(
                &mut *err,
                &format_args!(
                    "--type {} is the 0.x short name; write --type {}",
                    name.kind.as_str(),
                    name.kind.stored()
                ),
            )
            .map_err(CliError::Output)?;
        }
        // The filter owns its tags; the parsed arguments stay with clap's tree.
        let tags = self.tags.iter().chain(&self.session).cloned().collect();
        Ok(Filter {
            types: self.types.iter().map(|name| name.kind).collect(),
            tags,
        })
    }
}

/// Where `bench ledger` runs: the working directory, and how it reads environment variables.
pub(crate) struct Place<'a, L> {
    /// The working directory.
    pub(crate) cwd: &'a Path,
    /// Reads one environment variable.
    pub(crate) lookup: L,
}

/// Runs `bench ledger <action>` at `place`. Warnings (a 0.x `--type` name) go to `err`.
pub(crate) fn run<L, O, E>(
    args: &LedgerArgs,
    place: &Place<'_, L>,
    out: &mut O,
    err: &mut E,
) -> Result<Code, CliError>
where
    L: Fn(&str) -> Option<OsString>,
    O: Write,
    E: Write,
{
    let locate = || paths::ledger_store(args.store.as_deref(), &place.lookup, place.cwd);
    match &args.action {
        LedgerAction::Append(append_args) => append(append_args, locate, out),
        LedgerAction::Query(filter) => {
            let filter = filter.to_filter(err)?;
            read(
                &locate()?,
                "query",
                |store| store.query_stored(&filter),
                out,
            )
        }
        LedgerAction::Tail(tail) => {
            let filter = tail.filter.to_filter(err)?;
            read(
                &locate()?,
                "tail",
                |store| store.tail_stored(&filter, tail.lines),
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
    event: &'a crate::store::CoreEnvelope,
}

#[derive(Serialize)]
struct Listing<'a> {
    store: &'a Path,
    source: StoreSource,
    events: &'a [StoredEvent],
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
    let stored = match legacy_import::open_for_command(&location)?.append_stored(&event) {
        Ok(stored) => stored,
        Err(refused) if refused.is_schema() => return schema_error(SUBCOMMAND, &refused, out),
        Err(other) => return Err(other.into()),
    };
    let data = Appended {
        store: &location.path,
        source: location.source,
        position: stored.position,
        event: &stored.event,
    };
    emit(
        &Envelope::new(CommandName::Ledger, Some(SUBCOMMAND), exit_codes::OK, data),
        out,
    )
}

/// The EV-01 envelope in `args`, with `actor`, `session` and `ts` filled in where it leaves
/// them out or `null`. The outer `Result` is a failure to run (the clock); the inner one a
/// schema error.
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
            fill(fields, "session", || Ok(Value::from(session.as_str())))?;
        }
        fill(fields, "ts", || Ok(Value::from(Timestamp::now()?.as_str())))?;
    }
    Ok(EventEnvelope::from_ev01(&value))
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
    select: impl FnOnce(&Store) -> Result<Read<StoredEvent>, StoreError>,
    out: &mut O,
) -> Result<Code, CliError> {
    let read = match Store::open_existing(&location.path, Wait::Command)? {
        Some(store) => select(&store)?,
        None => Read::default(),
    };
    let data = Listing {
        store: &location.path,
        source: location.source,
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

/// The envelope for an event that does not match the schema: exit 1, one finding, and the
/// types `append` takes in `data.types`.
fn schema_error<O: Write>(
    subcommand: &str,
    error: &dyn std::error::Error,
    out: &mut O,
) -> Result<Code, CliError> {
    let findings = [Finding::new("ledger/schema", error_chain::reason(error))];
    let types: Vec<&str> = EventKind::ALL.iter().map(|kind| kind.stored()).collect();
    let data = serde_json::json!({ "types": types });
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
    use super::{AppendArgs, FilterArgs, parse_event, type_name};
    use crate::store::EventKind;

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
            r#"{"type":"bench.task_done","data":{"task":"T1"},"actor":"hook","session":null}"#,
            Some("s9"),
        );

        let event = parse_event(&given)
            .expect("the clock works")
            .expect("a valid event");

        assert_eq!(event.actor, "hook", "the event's own actor wins");
        assert_eq!(event.session_id.as_deref(), Some("s9"), "null is filled");
        assert!(event.ts.as_str().ends_with('Z'), "ts filled with now");
    }

    #[test]
    fn type_takes_the_stored_name_and_the_short_one_with_a_warning() {
        let filter = |names: &[&str]| FilterArgs {
            types: names
                .iter()
                .map(|name| type_name(name).expect("a Bench type"))
                .collect(),
            tags: Vec::new(),
            session: None,
        };
        let (mut stored_err, mut short_err) = (Vec::new(), Vec::new());

        let stored = filter(&["bench.task_done", "bench.migrate.contract"])
            .to_filter(&mut stored_err)
            .expect("a Vec takes the warnings");
        let short = filter(&["task_done"])
            .to_filter(&mut short_err)
            .expect("a Vec takes the warnings");

        assert_eq!(
            stored.types,
            [EventKind::TaskDone, EventKind::MigrateContract],
            "stored names"
        );
        assert!(stored_err.is_empty(), "no warning for a stored name");
        assert_eq!(short.types, [EventKind::TaskDone], "the short name reads");
        assert_eq!(
            String::from_utf8(short_err).expect("UTF-8"),
            "bench: warning: --type task_done is the 0.x short name; write --type \
             bench.task_done\n",
            "one CLI-08 warning naming the stored name"
        );
        assert!(type_name("bench.task_started").is_err(), "not a Bench type");
        assert!(
            type_name("weighin.item_raised").is_err(),
            "another plugin's"
        );
    }
}
