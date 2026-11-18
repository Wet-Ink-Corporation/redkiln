//! The bridge through a fake [`Runner`]: what `weighin` is asked, and how its answers read.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::status::STATUS_TIMEOUT;
use super::{BridgeError, Call, Ran, RunError, Runner, Unable, able, decided_items};
use crate::process::WaitError;

/// What a fake `weighin` was asked.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Asked {
    program: PathBuf,
    args: Vec<String>,
    input: String,
    cwd: PathBuf,
    timeout: Duration,
}

/// Answers every call with one fixed result and keeps what it was asked.
struct Fake {
    answer: Result<Ran, fn() -> RunError>,
    asked: RefCell<Vec<Asked>>,
}

impl Fake {
    fn answering(code: Option<i32>, stdout: &str, stderr: &str) -> Self {
        Self {
            answer: Ok(Ran {
                code,
                stdout: stdout.as_bytes().to_vec(),
                stderr: stderr.as_bytes().to_vec(),
            }),
            asked: RefCell::new(Vec::new()),
        }
    }

    fn failing(error: fn() -> RunError) -> Self {
        Self {
            answer: Err(error),
            asked: RefCell::new(Vec::new()),
        }
    }

    fn asked(&self) -> Vec<Asked> {
        // The test reads its own copy of the calls.
        self.asked.borrow().clone()
    }
}

impl Runner for Fake {
    fn run(&self, call: &Call<'_>) -> Result<Ran, RunError> {
        self.asked.borrow_mut().push(Asked {
            program: call.program.to_path_buf(),
            args: call.args.iter().map(|arg| (*arg).to_owned()).collect(),
            input: String::from_utf8_lossy(call.input).into_owned(),
            cwd: call.cwd.to_path_buf(),
            timeout: call.timeout,
        });
        match &self.answer {
            // Each call hands out its own copy of the fixed answer.
            Ok(ran) => Ok(ran.clone()),
            Err(error) => Err(error()),
        }
    }
}

/// What a weighin that folds requests prints for `status --json` (the SPEC's golden).
const STATUS: &str = r#"{"schema":"weighin.status/1","ok":true,"command":"status","data":{"plugin":"rk-weigh-in","version":"2.0.0","spec":"1.0.0","store":"/work/repo/.git/redkiln/events.sqlite3","store_source":"git_common_dir","store_schema":"1.0","repo_class":"own","unreadable":{},"dropped":0,"health":"ok","hook_errors":0,"hooks":{},"requests":true},"findings":[]}"#;

/// `able` with `fake`, for Bench's store at the golden's path.
fn status(fake: &Fake) -> Result<(), Unable> {
    able(
        fake,
        Path::new("w"),
        Path::new("."),
        Path::new("/work/repo/.git/redkiln/events.sqlite3"),
    )
}

#[test]
fn status_is_asked_in_the_working_directory_with_nothing_on_stdin() {
    let fake = Fake::answering(Some(0), STATUS, "");

    let result = able(
        &fake,
        Path::new("/opt/weighin"),
        Path::new("/repo"),
        Path::new("/work/repo/.git/redkiln/events.sqlite3"),
    );

    assert!(result.is_ok(), "{result:?}");
    let asked = fake.asked();
    let [call] = asked.as_slice() else {
        panic!("one call: {asked:?}");
    };
    assert_eq!(call.program, Path::new("/opt/weighin"));
    assert_eq!(call.args, ["status", "--json"]);
    assert_eq!(call.cwd, Path::new("/repo"));
    assert_eq!(call.timeout, STATUS_TIMEOUT);
    assert_eq!(call.input, "", "nothing on stdin");
}

#[test]
fn a_non_zero_exit_is_a_failure_naming_its_code_and_first_stderr_line() {
    let fake = Fake::answering(
        Some(1),
        "",
        "weighin: 1 of 2 operations failed; the rest were applied\nmore\n",
    );

    let error = status(&fake).expect_err("exit 1");

    assert_eq!(
        error.to_string(),
        "weighin exited with 1: weighin: 1 of 2 operations failed; the rest were applied"
    );
}

