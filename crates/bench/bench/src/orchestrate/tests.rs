//! `bench orchestrate` through `cli`'s argument parsing, a ledger of its own and a fake
//! `weighin` behind the [`Runner`] port.

use std::cell::RefCell;
use std::ffi::OsString;
use std::fs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use crate::store::{Filter, LedgerEvent, Store};
use clap::Parser;
use redkiln_core::envelope::decode;
use redkiln_core::store::Filter as CoreFilter;
use serde_json::{Value, json};
use tempfile::TempDir;
use weighin_contract::request::{Blocks, OptionId, Request, TrustNeeded};
use weighin_contract::vocab::{Authority, Door};

use super::{OrchestrateArgs, Ports, run};
use crate::cli::{Cli, Command};
use crate::error::CliError;
use crate::exit_codes::Code;
use crate::weighin::{Call, Ran, RunError, Runner};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// `weighin status --json` of a weighin that folds requests (`requests`) into `store`.
fn status(store: &Path, requests: bool) -> String {
    let mut data = json!({
        "plugin": "rk-weigh-in", "version": "2.0.0", "spec": "1.0.0",
        "store": store.to_string_lossy(), "store_source": "env", "store_schema": "1.0",
        "repo_class": "own", "unreadable": {}, "dropped": 0, "health": "ok",
        "hook_errors": 0, "hooks": {},
    });
    if requests {
        data["requests"] = json!(true);
    }
    json!({"schema": "weighin.status/1", "ok": true, "command": "status", "data": data,
           "findings": []})
    .to_string()
}

/// A fake `weighin`: one fixed answer, and the arguments of each call.
struct Fake {
    answer: Ran,
    calls: RefCell<Vec<Vec<String>>>,
}

impl Fake {
    fn answering(code: i32, stdout: &str) -> Self {
        Self {
            answer: Ran {
                code: Some(code),
                stdout: stdout.as_bytes().to_vec(),
                stderr: b"weighin: something failed\n".to_vec(),
            },
            calls: RefCell::new(Vec::new()),
        }
    }

    fn calls(&self) -> usize {
        self.calls.borrow().len()
    }
}

impl Runner for Fake {
    fn run(&self, call: &Call<'_>) -> Result<Ran, RunError> {
        self.calls
            .borrow_mut()
            .push(call.args.iter().map(|arg| (*arg).to_owned()).collect());
        // Each call hands out its own copy of the fixed answer.
        Ok(self.answer.clone())
    }
}

/// A working directory with a ledger file and a `weighin` file to point `WEIGHIN_BIN` at.
struct World {
    dir: TempDir,
}

impl World {
    fn new() -> TestResult<Self> {
        let dir = tempfile::tempdir()?;
        fs::write(dir.path().join("weighin"), "")?;
        Ok(Self { dir })
    }

    fn cwd(&self) -> &Path {
        self.dir.path()
    }

    fn store(&self) -> PathBuf {
        self.dir.path().join("ledger.sqlite3")
    }

    /// A fake `weighin` that folds requests into this world's store.
    fn able(&self) -> Fake {
        Fake::answering(0, &status(&self.store(), true))
    }

