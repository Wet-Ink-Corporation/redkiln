//! The frontmatter writer: the ten required keys in their order, the decision-only keys, then
//! the producer's lineage keys.

use std::fmt::{self, Display, Formatter};

use crate::id::AtomId;
use crate::yaml::{yaml_list, yaml_str};

/// Where an atom stands in its lifecycle (`references/docs/knowledge.md`, States).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Written by an agent or a tool, not yet accepted.
    Proposed,
    /// In force and injectable as binding.
    Accepted,
    /// Replaced in full by the atom named in `superseded_by`.
    Superseded,
    /// Withdrawn with no successor.
    Withdrawn,
}

impl Status {
    /// The status as written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Accepted => "accepted",
            Self::Superseded => "superseded",
            Self::Withdrawn => "withdrawn",
        }
    }
}

/// How binding an atom is: `authority_tier`. Readers accept any non-empty value, as the
/// prototype's schema did; writers use these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorityTier {
    /// A decision someone with authority took.
    Decision,
    /// A note: tracked, not binding.
    Note,
}

impl AuthorityTier {
    /// The tier as written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Decision => "decision",
            Self::Note => "note",
        }
    }
}

/// How easily a decision is undone: a one-way door is `low`, unsure `medium`, two-way `high`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reversibility {
    /// Hard to undo.
    Low,
    /// Unsure.
    Medium,
    /// Easy to undo.
    High,
}

impl Reversibility {
    /// The reversibility as written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

/// A lineage value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value<'a> {
    /// Any text, written as a quoted scalar ([`crate::yaml_str`]).
    Str(&'a str),
    /// A producer's fixed word written bare (`two-way`, `user-approved`). Only constants: a
    /// word must be a plain YAML scalar, which arbitrary text is not.
    Word(&'static str),
    /// A list of texts, written as a flow sequence ([`crate::yaml_list`]).
    List(&'a [String]),
}

/// A producer's own key, written after Knowledge's keys. Unknown keys pass validation, so
/// producers carry lineage (Weigh-In's `door`, `decider`, `tree_hash`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field<'a> {
    /// The key, a fixed name.
    pub key: &'static str,
    /// Its value.
    pub value: Value<'a>,
}

/// An atom's frontmatter. [`Display`] writes it between `---` lines, ending with the closing
/// `---` and its newline; [`Frontmatter::atom`] adds the body.
///
/// ```
/// use knowledge_contract::{AtomId, AuthorityTier, Frontmatter, Kind, Status};
/// let Some(id) = AtomId::kept(Kind::OpenQuestion, "wi-4dfe38") else { return };
/// let text = Frontmatter {
///     id: &id,
///     title: "Ship it?",
///     status: Status::Proposed,
///     authority_tier: AuthorityTier::Note,
///     summary: "Open.",
///     depends_on: &[],
///     related: &[],
///     source_paths: &[],
///     last_reviewed: "2026-11-08",
///     reversibility: None,
///     lineage: &[],
/// }
/// .atom("# Ship it?\n");
/// assert!(text.starts_with("---\nid: \"kb-open-question-wi-4dfe38\"\n"));
/// assert!(text.contains("\nkind: open-question\nstatus: proposed\n"));
/// assert!(text.ends_with("---\n\n# Ship it?\n"));
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Frontmatter<'a> {
    /// `id`; its kind is the atom's `kind`.
    pub id: &'a AtomId,
    /// `title`.
    pub title: &'a str,
    /// `status`.
    pub status: Status,
    /// `authority_tier`.
    pub authority_tier: AuthorityTier,
    /// `summary`.
    pub summary: &'a str,
    /// `depends_on`: atom ids this one rests on.
    pub depends_on: &'a [String],
    /// `related`: atom ids worth reading beside this one.
    pub related: &'a [String],
    /// `source_paths`: where the fact came from.
    pub source_paths: &'a [String],
    /// `last_reviewed`: an ISO date.
    pub last_reviewed: &'a str,
    /// For a decision, its reversibility; written with `phase: null`, `supersedes: []` and
    /// `superseded_by: null`, the decision-only keys.
    pub reversibility: Option<Reversibility>,
    /// The producer's keys, in order, after Knowledge's.
    pub lineage: &'a [Field<'a>],
}