#[test]
fn a_signal_a_timeout_and_an_unreadable_status_are_failures() {
    let killed = Fake::answering(None, STATUS, "");
    let slow = Fake::failing(|| RunError::Wait(WaitError::Timeout));
    let garbled = Fake::answering(Some(0), "✓ ok\n", "");

    let killed = status(&killed);
    let slow = status(&slow);
    let garbled = status(&garbled);

    assert!(
        matches!(
            killed,
            Err(Unable::Bridge(BridgeError::Exit { code: None, .. }))
        ),
        "{killed:?}"
    );
    assert!(
        matches!(
            slow,
            Err(Unable::Bridge(BridgeError::Run(RunError::Wait(
                WaitError::Timeout
            ))))
        ),
        "{slow:?}"
    );
    assert!(matches!(garbled, Err(Unable::Status(_))), "{garbled:?}");
}

#[test]
fn output_over_the_cap_is_refused_rather_than_read_in_part() {
    let long = "x".repeat(super::MAX_OUTPUT_BYTES + 1);
    let fake = Fake::answering(Some(0), &long, "");

    let error = status(&fake).expect_err("too long");

    assert!(
        matches!(error, Unable::Bridge(BridgeError::TooLarge { .. })),
        "{error:?}"
    );
}

#[test]
fn decided_items_are_read_from_open_all_json_within_the_given_time() {
    let listed = r#"[{"id": "wi-e41e02", "status": "decided", "question": "Shared?",
                      "decision": {"option": "shared"}}]"#;
    let fake = Fake::answering(Some(0), listed, "");

    let found = decided_items(
        &fake,
        Path::new("w"),
        Path::new("/repo"),
        Duration::from_millis(700),
    )
    .expect("read");

    let asked = fake.asked();
    let [call] = asked.as_slice() else {
        panic!("one call: {asked:?}");
    };
    assert_eq!(call.args, ["open", "--all", "--json", "--top", "4096"]);
    assert_eq!(call.input, "", "nothing on stdin");
    assert_eq!(call.timeout, Duration::from_millis(700));
    let ids: Vec<&str> = found.iter().map(|item| item.id.as_str()).collect();
    assert_eq!(ids, ["wi-e41e02"]);
}

#[test]
fn a_listing_that_does_not_parse_is_a_failure() {
    let fake = Fake::answering(Some(0), "1 open in repo\n", "");

    let error =
        decided_items(&fake, Path::new("w"), Path::new("."), STATUS_TIMEOUT).expect_err("not JSON");

    assert!(matches!(error, BridgeError::Listing(_)), "{error:?}");
}

#[test]
fn the_listing_asks_weighin_for_up_to_4096_items_not_its_default_12() {
    let fake = Fake::answering(Some(0), "[]", "");

    let found = decided_items(&fake, Path::new("w"), Path::new("."), STATUS_TIMEOUT).expect("read");

    assert_eq!(found, []);
    let asked = fake.asked();
    let [call] = asked.as_slice() else {
        panic!("one call: {asked:?}");
    };
    assert_eq!(call.args, ["open", "--all", "--json", "--top", "4096"]);
}

/// A listing of `count` decided items, `wi-000000` up (real Weigh-In ids, six hex digits).
fn listing(count: usize) -> String {
    let items: Vec<serde_json::Value> = (0..count)
        .map(|n| {
            serde_json::json!({"id": format!("wi-{n:06x}"), "status": "decided", "question": "Q?",
                               "decision": {"option": "yes"}})
        })
        .collect();
    serde_json::Value::Array(items).to_string()
}

#[test]
fn a_listing_as_long_as_the_cap_may_be_cut_so_it_is_refused_not_read_in_part() {
    let full = Fake::answering(Some(0), &listing(4096), "");
    let under = Fake::answering(Some(0), &listing(4095), "");

    let full = decided_items(&full, Path::new("w"), Path::new("."), STATUS_TIMEOUT);
    let under = decided_items(&under, Path::new("w"), Path::new("."), STATUS_TIMEOUT);

    assert!(
        matches!(full, Err(BridgeError::Truncated { cap: 4096 })),
        "{full:?}"
    );
    assert_eq!(under.expect("under the cap").len(), 4095);
}