    /// Runs `bench orchestrate <words>` with `WEIGHIN_BIN` set to `bin` (none: unset, and no
    /// `PATH`), returning the exit code and the envelope.
    fn run(
        &self,
        words: &[&str],
        bin: Option<&Path>,
        runner: &impl Runner,
    ) -> (Result<u8, CliError>, Value) {
        let store = self.store();
        let lookup = |name: &str| -> Option<OsString> {
            match name {
                "REDKILN_STORE" => Some(store.clone().into_os_string()),
                "WEIGHIN_BIN" => bin.map(|bin| bin.as_os_str().to_owned()),
                _ => None,
            }
        };
        let mut out = Vec::new();
        let ports = Ports {
            cwd: self.cwd(),
            lookup,
            runner,
        };
        let code = run(&args(words), &ports, &mut out).map(Code::value);
        let envelope = if out.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&out).expect("one JSON envelope")
        };
        (code, envelope)
    }

    fn bin(&self) -> PathBuf {
        self.dir.path().join("weighin")
    }

    /// The Bench events in the ledger; none without a ledger file.
    fn events(&self) -> TestResult<Vec<LedgerEvent>> {
        if !self.store().exists() {
            return Ok(Vec::new());
        }
        let store = Store::open(&self.store())?;
        Ok(store
            .query(&Filter::default())?
            .into_iter()
            .map(|recorded| recorded.event.event)
            .collect())
    }

    /// The decision requests in the store, with their positions; none without a store file.
    fn requests(&self) -> TestResult<Vec<(u64, Request)>> {
        if !self.store().exists() {
            return Ok(Vec::new());
        }
        let store = Store::open(&self.store())?;
        let filter = CoreFilter::all().of_type("weighin.request_raised")?;
        let max = NonZeroUsize::new(64).ok_or("64 is not zero")?;
        Ok(store
            .read_foreign(&filter, max)?
            .iter()
            .map(|stored| {
                let read = decode::<Request>(stored).expect("a usable request");
                (stored.position, read.data)
            })
            .collect())
    }
}

fn args(words: &[&str]) -> OrchestrateArgs {
    let parsed = Cli::try_parse_from(["anneal", "orchestrate"].iter().chain(words))
        .expect("the arguments parse");
    match parsed.command {
        Some(Command::Orchestrate(args)) => args,
        other => panic!("not orchestrate: {other:?}"),
    }
}

/// A park of "should the cache be shared", blocking ticket 7, with two options.
const PARK: [&str; 21] = [
    "park",
    "--ticket",
    "7",
    "--q",
    "Should the cache be shared across tenants?",
    "--option",
    "Per tenant",
    "--holds",
    "a tenant needs isolation",
    "--cost",
    "more memory",
    "--option",
    "Shared",
    "--holds",
    "tenants trust each other",
    "--cost",
    "a migration to split it",
    "--at",
    "src/cache.rs:12",
    "--resolve-by",
    "2099-10-19",
];

#[test]
fn a_park_raises_one_one_way_request_with_its_options_and_deadline() -> TestResult {
    let world = World::new()?;
    let fake = world.able();

    let (code, envelope) = world.run(&PARK, Some(&world.bin()), &fake);

    assert_eq!(code.ok(), Some(0), "{envelope}");
    assert_eq!(envelope["data"]["parked"], json!(true));
    assert_eq!(envelope["data"]["recorded"], json!(true));
    assert_eq!(*fake.calls.borrow(), [["status", "--json"]]);
    let requests = world.requests()?;
    let [(position, request)] = requests.as_slice() else {
        panic!("one request: {requests:?}");
    };
    assert_eq!(envelope["data"]["position"], json!(position));
    assert_eq!(
        envelope["data"]["item_id"],
        json!(request.item_id()?.as_str())
    );
    let labels: Vec<(OptionId, &str)> = request
        .options
        .iter()
        .map(|option| (option.id, option.label.as_str()))
        .collect();
    assert_eq!(
        labels,
        [(OptionId::A, "Per tenant"), (OptionId::B, "Shared")]
    );
    assert_eq!(request.recommendation, OptionId::A);
    assert_eq!(
        (request.door, request.default, request.blocks),
        (Door::OneWay, None, Blocks::Run),
        "one-way, never defaulted"
    );
    assert_eq!(request.deadline.as_deref(), Some("2099-10-19T23:59:59Z"));
    assert!(
        request.raiser.key.starts_with("orchestrate/park/7/"),
        "{}",
        request.raiser.key
    );
    assert_eq!(
        world.events()?,
        [],
        "no Bench event: the request is the record"
    );
    Ok(())
}

