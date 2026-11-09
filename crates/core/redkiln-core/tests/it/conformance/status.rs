//! `status --json` goldens (`spec/fixtures/status/`, CLI-12): each is a CLI envelope whose
//! `data` passes `status-1`, and whose output id is `<namespace>.status/1` for a registered
//! namespace. The invalid `data` files are in `schema.rs`'s table, with where each breaks
//! `status-1`.

use redkiln_core::envelope::Namespace;

use super::schema::assert_valid;
use super::{one_line, value_of};

const VALID: [&str; 2] = [
    "status/valid/weighin-ok.json",
    "status/valid/weighin-off.json",
];

#[test]
fn each_golden_passes_the_envelope_and_status_schemas() {
    for rel in VALID {
        let doc = value_of(&one_line(rel));
        assert_valid("cli-envelope-1", &doc, rel);
        assert_valid("status-1", &doc["data"], rel);
        let id = doc["schema"].as_str().expect("an output id");
        let (namespace, rest) = id.split_once('.').expect("<namespace>.<command>/<n>");
        assert!(namespace.parse::<Namespace>().is_ok(), "{rel}: {id}");
        assert_eq!(rest, "status/1", "{rel}: CLI-12's output id");
        assert_eq!(doc["command"], "status", "{rel}");
    }
}
