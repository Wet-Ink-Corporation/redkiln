//! The grounded decision card. Its field names are the board's (`board.html` renders them as-is),
//! so a card written by `weighin ground` is exactly what the board shows.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One piece of evidence on a card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Evidence {
    /// Where it comes from, usually a `path:line` anchor.
    pub src: String,
    /// What it shows.
    pub text: String,
}

/// A few lines of a file, shown beside the evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Excerpt {
    /// The file the lines come from.
    pub path: String,
    /// The line number of the first line; the board numbers the rest from it.
    pub start: usize,
    /// The lines, in file order.
    pub lines: Vec<String>,
}

/// One option on a card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CardOption {
    /// The option's id, unique on its card (`A`, `B`, ...); answers name options by it.
    pub id: String,
    /// A short name for the option.
    pub title: String,
    /// What the option means, in a sentence or two.
    #[serde(default)]
    pub summary: String,
    /// What would have to be true for this option to be right ("Holds if" on the board).
    #[serde(default)]
    pub holds_if: String,
    /// What it costs if this option turns out wrong ("If wrong" on the board).
    #[serde(default)]
    pub if_wrong: String,
    /// The MUST this option fails. An option that fails a MUST is shown struck through, never scored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fails_must: Option<String>,
    /// The evasion pattern this option matches, if any ("revisit later" with no trigger, ...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evasion: Option<String>,
}

/// One row of the comparison between a card's options.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Criterion {
    /// What the options are compared on.
    pub name: String,
    /// How each option fares on it, in words, keyed by option id.
    pub values: BTreeMap<String, String>,
}

/// How consistently repeated runs picked the recommended option.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Stability {
    /// How many runs picked the recommended option.
    pub agree: u32,
    /// How many runs there were.
    pub of: u32,
    /// What the runs were, in words; the board shows it beside the count.
    #[serde(default)]
    pub samples: String,
    /// A remark on the runs; the board shows it after `samples`.
    #[serde(default)]
    pub note: String,
}

impl Stability {
    /// Stable means at least three of four picks agreed (design: Opus plus three Sonnet samples).
    #[must_use]
    pub fn is_stable(&self) -> bool {
        // Widened so agent-supplied counts near `u32::MAX` cannot overflow.
        self.of > 0 && u64::from(self.agree) * 4 >= u64::from(self.of) * 3
    }
}

/// The option the card recommends, and how far to trust it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Recommendation {
    /// The id of the recommended option.
    pub option: String,
    /// "A unless X": the fact that would change the pick.
    #[serde(default)]
    pub flip: String,
    /// The trade-off the pick makes ("You are trading X for Y").
    #[serde(default)]
    pub tradeoff: String,
    /// Why this recommendation might be wrong.
    #[serde(default)]
    pub why_wrong: String,
    /// How consistently repeated runs picked it; absent when it was not measured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stability: Option<Stability>,
}

/// The result of the evasion check on a card's recommendation: whether it answers the question
/// or only moves it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EvasionCheck {
    /// Whether the recommendation passed. One that did not is never pre-selected.
    pub passed: bool,
    /// What the check flagged, in words.
    #[serde(default)]
    pub flags: Vec<String>,
}

/// Everything grounding adds to a raised question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Card {
    /// Why the question matters, in one sentence.
    #[serde(default)]
    pub why: String,
    /// A remark on the item's door, shown as "Door: …"; absent when there is none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub door_note: Option<String>,
    /// The result of the decide-now test, in one sentence.
    #[serde(default)]
    pub timing: String,
    /// The evidence the card rests on.
    #[serde(default)]
    pub evidence: Vec<Evidence>,
    /// A code excerpt, if one helps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excerpt: Option<Excerpt>,
    /// The options, two to four with unique ids ([`Card::validate`]).
    pub options: Vec<CardOption>,
    /// How the options compare, one row per criterion.
    #[serde(default)]
    pub criteria: Vec<Criterion>,
    /// The recommended option.
    pub recommendation: Recommendation,
    /// The evasion check on the recommendation; absent when it was not recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evasion: Option<EvasionCheck>,
}

/// A card that breaks one of the rules every card must pass.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CardError {
    /// The card has fewer than two or more than four options; how many it has.
    #[error("a card needs between 2 and 4 options, this one has {0}")]
    OptionCount(usize),
    /// Two options share an id; the repeated id.
    #[error("option ids must be unique; `{0}` repeats")]
    DuplicateOption(String),
    /// The recommendation names an option the card does not have.
    #[error("the recommendation names `{0}`, which is not an option")]
    UnknownRecommendation(String),
    /// The recommended option fails a MUST.
    #[error("the recommended option `{0}` fails a MUST")]
    RecommendsIneligible(String),
    /// The recommended option is flagged as evasive; its id and the evasion.
    #[error("the recommended option `{0}` is flagged as evasive: {1}")]
    RecommendsEvasion(String, String),
}

