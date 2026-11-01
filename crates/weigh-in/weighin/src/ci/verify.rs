//! The append-only check on committed journals: every complete line a journal held at the base
//! revision is still there at head.
//!
//! Membership is a set, not a multiset: one session id can write the same journal in two
//! branches, and the hook script appends a whole-ledger dump to it, so one event can rightly
//! appear twice.

use serde::Serialize;
use std::collections::{HashMap, HashSet};
use weighin_core::EventEnvelope;

use crate::journal::lines;

/// Why a journal breaks the append-only rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Reason {
    /// The file is gone at head.
    Removed,
    /// A complete base line's event is nowhere in the head file.
    Missing,
    /// As `Missing`, and the head line at that number holds an event the base file did not.
    Altered,
    /// A complete head line that is not an event.
    Undecodable,
}

/// One break of the append-only rule.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(crate) struct Violation {
    /// The journal, relative to the worktree root.
    pub(crate) path: String,
    /// The 1-based line: the base line for `Missing` and `Altered`, the head line for
    /// `Undecodable`, and none for `Removed`.
    pub(crate) line: Option<u64>,
    /// What is wrong.
    pub(crate) reason: Reason,
}

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
        path: path.to_owned(),
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
            Ok(e) => events.push((l.number, e)),
            // A cut-short last line is a write the next run repeats, not a broken journal.
            Err(_) if !l.complete => {}
            Err(_) => violations.push(Violation {
                path: path.to_owned(),
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
            path: PATH.to_owned(),
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
}
