//! A Weigh-In card the person answers in Claude Code's `AskUserQuestion` dialog is decided at
//! `witnessed` trust by the `PostToolUse` hook, from its own input (#125 PR 1b, SPEC TR-03,
//! DR-06), and no command writes above `asserted` (TR-05). The payloads are the live capture
//! with the card's question and labels in place (`support::ask_user_question`).

use crate::scratch::{Repo, TestResult};
use crate::support::{Dialog, ask_user_question, git, state_of, store_of};
use redkiln_core::envelope::{Envelope, decode};
use redkiln_core::store::{Filter, Store, Stored, Wait};
use redkiln_core::trust::{Level, Via};
use serde_json::{Value, json};
use std::num::NonZeroUsize;
use std::path::Path;
use std::process::Output;
use weighin_contract::event::Decided;

const QUESTION: &str = "Keep retention per series?";
const TOOL_USE: &str = "toolu_01QKbVNaJMqaP8VSwGhuFxkh";
const LABELS: [&str; 2] = ["A: Per series", "B: Per record"];

/// Raise `q` as a two-way item in `s1`, ground it with a two-option card and present it; its id.
fn presented(repo: &Repo, q: &str) -> TestResult<String> {
    let batch = json!([
        {"op": "raise", "q": q, "kind": "question", "door": "two-way", "authority": "judgment"},
        {"op": "triage", "id": "@1", "door": "two-way", "blast": "module", "route": "present",
         "reason": "r"},
        {"op": "ground", "id": "@1", "card": {
            "options": [{"id": "A", "title": "Per series"}, {"id": "B", "title": "Per record"}],
            "recommendation": {"option": "A"}
        }},
        {"op": "present", "id": "@1", "surface": "chat"}
    ]);
    let out = repo.ok("s1", &["batch"], Some(&batch.to_string()))?;
    let id = out
        .split_whitespace()
        .find(|w| w.starts_with("wi-"))
        .ok_or_else(|| format!("no id in {out}"))?;
    Ok(id.to_owned())
}

/// Run the `PostToolUse` hook in `repo` on a dialog shown and answered as `dialog`.
fn answer(repo: &Repo, root: &Path, dialog: Dialog<'_>) -> TestResult<Output> {
    let out = repo.hook(&ask_user_question(root, "s1", TOOL_USE, dialog)?)?;
    assert_eq!(out.status.code(), Some(0), "the hook never fails");
    Ok(out)
}

/// The context lines the hook's output added, none when it printed nothing.
fn context(out: &Output) -> TestResult<Vec<String>> {
    let printed = std::str::from_utf8(&out.stdout)?;
    if printed.is_empty() {
        return Ok(Vec::new());
    }
    let json: Value = serde_json::from_str(printed)?;
    let text = json
        .pointer("/hookSpecificOutput/additionalContext")
        .and_then(Value::as_str)
        .unwrap_or_default();
    Ok(text.lines().map(str::to_owned).collect())
}

/// Every event in the store at `store`, in order.
fn stored(store: &Path) -> TestResult<Vec<Stored>> {
    let page = NonZeroUsize::new(4096).ok_or("a page")?;
    Ok(Store::open(store, Wait::Command)?.read(&Filter::all(), 0, page)?)
}

