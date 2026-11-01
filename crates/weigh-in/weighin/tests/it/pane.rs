//! What the local board pane needs from the CLI (#115, D-0017): the board state on stdout under a
//! `local` or `artifact` board and never a file, and one answer at a time from stdin, recorded
//! with surface `pane`.

use crate::support::{setup, weighin};
use serde_json::{Value, json};
use std::path::Path;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Raise a question, triage and ground it with a two-option card; its id.
fn grounded(repo: &Path) -> Result<String, String> {
    let batch = json!([
        {"op": "raise", "q": "Keep retention per series?", "door": "two-way"},
        {"op": "triage", "id": "@1", "door": "two-way", "blast": "module", "route": "present",
         "reason": "authority-bound"},
        {"op": "ground", "id": "@1", "card": {
            "options": [{"id": "A", "title": "Per series"}, {"id": "B", "title": "Per record"}],
            "recommendation": {"option": "A"}
        }}
    ]);
    let (ok, out) = weighin(repo, "s1", &["batch"], Some(&batch.to_string()));
    if !ok {
        return Err(out);
    }
    out.split_whitespace()
        .find(|w| w.starts_with("wi-"))
        .map(str::to_owned)
        .ok_or(out)
}

fn init(repo: &Path, args: &[&str]) -> Result<(), String> {
    let (ok, out) = weighin(repo, "s1", &[&["init"], args].concat(), None);
    if ok { Ok(()) } else { Err(out) }
}

/// The board state `board export --stdout` prints.
fn state(repo: &Path) -> Result<Value, Box<dyn std::error::Error>> {
    let (ok, out) = weighin(repo, "s1", &["board", "export", "--stdout"], None);
    if !ok {
        return Err(out.into());
    }
    Ok(serde_json::from_str(&out)?)
}

/// The card revision the board state shows for its first card.
fn card_rev(state: &Value) -> Option<u64> {
    state.get("items")?.get(0)?.get("card_rev")?.as_u64()
}

fn item(repo: &Path, id: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let (ok, out) = weighin(repo, "s1", &["show", id, "--json"], None);
    if !ok {
        return Err(out.into());
    }
    Ok(serde_json::from_str(&out)?)
}

