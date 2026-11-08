//! The eight atom kinds (Knowledge call A).

use std::fmt;

/// What an atom holds. The spelling is the frontmatter `kind` value, the folder under `.kb/` and
/// the word in the atom id (`kb-<kind>-…`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    /// A choice between options, with who decided.
    Decision,
    /// A must or must-not with no choice behind it.
    Constraint,
    /// A scope ruling.
    Ruling,
    /// A measured or documented fact about the system.
    Finding,
    /// A source the findings cite.
    Evidence,
    /// A lesson from an outcome.
    Learning,
    /// A word in the project's language and what it means.
    Term,
    /// A question raised and not yet settled.
    OpenQuestion,
}

impl Kind {
    /// Every kind, in the order Knowledge lists them.
    pub const ALL: [Self; 8] = [
        Self::Decision,
        Self::Constraint,
        Self::Ruling,
        Self::Finding,
        Self::Evidence,
        Self::Learning,
        Self::Term,
        Self::OpenQuestion,
    ];

    /// The kind as written: `decision`, …, `open-question`.
    ///
    /// ```
    /// use knowledge_contract::Kind;
    /// assert_eq!(Kind::OpenQuestion.as_str(), "open-question");
    /// ```
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Decision => "decision",
            Self::Constraint => "constraint",
            Self::Ruling => "ruling",
            Self::Finding => "finding",
            Self::Evidence => "evidence",
            Self::Learning => "learning",
            Self::Term => "term",
            Self::OpenQuestion => "open-question",
        }
    }

    /// The kind written as `text`, if it is one of the eight.
    ///
    /// ```
    /// use knowledge_contract::Kind;
    /// assert_eq!(Kind::parse("decision"), Some(Kind::Decision));
    /// assert_eq!(Kind::parse("open_question"), None);
    /// ```
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == text)
    }

    /// The folder atoms of this kind live in, relative to the repository root: `.kb/<kind>`
    /// (Knowledge call B).
    #[must_use]
    pub fn dir(self) -> String {
        format!(".kb/{}", self.as_str())
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_reads_back_from_its_spelling() {
        for kind in Kind::ALL {
            assert_eq!(Kind::parse(kind.as_str()), Some(kind));
        }
    }

    #[test]
    fn a_kind_lives_in_its_own_folder() {
        assert_eq!(Kind::Decision.dir(), ".kb/decision");
        assert_eq!(Kind::OpenQuestion.dir(), ".kb/open-question");
    }
}