/// Every `item_decided` in the store at `store`.
fn decisions(store: &Path) -> TestResult<Vec<Envelope<Decided>>> {
    Ok(stored(store)?
        .iter()
        .filter_map(|s| decode::<Decided>(s).ok())
        .collect())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_card_answered_in_ask_user_question_is_recorded_witnessed() -> TestResult {
    let repo = Repo::new(true)?;
    let id = presented(&repo, QUESTION)?;
    let text = format!("{id} {QUESTION}");

    let out = answer(&repo, &repo.root, (&text, &LABELS, "B: Per record"))?;

    let lines = context(&out)?;
    assert!(
        matches!(lines.as_slice(), [line] if line.contains(&format!("decided {id} → B"))
            && line.contains("witnessed")),
        "{lines:?}"
    );
    let decided = decisions(&store_of(&repo.root))?;
    let [one] = decided.as_slice() else {
        return Err(format!("not one decision: {decided:?}").into());
    };
    assert_eq!(
        (
            one.trust.level(),
            one.trust.via(),
            one.trust.reference(),
            one.trust.claimed()
        ),
        (
            Level::Witnessed,
            Some(Via::AskUserQuestion),
            Some(TOOL_USE),
            None
        )
    );
    assert_eq!(
        one.actor.to_string(),
        one.data.decider,
        "actor human:<owner>"
    );
    assert!(
        one.data.decider.starts_with("human:"),
        "{}",
        one.data.decider
    );
    assert_eq!(
        (one.data.option.as_str(), one.data.via.as_str()),
        ("B", "ask-user-question")
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_repeated_answer_payload_writes_nothing() -> TestResult {
    let repo = Repo::new(true)?;
    let id = presented(&repo, QUESTION)?;
    let text = format!("{id} {QUESTION}");
    answer(&repo, &repo.root, (&text, &LABELS, "B: Per record"))?;
    let before = stored(&store_of(&repo.root))?.len();

    let again = answer(&repo, &repo.root, (&text, &LABELS, "B: Per record"))?;

    assert_eq!(
        std::str::from_utf8(&again.stdout)?,
        "",
        "no context and no failure note"
    );
    assert_eq!(stored(&store_of(&repo.root))?.len(), before);
    assert!(
        !state_of(&repo.root).join("hook-errors.log").exists(),
        "a repeat is no error"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn an_answer_that_does_not_witness_a_pick_writes_nothing_and_says_why() -> TestResult {
    let repo = Repo::new(true)?;
    let id = presented(&repo, QUESTION)?;
    let text = format!("{id} {QUESTION}");
    let before = stored(&store_of(&repo.root))?.len();

    let out = answer(
        &repo,
        &repo.root,
        (&text, &LABELS, "Per series, but ask Sam"),
    )?;

    let lines = context(&out)?;
    assert!(
        matches!(lines.as_slice(), [line] if line.starts_with(&format!("Weigh-In did not record {id}"))
            && line.contains("was not shown")),
        "{lines:?}"
    );
    assert_eq!(stored(&store_of(&repo.root))?.len(), before);
    Ok(())
}

/// The answers are keyed by question text, so a text shown twice cannot say which question an
/// answer was to.
#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_card_shown_twice_in_one_dialog_is_not_recorded() -> TestResult {
    let repo = Repo::new(true)?;
    let id = presented(&repo, QUESTION)?;
    let text = format!("{id} {QUESTION}");
    let mut payload = ask_user_question(
        &repo.root,
        "s1",
        TOOL_USE,
        (&text, &LABELS, "B: Per record"),
    )?;
    let questions = payload["tool_response"]["questions"]
        .as_array_mut()
        .ok_or("questions")?;
    let first = questions.first().cloned().ok_or("one question")?;
    questions.push(first);
    let before = stored(&store_of(&repo.root))?.len();

    let out = repo.hook(&payload)?;

    let lines = context(&out)?;
    assert!(
        lines.len() == 2
            && lines
                .iter()
                .all(|l| l.contains("its question was shown more than once")),
        "{lines:?}"
    );
    assert_eq!(stored(&store_of(&repo.root))?.len(), before);
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_question_that_is_no_card_is_passed_over_in_silence() -> TestResult {
    let repo = Repo::new(true)?;
    presented(&repo, QUESTION)?;
    let before = stored(&store_of(&repo.root))?.len();

    let out = answer(
        &repo,
        &repo.root,
        ("What should I work on next?", &["Tests", "Docs"], "Tests"),
    )?;

    assert_eq!(std::str::from_utf8(&out.stdout)?, "");
    assert_eq!(stored(&store_of(&repo.root))?.len(), before);
    Ok(())
}

/// TR-05: whatever a command is given, `via` and provenance included, it writes `asserted`.
#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn no_command_writes_above_asserted() -> TestResult {
    let repo = Repo::new(true)?;
    let typed = presented(&repo, "Typed at the command line?")?;
    let batched = presented(&repo, "Decided in a batch claiming the dialog?")?;
    let paned = presented(&repo, "Answered in the pane?")?;
    let state: Value =
        serde_json::from_str(&repo.ok("s1", &["board", "export", "--stdout"], None)?)?;
    let rev = state["items"]
        .as_array()
        .ok_or("items")?
        .iter()
        .find(|i| i["id"] == json!(paned))
        .and_then(|i| i["card_rev"].as_u64())
        .ok_or("the pane item's card_rev")?;

    repo.ok("s1", &["decide", &typed, "A"], None)?;
    let batch = json!([{"op": "decide", "id": batched, "option": "B",
        "via": "ask-user-question", "provenance": "user-approved"}]);
    repo.ok("s1", &["batch"], Some(&batch.to_string()))?;
    let pane = json!({"item_id": paned, "action": "choose", "option": "B", "card_rev": rev});
    repo.ok(
        "s1",
        &["board", "apply", "--surface", "pane"],
        Some(&pane.to_string()),
    )?;

    let events = stored(&store_of(&repo.root))?;
    let levels: Vec<Option<&str>> = events
        .iter()
        .map(|e| {
            e.tags
                .iter()
                .find(|t| t.key() == Some("trust"))
                .and_then(|t| t.value())
        })
        .collect();
    assert!(events.len() >= 15, "{} events", events.len());
    assert_eq!(levels, vec![Some("asserted"); events.len()]);
    let decided = decisions(&store_of(&repo.root))?;
    assert_eq!(
        decided
            .iter()
            .map(|d| (d.trust.level(), d.data.item_id.as_str()))
            .collect::<Vec<_>>(),
        [
            (Level::Asserted, typed.as_str()),
            (Level::Asserted, batched.as_str()),
            (Level::Asserted, paned.as_str()),
        ]
    );
    Ok(())
}

/// TR-06: a witnessed decision another clone exported reads back from the journal `asserted`,
/// with the level it carried as `claimed`.
#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_witnessed_decision_imported_from_a_journal_reads_back_asserted_claimed_witnessed() -> TestResult
{
    let (repo, remote) = Repo::with_remote(true)?;
    let id = presented(&repo, QUESTION)?;
    let text = format!("{id} {QUESTION}");
    answer(&repo, &repo.root, (&text, &LABELS, "B: Per record"))?;
    repo.ok("s1", &["journal", "checkpoint"], None)?;
    let parent = repo.sibling("clones")?;
    git(&parent, &["clone", "-q", &remote.to_string_lossy(), "b"]);
    let clone = parent.join("b");
    let init = repo.run_in(&clone, &["init", "--class", "own"])?;
    assert!(init.status.success(), "{init:?}");

    let imported = repo.run_in(&clone, &["journal", "import"])?;

    assert!(imported.status.success(), "{imported:?}");
    let decided = decisions(&store_of(&clone))?;
    let [one] = decided.as_slice() else {
        return Err(format!("not one decision: {decided:?}").into());
    };
    assert_eq!(
        (
            one.trust.level(),
            one.trust.claimed(),
            one.trust.via(),
            one.trust.reference()
        ),
        (
            Level::Asserted,
            Some(Level::Witnessed),
            Some(Via::AskUserQuestion),
            Some(TOOL_USE)
        )
    );
    Ok(())
}
