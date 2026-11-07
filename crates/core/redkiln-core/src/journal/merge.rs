//! Every write to a journal ref is a merge (SPEC JR-07): the ref's current file, then the
//! incoming journal's complete lines that read as envelopes with an `origin` the file does not
//! hold, each `origin` once, first occurrence winning, in the incoming order. A merge only ever
//! appends, so it keeps JR-05 by construction whoever else wrote in between.

use std::collections::HashSet;

use happenstance_core::EventId;

use super::line::{Held, held, lines};

/// The result of a [`merge`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub(crate) struct Merged {
    /// The new file: the current one with the added lines after it.
    pub(crate) bytes: Vec<u8>,
    /// How many lines were added; `0` means nothing moves.
    pub(crate) added: u64,
}

/// The origins `file`'s lines hold, blank lines and lines without one left out.
pub(crate) fn origins(file: &[u8]) -> HashSet<EventId> {
    lines(file)
        .filter_map(|line| match held(line.text) {
            Held::Origin(id) => Some(id),
            Held::NoOrigin | Held::Torn => None,
        })
        .collect()
}

/// `current` followed by every complete line of `incoming` whose `origin` neither `current`
/// nor an earlier incoming line holds. `current` is kept byte for byte; each added line is
/// written without a `\r` and ended by `\n`.
pub(crate) fn merge(current: &[u8], incoming: &[u8]) -> Merged {
    let mut held_already = origins(current);
    let mut bytes = current.to_vec();
    let mut added = 0_u64;
    for line in lines(incoming).filter(|line| line.complete) {
        if let Held::Origin(id) = held(line.text)
            && held_already.insert(id)
        {
            bytes.extend_from_slice(line.text);
            bytes.push(b'\n');
            added = added.saturating_add(1);
        }
    }
    Merged { bytes, added }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(position: u32, item: &str) -> String {
        format!(
            r#"{{"type":"weighin.item_raised","version":1,"ts":"2026-10-31T04:12:40Z","actor":"agent","trust":{{"level":"asserted"}},"writer":{{"binary":"weighin","version":"2.0.0","spec":"1.0.0"}},"origin":"0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f:{position}","data":{{"item":"{item}"}}}}"#
        )
    }

    fn journal(lines: &[String]) -> Vec<u8> {
        lines
            .iter()
            .flat_map(|l| format!("{l}\n").into_bytes())
            .collect()
    }

    #[test]
    fn new_origins_are_appended_in_incoming_order_and_known_ones_skipped() {
        let current = journal(&[line(1, "a"), line(2, "b")]);
        let incoming = journal(&[line(3, "c"), line(2, "b"), line(4, "d")]);
        let merged = merge(&current, &incoming);
        assert_eq!(merged.added, 2);
        assert_eq!(
            merged.bytes,
            journal(&[line(1, "a"), line(2, "b"), line(3, "c"), line(4, "d")])
        );
    }

    #[test]
    fn the_first_occurrence_of_an_origin_wins() {
        let incoming = journal(&[line(1, "first"), line(1, "second")]);
        let merged = merge(b"", &incoming);
        assert_eq!(merged.added, 1);
        assert_eq!(merged.bytes, journal(&[line(1, "first")]));
    }

    #[test]
    fn a_merge_that_adds_nothing_keeps_the_file() {
        let current = journal(&[line(1, "a")]);
        let merged = merge(&current, &current);
        assert_eq!(merged.added, 0);
        assert_eq!(merged.bytes, current);
    }

    #[test]
    fn only_complete_envelope_lines_with_an_origin_are_taken() {
        let no_origin =
            line(9, "x").replace(r#""origin":"0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f:9","#, "");
        let incoming = format!(
            "{}\r\n\n{no_origin}\nnot json\n{}",
            line(1, "a"),
            line(2, "torn-but-whole")
        );
        let merged = merge(b"", incoming.as_bytes());
        assert_eq!(merged.added, 1, "the unended last line is not complete");
        assert_eq!(merged.bytes, journal(&[line(1, "a")]), "the \\r is dropped");
    }

    #[test]
    fn the_current_file_is_kept_byte_for_byte() {
        let current = format!("{}\r\n\n", line(1, "a")).into_bytes();
        let merged = merge(&current, &journal(&[line(1, "a"), line(2, "b")]));
        assert_eq!(merged.added, 1);
        assert!(merged.bytes.starts_with(&current));
    }
}