impl Card {
    /// Structural checks every card must pass before it is recorded.
    ///
    /// # Errors
    /// Returns the first rule the card breaks.
    pub fn validate(&self) -> Result<(), CardError> {
        let n = self.options.len();
        if !(2..=4).contains(&n) {
            return Err(CardError::OptionCount(n));
        }
        let mut seen = std::collections::BTreeSet::new();
        for o in &self.options {
            if !seen.insert(o.id.as_str()) {
                return Err(CardError::DuplicateOption(o.id.clone()));
            }
        }
        let rec = self
            .option(&self.recommendation.option)
            .ok_or_else(|| CardError::UnknownRecommendation(self.recommendation.option.clone()))?;
        if rec.fails_must.is_some() {
            return Err(CardError::RecommendsIneligible(rec.id.clone()));
        }
        if let Some(e) = &rec.evasion {
            return Err(CardError::RecommendsEvasion(rec.id.clone(), e.clone()));
        }
        Ok(())
    }

    /// The option with this id, if the card has one.
    #[must_use]
    pub fn option(&self, id: &str) -> Option<&CardOption> {
        self.options.iter().find(|o| o.id == id)
    }

    /// Whether the recommendation may be pre-selected: it must be stable and pass the evasion check.
    /// One-way items are presented options-first regardless (decided by the caller from the door).
    #[must_use]
    pub fn recommendation_earned(&self) -> bool {
        let stable = self
            .recommendation
            .stability
            .as_ref()
            .is_none_or(Stability::is_stable);
        let evasion_ok = self.evasion.as_ref().is_none_or(|e| e.passed);
        stable && evasion_ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opt(id: &str) -> CardOption {
        CardOption {
            id: id.into(),
            title: format!("Option {id}"),
            ..Default::default()
        }
    }

    fn card() -> Card {
        Card {
            options: vec![opt("A"), opt("B")],
            recommendation: Recommendation {
                option: "A".into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn valid_card_passes() {
        assert_eq!(card().validate(), Ok(()));
    }

    #[test]
    fn rejects_bad_shapes() {
        let mut c = card();
        c.options.truncate(1);
        assert_eq!(c.validate(), Err(CardError::OptionCount(1)));
        let mut c = card();
        c.recommendation.option = "Z".into();
        assert!(matches!(
            c.validate(),
            Err(CardError::UnknownRecommendation(_))
        ));
        let mut c = card();
        c.options[0].fails_must = Some("one-way doors get a human".into());
        assert!(matches!(
            c.validate(),
            Err(CardError::RecommendsIneligible(_))
        ));
        let mut c = card();
        c.options[0].evasion = Some("revisit later with no trigger".into());
        assert!(matches!(
            c.validate(),
            Err(CardError::RecommendsEvasion(..))
        ));
    }

    #[test]
    fn a_repeated_option_id_is_rejected() {
        let mut c = card();
        c.options.push(opt("A"));

        assert_eq!(c.validate(), Err(CardError::DuplicateOption("A".into())));
    }

    #[test]
    fn more_than_four_options_are_rejected() {
        let mut c = card();
        c.options.extend(["C", "D"].map(opt));
        assert_eq!(c.validate(), Ok(()), "four options are allowed");

        c.options.push(opt("E"));

        assert_eq!(c.validate(), Err(CardError::OptionCount(5)));
    }

    #[test]
    fn a_failed_evasion_check_withholds_a_stable_recommendation() {
        let mut c = card();
        c.recommendation.stability = Some(Stability {
            agree: 4,
            of: 4,
            ..Default::default()
        });
        c.evasion = Some(EvasionCheck {
            passed: true,
            flags: Vec::new(),
        });
        assert!(c.recommendation_earned());

        c.evasion = Some(EvasionCheck {
            passed: false,
            flags: vec!["moves the question to a later sprint".into()],
        });

        assert!(!c.recommendation_earned());
    }

    #[test]
    fn earned_needs_three_of_four() {
        let mut c = card();
        c.recommendation.stability = Some(Stability {
            agree: 2,
            of: 4,
            ..Default::default()
        });
        assert!(!c.recommendation_earned());
        c.recommendation.stability = Some(Stability {
            agree: 3,
            of: 4,
            ..Default::default()
        });
        assert!(c.recommendation_earned());
    }

    #[test]
    fn stability_counts_near_the_top_of_u32_do_not_overflow() {
        let s = |agree, of| Stability {
            agree,
            of,
            ..Default::default()
        };
        assert!(s(2_000_000_000, 4).is_stable());
        assert!(s(u32::MAX, 4).is_stable());
        assert!(s(u32::MAX, u32::MAX).is_stable());
        assert!(!s(1, u32::MAX).is_stable());
        assert!(!s(1, 1_431_655_766).is_stable());
        assert!(!s(u32::MAX, 0).is_stable());
    }
}
