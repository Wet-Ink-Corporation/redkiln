//! Trust levels (SPEC TR-01 to TR-06, TR-10): the `trust` object every envelope carries.
//!
//! A [`Trust`] is built only in a combination the SPEC allows: `witnessed` through a surface the
//! agent cannot drive (TR-03), `attested` through an identity outside its reach (TR-04),
//! `asserted` through anything or nothing. Which code path may build a level above `asserted`
//! is TR-05's rule for the writers: no argument, variable, stdin field or config key raises
//! one, and this crate cannot see where its caller's values came from.

use std::fmt;
use std::str::FromStr;

/// A trust level, lowest first (TR-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    /// Any write through a CLI or hook, and every imported journal line: gates nothing.
    Asserted,
    /// An answer taken from a surface the agent cannot drive (TR-03).
    Witnessed,
    /// A record verifiable against an identity the agent and the author do not hold (TR-04).
    Attested,
}

impl Level {
    /// The level as written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Asserted => "asserted",
            Self::Witnessed => "witnessed",
            Self::Attested => "attested",
        }
    }
}

impl FromStr for Level {
    type Err = TrustError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        [Self::Asserted, Self::Witnessed, Self::Attested]
            .into_iter()
            .find(|level| level.as_str() == text)
            .ok_or(TrustError::UnknownLevel)
    }
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A registered surface or mechanism (TR-03, TR-04).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Via {
    /// An `AskUserQuestion` tool result, read by a hook from its own input.
    AskUserQuestion,
    /// A board submission, validated and checked against the card's revision.
    Board,
    /// A decision card answered by a member's tap.
    DecisionCard,
    /// A click in the suite mod's pane.
    ModPane,
    /// A pull request review by a named user; `asserted` in SPEC 1.0 (TR-10).
    GithubReview,
    /// A check run, or a comment command (`asserted` in SPEC 1.0), recorded by the Review App.
    GithubApp,
    /// A compare-and-set on the ledger repository that only its App or key may push.
    LedgerCas,
    /// A commit signed by a user key on the decider's account, never the web-flow key.
    SignedCommit,
}

impl Via {
    /// Every registered `via`.
    pub const ALL: [Self; 8] = [
        Self::AskUserQuestion,
        Self::Board,
        Self::DecisionCard,
        Self::ModPane,
        Self::GithubReview,
        Self::GithubApp,
        Self::LedgerCas,
        Self::SignedCommit,
    ];

    /// The `via` as written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AskUserQuestion => "ask-user-question",
            Self::Board => "board",
            Self::DecisionCard => "decision-card",
            Self::ModPane => "mod-pane",
            Self::GithubReview => "github-review",
            Self::GithubApp => "github-app",
            Self::LedgerCas => "ledger-cas",
            Self::SignedCommit => "signed-commit",
        }
    }

    /// The highest level this `via` may carry in SPEC 1.0: `github-review` is `asserted`
    /// (TR-10).
    const fn ceiling(self) -> Level {
        match self {
            Self::AskUserQuestion | Self::Board | Self::DecisionCard | Self::ModPane => {
                Level::Witnessed
            }
            Self::GithubReview => Level::Asserted,
            Self::GithubApp | Self::LedgerCas | Self::SignedCommit => Level::Attested,
        }
    }
}

impl FromStr for Via {
    type Err = TrustError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|via| via.as_str() == text)
            .ok_or(TrustError::UnknownVia)
    }
}

