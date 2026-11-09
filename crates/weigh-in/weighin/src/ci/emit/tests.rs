//! The envelope's findings and the text form (pure).

use std::collections::BTreeMap;

use weighin_contract::ci_check::Blocks;
use weighin_contract::ci_check::v2::{
    Data, JournalCheck, JournalKind, OpenItem, Read, Reason, RefKind, RefRead, Violation,
};
use weighin_contract::id::ItemId;
use weighin_contract::open::Status;
use weighin_contract::vocab::Kind;

use super::{findings, render_text, write_envelope};
use crate::cli_envelope::{Finding, Severity};

const FILE: &str = ".kb/_ledger/journal/s.jsonl";
const REF: &str = "refs/redkiln/journal/s1";

fn file_violation() -> Violation {
    Violation {
        journal: FILE.to_owned(),
        kind: JournalKind::File,
        commit: None,
        line: Some(2),
        reason: Reason::Missing,
    }
}

fn ref_violation() -> Violation {
    Violation {
        journal: REF.to_owned(),
        kind: JournalKind::Ref,
        commit: Some("c".repeat(40)),
        line: None,
        reason: Reason::Rewritten,
    }
}

fn ref_read() -> RefRead {
    RefRead {
        refname: REF.to_owned(),
        kind: RefKind::Journal,
        tip: "a".repeat(40),
        commits: 2,
    }
}

fn data(violations: Vec<Violation>, refs: Vec<RefRead>) -> Data {
    Data {
        base: "abcdef0123".to_owned(),
        head: "0123456789".to_owned(),
        open_items: Vec::new(),
        decisions: Vec::new(),
        journal: JournalCheck {
            ok: violations.is_empty(),
            violations,
        },
        read: Read {
            files: vec![FILE.to_owned()],
            refs,
        },
        unreadable: BTreeMap::new(),
    }
}

#[test]
fn each_violation_is_an_error_finding_with_a_stable_key() {
    let got = findings(&data(
        vec![file_violation(), ref_violation()],
        vec![ref_read()],
    ));

    let c = "c".repeat(40);
    let expected = vec![
        Finding::new(
            format!("ci-check/missing:{FILE}:2"),
            &format!("{FILE}:2 missing"),
            Severity::Error,
        ),
        Finding::new(
            format!("ci-check/rewritten:{REF}@{c}"),
            &format!("{REF}@ccccccc:- rewritten"),
            Severity::Error,
        ),
    ];
    assert_eq!(got, expected);
}

#[test]
fn no_refs_read_is_one_warning_finding() {
    let got = findings(&data(Vec::new(), Vec::new()));

    assert_eq!(
        got,
        vec![Finding::new(
            "ci-check/no-journal-refs".to_owned(),
            "no journal refs in this clone; fetch +refs/redkiln/journal/*:refs/redkiln/journal/* to read them",
            Severity::Warning,
        )]
    );
}

#[test]
fn the_envelope_of_each_golden_s_data_is_the_golden_byte_for_byte()
-> Result<(), Box<dyn std::error::Error>> {
    let goldens = [
        include_str!("../../../../weighin-contract/fixtures/ci-check/v2-full.json"),
        include_str!("../../../../weighin-contract/fixtures/ci-check/v2-nothing-read.json"),
    ];
    for golden in goldens {
        let data = Data::from_envelope(golden)?;
        let mut out = Vec::new();

        write_envelope(&data, &mut out)?;

        assert_eq!(String::from_utf8_lossy(&out), golden);
    }
    Ok(())
}

#[test]
fn refs_that_exist_but_were_all_passed_over_are_no_no_refs_warning() {
    let passed_over = Violation {
        journal: REF.to_owned(),
        kind: JournalKind::Ref,
        commit: None,
        line: None,
        reason: Reason::Unreadable,
    };

    let got = findings(&data(vec![passed_over], Vec::new()));

    assert_eq!(
        got,
        vec![Finding::new(
            format!("ci-check/unreadable:{REF}"),
            &format!("{REF}:- unreadable"),
            Severity::Error,
        )]
    );
}