fn pane_apply(repo: &Path, answer: &Value) -> (bool, String) {
    weighin(
        repo,
        "s1",
        &["board", "apply", "--surface", "pane"],
        Some(&answer.to_string()),
    )
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn export_to_stdout_under_a_local_board_prints_the_state_and_writes_no_file() -> TestResult {
    let (_tmp, repo) = setup();
    init(&repo, &["--class", "client"])?;
    let id = grounded(&repo)?;

    let printed = state(&repo)?;

    assert_eq!(
        (
            printed.get("schema"),
            printed.pointer("/scope/board_mode"),
            printed.pointer("/items/0/id")
        ),
        (Some(&json!(1)), Some(&json!("local")), Some(&json!(id)))
    );
    assert!(
        !repo.join(".git/weighin/board-state.json").exists(),
        "no state file beside the ledger"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn export_to_stdout_before_any_item_prints_an_empty_board() -> TestResult {
    let (_tmp, repo) = setup();
    init(&repo, &["--class", "own"])?;

    let printed = state(&repo)?;

    assert_eq!(printed.get("items"), Some(&json!([])));
    assert!(
        !repo.join(".git/weighin").exists(),
        "no ledger directory created"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn export_to_stdout_is_refused_when_the_board_is_off() -> TestResult {
    let (_tmp, repo) = setup();
    init(&repo, &["--board", "off"])?;
    grounded(&repo)?;

    let (ok, out) = weighin(&repo, "s1", &["board", "export", "--stdout"], None);

    assert!(!ok && out.contains("board mode is `off`"), "{out}");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn export_to_stdout_is_refused_in_a_folder_never_initialised() -> TestResult {
    let (_tmp, repo) = setup();
    grounded(&repo)?;

    let (ok, out) = weighin(&repo, "s1", &["board", "export", "--stdout"], None);

    assert!(!ok && out.contains("weighin init --class own"), "{out}");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn an_answer_from_the_pane_on_stdin_is_decided_via_pane() -> TestResult {
    let (_tmp, repo) = setup();
    init(&repo, &["--class", "client"])?;
    let id = grounded(&repo)?;
    let rev = card_rev(&state(&repo)?);

    let (ok, out) = pane_apply(
        &repo,
        &json!({"item_id": id, "action": "choose", "option": "B", "card_rev": rev}),
    );

    assert!(ok && out.contains(&format!("applied: {id}")), "{out}");
    let decided = item(&repo, &id)?;
    assert_eq!(
        (decided.pointer("/status"), decided.pointer("/decision/via")),
        (Some(&json!("decided")), Some(&json!("pane")))
    );
    let (_, stats) = weighin(&repo, "s1", &["stats", "--json"], None);
    let h: Value = serde_json::from_str(&stats)?;
    assert_eq!(h.get("decided_via"), Some(&json!({"pane": 1})), "{stats}");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn the_pane_cannot_answer_an_item_no_sitting_grounded() -> TestResult {
    let (_tmp, repo) = setup();
    init(&repo, &["--class", "client"])?;
    let (ok, out) = weighin(&repo, "s1", &["raise", "--q", "A raw question?"], None);
    assert!(ok, "{out}");
    let id = out
        .split_whitespace()
        .find(|w| w.starts_with("wi-"))
        .ok_or("no id")?
        .to_owned();

    let (ok, out) = pane_apply(
        &repo,
        &json!({"item_id": id, "action": "dismiss", "reason": "moot"}),
    );

    assert!(!ok && out.contains("not grounded by a sitting"), "{out}");
    assert_eq!(item(&repo, &id)?.pointer("/status"), Some(&json!("raised")));
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn the_pane_is_refused_an_answer_read_from_an_older_card() -> TestResult {
    let (_tmp, repo) = setup();
    init(&repo, &["--class", "client"])?;
    let id = grounded(&repo)?;
    let rev = card_rev(&state(&repo)?).ok_or("no card_rev")?;

    let (ok, out) = pane_apply(
        &repo,
        &json!({"item_id": id, "action": "choose", "option": "A", "card_rev": rev - 1}),
    );

    assert!(!ok && out.contains("re-grounded"), "{out}");
    assert_eq!(
        item(&repo, &id)?.pointer("/status"),
        Some(&json!("grounded"))
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn the_pane_cannot_answer_when_the_board_is_off() -> TestResult {
    let (_tmp, repo) = setup();
    init(&repo, &["--board", "off"])?;
    let id = grounded(&repo)?;

    let (ok, out) = pane_apply(
        &repo,
        &json!({"item_id": id, "action": "dismiss", "reason": "moot"}),
    );

    assert!(!ok && out.contains("board mode is `off`"), "{out}");
    assert_eq!(
        item(&repo, &id)?.pointer("/status"),
        Some(&json!("grounded"))
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn board_apply_with_a_dash_reads_the_answers_from_stdin_for_the_board() -> TestResult {
    let (_tmp, repo) = setup();
    init(&repo, &["--class", "own"])?;
    let id = grounded(&repo)?;
    let rev = card_rev(&state(&repo)?);
    let answers = json!([{"id": id, "data": {"item_id": id, "action": "choose", "option": "A",
                                             "card_rev": rev}}]);

    let (ok, out) = weighin(
        &repo,
        "s1",
        &["board", "apply", "-"],
        Some(&answers.to_string()),
    );

    assert!(ok && out.contains(&format!("applied: {id}")), "{out}");
    assert_eq!(
        item(&repo, &id)?.pointer("/decision/via"),
        Some(&json!("board"))
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn the_pane_cannot_answer_in_a_folder_never_initialised() -> TestResult {
    let (_tmp, repo) = setup();
    let id = grounded(&repo)?;

    let (ok, out) = pane_apply(
        &repo,
        &json!({"item_id": id, "action": "dismiss", "reason": "moot"}),
    );

    assert!(!ok && out.contains("weighin init --class own"), "{out}");
    assert_eq!(
        item(&repo, &id)?.pointer("/status"),
        Some(&json!("grounded"))
    );
    Ok(())
}
