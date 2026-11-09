//! Core's ref states as `/2` violations and reads (pure).

use redkiln_core::journal::refs::{Breach, ChainBreak, Flaw, LocalSource, SourceKind};
use weighin_contract::ci_check::v2::{JournalKind, Reason, RefKind, RefRead, Violation};

use super::{passed_over, split};

const REF: &str = "refs/redkiln/journal/s1";

fn source(kind: SourceKind, breaks: Vec<ChainBreak>) -> LocalSource {
    LocalSource {
        refname: REF.to_owned(),
        kind,
        tip: "a".repeat(40),
        commits: 3,
        breaks,
    }
}

#[test]
fn each_flaw_maps_to_its_reason_naming_the_ref_and_commit() {
    let flaws = [
        (Flaw::Breach(Breach::Merge), Reason::Merge),
        (Flaw::Breach(Breach::Tree), Reason::Tree),
        (Flaw::Breach(Breach::Unended), Reason::Unended),
        (Flaw::Breach(Breach::TooLarge), Reason::TooLarge),
        (Flaw::Rewritten, Reason::Rewritten),
    ];
    let breaks = flaws
        .iter()
        .zip(1_u8..)
        .map(|((flaw, _), n)| ChainBreak {
            commit: n.to_string().repeat(40),
            flaw: *flaw,
        })
        .collect();

    let (_, got) = split(source(SourceKind::Journal, breaks)).expect("a small count");

    let expected: Vec<Violation> = flaws
        .iter()
        .zip(1_u8..)
        .map(|((_, reason), n)| Violation {
            journal: REF.to_owned(),
            kind: JournalKind::Ref,
            commit: Some(n.to_string().repeat(40)),
            line: None,
            reason: *reason,
        })
        .collect();
    assert_eq!(got, expected);
}

#[test]
fn a_passed_over_ref_is_one_unreadable_violation() {
    assert_eq!(
        passed_over(REF.to_owned()),
        Violation {
            journal: REF.to_owned(),
            kind: JournalKind::Ref,
            commit: None,
            line: None,
            reason: Reason::Unreadable,
        }
    );
}

#[test]
fn a_source_reads_as_its_kind_tip_and_commits() {
    let kinds = [
        (SourceKind::Journal, RefKind::Journal),
        (SourceKind::Inbox, RefKind::Inbox),
        (SourceKind::Broken, RefKind::Broken),
    ];
    for (kind, wire) in kinds {
        let (read, violations) = split(source(kind, Vec::new())).expect("a small count");
        assert_eq!(
            read,
            RefRead {
                refname: REF.to_owned(),
                kind: wire,
                tip: "a".repeat(40),
                commits: 3,
            }
        );
        assert!(violations.is_empty(), "{violations:?}");
    }
}
