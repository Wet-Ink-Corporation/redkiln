//! The append-only check on committed journals: every complete line a journal held at the base
//! revision is still there at head.
//!
//! Membership is a set, not a multiset: one session id can write the same journal in two
//! branches, and the hook script appends a whole-ledger dump to it, so one event can rightly
//! appear twice.

use std::collections::{HashMap, HashSet};
use weighin_contract::ci_check::v2::{JournalKind, Reason, Violation};
use weighin_core::{EventEnvelope, LedgerEvent};

use crate::journal::lines;

/// The comparison key of a journal line: the event re-encoded when the line decodes, else the
/// raw bytes. Two lines of one event compare equal whatever their key order or spacing.
pub(crate) fn key(text: &[u8]) -> Vec<u8> {
    EventEnvelope::from_json(text)
        .ok()
        .and_then(|e| e.to_json().ok())
        .unwrap_or_else(|| text.to_vec())
}

/// The keys of the complete, non-blank lines of `bytes`, by line number.
fn complete_keys(bytes: &[u8]) -> Vec<(u64, Vec<u8>)> {
    lines(bytes)
        .filter(|l| l.complete && !l.is_blank())
        .map(|l| (l.number, key(l.text)))
        .collect()
}

/// Every complete, non-blank line of `base` must have its key among the keys of head's
/// complete lines, wherever it now stands. An unterminated last base line is not checked (the
/// writer drops and rewrites a cut-short line), and neither are lines head added. `head` of
/// `None`, the file gone, is one `Removed` violation when base has a line to keep.
pub(crate) fn verify(path: &str, base: &[u8], head: Option<&[u8]>) -> Vec<Violation> {
    let base_keys = complete_keys(base);
    if base_keys.is_empty() {
        return Vec::new();
    }
    let violation = |line, reason| Violation {
        journal: path.to_owned(),
        kind: JournalKind::File,
        commit: None,
        line,
        reason,
    };
    let Some(head) = head else {
        return vec![violation(None, Reason::Removed)];
    };
    let head_keys: HashMap<u64, Vec<u8>> = complete_keys(head).into_iter().collect();
    let in_head: HashSet<&[u8]> = head_keys.values().map(Vec::as_slice).collect();
    let in_base: HashSet<&[u8]> = base_keys.iter().map(|(_, k)| k.as_slice()).collect();
    base_keys
        .iter()
        .filter(|(_, k)| !in_head.contains(k.as_slice()))
        .map(|(n, _)| {
            let replaced = head_keys
                .get(n)
                .is_some_and(|k| !in_base.contains(k.as_slice()));
            let reason = if replaced {
                Reason::Altered
            } else {
                Reason::Missing
            };
            violation(Some(*n), reason)
        })
        .collect()
}

