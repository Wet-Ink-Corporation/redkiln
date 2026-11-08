//! The decision request golden (`spec/fixtures/decision-request/`, DR-02). Its shape is the
//! schema's (`schema.rs`); here its subject is tied to Core's subjects (ID-09). DR-04's guards
//! are `weighin-contract`'s, which reads this golden when it arrives (Move 1 step 5).

use redkiln_core::subject::Subject;

use super::{one_line, value_of};

#[test]
fn subject_hash_is_of_its_method() {
    let doc = value_of(&one_line("decision-request/valid/dr-02.json"));
    let subject = &doc["subject"];
    let hash = subject["hash"].as_str().expect("a hash");
    let parsed = Subject::parse(hash).expect("a subject Core reads");
    assert_eq!(
        Some(parsed.method().as_str()),
        subject["method"].as_str(),
        "{hash}"
    );
}
