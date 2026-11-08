//! An answer to a decision request (SPEC DR-05, DR-06), and the raiser's check before it acts.
//!
//! An answer is Weigh-In's decision event for the item ([`Decided`](crate::event::Decided)); its
//! trust is the envelope's. A raiser reads it into an [`Answer`] and accepts it only through
//! [`accepts`]: trusted enough, one of the request's options, and about the subject as it is now.

use crate::request::{OptionId, Request};
use redkiln_core::trust::Level;

/// Whether an answer holds once or always (Knowledge call G).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Applies {
    /// This occurrence only. What every decision Weigh-In writes today says.
    #[default]
    Once,
    /// Every later occurrence: the answer becomes a constraint.
    Always,
}

/// An answer as a raiser reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// The option chosen, as the decision event names it.
    pub option: String,
    /// The trust level of the decision's envelope.
    pub trust: Level,
    /// Once or always.
    pub applies: Applies,
    /// For an answer that always applies, the check that enforces it, if it names one.
    pub confirmation: Option<String>,
}

/// Why a raiser does not act on an answer (DR-06).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AnswerRefused {
    /// The answer's trust is below what the request needs.
    #[error("the answer is {found}, the request needs {needed}")]
    Trust {
        /// What the request needs.
        needed: Level,
        /// What the answer has.
        found: Level,
    },
    /// The option is none of the request's.
    #[error("`{0}` is not one of the request's options")]
    Option(String),
    /// The request has a subject and the raiser's hash of it now differs: the answer is stale,
    /// and the raiser raises again with a new key.
    #[error("the subject changed since the request: it was {asked}, it is {now}")]
    Stale {
        /// The hash the request carried.
        asked: String,
        /// The hash the raiser computed now.
        now: String,
    },
    /// The request has a subject and the raiser gave no current hash to compare.
    #[error("the request has a subject, and no current hash of it was given")]
    NoCurrentSubject,
}

/// Whether the raiser of `request` may act on `answer` (DR-06). `subject_now` is the raiser's
/// own fresh hash of the request's subject, `<method>:<hash>`, needed when the request has one.
///
/// # Errors
/// The first condition the answer fails, as an [`AnswerRefused`].
pub fn accepts(
    request: &Request,
    answer: &Answer,
    subject_now: Option<&str>,
) -> Result<OptionId, AnswerRefused> {
    let needed = request.trust_needed.level();
    if answer.trust < needed {
        return Err(AnswerRefused::Trust {
            needed,
            found: answer.trust,
        });
    }
    let option = request
        .options
        .iter()
        .map(|o| o.id)
        .find(|id| id.as_str() == answer.option)
        .ok_or_else(|| AnswerRefused::Option(answer.option.clone()))?;
    if let Some(subject) = &request.subject {
        let now = subject_now.ok_or(AnswerRefused::NoCurrentSubject)?;
        if now != subject.hash {
            return Err(AnswerRefused::Stale {
                asked: subject.hash.clone(),
                now: now.to_owned(),
            });
        }
    }
    Ok(option)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DR_02: &str = include_str!("../../../../spec/fixtures/decision-request/valid/dr-02.json");
    const HASH: &str = "rust-fingerprint:4be1c0d29a7f3e85";

    fn request() -> Request {
        serde_json::from_str(DR_02).expect("DR-02's golden reads")
    }

    fn answer(option: &str, trust: Level) -> Answer {
        Answer {
            option: option.to_owned(),
            trust,
            applies: Applies::default(),
            confirmation: None,
        }
    }

    #[test]
    fn an_attested_answer_naming_an_option_about_the_same_subject_is_accepted() {
        let accepted = accepts(&request(), &answer("b", Level::Attested), Some(HASH));
        assert_eq!(accepted, Ok(OptionId::B));
    }

    #[test]
    fn a_witnessed_answer_to_an_attested_request_is_refused() {
        assert_eq!(
            accepts(&request(), &answer("b", Level::Witnessed), Some(HASH)),
            Err(AnswerRefused::Trust {
                needed: Level::Attested,
                found: Level::Witnessed
            })
        );
    }

    #[test]
    fn an_option_the_request_does_not_have_is_refused() {
        assert_eq!(
            accepts(&request(), &answer("c", Level::Attested), Some(HASH)),
            Err(AnswerRefused::Option("c".to_owned()))
        );
    }

    #[test]
    fn an_answer_about_a_changed_subject_is_stale() {
        let now = "rust-fingerprint:0000000000000000";
        assert_eq!(
            accepts(&request(), &answer("a", Level::Attested), Some(now)),
            Err(AnswerRefused::Stale {
                asked: HASH.to_owned(),
                now: now.to_owned()
            })
        );
        assert_eq!(
            accepts(&request(), &answer("a", Level::Attested), None),
            Err(AnswerRefused::NoCurrentSubject)
        );
    }

    #[test]
    fn a_request_without_a_subject_needs_no_current_hash() {
        let mut request = request();
        request.subject = None;
        assert_eq!(
            accepts(&request, &answer("a", Level::Attested), None),
            Ok(OptionId::A)
        );
    }
}
