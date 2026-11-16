//! The end-of-turn cases (`spec/fixtures/turn/`): Core reads each message as its reading says
//! (ET-02 to ET-07). A reading's `blocks` is Weigh-In's to check (ET-08), in `weighin-core`'s
//! `turn_blocks` tests.

use redkiln_core::turn::{Section, read};
use serde_json::{Map, Value, json};

use super::{golden, value_of};

/// Every case, by stem: `<case>.md` is the message, `<case>.reading.json` its reading.
const CASES: &[&str] = &[
    "et-10-example",
    "temper-fenced-done",
    "bench-emphasis-list",
    "heading-and-quote",
    "wip-with-reason",
    "blocked-lowercase",
    "unknown-word",
    "no-status",
    "last-line-wins",
    "prose-last",
    "continuations",
    "declined-ends-decisions-needed",
    "underscore-emphasis-not-read",
    "block-after-report",
    "block-in-example-fence",
    "unclosed-fence-then-block",
    "indented-four-not-a-fence",
    "indented-weigh-in-block",
    "unclosed-weigh-in-block",
    "closer-at-any-indent",
    "closer-with-text-is-content",
];

/// The files of every case, for `every_fixture_and_schema_is_listed`.
pub(super) fn listed() -> impl Iterator<Item = String> {
    CASES.iter().flat_map(|case| {
        [
            format!("fixtures/turn/{case}.md"),
            format!("fixtures/turn/{case}.reading.json"),
        ]
    })
}

/// What Core reads in `message`, in the reading's form without `blocks`: `status` the known
/// word in lower case, `word` the word Core keeps (the SPEC's for a known one, as written for
/// an unknown one), and the sections present.
fn reading_of(message: &str) -> Value {
    let Some(report) = read(message) else {
        return json!({"status": null, "word": null, "sections": {}});
    };
    let (status, word) = match report.status() {
        Ok(known) => (
            Value::from(known.word().to_ascii_lowercase()),
            known.word().to_owned(),
        ),
        Err(unknown) => (Value::Null, unknown.word().to_owned()),
    };
    let sections: Map<String, Value> = Section::ALL
        .into_iter()
        .filter_map(|section| {
            report
                .section(section)
                .map(|text| (section.key().to_owned(), Value::from(text)))
        })
        .collect();
    json!({"status": status, "word": word, "sections": sections})
}

#[test]
fn every_turn_case_reads_as_its_reading() {
    for case in CASES {
        let message = String::from_utf8(golden(&format!("turn/{case}.md"))).expect("UTF-8");
        let mut expected = value_of(&golden(&format!("turn/{case}.reading.json")));
        let blocks = expected
            .as_object_mut()
            .and_then(|reading| reading.remove("blocks"));
        assert!(
            blocks.as_ref().is_some_and(Value::is_array),
            "{case}: the reading lists its blocks"
        );

        assert_eq!(reading_of(&message), expected, "{case}");
    }
}
