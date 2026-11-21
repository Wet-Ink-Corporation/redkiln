//! The `AskUserQuestion` payload read into [`Answered`], driven by the live captures in
//! `tests/fixtures/ask-user-question/` (Claude Code 2.1.325, #125 PR 0; their README is the
//! evidence for the shape).

use super::input_of;
use crate::hooks::{Answered, Asked, HookEvent};
use serde_json::{Value, json};
use weighin_core::answered::{Answer, Resolution, ShownQuestion};

const ANSWERED: &str = include_str!("../../../tests/fixtures/ask-user-question/answered.post.json");
const MULTI: &str =
    include_str!("../../../tests/fixtures/ask-user-question/multi-select.post.json");
const OTHER: &str = include_str!("../../../tests/fixtures/ask-user-question/other-text.post.json");
const OVERRIDDEN: &str =
    include_str!("../../../tests/fixtures/ask-user-question/prefill-overridden.post.json");
const PRE: &str = include_str!("../../../tests/fixtures/ask-user-question/answered.pre.json");

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn payload(text: &str) -> TestResult<Value> {
    Ok(serde_json::from_str(text)?)
}

/// The tool use id and the one question the payload was read as answering.
fn only_asked(payload: Value) -> TestResult<(String, Asked)> {
    match input_of(payload).event {
        HookEvent::Answered(Some(Answered { tool_use_id, asked })) => {
            let [one]: [Asked; 1] = asked
                .try_into()
                .map_err(|all: Vec<Asked>| format!("not one question: {all:?}"))?;
            Ok((tool_use_id, one))
        }
        other => Err(format!("not an answered dialog: {other:?}").into()),
    }
}

fn picked(selected: &[&str]) -> Answer {
    Answer {
        resolution: Resolution::Person,
        selected: selected.iter().map(|s| (*s).to_owned()).collect(),
    }
}

#[test]
fn the_answered_capture_reads_the_shown_question_and_the_pick() -> TestResult {
    let (tool_use_id, asked) = only_asked(payload(ANSWERED)?)?;

    assert_eq!(tool_use_id, "toolu_01QKbVNaJMqaP8VSwGhuFxkh");
    assert_eq!(
        asked,
        Asked {
            shown: ShownQuestion {
                text: "What should I work on next in ask-capture-kit?".to_owned(),
                multi_select: false,
                labels: vec![
                    "Explore the repo".to_owned(),
                    "Run the tests".to_owned(),
                    "Review recent changes".to_owned(),
                ],
            },
            answer: picked(&["Run the tests"]),
        }
    );
    Ok(())
}

#[test]
fn multi_select_and_other_text_captures_read_as_they_arrived() -> TestResult {
    let (_, multi) = only_asked(payload(MULTI)?)?;
    let (_, other) = only_asked(payload(OTHER)?)?;

    assert!(multi.shown.multi_select);
    assert_eq!(multi.answer, picked(&["Session ID, Raw payload"]));
    assert!(!other.shown.multi_select);
    assert_eq!(other.answer, picked(&["Typing some \"other\" text"]));
    Ok(())
}

#[test]
fn the_person_s_pick_over_a_prefill_is_what_is_read() -> TestResult {
    let (_, asked) = only_asked(payload(OVERRIDDEN)?)?;

    assert_eq!(asked.answer, picked(&["No limit"]), "not the prefill");
    Ok(())
}

#[test]
fn the_answer_is_read_from_tool_response_never_tool_input() -> TestResult {
    let mut emptied = payload(ANSWERED)?;
    emptied["tool_response"]["answers"] = json!({});
    let mut dropped = payload(ANSWERED)?;
    dropped
        .as_object_mut()
        .ok_or("an object")?
        .remove("tool_response");

    let (_, asked) = only_asked(emptied)?;

    assert_eq!(asked.answer, picked(&[]));
    assert_eq!(input_of(dropped).event, HookEvent::Answered(None));
    Ok(())
}

#[test]
fn a_timeout_follow_up_or_free_text_field_is_read_as_the_resolution() -> TestResult {
    for (field, value, resolution) in [
        ("afkTimeoutMs", json!(60000), Resolution::AfkTimeout),
        ("followUp", json!("Why not B?"), Resolution::FollowUp),
        ("response", json!("my own words"), Resolution::FreeText),
    ] {
        let mut answered = payload(ANSWERED)?;
        answered["tool_response"][field] = value;

        let (_, asked) = only_asked(answered)?;

        assert_eq!(asked.answer.resolution, resolution, "{field}");
    }
    Ok(())
}

#[test]
fn a_null_timeout_follow_up_or_response_is_absent() -> TestResult {
    let mut answered = payload(ANSWERED)?;
    for field in ["afkTimeoutMs", "followUp", "response"] {
        answered["tool_response"][field] = Value::Null;
    }

    let (_, asked) = only_asked(answered)?;

    assert_eq!(asked.answer.resolution, Resolution::Person);
    Ok(())
}

#[test]
fn an_answer_without_a_tool_use_id_is_none() -> TestResult {
    let mut answered = payload(ANSWERED)?;
    answered["tool_use_id"] = json!(7);

    assert_eq!(input_of(answered).event, HookEvent::Answered(None));
    Ok(())
}

#[test]
fn a_question_without_text_or_options_is_read_with_what_it_has() -> TestResult {
    let answered = json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "AskUserQuestion",
        "tool_use_id": "toolu_1",
        "tool_response": {
            "questions": [{"options": [{"label": "A: Keep"}, {"description": "no label"}]}, 3],
            "answers": {"": "A: Keep"}
        }
    });

    let (_, asked) = only_asked(answered)?;

    assert_eq!(
        asked,
        Asked {
            shown: ShownQuestion {
                text: String::new(),
                multi_select: false,
                labels: vec!["A: Keep".to_owned()],
            },
            answer: picked(&["A: Keep"]),
        }
    );
    Ok(())
}

#[test]
fn the_pre_tool_use_capture_is_not_an_answer() -> TestResult {
    assert_eq!(
        input_of(payload(PRE)?).event,
        HookEvent::Other("PreToolUse".to_owned())
    );
    Ok(())
}