#[test]
fn a_park_run_again_names_the_same_item_and_writes_nothing() -> TestResult {
    let world = World::new()?;
    let fake = world.able();

    let (first_code, first) = world.run(&PARK, Some(&world.bin()), &fake);
    let (code, envelope) = world.run(&PARK, Some(&world.bin()), &fake);

    assert_eq!((first_code.ok(), code.ok()), (Some(0), Some(0)));
    assert_eq!(envelope["data"]["item_id"], first["data"]["item_id"]);
    assert_eq!(
        envelope["data"]["recorded"],
        json!(false),
        "already in the store"
    );
    assert_eq!(envelope["data"]["position"], first["data"]["position"]);
    assert_eq!(world.requests()?.len(), 1, "one request");
    Ok(())
}

#[test]
fn with_weigh_in_absent_the_park_exits_4_and_writes_nothing() -> TestResult {
    let world = World::new()?;
    let fake = world.able();

    let (code, envelope) = world.run(&PARK, None, &fake);

    assert_eq!(code.ok(), Some(4), "{envelope}");
    assert_eq!(envelope["ok"], json!(false));
    assert_eq!(envelope["data"]["parked"], json!(false));
    assert_eq!(
        envelope["data"]["reason"],
        json!("no weighin: WEIGHIN_BIN is unset and none is on PATH")
    );
    assert_eq!(envelope["data"].get("position"), None, "nothing written");
    assert_eq!(
        envelope["findings"][0]["key"],
        json!("orchestrate/weigh-in-unavailable:7")
    );
    assert_eq!(fake.calls(), 0, "nothing to run");
    assert!(!world.store().exists(), "no store created (D3)");
    assert_eq!(world.events()?, []);
    assert_eq!(world.requests()?.len(), 0);
    Ok(())
}

