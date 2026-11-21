//! `weighin-contract`'s decision request against the SPEC's schema,
//! `spec/schemas/decision-request-1.schema.json`: what `Request::check` accepts the schema
//! accepts, and each per-field shape the schema refuses `check` refuses too. Here because weighin
//! already has `jsonschema` as a dev-dependency and the contract with its `core` feature. Also a
//! request a sibling appends through the contract, folded, listed and decided end to end (DR-01).

use crate::scratch::{Repo, TestResult};
use crate::support::{ask_user_question, store_of};
use redkiln_core::envelope::{Actor, Author, Envelope, Timestamp, Writer, decode};
use redkiln_core::store::{Appended, Filter, Store, Wait};
use redkiln_core::trust::{Level, Trust};
use serde::Deserialize as _;
use serde_json::{Value, json};
use std::num::NonZeroUsize;
use std::path::Path;
use weighin_contract::answer::{Answer, AnswerRefused, accepts};
use weighin_contract::event::{Applies, Decided};
use weighin_contract::names::REQUEST_RAISED;
use weighin_contract::request::{OptionId, Request, raise_condition, raise_event};

const TS: &str = "2026-11-08T12:00:00Z";

fn schema_errors(record: &Value) -> TestResult<Vec<String>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../spec/schemas/decision-request-1.schema.json");
    let schema: Value = serde_json::from_slice(&std::fs::read(path)?)?;
    let validator = jsonschema::draft202012::new(&schema)?;
    Ok(validator
        .iter_errors(record)
        .map(|e| format!("{}: {e}", e.instance_path().as_str()))
        .collect())
}

/// A request with every optional field set, which passes every guard.
fn full() -> Value {
    json!({
        "question": "Ship the bench baseline?",
        "options": [
            {"id": "a", "label": "Ship", "holds_when": "The run is stable", "cost_if_wrong": "A noisy gate"},
            {"id": "b", "label": "Rerun", "holds_when": "The run was noisy", "cost_if_wrong": "An hour"}
        ],
        "recommendation": "a",
        "door": "two-way",
        "authority": "judgment",
        "blast": "module",
        "default": "b",
        "deadline": "2026-11-09T12:00:00.000Z",
        "subject": {
            "kind": "bench.baseline",
            "method": "git-commit",
            "hash": format!("git-commit:{}", "a".repeat(40)),
            "base": "b".repeat(40),
            "locator": "benches/baseline.json"
        },
        "blocks": "run",
        "trust_needed": "witnessed",
        "raiser": {"namespace": "bench", "key": "baseline/main", "run": "r-1", "session": "s-1"},
        "attachments": [{"kind": "log", "locator": "target/bench.log", "hash": "sha256:0a1b"}]
    })
}

fn checked(record: &Value) -> TestResult<bool> {
    let request = Request::deserialize(record)?;
    Ok(request.check(&Timestamp::parse(TS)?).is_ok())
}

#[test]
fn a_request_check_accepts_passes_the_schema() -> TestResult {
    let record = full();
    assert!(checked(&record)?, "the full request passes every guard");
    assert_eq!(schema_errors(&record)?, Vec::<String>::new());
    Ok(())
}

#[test]
fn each_field_shape_the_schema_refuses_check_refuses() -> TestResult {
    let cases: [(&str, &str, Value); 9] = [
        ("attachments", "", json!([])),
        ("raiser", "run", json!("")),
        ("raiser", "session", json!("")),
        ("subject", "kind", json!("commit")),
        ("subject", "base", json!("abc")),
        ("subject", "locator", json!("")),
        ("subject", "locator", json!("a\u{1}b")),
        ("question", "", json!("")),
        ("attachments", "hash", json!("SHA256:0a")),
    ];
    for (field, inner, value) in cases {
        let mut record = full();
        let slot = match (field, inner) {
            (_, "") => record.get_mut(field),
            ("attachments", _) => record
                .get_mut(field)
                .and_then(|a| a.get_mut(0))
                .and_then(|a| a.get_mut(inner)),
            _ => record.get_mut(field).and_then(|f| f.get_mut(inner)),
        }
        .ok_or("the field is in the full request")?;
        *slot = value;
        assert!(
            !schema_errors(&record)?.is_empty(),
            "{field}.{inner}: the schema refuses it"
        );
        assert!(!checked(&record)?, "{field}.{inner}: check refuses it too");
    }
    Ok(())
}

/// A two-way request as Bench raises one (no default, so it waits on a human).
fn bench_request() -> TestResult<Request> {
    let mut record = full();
    let object = record.as_object_mut().ok_or("an object")?;
    for key in ["default", "deadline", "subject", "attachments"] {
        object.remove(key);
    }
    Ok(Request::deserialize(&record)?)
}