impl fmt::Display for Via {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why a trust object was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TrustError {
    /// A level outside `asserted`, `witnessed`, `attested`.
    #[error("not a trust level")]
    UnknownLevel,
    /// A `via` outside the registry.
    #[error("not a registered via")]
    UnknownVia,
    /// A level above `asserted` with no `via` to back it.
    #[error("trust level {level} needs a via")]
    MissingVia {
        /// The level asked for.
        level: Level,
    },
    /// A `via` that cannot back the level: `witnessed` needs TR-03's surfaces, `attested`
    /// TR-04's identities, and `github-review` is `asserted` in SPEC 1.0.
    #[error("{via} cannot make an event {level}")]
    Via {
        /// The level asked for.
        level: Level,
        /// The `via` given.
        via: Via,
    },
    /// A `by` or `ref` that is empty or holds a control character.
    #[error("trust evidence text must be non-empty with no control characters")]
    Text,
}

/// An event's trust: its level and the evidence for it (TR-01).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Trust {
    level: Level,
    via: Option<Via>,
    by: Option<String>,
    reference: Option<String>,
    claimed: Option<Level>,
}

impl Trust {
    /// `asserted` with no `via`: any write through a CLI or a hook.
    #[must_use]
    pub const fn asserted() -> Self {
        Self {
            level: Level::Asserted,
            via: None,
            by: None,
            reference: None,
            claimed: None,
        }
    }

    /// `level` through `via`, when TR-03, TR-04 and TR-10 let that `via` back it. `asserted`
    /// keeps any `via` (TR-06, TR-10 lower a level and keep it).
    ///
    /// # Errors
    /// [`TrustError::MissingVia`] for a level above `asserted` with no `via`;
    /// [`TrustError::Via`] for a `via` that cannot back the level.
    pub fn new(level: Level, via: Option<Via>) -> Result<Self, TrustError> {
        match via {
            None if level > Level::Asserted => Err(TrustError::MissingVia { level }),
            Some(via) if level == Level::Witnessed && via.ceiling() != Level::Witnessed => {
                Err(TrustError::Via { level, via })
            }
            Some(via) if level == Level::Attested && via.ceiling() != Level::Attested => {
                Err(TrustError::Via { level, via })
            }
            _ => Ok(Self {
                level,
                via,
                ..Self::asserted()
            }),
        }
    }

    /// The same trust, naming the identity the surface reports (a GitHub login).
    ///
    /// # Errors
    /// [`TrustError::Text`] for an empty value or one with a control character.
    pub fn with_by(mut self, by: impl Into<String>) -> Result<Self, TrustError> {
        self.by = Some(evidence(by.into())?);
        Ok(self)
    }

    /// The same trust, saying where the evidence is (a tool use id, a check run id).
    ///
    /// # Errors
    /// [`TrustError::Text`] for an empty value or one with a control character.
    pub fn with_reference(mut self, reference: impl Into<String>) -> Result<Self, TrustError> {
        self.reference = Some(evidence(reference.into())?);
        Ok(self)
    }

    /// The trust an import stores (TR-06): `asserted`, keeping the evidence, with the level the
    /// journal line carried in `claimed`.
    #[must_use]
    pub fn imported(self) -> Self {
        Self {
            level: Level::Asserted,
            claimed: Some(self.claimed.unwrap_or(self.level)),
            ..self
        }
    }

    /// The same trust with `claimed` as read from an imported envelope (TR-06); only a reader
    /// sets it, since a writer's import goes through [`Trust::imported`].
    pub(crate) const fn with_claimed(mut self, claimed: Option<Level>) -> Self {
        self.claimed = claimed;
        self
    }

    /// The level.
    #[must_use]
    pub const fn level(&self) -> Level {
        self.level
    }

    /// The surface or mechanism, if any.
    #[must_use]
    pub const fn via(&self) -> Option<Via> {
        self.via
    }

    /// The identity the surface reports, if any.
    #[must_use]
    pub fn by(&self) -> Option<&str> {
        self.by.as_deref()
    }

    /// Where the evidence is, if recorded (the envelope's `ref`).
    #[must_use]
    pub fn reference(&self) -> Option<&str> {
        self.reference.as_deref()
    }

    /// On an imported event, the level its journal line carried.
    #[must_use]
    pub const fn claimed(&self) -> Option<Level> {
        self.claimed
    }
}

