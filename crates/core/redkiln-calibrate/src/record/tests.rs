use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::{Value, json};

use super::{
    Answer, JUDGEMENT_LABELLED, JUDGEMENT_RECORDED, JudgementId, JudgementLabelled,
    JudgementRecorded, LabelActor, LabelBy, LabelledWire, Mode, Outcome, Producer, ProducerError,
    RecordError, RecordedWire, Subject, SubjectError, VERSION, Vocab,
};
use crate::score::Probability;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// `spec/fixtures/<rel>`.
fn fixture(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../spec/fixtures")
        .join(rel)
}

/// The golden envelope at `rel`: its `type` and `version`, and the exact bytes of its `data`,
/// which every record golden writes last.
fn golden_data(rel: &str) -> Result<(String, u64, String), Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(fixture(rel))?;
    let envelope: Value = serde_json::from_str(&text)?;
    let (_, data) = text.split_once("\"data\":").ok_or("a data key")?;
    let data = data.strip_suffix("}\n").ok_or("data last, one line")?;
    Ok((
        envelope["type"].as_str().ok_or("a type")?.to_owned(),
        envelope["version"].as_u64().ok_or("a version")?,
        data.to_owned(),
    ))
}

fn p(value: f64) -> Probability {
    Probability::try_from(value).expect("a probability")
}

#[test]
fn each_golden_reads_as_its_type_and_writes_back_byte_for_byte() -> TestResult {
    for rel in [
        "envelope/valid/judgement-recorded-answered.json",
        "envelope/valid/judgement-recorded-failed.json",
    ] {
        let (event_type, version, data) = golden_data(rel)?;
        assert_eq!(
            (event_type.as_str(), version),
            (JUDGEMENT_RECORDED, u64::from(VERSION))
        );
        let record: JudgementRecorded = serde_json::from_str(&data)?;
        assert_eq!(serde_json::to_string(&record)?, data, "{rel}");
    }
    let (event_type, version, data) = golden_data("envelope/valid/judgement-labelled.json")?;
    assert_eq!(
        (event_type.as_str(), version),
        (JUDGEMENT_LABELLED, u64::from(VERSION))
    );
    let label: JudgementLabelled = serde_json::from_str(&data)?;
    assert_eq!(serde_json::to_string(&label)?, data);
    Ok(())
}

#[test]
fn the_answered_golden_reads_into_its_fields() -> TestResult {
    let (_, _, data) = golden_data("envelope/valid/judgement-recorded-answered.json")?;

    let record: JudgementRecorded = serde_json::from_str(&data)?;

    assert_eq!(
        (
            record.producer.as_str(),
            record.id.as_str(),
            record.point.as_str(),
            record.question.as_str(),
            record.mode,
        ),
        ("bench", "d1763287200003-41-0", "D1", "noul", Mode::Shadow)
    );
    assert_eq!(
        record.answer,
        Answer::Given {
            probability: p(0.95),
            options: BTreeMap::from([("no".to_owned(), p(0.05)), ("yes".to_owned(), p(0.95))]),
            top: None,
            action: Some("deny".to_owned()),
            asked: false,
        }
    );
    Ok(())
}

#[test]
fn a_failed_record_reads_its_kind_and_reason() -> TestResult {
    let (_, _, data) = golden_data("envelope/valid/judgement-recorded-failed.json")?;

    let record: JudgementRecorded = serde_json::from_str(&data)?;

    assert_eq!(
        (record.mode, record.answer),
        (
            Mode::Enforce,
            Answer::Failed {
                kind: "timed_out".to_owned(),
                reason: Some("timed out after 2 s".to_owned()),
            }
        )
    );
    Ok(())
}

#[test]
fn the_labelled_golden_reads_into_its_fields() -> TestResult {
    let (_, _, data) = golden_data("envelope/valid/judgement-labelled.json")?;

    let label: JudgementLabelled = serde_json::from_str(&data)?;

    assert_eq!(
        label,
        JudgementLabelled {
            producer: "bench".parse()?,
            judgement: "d1763287200003-41-0".parse()?,
            outcome: "no".parse()?,
            by: Some(LabelBy {
                actor: Some(LabelActor::Human),
                via: Some("mod".to_owned()),
            }),
        }
    );
    Ok(())
}

/// The wire shape of the `invalid-data` golden `name`, as `T` reads it.
fn invalid_data<T: serde::de::DeserializeOwned>(
    name: &str,
) -> Result<T, Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(fixture(&format!("envelope/invalid-data/{name}")))?;
    Ok(serde_json::from_str(&text)?)
}

#[test]
fn each_invalid_data_golden_is_refused_with_its_variant() -> TestResult {
    let recorded = |name: &str| -> Result<Option<RecordError>, Box<dyn std::error::Error>> {
        Ok(JudgementRecorded::try_from(invalid_data::<RecordedWire>(name)?).err())
    };

    let both = recorded("judgement-both-probability-and-failure.json")?;
    let both_with_options = recorded("judgement-both-with-options.json")?;
    let asked = recorded("judgement-asked-without-probability.json")?;
    let above = recorded("judgement-probability-above-one.json")?;
    let outcome = JudgementLabelled::try_from(invalid_data::<LabelledWire>(
        "labelled-outcome-bad-chars.json",
    )?)
    .err();

    assert_eq!(both, Some(RecordError::Answer { field: "failure" }));
    assert_eq!(
        both_with_options,
        Some(RecordError::Answer { field: "failure" })
    );
    assert_eq!(asked, Some(RecordError::Answer { field: "asked" }));
    assert!(
        matches!(
            above,
            Some(RecordError::Probability {
                field: "probability",
                ..
            })
        ),
        "{above:?}"
    );
    assert!(
        matches!(outcome, Some(RecordError::Outcome { .. })),
        "a space is not an outcome character: {outcome:?}"
    );
    Ok(())
}