#[test]
fn each_unreadable_type_is_one_warning_finding() {
    let mut data = data(Vec::new(), vec![ref_read()]);
    data.unreadable = [
        ("weighin.item_decided".to_owned(), 1),
        ("weighin.item_raised".to_owned(), 3),
    ]
    .into();

    let got = findings(&data);

    let warning = |ty: &str, n: u64| {
        Finding::new(
            format!("ci-check/unreadable:{ty}"),
            &format!("{n} {ty} event(s) this build cannot read were skipped (EV-10)"),
            Severity::Warning,
        )
    };
    assert_eq!(
        got,
        vec![
            warning("weighin.item_decided", 1),
            warning("weighin.item_raised", 3)
        ]
    );
}

#[test]
fn the_text_form_counts_the_refs_and_names_a_ref_violation_with_its_commit() -> std::io::Result<()>
{
    let mut data = data(vec![file_violation(), ref_violation()], vec![ref_read()]);
    data.open_items = vec![OpenItem {
        id: ItemId::parse("wi-0000a1").expect("an id"),
        status: Status::Raised,
        kind: Kind::Question,
        door: None,
        authority: None,
        blocks: Blocks::Merge,
        resolve_by: Some("before-merge".to_owned()),
        question: "Q?".to_owned(),
    }];
    let mut out = Vec::new();

    render_text(&data, &mut out)?;

    assert_eq!(
        String::from_utf8_lossy(&out),
        format!(
            "ci-check abcdef0..0123456: 1 open (1 block merge, 0 block release, 0 unknown) · 0 decision(s) · journal: 2 violation(s) · 1 journal ref(s)\n  {FILE}:2 missing\n  {REF}@ccccccc:- rewritten\n"
        )
    );
    Ok(())
}

/// Moved from `report.rs` with `render_text` (#48): the summary line gains the refs count.
#[test]
fn the_text_form_is_one_summary_line_and_one_line_per_violation() -> std::io::Result<()> {
    use super::super::report::{Attested, Inputs, Source, build};
    use weighin_core::{EventEnvelope, Ledger};

    let raised = |item: &str| {
        format!(
            r#"{{"type":"item_raised","item_id":"{item}","kind":"question","question":"Question {item}?","door_hint":"unsure","authority_hint":"authority","channel":"manual","quote":"q","actor":"cli","session_id":"s","ts":"2026-10-09T03:29:36Z"}}"#
        )
    };
    let triaged = r#"{"type":"item_triaged","item_id":"wi-0000a1","door":"one-way","blast":"file","authority":"judgment","route":"present","resolve_by":"before-merge","reason":"r","actor":"cli","session_id":"s","ts":"2026-10-09T03:29:37Z"}"#.to_owned();
    let lines = [raised("wi-0000a1"), triaged, raised("wi-0000b2")];
    let events: Vec<EventEnvelope> = lines
        .iter()
        .map(|l| EventEnvelope::from_json(l.as_bytes()).expect("a test event"))
        .collect();
    let l = Ledger::fold((1_u64..).zip(events.iter()));
    let none = BTreeMap::new();
    let attested = [Attested {
        id: ItemId::parse("wi-0000c3").expect("an id"),
        option: Some("A".to_owned()),
        source: Source::Trailer,
        commit: Some("t".to_owned()),
    }];
    let violations = vec![Violation {
        journal: "j.jsonl".to_owned(),
        kind: JournalKind::File,
        commit: None,
        line: Some(2),
        reason: Reason::Missing,
    }];
    let r = build(Inputs {
        base: "abcdef0123",
        head: "0123456789",
        ledger: &l,
        closing: &none,
        reopens: &none,
        attested: &attested,
        violations,
        read: Read {
            files: Vec::new(),
            refs: Vec::new(),
        },
        unreadable: BTreeMap::new(),
    });
    let mut out = Vec::new();

    render_text(&r, &mut out)?;

    assert_eq!(
        String::from_utf8_lossy(&out),
        "ci-check abcdef0..0123456: 2 open (1 block merge, 0 block release, 1 unknown) · 1 decision(s) · journal: 1 violation(s) · 0 journal ref(s)\n  j.jsonl:2 missing\n"
    );
    Ok(())
}
