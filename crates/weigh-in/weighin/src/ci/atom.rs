//! The decision an accepted decision atom in `.kb` records, read from its front matter.

use std::borrow::Cow;
use weighin_core::ItemId;

/// The decision an accepted decision atom records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AtomDecision {
    /// The item, from `weighin_item`.
    pub(crate) id: ItemId,
    /// The chosen option. Decision atoms carry no option id yet, so this is always `None`, and
    /// the trailer or the journal gives the option (B-28, HITL-1).
    pub(crate) option: Option<String>,
}

/// The decision an accepted decision atom records, or `None` when `text` is not one. The front
/// matter is the lines between a first line `---` and the next line `---`, one leading byte
/// order mark and each line's `\r` passed over. It needs `kind: decision`, `status: accepted`
/// and a `weighin_item` that [`ItemId::parse`] accepts.
pub(crate) fn parse(text: &str) -> Option<AtomDecision> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.lines().map(|l| l.strip_suffix('\r').unwrap_or(l));
    if lines.next()? != "---" {
        return None;
    }
    let (mut kind, mut status, mut item) = (None, None, None);
    for line in lines.by_ref() {
        if line == "---" {
            return match (kind.as_deref(), status.as_deref(), item) {
                (Some("decision"), Some("accepted"), Some(id)) => {
                    Some(AtomDecision { id, option: None })
                }
                _ => None,
            };
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        match key {
            "kind" => kind = Some(scalar(value)),
            "status" => status = Some(scalar(value)),
            "weighin_item" => item = ItemId::parse(&scalar(value)),
            _ => {}
        }
    }
    // No closing `---`: not front matter.
    None
}

/// A front-matter scalar: a JSON string, as the atom writer quotes values, decoded; anything
/// else trimmed.
fn scalar(value: &str) -> Cow<'_, str> {
    let v = value.trim();
    if v.starts_with('"')
        && let Ok(s) = serde_json::from_str::<String>(v)
    {
        return Cow::Owned(s);
    }
    Cow::Borrowed(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The front matter of `weighin-core`'s golden decision atom, and the start of its body.
    const DECISION: &str = r#"---
id: "kb-decision-wi-4dfe38"
title: "Common-dir store"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Where should the ledger live?\", we decided for Common-dir store."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-09-26"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-4dfe38"
question: "Where should the ledger live?"
door: two-way
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-09-26T18:00:00Z"
tree_hash: "abc"
recommended: "A"
flip_condition: "others need open items"
---

# Common-dir store
"#;

    const OPEN_QUESTION: &str = r#"---
id: "kb-open-question-wi-4dfe38"
kind: open_question
status: proposed
tracks: "wi-4dfe38"
---
"#;

    #[test]
    fn a_decision_atom_gives_its_item_and_no_option() {
        let parsed = parse(DECISION);

        let expected = AtomDecision {
            id: ItemId::parse("wi-4dfe38").expect("a valid id"),
            option: None,
        };
        assert_eq!(parsed, Some(expected));
    }

    #[test]
    fn a_decision_atom_with_a_bom_and_crlf_lines_parses() {
        let text = format!("\u{feff}{}", DECISION.replace('\n', "\r\n"));

        let parsed = parse(&text).map(|d| d.id);

        assert_eq!(parsed, ItemId::parse("wi-4dfe38"));
    }

    #[test]
    fn atoms_that_record_no_accepted_decision_give_none() {
        let superseded = DECISION.replace("status: accepted", "status: superseded");
        let texts = [
            OPEN_QUESTION,
            superseded.as_str(),
            "# Notes\n\nweighin_item: \"wi-4dfe38\"\n",
            "---\nkind: decision\nstatus: accepted\nweighin_item: \"wi-XYZ\"\n---\n",
        ];

        let parsed = texts.map(parse);

        assert_eq!(parsed, [None, None, None, None]);
    }
}