/// `request` appended to the store at `path` as a sibling appends it: Core directly, the event
/// `raise_event` builds, under `raise_condition` (DR-01, DR-03).
fn raise_as_a_sibling(path: &Path, request: Request) -> TestResult<String> {
    let id = request.item_id()?;
    let author = Author {
        actor: Actor::parse("agent")?,
        trust: Trust::asserted(),
        writer: Writer::new("bench", "2.0.0")?,
    };
    let event = raise_event(&Envelope::new(Timestamp::parse(TS)?, author, request))?;
    let store = Store::open(path, Wait::Command)?;
    let appended = store.append(vec![event], Some(&raise_condition(&id)?))?;
    assert!(matches!(appended, Appended::At(_)), "{appended:?}");
    Ok(id.as_str().to_owned())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_request_appended_through_the_contract_is_listed_by_open_json() -> TestResult {
    let repo = Repo::new(true)?;
    let store = store_of(&repo.root);
    let id = raise_as_a_sibling(&store, bench_request()?)?;

    let listed: Value =
        serde_json::from_str(&repo.ok("s1", &["open", "--all", "--json"], None)?)?;
    repo.ok("s1", &["decide", &id, "b"], None)?;

    let items = listed.as_array().ok_or("an array")?;
    assert_eq!(items.len(), 1, "{listed}");
    let item = items.first().ok_or("one item")?;
    assert_eq!(item["id"], json!(id));
    assert_eq!(item["channel"], json!("request"));
    assert_eq!(item["status"], json!("grounded"));
    let ids: Vec<&str> = item["card"]["options"]
        .as_array()
        .ok_or("options")?
        .iter()
        .filter_map(|o| o["id"].as_str())
        .collect();
    assert_eq!(ids, ["a", "b"]);
    let decided = Store::open(&store, Wait::Command)?.read(
        &Filter::all().of_type("weighin.item_decided")?,
        0,
        NonZeroUsize::MIN,
    )?;
    let decided = decode::<Decided>(decided.first().ok_or("the decision")?)?;
    let request = bench_request()?;
    // DR-06: a decision typed at weighin's command line is asserted, which no request accepts.
    // The same decision from a witnessed surface is accepted.
    assert_eq!(
        accepts(
            &request,
            &Answer::of(&decided.data, decided.trust.level()),
            None
        ),
        Err(AnswerRefused::Trust {
            needed: Level::Witnessed,
            found: Level::Asserted,
        })
    );
    assert_eq!(
        accepts(&request, &Answer::of(&decided.data, Level::Witnessed), None),
        Ok(OptionId::B)
    );
    // DR-05: the answer to a request says how far it applies, and cites the request it answers.
    let raised = Store::open(&store, Wait::Command)?.read(
        &Filter::all().of_type(REQUEST_RAISED.name)?,
        0,
        NonZeroUsize::MIN,
    )?;
    let raised = raised.first().ok_or("the request")?;
    assert_eq!(decided.data.applies, Some(Applies::Once));
    assert_eq!(decided.cites.as_slice(), std::slice::from_ref(&raised.id));
    Ok(())
}

/// DR-06 end to end (#125): a request a sibling raised through the contract, presented as a
/// sitting presents it and answered by the person in the `AskUserQuestion` dialog, is decided
/// `witnessed` by the `PostToolUse` hook, and its raiser accepts the answer.
#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_request_answered_in_ask_user_question_is_accepted_by_its_raiser() -> TestResult {
    let repo = Repo::new(true)?;
    let store = store_of(&repo.root);
    let request = bench_request()?;
    let id = raise_as_a_sibling(&store, bench_request()?)?;
    let text = format!("{id} {}", request.question);
    let payload = ask_user_question(
        &repo.root,
        "s1",
        "toolu_01EvEUdQAy1Q9Wx4BiceFect",
        (&text, &["a: Ship", "b: Rerun"], "b: Rerun"),
    )?;

    let out = repo.hook(&payload)?;

    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let decided = Store::open(&store, Wait::Command)?.read(
        &Filter::all().of_type("weighin.item_decided")?,
        0,
        NonZeroUsize::MIN,
    )?;
    let decided = decode::<Decided>(decided.first().ok_or("the decision")?)?;
    assert_eq!(decided.trust.level(), Level::Witnessed);
    assert_eq!(
        accepts(
            &request,
            &Answer::of(&decided.data, decided.trust.level()),
            None
        ),
        Ok(OptionId::B)
    );
    let raised = Store::open(&store, Wait::Command)?.read(
        &Filter::all().of_type(REQUEST_RAISED.name)?,
        0,
        NonZeroUsize::MIN,
    )?;
    let raised = raised.first().ok_or("the request")?;
    assert_eq!(decided.data.applies, Some(Applies::Once));
    assert_eq!(decided.cites.as_slice(), std::slice::from_ref(&raised.id));
    Ok(())
}