impl Frontmatter<'_> {
    /// The whole atom: this frontmatter, a blank line, then `body` as given.
    #[must_use]
    pub fn atom(&self, body: &str) -> String {
        // `format!` panics only when the `Display` impl errs, and this one errs only when its
        // sink does; a `String` sink never fails.
        format!("{self}\n{body}")
    }
}

impl Display for Frontmatter<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "---")?;
        writeln!(f, "id: {}", yaml_str(&self.id.to_string()))?;
        writeln!(f, "title: {}", yaml_str(self.title))?;
        writeln!(f, "kind: {}", self.id.kind())?;
        writeln!(f, "status: {}", self.status.as_str())?;
        writeln!(f, "authority_tier: {}", self.authority_tier.as_str())?;
        writeln!(f, "summary: {}", yaml_str(self.summary))?;
        writeln!(f, "depends_on: {}", yaml_list(self.depends_on))?;
        writeln!(f, "related: {}", yaml_list(self.related))?;
        writeln!(f, "source_paths: {}", yaml_list(self.source_paths))?;
        writeln!(f, "last_reviewed: {}", yaml_str(self.last_reviewed))?;
        if let Some(r) = self.reversibility {
            writeln!(f, "reversibility: {}", r.as_str())?;
            writeln!(f, "phase: null\nsupersedes: []\nsuperseded_by: null")?;
        }
        for field in self.lineage {
            match field.value {
                Value::Str(s) => writeln!(f, "{}: {}", field.key, yaml_str(s))?,
                Value::Word(w) => writeln!(f, "{}: {w}", field.key)?,
                Value::List(l) => writeln!(f, "{}: {}", field.key, yaml_list(l))?,
            }
        }
        writeln!(f, "---")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kind::Kind;

    #[test]
    fn a_decision_carries_the_decision_only_keys_before_its_lineage() {
        let id = AtomId::kept(Kind::Decision, "wi-4dfe38").expect("a valid key");
        let sources = ["a.md".to_owned()];
        let lineage = [
            Field {
                key: "door",
                value: Value::Word("two-way"),
            },
            Field {
                key: "decider",
                value: Value::Str("human:ryan"),
            },
            Field {
                key: "also",
                value: Value::List(&sources),
            },
        ];
        let text = Frontmatter {
            id: &id,
            title: "T",
            status: Status::Accepted,
            authority_tier: AuthorityTier::Decision,
            summary: "S",
            depends_on: &[],
            related: &[],
            source_paths: &sources,
            last_reviewed: "2026-11-08",
            reversibility: Some(Reversibility::High),
            lineage: &lineage,
        }
        .to_string();

        assert_eq!(
            text,
            "---\nid: \"kb-decision-wi-4dfe38\"\ntitle: \"T\"\nkind: decision\nstatus: accepted\n\
             authority_tier: decision\nsummary: \"S\"\ndepends_on: []\nrelated: []\n\
             source_paths: [\"a.md\"]\nlast_reviewed: \"2026-11-08\"\nreversibility: high\n\
             phase: null\nsupersedes: []\nsuperseded_by: null\ndoor: two-way\n\
             decider: \"human:ryan\"\nalso: [\"a.md\"]\n---\n"
        );
    }

    #[test]
    fn every_status_tier_and_reversibility_has_its_spelling() {
        let statuses = [
            Status::Proposed,
            Status::Accepted,
            Status::Superseded,
            Status::Withdrawn,
        ]
        .map(Status::as_str);
        assert_eq!(
            statuses,
            ["proposed", "accepted", "superseded", "withdrawn"]
        );
        let tiers = [AuthorityTier::Decision, AuthorityTier::Note].map(AuthorityTier::as_str);
        assert_eq!(tiers, ["decision", "note"]);
        let reversibility = [
            Reversibility::Low,
            Reversibility::Medium,
            Reversibility::High,
        ]
        .map(Reversibility::as_str);
        assert_eq!(reversibility, ["low", "medium", "high"]);
    }
}
