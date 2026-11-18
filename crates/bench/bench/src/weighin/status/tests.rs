//! `able` through a fake `weighin` answering `status --json`.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use super::{STATUS_TIMEOUT, Unable, able};
use crate::process::WaitError;
use crate::weighin::{BridgeError, Call, Ran, RunError, Runner};

/// Answers every call with one fixed result and keeps the arguments it was given.
struct Fake {
    answer: Result<Ran, fn() -> RunError>,
    asked: RefCell<Vec<(Vec<String>, std::time::Duration)>>,
}

impl Fake {
    fn answering(code: Option<i32>, stdout: &str) -> Self {
        Self {
            answer: Ok(Ran {
                code,
                stdout: stdout.as_bytes().to_vec(),
                stderr: b"weighin: no repository\n".to_vec(),
            }),
            asked: RefCell::new(Vec::new()),
        }
    }
}

impl Runner for Fake {
    fn run(&self, call: &Call<'_>) -> Result<Ran, RunError> {
        self.asked.borrow_mut().push((
            call.args.iter().map(|arg| (*arg).to_owned()).collect(),
            call.timeout,
        ));
        match &self.answer {
            // Each call hands out its own copy of the fixed answer.
            Ok(ran) => Ok(ran.clone()),
            Err(error) => Err(error()),
        }
    }
}

/// The SPEC's `weighin status --json` golden `name`, its store replaced by `store`.
fn golden(name: &str, store: &Path) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../spec/fixtures/status/valid")
        .join(name);
    let text = std::fs::read_to_string(path).expect("the SPEC's golden is in the tree");
    let store = serde_json::Value::String(store.to_string_lossy().into_owned()).to_string();
    text.replace(r#""/work/repo/.git/redkiln/events.sqlite3""#, &store)
}

/// A store path inside a temporary directory, kept alive with it.
fn ours() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a directory");
    let store = dir.path().join("events.sqlite3");
    (dir, store)
}

fn check(fake: &Fake, ours: &Path) -> Result<(), Unable> {
    able(fake, Path::new("/opt/weighin"), Path::new("/repo"), ours)
}

#[test]
fn a_weighin_that_folds_requests_into_the_same_store_is_able() {
    let (_dir, store) = ours();
    let fake = Fake::answering(Some(0), &golden("weighin-ok-requests.json", &store));

    let result = check(&fake, &store);

    assert!(result.is_ok(), "{result:?}");
    let asked = fake.asked.borrow();
    let [(args, timeout)] = asked.as_slice() else {
        panic!("one call: {asked:?}");
    };
    assert_eq!(args, &["status", "--json"]);
    assert_eq!(*timeout, STATUS_TIMEOUT);
}

#[test]
fn a_2_0_0_status_without_requests_is_no_requests() {
    let (_dir, store) = ours();
    let fake = Fake::answering(Some(0), &golden("weighin-ok.json", &store));

    let result = check(&fake, &store);

    assert!(
        matches!(&result, Err(Unable::NoRequests { version }) if version == "2.0.0"),
        "{result:?}"
    );
}

#[test]
fn another_store_is_refused() {
    let (_dir, store) = ours();
    let (_other_dir, other) = ours();
    let fake = Fake::answering(Some(0), &golden("weighin-ok-requests.json", &other));
    let unlocated = golden("weighin-ok-requests.json", &store).replace(
        &serde_json::Value::String(store.to_string_lossy().into_owned()).to_string(),
        "null",
    );
    let none = Fake::answering(Some(0), &unlocated);

    let other = check(&fake, &store);
    let none = check(&none, &store);

    assert!(matches!(other, Err(Unable::OtherStore { .. })), "{other:?}");
    assert!(matches!(none, Err(Unable::OtherStore { .. })), "{none:?}");
}

#[test]
fn the_same_store_named_through_another_path_is_the_same() {
    let (dir, store) = ours();
    let dotted = dir.path().join(".").join("events.sqlite3");
    let fake = Fake::answering(Some(0), &golden("weighin-ok-requests.json", &dotted));

    let result = check(&fake, &store);

    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn a_status_of_another_schema_or_shape_is_refused() {
    let (_dir, store) = ours();
    let good = golden("weighin-ok-requests.json", &store);
    let answers = [
        good.replace("weighin.status/1", "weighin.status/2"),
        good.replace(r#""repo_class":"own""#, r#""repo_class":7"#),
        "1 open in repo\n".to_owned(),
        r#"{"schema":"weighin.status/1"}"#.to_owned(),
    ];

    for answer in answers {
        let result = check(&Fake::answering(Some(0), &answer), &store);

        assert!(
            matches!(result, Err(Unable::Status(_))),
            "{answer}: {result:?}"
        );
    }
}

#[test]
fn a_failing_or_silent_weighin_is_unable_with_its_reason() {
    let (_dir, store) = ours();
    let failing = Fake::answering(Some(1), "");
    let killed = Fake::answering(None, "");
    let slow = Fake {
        answer: Err(|| RunError::Wait(WaitError::Timeout)),
        asked: RefCell::new(Vec::new()),
    };

    let failing = check(&failing, &store);
    let killed = check(&killed, &store);
    let slow = check(&slow, &store);

    assert_eq!(
        failing.map_err(|error| error.to_string()),
        Err("weighin exited with 1: weighin: no repository".to_owned())
    );
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
}