/// `text` when it is non-empty with no control characters.
fn evidence(text: String) -> Result<String, TrustError> {
    if text.is_empty() || text.chars().any(char::is_control) {
        Err(TrustError::Text)
    } else {
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cli_write_is_asserted_with_no_via() {
        let trust = Trust::asserted();
        assert_eq!(trust.level(), Level::Asserted);
        assert_eq!(trust.via(), None);
    }

    #[test]
    fn witnessed_needs_a_surface_the_agent_cannot_drive() {
        for via in [
            Via::AskUserQuestion,
            Via::Board,
            Via::DecisionCard,
            Via::ModPane,
        ] {
            let trust = Trust::new(Level::Witnessed, Some(via)).expect("a TR-03 surface");
            assert_eq!((trust.level(), trust.via()), (Level::Witnessed, Some(via)));
        }
        for via in [
            Via::GithubReview,
            Via::GithubApp,
            Via::LedgerCas,
            Via::SignedCommit,
        ] {
            assert_eq!(
                Trust::new(Level::Witnessed, Some(via)),
                Err(TrustError::Via {
                    level: Level::Witnessed,
                    via
                }),
                "{via:?}"
            );
        }
        assert_eq!(
            Trust::new(Level::Witnessed, None),
            Err(TrustError::MissingVia {
                level: Level::Witnessed
            })
        );
    }

    #[test]
    fn attested_needs_an_identity_outside_the_agents_reach() {
        for via in [Via::GithubApp, Via::LedgerCas, Via::SignedCommit] {
            let trust = Trust::new(Level::Attested, Some(via)).expect("a TR-04 identity");
            assert_eq!((trust.level(), trust.via()), (Level::Attested, Some(via)));
        }
        for via in [Via::Board, Via::GithubReview] {
            assert_eq!(
                Trust::new(Level::Attested, Some(via)),
                Err(TrustError::Via {
                    level: Level::Attested,
                    via
                }),
                "{via:?}"
            );
        }
    }

    #[test]
    fn a_github_review_is_asserted_and_keeps_its_via_and_by() {
        // TR-10: while agents act as the owner's account, a review is asserted.
        let trust = Trust::new(Level::Asserted, Some(Via::GithubReview))
            .expect("asserted takes any via")
            .with_by("ryan")
            .expect("a login");
        assert_eq!(trust.level(), Level::Asserted);
        assert_eq!(trust.via(), Some(Via::GithubReview));
        assert_eq!(trust.by(), Some("ryan"));
    }

    #[test]
    fn an_import_is_asserted_and_keeps_the_claimed_level() {
        let witnessed = Trust::new(Level::Witnessed, Some(Via::Board))
            .expect("valid")
            .with_reference("board-17")
            .expect("a ref");
        let imported = witnessed.imported();
        assert_eq!(imported.level(), Level::Asserted);
        assert_eq!(imported.claimed(), Some(Level::Witnessed));
        assert_eq!(imported.via(), Some(Via::Board));
        assert_eq!(imported.reference(), Some("board-17"));
    }

    #[test]
    fn evidence_text_must_be_non_empty_and_printable() {
        assert_eq!(Trust::asserted().with_by(""), Err(TrustError::Text));
        assert_eq!(
            Trust::asserted().with_reference("a\nb"),
            Err(TrustError::Text)
        );
    }

    #[test]
    fn levels_and_vias_read_back_from_their_names() {
        for level in [Level::Asserted, Level::Witnessed, Level::Attested] {
            assert_eq!(level.as_str().parse(), Ok(level));
        }
        for via in Via::ALL {
            assert_eq!(via.as_str().parse(), Ok(via));
        }
        assert_eq!("trusted".parse::<Level>(), Err(TrustError::UnknownLevel));
        assert_eq!("email".parse::<Via>(), Err(TrustError::UnknownVia));
        assert_eq!(Via::AskUserQuestion.as_str(), "ask-user-question");
        assert_eq!(Via::LedgerCas.as_str(), "ledger-cas");
    }

    #[test]
    fn levels_order_lowest_first() {
        assert!(Level::Asserted < Level::Witnessed && Level::Witnessed < Level::Attested);
    }
}
