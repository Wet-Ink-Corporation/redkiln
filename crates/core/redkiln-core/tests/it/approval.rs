//! `core.approval_recorded`'s payload (SPEC AP-01), as a writer builds it. The SPEC's approval
//! goldens round-trip through it in `conformance::envelope`.

use redkiln_core::approval::{ApprovalRecorded, ApprovalSubject, Scope, Verdict};
use redkiln_core::envelope::{Actor, Author, Class, Envelope, Payload, Timestamp, Writer};
use redkiln_core::subject::Subject;
use redkiln_core::trust::Trust;

/// A plan approval as Bench records one (plan D3): repository scope, no gates, no cites.
fn plan_approval() -> ApprovalRecorded {
    let subject =
        Subject::parse("sha256:9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08")
            .expect("a subject");
    ApprovalRecorded {
        subject: ApprovalSubject::new("plan", &subject, "docs/plans/retention.md"),
        verdict: Verdict::Approved,
        scope: Some(Scope::Repository),
        gates: None,
        decider: "human:ryan".to_owned(),
        expires: None,
        request: None,
    }
}

#[test]
fn approval_recorded_is_core_s_durable_type_at_version_1() {
    assert_eq!(
        (
            ApprovalRecorded::TYPE,
            ApprovalRecorded::VERSION,
            ApprovalRecorded::CLASS
        ),
        ("core.approval_recorded", 1, Class::Durable)
    );
}

#[test]
fn absent_gates_expires_and_request_are_written_absent() {
    let data = serde_json::to_string(&plan_approval()).expect("serialises");
    assert_eq!(
        data,
        r#"{"subject":{"kind":"plan","method":"sha256","hash":"sha256:9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08","locator":"docs/plans/retention.md"},"verdict":"approved","scope":"repository","decider":"human:ryan"}"#
    );
}

#[test]
fn a_subject_takes_its_method_and_hash_from_one_core_subject() {
    let subject =
        Subject::parse("patch-id:3b18e512dba79e4c8300dd08aeb37f8e728b8dad").expect("a subject");
    let approval = ApprovalSubject::new("pull_request", &subject, "pr:42")
        .with_base("e83c5163316f89bfbde7d9ab23ca2e25604af290");
    assert_eq!(
        approval,
        ApprovalSubject {
            kind: "pull_request".to_owned(),
            method: "patch-id".to_owned(),
            hash: "patch-id:3b18e512dba79e4c8300dd08aeb37f8e728b8dad".to_owned(),
            base: Some("e83c5163316f89bfbde7d9ab23ca2e25604af290".to_owned()),
            locator: "pr:42".to_owned(),
        }
    );
}

#[test]
fn data_without_a_decider_does_not_read_as_an_approval() {
    let without = r#"{"subject":{"kind":"plan","method":"sha256","hash":"sha256:9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08","locator":"docs/plans/retention.md"},"verdict":"approved"}"#;
    let read = serde_json::from_str::<ApprovalRecorded>(without);
    assert!(
        read.is_err_and(|e| e.to_string().contains("missing field `decider`")),
        "AP-01 refuses a decider-less record"
    );
}

#[test]
fn an_approval_envelope_encodes_with_core_s_type() {
    let author = Author {
        actor: Actor::parse("human:ryan").expect("an actor"),
        trust: Trust::asserted(),
        writer: Writer::new("bench", "0.1.0").expect("a writer"),
    };
    let ts = Timestamp::parse("2026-11-02T10:04:00Z").expect("a ts");
    let encoded = Envelope::new(ts, author, plan_approval())
        .encode()
        .expect("encodes");
    let value: serde_json::Value = serde_json::from_slice(&encoded).expect("JSON");
    assert_eq!(
        (&value["type"], &value["version"], &value["data"]["decider"]),
        (
            &serde_json::json!("core.approval_recorded"),
            &serde_json::json!(1),
            &serde_json::json!("human:ryan")
        )
    );
}