/// The events of a head journal, with their line numbers, and an `Undecodable` violation for
/// each complete, non-blank line that is not an event. A cut-short last line is passed over, as
/// `journal::read` passes it over.
pub(crate) fn decode(path: &str, bytes: &[u8]) -> (Vec<(u64, EventEnvelope)>, Vec<Violation>) {
    let mut events = Vec::new();
    let mut violations = Vec::new();
    for l in lines(bytes).filter(|l| !l.is_blank()) {
        match EventEnvelope::from_json(l.text) {
            // No 1.x journal ever held a decision request: such a line names an item with no
            // check that the request minted it (ID-04), so it is no 1.x event.
            Ok(e) if !matches!(e.event, LedgerEvent::RequestRaised(_)) => {
                events.push((l.number, e));
            }
            // A cut-short last line is a write the next run repeats, not a broken journal.
            Ok(_) | Err(_) if !l.complete => {}
            Ok(_) | Err(_) => violations.push(Violation {
                journal: path.to_owned(),
                kind: JournalKind::File,
                commit: None,
                line: Some(l.number),
                reason: Reason::Undecodable,
            }),
        }
    }
    (events, violations)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RAISED: &str = r#"{"type":"item_raised","item_id":"wi-b29e54","kind":"question","question":"Keep the cache warm or cold?","door_hint":"unsure","authority_hint":"authority","channel":"manual","quote":"Keep the cache warm or cold?","actor":"cli","session_id":"s","ts":"2026-10-09T03:29:36Z"}"#;
    const PRESENTED: &str = r#"{"type":"item_presented","item_id":"wi-b29e54","surface":"chat","reason":"","actor":"cli","session_id":"s","ts":"2026-10-09T03:29:37Z"}"#;
    const ON_BOARD: &str = r#"{"type":"item_presented","item_id":"wi-b29e54","surface":"board","reason":"","actor":"cli","session_id":"s","ts":"2026-10-09T03:29:38Z"}"#;
    const PATH: &str = ".kb/_ledger/journal/s.jsonl";

    fn violation(line: Option<u64>, reason: Reason) -> Violation {
        Violation {
            journal: PATH.to_owned(),
            kind: JournalKind::File,
            commit: None,
            line,
            reason,
        }
    }

    #[test]
    fn one_event_with_its_keys_reordered_has_one_key() {
        let reordered = r#"{"actor":"cli","type":"item_presented","surface":"chat","item_id":"wi-b29e54","reason":"","session_id":"s","ts":"2026-10-09T03:29:37Z"}"#;

        assert_eq!(key(reordered.as_bytes()), key(PRESENTED.as_bytes()));
        assert_ne!(
            key(b"not an event"),
            key(PRESENTED.as_bytes()),
            "raw bytes differ"
        );
    }

    #[test]
    fn a_base_that_head_extends_has_no_violation() {
        let base = format!("{RAISED}\n");
        let head = format!("{RAISED}\n{PRESENTED}\n");

        let found = verify(PATH, base.as_bytes(), Some(head.as_bytes()));

        assert_eq!(found, Vec::new());
    }

    #[test]
    fn an_unterminated_last_base_line_need_not_survive() {
        let cut = PRESENTED.get(..40).expect("a long line");
        let base = format!("{RAISED}\n{cut}");
        let head = format!("{RAISED}\n{ON_BOARD}\n");

        let found = verify(PATH, base.as_bytes(), Some(head.as_bytes()));

        assert_eq!(found, Vec::new());
    }

    #[test]
    fn a_base_line_gone_from_head_is_missing_at_its_number() {
        let base = format!("{RAISED}\n{PRESENTED}\n");
        let head = format!("{RAISED}\n");

        let found = verify(PATH, base.as_bytes(), Some(head.as_bytes()));

        assert_eq!(found, vec![violation(Some(2), Reason::Missing)]);
    }

    #[test]
    fn a_base_line_replaced_in_place_is_altered() {
        let base = format!("{RAISED}\n{PRESENTED}\n");
        let head = format!("{RAISED}\n{ON_BOARD}\n");

        let found = verify(PATH, base.as_bytes(), Some(head.as_bytes()));

        assert_eq!(found, vec![violation(Some(2), Reason::Altered)]);
    }

    #[test]
    fn a_journal_gone_at_head_is_one_removed_violation() {
        let base = format!("{RAISED}\n");

        let found = verify(PATH, base.as_bytes(), None);

        assert_eq!(found, vec![violation(None, Reason::Removed)]);
    }

    #[test]
    fn a_base_event_moved_to_another_line_has_no_violation() {
        let base = format!("{RAISED}\n{PRESENTED}\n");
        let head = format!("{PRESENTED}\n{ON_BOARD}\n{RAISED}\n");

        let found = verify(PATH, base.as_bytes(), Some(head.as_bytes()));

        assert_eq!(found, Vec::new());
    }

    #[test]
    fn decode_flags_a_bad_complete_line_and_skips_a_cut_short_last_line() {
        let cut = PRESENTED.get(..40).expect("a long line");
        let head = format!("{RAISED}\nnot an event\n\n{cut}");

        let (events, found) = decode(PATH, head.as_bytes());

        let raised = EventEnvelope::from_json(RAISED.as_bytes()).expect("an event");
        assert_eq!(events, vec![(1, raised)]);
        assert_eq!(found, vec![violation(Some(2), Reason::Undecodable)]);
    }

    /// A decision request written as a 1.x line, naming an item it was never minted for.
    const REQUESTED: &str = r#"{"type":"request_raised","item_id":"wi-b29e54","request":{"question":"Take it over?","options":[{"id":"a","label":"Yes","holds_when":"Always","cost_if_wrong":"None"},{"id":"b","label":"No","holds_when":"Never","cost_if_wrong":"None"}],"recommendation":"a","door":"two-way","authority":"judgment","blocks":"none","trust_needed":"witnessed","raiser":{"namespace":"bench","key":"forged"}},"actor":"cli","session_id":"s","ts":"2026-10-09T03:29:37Z"}"#;

    #[test]
    fn a_request_raised_line_in_a_committed_journal_is_undecodable() {
        let head = format!("{RAISED}\n{REQUESTED}\n");

        let (events, found) = decode(PATH, head.as_bytes());

        let raised = EventEnvelope::from_json(RAISED.as_bytes()).expect("an event");
        assert_eq!(events, vec![(1, raised)]);
        assert_eq!(found, vec![violation(Some(2), Reason::Undecodable)]);
    }
}
