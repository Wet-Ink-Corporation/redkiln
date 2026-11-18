//! Deciding a request item (DR-05): the answer says how far it applies and cites the request.

use std::num::NonZeroUsize;

use anyhow::{Context as _, Result, bail};
use redkiln_core::envelope::{Actor, Author, Envelope, Timestamp, Writer};
use redkiln_core::journal::{ImportSource, Recording, encode_line, import};
use redkiln_core::store::{Appended, Filter, Store, Stored, Wait};
use redkiln_core::trust::Trust;
use weighin_contract::names::REQUEST_RAISED;
use weighin_contract::request::{Request, raise_condition, raise_event};
use weighin_core::event::{Applies, LedgerEvent};

use crate::env::Env;
use crate::ops::test_support::{env_in, grounded, run};
use crate::ops::{Session, stored_events};

const TS: &str = "2026-11-08T12:00:00Z";

/// A two-way request a sibling raises under `key`: each key is its own item (DR-03).
fn request(key: &str) -> Result<Request> {
    Ok(serde_json::from_value(serde_json::json!({
        "question": "Keep the retry at three attempts?",
        "options": [
            {"id": "a", "label": "Three", "holds_when": "Flakes are rare", "cost_if_wrong": "A red run"},
            {"id": "b", "label": "Five", "holds_when": "Flakes are common", "cost_if_wrong": "Slow failures"}
        ],
        "recommendation": "b",
        "door": "two-way",
        "authority": "judgment",
        "blocks": "run",
        "trust_needed": "witnessed",
        "raiser": {"namespace": "bench", "key": key}
    }))?)
}

/// Bench, as the sibling that raises and imports.
fn sibling() -> Result<Author> {
    Ok(Author {
        actor: Actor::parse("agent")?,
        trust: Trust::asserted(),
        writer: Writer::new("bench", "2.0.0")?,
    })
}

/// The one event `store` holds at `position`.
fn at(store: &Store, position: u64) -> Result<Stored> {
    let after = position.checked_sub(1).context("positions start at 1")?;
    store
        .read(&Filter::all(), after, NonZeroUsize::MIN)?
        .into_iter()
        .find(|stored| stored.position == position)
        .with_context(|| format!("no event at {position}"))
}

/// `request` appended to `store` as a sibling appends one (DR-01): Core directly, under
/// `raise_condition`. Returns the stored event.
fn raise_in(store: &Store, request: Request) -> Result<Stored> {
    let id = request.item_id()?;
    let event = raise_event(&Envelope::new(Timestamp::parse(TS)?, sibling()?, request))?;
    let Appended::At(position) = store.append(vec![event], Some(&raise_condition(&id)?))? else {
        bail!("{id} was raised already");
    };
    at(store, position)
}

/// `env`'s store, as Core opens it.
fn local(env: &Env) -> Result<Store> {
    Ok(Store::open(env.store_path()?, env.wait)?)
}

/// Each `item_decided` in `env`'s store: its `applies` and the refs its envelope cites.
fn decisions(env: &Env) -> Result<Vec<(Option<Applies>, Vec<String>)>> {
    Ok(stored_events(env)?
        .into_iter()
        .filter_map(|(_, e)| match e.event {
            LedgerEvent::ItemDecided(d) => Some((d.applies, e.cites)),
            _ => None,
        })
        .collect())
}

/// A `decide` of option `a` on `key`'s item.
fn decide_a(key: &str) -> Result<serde_json::Value> {
    let id = request(key)?.item_id()?;
    Ok(serde_json::json!([{"op": "decide", "id": id.as_str(), "option": "a"}]))
}

#[test]
#[cfg_attr(miri, ignore = "opens SQLite through FFI")]
fn deciding_a_request_item_writes_applies_once_and_cites_the_request() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let env = env_in(dir.path(), "s1");
    let raised = raise_in(&local(&env)?, request("retry/attempts")?)?;

    let out = run(
        &mut Session::open(&env, "cli")?,
        &decide_a("retry/attempts")?,
    )?;

    assert!(out.iter().all(|l| l.starts_with('✓')), "{out:?}");
    assert_eq!(
        decisions(&env)?,
        [(Some(Applies::Once), vec![raised.id.to_string()])]
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "opens SQLite through FFI")]
fn deciding_a_captured_item_writes_neither_applies_nor_cites() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let env = env_in(dir.path(), "s1");
    let mut batch = grounded();
    batch.push(serde_json::json!({"op": "decide", "id": "@1", "option": "A"}));

    let out = run(
        &mut Session::open(&env, "cli")?,
        &serde_json::Value::Array(batch),
    )?;

    assert!(out.iter().all(|l| l.starts_with('✓')), "{out:?}");
    assert_eq!(decisions(&env)?, [(None, Vec::new())]);
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "opens SQLite through FFI")]
fn an_answer_to_a_journal_imported_request_cites_its_origin_not_the_local_position() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let env = env_in(dir.path(), "s1");
    let origin = Store::open(&dir.path().join("origin/events.sqlite3"), Wait::Command)?;
    // Another request first, so the request's origin position is not its local one.
    raise_in(&origin, request("retry/backoff")?)?;
    let raised = raise_in(&origin, request("retry/attempts")?)?;
    let mut line = encode_line(&raised)?;
    line.push(b'\n');
    let store = local(&env)?;
    let recording = Recording {
        source: ImportSource::Path("origin.jsonl".to_owned()),
        ts: Timestamp::parse(TS)?,
        author: sibling()?,
    };
    let report = import(&store, line.as_slice(), recording)?;
    assert_eq!(report.imported, 1, "{report:?}");

    let out = run(
        &mut Session::open(&env, "cli")?,
        &decide_a("retry/attempts")?,
    )?;

    assert!(out.iter().all(|l| l.starts_with('✓')), "{out:?}");
    let imported = store
        .read(
            &Filter::all().of_type(REQUEST_RAISED.name)?,
            0,
            NonZeroUsize::MIN,
        )?
        .into_iter()
        .next()
        .context("the imported request")?;
    let decided = store
        .read_last(
            &Filter::all().of_type("weighin.item_decided")?,
            NonZeroUsize::MIN,
        )?
        .into_iter()
        .next()
        .context("the decision")?;
    let local_ref = format!("{}:{}", decided.id.store(), imported.position);
    let origin_ref = raised.id.to_string();
    assert_eq!(imported.id, raised.id, "the import keeps the origin");
    assert_ne!(
        origin_ref, local_ref,
        "the origin is not the local position"
    );
    assert_eq!(decisions(&env)?, [(Some(Applies::Once), vec![origin_ref])]);
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "opens SQLite through FFI")]
fn a_request_whose_ref_cannot_be_read_is_not_decided_and_nothing_is_written() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let env = env_in(dir.path(), "s1");
    raise_in(&local(&env)?, request("retry/attempts")?)?;
    let mut session = Session::open(&env, "cli")?;
    let id = request("retry/attempts")?.item_id()?;
    let record = session
        .ledger
        .items
        .get_mut(&id)
        .and_then(|item| item.request.as_mut())
        .context("the request item")?;
    record.position = 99;

    let out = run(&mut session, &decide_a("retry/attempts")?)?;

    assert_eq!(out, ["✗ no decision request at store position 99"]);
    assert_eq!(decisions(&env)?, []);
    Ok(())
}