#[test]
fn a_weighin_that_cannot_take_requests_exits_4_and_writes_nothing() -> TestResult {
    let world = World::new()?;
    let failing = Fake::answering(1, "");
    let old = Fake::answering(0, &status(&world.store(), false));
    let elsewhere = Fake::answering(0, &status(&world.cwd().join("other.sqlite3"), true));

    let mut reasons = Vec::new();
    for fake in [&failing, &old, &elsewhere] {
        let (code, envelope) = world.run(&PARK, Some(&world.bin()), fake);
        assert_eq!(code.ok(), Some(4), "{envelope}");
        reasons.push(
            envelope["data"]["reason"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
        );
    }

    assert_eq!(
        reasons.first().map(String::as_str),
        Some("weighin exited with 1: weighin: something failed")
    );
    assert_eq!(
        reasons.get(1).map(String::as_str),
        Some("this weighin (2.0.0) does not fold decision requests")
    );
    assert!(
        reasons
            .get(2)
            .is_some_and(|reason| reason.starts_with("weighin keeps another store")),
        "{reasons:?}"
    );
    assert!(!world.store().exists(), "no store created (D3)");
    assert_eq!(world.events()?, []);
    assert_eq!(world.requests()?.len(), 0, "nothing written");
    Ok(())
}

#[test]
fn unequal_or_too_few_options_are_usage_errors_that_run_nothing() -> TestResult {
    let world = World::new()?;
    let fake = world.able();
    let one_option: Vec<&str> = PARK
        .iter()
        .copied()
        .enumerate()
        .filter(|(index, _)| !(11..=16).contains(index))
        .map(|(_, word)| word)
        .collect();
    let a_holds_short: Vec<&str> = PARK
        .iter()
        .copied()
        .enumerate()
        .filter(|(index, _)| !(13..=14).contains(index))
        .map(|(_, word)| word)
        .collect();

    for words in [one_option, a_holds_short] {
        let (code, envelope) = world.run(&words, Some(&world.bin()), &fake);
        assert!(
            matches!(&code, Err(CliError::Usage { reason }) if reason.contains("2 to 4 options")),
            "{words:?}: {code:?}"
        );
        assert_eq!(envelope, Value::Null, "nothing on stdout");
    }
    assert_eq!(fake.calls(), 0, "weighin never ran");
    assert!(!world.store().exists(), "no ledger created");
    Ok(())
}

#[test]
fn a_resolve_by_today_or_earlier_is_a_usage_error() -> TestResult {
    let now = crate::store::Timestamp::now()?;
    let today = now.as_str().get(..10).ok_or("a date")?;
    for day in ["2026-01-01", today] {
        let world = World::new()?;
        let fake = world.able();
        let mut words = PARK.to_vec();
        words[20] = day;

        let (code, envelope) = world.run(&words, Some(&world.bin()), &fake);

        assert!(
            matches!(&code, Err(CliError::Usage { reason }) if reason.contains("deadline")),
            "{day}: {code:?}"
        );
        assert_eq!(envelope, Value::Null, "{day}");
        assert_eq!(fake.calls(), 0, "{day}: weighin never ran");
        assert!(!world.store().exists(), "{day}: no ledger created");
    }
    Ok(())
}

#[test]
fn a_ci_session_asks_for_attested() -> TestResult {
    let world = World::new()?;
    let fake = world.able();
    let mut words = PARK.to_vec();
    words.extend(["--session", "gha:12345-1"]);

    let (code, envelope) = world.run(&words, Some(&world.bin()), &fake);

    assert_eq!(code.ok(), Some(0), "{envelope}");
    let requests = world.requests()?;
    let [(_, request)] = requests.as_slice() else {
        panic!("one request: {requests:?}");
    };
    assert_eq!(request.trust_needed, TrustNeeded::Attested);
    assert_eq!(request.raiser.session.as_deref(), Some("gha:12345-1"));
    Ok(())
}

#[test]
fn an_impossible_date_or_a_line_separator_is_a_usage_error_exiting_2_that_runs_nothing()
-> TestResult {
    let world = World::new()?;
    let fake = world.able();

    for (index, value) in [(20, "2026-02-31"), (4, "Shared\u{2028}cache?")] {
        let mut words = PARK.to_vec();
        words[index] = value;
        let (code, envelope) = world.run(&words, Some(&world.bin()), &fake);
        let error = code.expect_err("refused");
        assert!(
            matches!(error, CliError::Usage { .. }),
            "{value:?}: {error:?}"
        );
        assert_eq!(error.code().value(), 2, "{value:?}");
        assert_eq!(envelope, Value::Null, "nothing on stdout");
    }
    assert_eq!(fake.calls(), 0, "weighin never ran");
    assert!(!world.store().exists(), "no ledger created");
    Ok(())
}

#[test]
fn invalid_park_arguments_are_usage_errors_that_run_and_record_nothing() -> TestResult {
    let world = World::new()?;
    let fake = world.able();
    let cases: [(usize, &str); 5] = [
        (2, "../7"),
        (4, "  "),
        (4, "two\nlines"),
        (20, "next week"),
        (18, ""),
    ];

    for (index, value) in cases {
        let mut words = PARK.to_vec();
        words[index] = value;
        let (code, envelope) = world.run(&words, Some(&world.bin()), &fake);
        assert!(
            matches!(code, Err(CliError::Usage { .. })),
            "{value:?}: {code:?}"
        );
        assert_eq!(envelope, Value::Null, "nothing on stdout");
    }
    assert_eq!(fake.calls(), 0, "weighin never ran");
    assert!(!world.store().exists(), "no ledger created");
    Ok(())
}

/// Each option of `request` as its id, label, when it holds and its cost if wrong.
fn options_of(request: &Request) -> Vec<(OptionId, &str, &str, &str)> {
    request
        .options
        .iter()
        .map(|option| {
            (
                option.id,
                option.label.as_str(),
                option.holds_when.as_str(),
                option.cost_if_wrong.as_str(),
            )
        })
        .collect()
}

/// A ruling on ticket 7: one cache per tenant.
const RULE: [&str; 9] = [
    "rule",
    "--ticket",
    "7",
    "--what",
    "keep one cache per tenant",
    "--why",
    "reversible in one commit",
    "--cost",
    "a revert and a cache flush",
];

#[test]
fn a_ruling_raises_one_two_way_judgment_request_defaulting_to_what_was_ruled() -> TestResult {
    let world = World::new()?;
    let fake = world.able();

    let (code, envelope) = world.run(&RULE, Some(&world.bin()), &fake);

    assert_eq!(code.ok(), Some(0), "{envelope}");
    assert_eq!(envelope["schema"], json!("bench.orchestrate-rule/2"));
    assert_eq!(envelope["data"]["ticket"], json!("7"));
    assert_eq!(envelope["data"]["recorded"], json!(true));
    assert_eq!(envelope["findings"], json!([]));
    assert_eq!(fake.calls(), 0, "a ruling never asks weigh-in (T11)");
    let requests = world.requests()?;
    let [(position, request)] = requests.as_slice() else {
        panic!("one request: {requests:?}");
    };
    assert_eq!(envelope["data"]["position"], json!(position));
    assert_eq!(
        envelope["data"]["item_id"],
        json!(request.item_id()?.as_str())
    );
    assert_eq!(request.question, "Ticket 7: keep one cache per tenant");
    assert_eq!(
        options_of(request),
        [
            (
                OptionId::A,
                "keep one cache per tenant",
                "reversible in one commit",
                "a revert and a cache flush"
            ),
            (
                OptionId::B,
                "Undo the ruling",
                "the reason given does not hold",
                "the ruled work is undone for nothing"
            ),
        ]
    );
    assert_eq!(
        (
            request.recommendation,
            request.door,
            request.authority,
            request.default,
            request.deadline.as_deref(),
            request.blocks,
        ),
        (
            OptionId::A,
            Door::TwoWay,
            Authority::Judgment,
            Some(OptionId::A),
            None,
            Blocks::None
        ),
        "call F (D4 B): two-way, defaulted to the ruling, no deadline"
    );
    assert!(
        request.raiser.key.starts_with("orchestrate/rule/7/"),
        "{}",
        request.raiser.key
    );
    assert_eq!(request.attachments, None, "no --decision");
    assert_eq!(
        world.events()?,
        [],
        "no Bench event: the request is the record (D6 B)"
    );
    Ok(())
}

#[test]
fn a_ruling_carries_its_d2_decision_as_an_attachment() -> TestResult {
    let world = World::new()?;
    let fake = world.able();
    let words: Vec<&str> = RULE
        .iter()
        .copied()
        .chain(["--decision", "d1760000000000-42-7"])
        .collect();

    let (code, envelope) = world.run(&words, Some(&world.bin()), &fake);

    assert_eq!(code.ok(), Some(0), "{envelope}");
    let requests = world.requests()?;
    let [(_, request)] = requests.as_slice() else {
        panic!("one request: {requests:?}");
    };
    let attachments: Vec<(&str, &str)> = request
        .attachments
        .iter()
        .flatten()
        .map(|a| (a.kind.as_str(), a.locator.as_str()))
        .collect();
    assert_eq!(
        attachments,
        [("bench.clef_decision", "d1760000000000-42-7")],
        "T9: the D2 decision travels with the request"
    );
    Ok(())
}

#[test]
fn a_ruling_run_again_writes_nothing() -> TestResult {
    let world = World::new()?;
    let fake = world.able();

    let (first_code, first) = world.run(&RULE, Some(&world.bin()), &fake);
    let (code, envelope) = world.run(&RULE, Some(&world.bin()), &fake);

    assert_eq!((first_code.ok(), code.ok()), (Some(0), Some(0)));
    assert_eq!(envelope["data"]["item_id"], first["data"]["item_id"]);
    assert_eq!(
        envelope["data"]["recorded"],
        json!(false),
        "already in the store"
    );
    assert_eq!(envelope["data"]["position"], first["data"]["position"]);
    assert_eq!(world.requests()?.len(), 1, "one request");
    Ok(())
}

#[test]
fn a_ruling_with_weigh_in_absent_is_raised_and_says_so() -> TestResult {
    let world = World::new()?;
    let fake = world.able();

    let (code, envelope) = world.run(&RULE, None, &fake);

    assert_eq!(code.ok(), Some(0), "{envelope}");
    assert_eq!(envelope["data"]["recorded"], json!(true));
    assert_eq!(
        envelope["findings"],
        json!([{
            "key": "orchestrate/weigh-in-absent:7",
            "message": "recorded for Weigh-In, which is not installed to show it \
                        (no weighin: WEIGHIN_BIN is unset and none is on PATH)",
        }])
    );
    assert_eq!(fake.calls(), 0, "nothing to run");
    assert_eq!(world.requests()?.len(), 1, "the request waits in the store");
    Ok(())
}

/// A markdown tickets directory under `root` holding `tickets` as `(id, blocked_by)`.
fn tickets(root: &Path, tickets: &[(&str, &str)]) -> TestResult {
    let dir = root.join("tickets");
    fs::create_dir_all(&dir)?;
    for (id, blocked_by) in tickets {
        fs::write(
            dir.join(format!("{id}.md")),
            format!("---\ntitle: Ticket {id}\nblocked_by: [{blocked_by}]\n---\nWork.\n"),
        )?;
    }
    Ok(())
}

#[test]
fn fan_out_goes_ahead_with_three_independent_tickets() -> TestResult {
    let world = World::new()?;
    tickets(
        world.cwd(),
        &[
            ("001-a", ""),
            ("002-b", ""),
            ("003-c", ""),
            ("004-d", "001-a"),
        ],
    )?;

    let (code, envelope) = world.run(&["fan-out"], None, &Fake::answering(0, ""));

    assert_eq!(code.ok(), Some(0), "{envelope}");
    assert_eq!(envelope["data"]["fan_out"], json!(true));
    assert_eq!(envelope["data"]["independent"], json!(3));
    assert_eq!(
        envelope["data"]["tickets"],
        json!(["001-a", "002-b", "003-c"])
    );
    Ok(())
}

#[test]
fn fan_out_is_refused_with_fewer_than_three_independent_tickets() -> TestResult {
    let world = World::new()?;
    tickets(
        world.cwd(),
        &[("001-a", ""), ("002-b", ""), ("003-c", "002-b")],
    )?;

    let (code, envelope) = world.run(&["fan-out"], None, &Fake::answering(0, ""));

    assert_eq!(code.ok(), Some(3), "{envelope}");
    assert_eq!(envelope["data"]["fan_out"], json!(false));
    assert_eq!(envelope["data"]["independent"], json!(2));
    assert_eq!(
        envelope["findings"][0]["key"],
        json!("orchestrate/no-fan-out")
    );
    assert!(!world.store().exists(), "fan-out writes nothing");
    Ok(())
}

#[test]
fn fan_out_is_refused_when_the_graph_fails_its_check() -> TestResult {
    let world = World::new()?;
    tickets(
        world.cwd(),
        &[
            ("001-a", "002-b"),
            ("002-b", "001-a"),
            ("003-c", ""),
            ("004-d", ""),
            ("005-e", ""),
        ],
    )?;

    let (code, envelope) = world.run(&["fan-out"], None, &Fake::answering(0, ""));

    assert_eq!(code.ok(), Some(3), "{envelope}");
    assert_eq!(
        envelope["data"]["independent"],
        json!(0),
        "a failed check lists none"
    );
    let keys: Vec<&str> = envelope["findings"]
        .as_array()
        .map(|findings| findings.iter().filter_map(|f| f["key"].as_str()).collect())
        .unwrap_or_default();
    assert_eq!(keys, ["dag/cycle:001-a->002-b", "orchestrate/no-fan-out"]);
    Ok(())
}