#[test]
fn a_record_that_breaks_its_form_does_not_deserialize() -> TestResult {
    let text = std::fs::read_to_string(fixture(
        "envelope/invalid-data/judgement-both-probability-and-failure.json",
    ))?;

    let read = serde_json::from_str::<JudgementRecorded>(&text);

    let error = read.err().ok_or("refused")?;
    assert!(
        error
            .to_string()
            .starts_with(&RecordError::Answer { field: "failure" }.to_string()),
        "{error}"
    );
    Ok(())
}

#[test]
fn producers_are_plain_folder_names_and_hand_back_what_they_refuse() {
    let read = |text: &str| text.parse::<Producer>().map(Producer::into_string);

    let good = (read("bench"), read("rk-review2"));
    let bad = ["", "Bench", "-bench", "a/b", "a b"]
        .map(|text| Producer::try_from(text.to_owned()).map_err(ProducerError::into_string));

    assert_eq!(good, (Ok("bench".to_owned()), Ok("rk-review2".to_owned())));
    assert_eq!(
        bad,
        ["", "Bench", "-bench", "a/b", "a b"].map(|t| Err(t.to_owned()))
    );
}

#[test]
fn subjects_are_an_id_09_method_and_lowercase_hex() {
    let read = |text: &str| text.parse::<Subject>().map(|s| s.as_str().to_owned());

    let good = (read("sha256:9f86d0"), read("git-commit:0a"));
    let bad = [
        "md5:00",
        "sha256:",
        "sha256:ABCD",
        "sha256",
        ":00",
        "sha256:0g",
    ]
    .map(|text| Subject::try_from(text.to_owned()).map_err(SubjectError::into_string));

    assert_eq!(
        good,
        (
            Ok("sha256:9f86d0".to_owned()),
            Ok("git-commit:0a".to_owned())
        )
    );
    assert_eq!(
        bad,
        [
            "md5:00",
            "sha256:",
            "sha256:ABCD",
            "sha256",
            ":00",
            "sha256:0g"
        ]
        .map(|t| Err(t.to_owned()))
    );
}

#[test]
fn ids_and_outcomes_refuse_empty_long_and_bad_chars() {
    let longest = "a".repeat(64);
    assert!(
        longest
            .parse::<JudgementId>()
            .is_ok_and(|id| id.as_str() == longest)
    );
    assert_eq!(
        "d1-2_3.x:Y"
            .parse::<Outcome>()
            .map(|o| o.as_str().to_owned()),
        Ok("d1-2_3.x:Y".to_owned())
    );
    for bad in ["", &"a".repeat(65), "has space", "slash/", "é"] {
        assert!(bad.parse::<JudgementId>().is_err(), "id {bad:?}");
        assert!(bad.parse::<Outcome>().is_err(), "outcome {bad:?}");
        assert!(Vocab::try_from(bad.to_owned()).is_err(), "vocab {bad:?}");
    }
}

/// A record's `data`: a minimal one with the keys of `extra` set over it.
fn data_with(extra: Value) -> Value {
    let mut data = json!({
        "producer": "bench", "id": "d1", "point": "D1", "question": "noul",
        "model": "m", "mode": "enforce",
    });
    if let (Some(data), Value::Object(extra)) = (data.as_object_mut(), extra) {
        data.extend(extra);
    }
    data
}

/// Why `data` is not a record, or `None` when it is one.
fn refusal(data: Value) -> Option<RecordError> {
    let wire: RecordedWire = serde_json::from_value(data).expect("the wire shape");
    JudgementRecorded::try_from(wire).err()
}

#[test]
fn the_answer_is_given_or_failed_never_both() {
    assert_eq!(
        refusal(data_with(json!({"probability": 0.5, "asked": true}))),
        None
    );
    assert_eq!(
        refusal(data_with(json!({"failure": {"kind": "timed_out"}}))),
        None
    );
    assert_eq!(
        refusal(data_with(json!({}))),
        Some(RecordError::Answer {
            field: "probability"
        })
    );
    assert_eq!(
        refusal(data_with(json!({"probability": 0.5}))),
        Some(RecordError::Answer { field: "asked" })
    );
    for (key, value) in [
        ("options", json!({"yes": 0.5})),
        ("top", json!("x")),
        ("action", json!("x")),
    ] {
        let data = data_with(json!({"failure": {"kind": "k"}, key: value}));
        assert_eq!(
            refusal(data),
            Some(RecordError::Answer { field: key }),
            "{key}"
        );
    }
}

#[test]
fn a_producer_subject_option_or_point_out_of_form_is_refused_naming_its_field() {
    let cases = [
        (json!({"producer": "Bench"}), "producer"),
        (json!({"subject": "md5:00"}), "subject"),
        (json!({"subject": "sha256:"}), "subject"),
        (json!({"options": {"yes": 2.0}}), "options"),
        (json!({"point": "D 1"}), "point"),
        (json!({"id": ""}), "id"),
    ];
    for (extra, field) in cases {
        let mut data = data_with(json!({"probability": 0.5, "asked": false}));
        if let (Some(data), Value::Object(extra)) = (data.as_object_mut(), extra) {
            data.extend(extra);
        }
        let error = refusal(data).expect("refused");
        assert_eq!(error.field(), field, "{error:?}");
    }
}
