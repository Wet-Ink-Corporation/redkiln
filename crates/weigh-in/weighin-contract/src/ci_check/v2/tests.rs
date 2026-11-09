//! `/2` against its goldens: what weighin writes reads back, and a reader refuses what it is not.

use super::*;

const FULL: &str = include_str!("../../../fixtures/ci-check/v2-full.json");
const NOTHING_READ: &str = include_str!("../../../fixtures/ci-check/v2-nothing-read.json");

/// The bytes of a golden's `data`, as written: between `"data":` and `,"findings":`.
fn data_bytes(golden: &str) -> &str {
    let (_, data) = golden.split_once("\"data\":").expect("a data key");
    let (data, _) = data.rsplit_once(",\"findings\":").expect("a findings key");
    data
}

#[test]
fn the_full_golden_reads_and_its_data_writes_back_byte_for_byte() {
    let data = Data::from_envelope(FULL.trim_end()).expect("the golden reads");

    let written = serde_json::to_string(&data).expect("writes");

    assert_eq!(written, data_bytes(FULL));
    assert_eq!(data.journal.violations.len(), 2);
    assert_eq!(data.read.refs.len(), 3);
    assert_eq!(data.unreadable.get("weighin.item_raised"), Some(&2));
}

#[test]
fn the_nothing_read_golden_has_empty_files_and_refs() {
    let data = Data::from_envelope(NOTHING_READ).expect("the golden reads");

    assert!(data.read.files.is_empty(), "{data:?}");
    assert!(data.read.refs.is_empty(), "{data:?}");
    assert!(data.journal.ok);
    assert_eq!(
        serde_json::to_string(&data).expect("writes"),
        data_bytes(NOTHING_READ)
    );
}

#[test]
fn another_schema_is_refused_naming_it() {
    let later = FULL.replacen(SCHEMA, "weighin.ci-check/3", 1);
    let err = Data::from_envelope(&later).expect_err("a /3 envelope is refused");
    assert!(
        matches!(&err, ReadError::Schema { found } if found == "weighin.ci-check/3"),
        "{err:?}"
    );
    assert_eq!(
        err.to_string(),
        "the report is weighin.ci-check/3, not weighin.ci-check/2"
    );

    let flat = r#"{"schema":"weighin.ci-check/1","base":"b","head":"h","open_items":[],"decisions":[],"journal":{"ok":true,"violations":[]}}"#;
    let err = Data::from_envelope(flat).expect_err("a flat /1 report is refused");
    assert!(matches!(err, ReadError::Shape(_)), "{err:?}");
}

#[test]
fn another_command_is_refused() {
    let other = FULL.replacen("\"command\":\"ci-check\"", "\"command\":\"status\"", 1);

    let err = Data::from_envelope(&other).expect_err("another command is refused");

    assert!(
        matches!(&err, ReadError::Command { found } if found == "status"),
        "{err:?}"
    );
}

#[test]
fn a_failed_envelope_is_refused_as_failed() {
    let failed = r#"{"schema":"weighin.ci-check/2","ok":false,"command":"ci-check","data":{},"findings":[{"key":"internal","message":"resolving --base nope","severity":"error"}]}"#;

    let err = Data::from_envelope(failed).expect_err("a failed run is refused");

    assert!(matches!(err, ReadError::Failed), "{err:?}");
}

#[test]
fn an_unknown_key_in_data_or_an_open_item_is_accepted() {
    let added = FULL
        .replacen("\"base\":", "\"later\":1,\"base\":", 1)
        .replacen(
            "\"question\":\"Q?\"",
            "\"question\":\"Q?\",\"source\":\"ref\"",
            1,
        );

    let data = Data::from_envelope(&added).expect("an additive /2 field reads (CLI-04)");

    assert_eq!(data.open_items.len(), 2);
}

#[test]
fn from_envelope_data_reads_the_data_alone() {
    let envelope: serde_json::Value = serde_json::from_str(FULL).expect("json");

    let data = Data::from_envelope_data(&envelope["data"]).expect("the data reads");

    assert_eq!(
        serde_json::to_string(&data).expect("writes"),
        data_bytes(FULL)
    );
    let err = Data::from_envelope_data(&serde_json::json!({})).expect_err("empty data");
    assert!(matches!(err, ReadError::Shape(_)), "{err:?}");
}

#[test]
fn each_reason_and_kind_has_its_wire_name() {
    let reasons = [
        (Reason::Removed, "removed"),
        (Reason::Missing, "missing"),
        (Reason::Altered, "altered"),
        (Reason::Undecodable, "undecodable"),
        (Reason::Merge, "merge"),
        (Reason::Tree, "tree"),
        (Reason::Unended, "unended"),
        (Reason::TooLarge, "too-large"),
        (Reason::Rewritten, "rewritten"),
        (Reason::Unreadable, "unreadable"),
    ];
    for (reason, name) in reasons {
        assert_eq!(wire(&reason), format!("\"{name}\""));
    }
    for (kind, name) in [(JournalKind::File, "file"), (JournalKind::Ref, "ref")] {
        assert_eq!(wire(&kind), format!("\"{name}\""));
    }
    let kinds = [
        (RefKind::Journal, "journal"),
        (RefKind::Inbox, "inbox"),
        (RefKind::Broken, "broken"),
    ];
    for (kind, name) in kinds {
        assert_eq!(wire(&kind), format!("\"{name}\""));
    }
}

/// A value's JSON.
fn wire(value: &impl Serialize) -> String {
    serde_json::to_string(value).expect("an enum writes")
}

#[test]
fn the_v1_reader_refuses_a_v2_envelope() {
    let err = super::super::Report::from_json(FULL).expect_err("/1 does not read /2");

    assert!(matches!(err, super::super::ReadError::Shape(_)), "{err:?}");
}
