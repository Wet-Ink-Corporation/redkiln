//! The version-1 schemas froze with SPEC 1.0.0 (M2-G): a writer may rely on a `-1` schema never
//! changing under it. A change to one is a new `-2` file (EV-06, CLI-04), or an edit to this
//! table that a reviewer must justify.

use std::collections::BTreeMap;

use redkiln_core::ids::Digest;

use super::spec_dir;

/// Every `spec/schemas/*-1.schema.json` at the freeze, with the sha256 of its bytes.
const FROZEN: &[(&str, &str)] = &[
    (
        "cli-envelope-1.schema.json",
        "366d6e01e9c2869925ff1e9ac5e6eb4f769d5357a0865d75bbdd0e627ba5c3a3",
    ),
    (
        "core.approval_recorded-1.schema.json",
        "d120d5ed2dd1ef74c1b645fec658bae0f567b8a9b2078703a8186cbfb4eb78ff",
    ),
    (
        "core.import_completed-1.schema.json",
        "cae668c233719fa32e3356b99a684c27f1b9e2dd3eee1b8c4580b7078435a3ad",
    ),
    (
        "core.judgement_labelled-1.schema.json",
        "1d2a2711a7eaade2c600a489b30859c07beb9be7be02b894c003247edff76f88",
    ),
    (
        "core.judgement_recorded-1.schema.json",
        "4910b156917341a8c448ccc9e4cfc766f30aa9f2ab9642afc80ba28ff176bba3",
    ),
    (
        "decision-request-1.schema.json",
        "da984ce63d8893ce50f28a8002cd7bb5d0b88307969641ac7de40620d58936a8",
    ),
    (
        "envelope-1.schema.json",
        "800662c6302b3da25af3889e3ecf68571569dc2388967f1dcb73934d80749ee9",
    ),
    (
        "journal-line-1.schema.json",
        "548b1a5e274c756a21dbae8fc30f05db5439932204bde09653be7dcb802ea16b",
    ),
    (
        "status-1.schema.json",
        "ce58c10f76f35dc214d13c83acf6612864f0611cd88018415c243d219eb8331b",
    ),
];

/// Each `*-1.schema.json` file in `spec/schemas/`, by name, with the sha256 hex of its bytes.
fn on_disk() -> BTreeMap<String, String> {
    let dir = spec_dir().join("schemas");
    std::fs::read_dir(&dir)
        .expect("read spec/schemas")
        .map(|entry| entry.expect("an entry").path())
        .filter_map(|path| {
            let name = path.file_name()?.to_str()?.to_owned();
            name.ends_with("-1.schema.json").then_some((name, path))
        })
        .map(|(name, path)| {
            let bytes = std::fs::read(&path).expect("read the schema");
            (name, Digest::sha256(&bytes).hex().to_owned())
        })
        .collect()
}

#[test]
fn frozen_schemas_are_unchanged() {
    let frozen: BTreeMap<String, String> = FROZEN
        .iter()
        .map(|(name, hex)| ((*name).to_owned(), (*hex).to_owned()))
        .collect();
    assert_eq!(
        on_disk(),
        frozen,
        "every version-1 schema is in the table, at the bytes it froze with"
    );
}
