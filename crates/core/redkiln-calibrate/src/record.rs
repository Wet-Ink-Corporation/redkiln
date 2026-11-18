//! Calibration records, the two types of SPEC CA-02 (`spec/calibration.md`).
//!
//! `core.judgement_recorded` is one answer a producer's model gave, or the lack of one, and
//! `core.judgement_labelled` the outcome given for it later.
//!
//! These are the `data` of the two types. Both read and write through private wire shapes and
//! are checked on the way in (OUT-05): the answer is given or failed, never both
//! ([`Answer`]); ids, outcomes, points and questions are 1 to 64 of `[A-Za-z0-9_.:-]`; a
//! producer is one plain folder name (ST-17); a subject is `<method>:<hash>` (ID-09). Keys the
//! version does not define are ignored and `null` reads as absent (EV-01, EV-09). Nothing here
//! writes an envelope: Move 2 has no writer (CAL-D1), and [`crate::rows`] reads both these and
//! anneal's v1 rows.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize, Serializer};
use serde_json::{Map, Value};

use crate::score::{Probability, ProbabilityError};

mod name;
mod word;

pub use name::{Producer, ProducerError, Subject, SubjectError};
pub use word::{JudgementId, Outcome, Vocab, WordError};

/// The type of a recorded judgement.
pub const JUDGEMENT_RECORDED: &str = "core.judgement_recorded";

/// The type of an outcome label.
pub const JUDGEMENT_LABELLED: &str = "core.judgement_labelled";

/// The version of both types this build reads and writes.
pub const VERSION: u32 = 1;

/// The mode in effect when a judgement was asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Asked and recorded; nothing acted on the answer.
    Shadow,
    /// Asked, and the producer acted on the answer.
    Enforce,
}

/// A judgement's answer: given, or failed. Exactly one, by construction (API-02).
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    /// The model answered.
    Given {
        /// P(yes), or P(top option) for a choice.
        probability: Probability,
        /// The probability of each option of a choice; empty when there are none.
        options: BTreeMap<String, Probability>,
        /// The top option.
        top: Option<String>,
        /// What the producer's thresholds made of it, in its vocabulary.
        action: Option<String>,
        /// Whether the action handed the decision to a human.
        asked: bool,
    },
    /// No answer came.
    Failed {
        /// Why, in the producer's vocabulary (`timed_out`).
        kind: String,
        /// The failure's own words.
        reason: Option<String>,
    },
}

/// The `data` of `core.judgement_recorded` version 1.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(try_from = "RecordedWire")]
pub struct JudgementRecorded {
    /// The namespace whose judgement it is, also its user-scope folder (ST-17).
    pub producer: Producer,
    /// Unique per producer; labels name it.
    pub id: JudgementId,
    /// The decision point.
    pub point: Vocab,
    /// The question type.
    pub question: Vocab,
    /// The model asked.
    pub model: String,
    /// The mode in effect when it was asked.
    pub mode: Mode,
    /// The answer, or why there is none.
    pub answer: Answer,
    /// The content the judgement binds to, `<method>:<hash>` (ID-09).
    pub subject: Option<Subject>,
    /// The producer's own fields; Core reads none of them (CA-06).
    pub detail: Option<Map<String, Value>>,
}

/// The `data` of `core.judgement_labelled` version 1.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "LabelledWire")]
pub struct JudgementLabelled {
    /// As [`JudgementRecorded::producer`].
    pub producer: Producer,
    /// The id of the judgement it labels.
    pub judgement: JudgementId,
    /// What turned out right, in the producer's vocabulary.
    pub outcome: Outcome,
    /// Who gave it and through what, as asserted.
    pub by: Option<LabelBy>,
}

/// Who gave a label and through what (CA-05: asserted, kept as data).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelBy {
    /// Who gave it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<LabelActor>,
    /// The surface it came through (`mod`, `bash`, `terminal`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub via: Option<String>,
}

/// Who gave a label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LabelActor {
    /// A person.
    Human,
    /// A model.
    Model,
}

/// Why a record's `data` is not one of these types. Each variant names the key at fault.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RecordError {
    /// The producer is not one plain folder name.
    #[error("{field}: {value:?} is not a producer (lowercase letters, digits and `-`, not first)")]
    Producer {
        /// The key.
        field: &'static str,
        /// The value given.
        value: String,
    },
    /// An id (`id`, `judgement`) out of form.
    #[error("{field}: {source}")]
    Id {
        /// The key.
        field: &'static str,
        /// What is wrong with it.
        source: WordError,
    },
    /// An outcome out of form.
    #[error("{field}: {source}")]
    Outcome {
        /// The key.
        field: &'static str,
        /// What is wrong with it.
        source: WordError,
    },
    /// A point or question out of form.
    #[error("{field}: {source}")]
    Vocab {
        /// The key.
        field: &'static str,
        /// What is wrong with it.
        source: WordError,
    },
    /// A probability (`probability`, or one of `options`) outside 0..=1.
    #[error("{field}: {source}")]
    Probability {
        /// The key.
        field: &'static str,
        /// The number.
        source: ProbabilityError,
    },
    /// The answer is neither given nor failed, or mixes the two: `failure` beside a
    /// `probability`, neither of them (`probability`), `asked` without a `probability` or
    /// missing beside one (`asked`), or `options`, `top` or `action` beside a `failure`.
    #[error("{field}: a judgement is answered (probability, asked) or failed (failure), not both")]
    Answer {
        /// The key at fault.
        field: &'static str,
    },
    /// A subject that is not `<method>:<lowercase hex>` of an ID-09 method.
    #[error("{field}: {value:?} is not <method>:<hash>")]
    Subject {
        /// The key.
        field: &'static str,
        /// The value given.
        value: String,
    },
}

impl RecordError {
    /// The key at fault.
    #[must_use]
    pub const fn field(&self) -> &'static str {
        match self {
            Self::Producer { field, .. }
            | Self::Id { field, .. }
            | Self::Outcome { field, .. }
            | Self::Vocab { field, .. }
            | Self::Probability { field, .. }
            | Self::Answer { field }
            | Self::Subject { field, .. } => field,
        }
    }
}

/// `core.judgement_recorded`'s `data` as it is read: every key optional but the required ones.
#[derive(Debug, Deserialize)]
struct RecordedWire {
    producer: String,
    id: String,
    point: String,
    question: String,
    model: String,
    mode: Mode,
    probability: Option<f64>,
    options: Option<BTreeMap<String, f64>>,
    top: Option<String>,
    action: Option<String>,
    asked: Option<bool>,
    failure: Option<FailureWire>,
    subject: Option<String>,
    detail: Option<Map<String, Value>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct FailureWire {
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

/// `core.judgement_labelled`'s `data` as it is read.
#[derive(Debug, Deserialize)]
struct LabelledWire {
    producer: String,
    judgement: String,
    outcome: String,
    by: Option<LabelBy>,
}

impl TryFrom<RecordedWire> for JudgementRecorded {
    type Error = RecordError;

    fn try_from(wire: RecordedWire) -> Result<Self, Self::Error> {
        let answer = match (wire.probability, wire.failure) {
            (Some(_), Some(_)) => return Err(RecordError::Answer { field: "failure" }),
            (None, None) => {
                return Err(RecordError::Answer {
                    field: "probability",
                });
            }
            (Some(probability), None) => Answer::Given {
                probability: probability_of("probability", probability)?,
                options: match wire.options {
                    Some(options) => options_of(options)?,
                    // An absent `options` is a judgement with no options, not a failure.
                    None => BTreeMap::new(),
                },
                top: wire.top,
                action: wire.action,
                asked: wire.asked.ok_or(RecordError::Answer { field: "asked" })?,
            },
            (None, Some(failure)) => {
                let beside = [
                    ("asked", wire.asked.is_some()),
                    ("options", wire.options.is_some()),
                    ("top", wire.top.is_some()),
                    ("action", wire.action.is_some()),
                ];
                if let Some((field, _)) = beside.into_iter().find(|(_, present)| *present) {
                    return Err(RecordError::Answer { field });
                }
                Answer::Failed {
                    kind: failure.kind,
                    reason: failure.reason,
                }
            }
        };
        Ok(Self {
            producer: producer_of(wire.producer)?,
            id: JudgementId::try_from(wire.id).map_err(|source| RecordError::Id {
                field: "id",
                source,
            })?,
            point: vocab_of("point", wire.point)?,
            question: vocab_of("question", wire.question)?,
            model: wire.model,
            mode: wire.mode,
            answer,
            subject: wire.subject.map(subject_of).transpose()?,
            detail: wire.detail,
        })
    }
}

impl TryFrom<LabelledWire> for JudgementLabelled {
    type Error = RecordError;

    fn try_from(wire: LabelledWire) -> Result<Self, Self::Error> {
        Ok(Self {
            producer: producer_of(wire.producer)?,
            judgement: JudgementId::try_from(wire.judgement).map_err(|source| RecordError::Id {
                field: "judgement",
                source,
            })?,
            outcome: Outcome::try_from(wire.outcome).map_err(|source| RecordError::Outcome {
                field: "outcome",
                source,
            })?,
            by: wire.by,
        })
    }
}

fn producer_of(value: String) -> Result<Producer, RecordError> {
    Producer::try_from(value).map_err(|error| RecordError::Producer {
        field: "producer",
        value: error.into_string(),
    })
}

fn vocab_of(field: &'static str, value: String) -> Result<Vocab, RecordError> {
    Vocab::try_from(value).map_err(|source| RecordError::Vocab { field, source })
}

fn probability_of(field: &'static str, value: f64) -> Result<Probability, RecordError> {
    Probability::try_from(value).map_err(|source| RecordError::Probability { field, source })
}

fn options_of(
    options: BTreeMap<String, f64>,
) -> Result<BTreeMap<String, Probability>, RecordError> {
    options
        .into_iter()
        .map(|(name, value)| Ok((name, probability_of("options", value)?)))
        .collect()
}

fn subject_of(value: String) -> Result<Subject, RecordError> {
    Subject::try_from(value).map_err(|error| RecordError::Subject {
        field: "subject",
        value: error.into_string(),
    })
}

/// `core.judgement_recorded`'s `data` as it is written: keys in the schema's order, absent ones
/// left out (EV-05).
#[derive(Serialize)]
struct RecordedOut<'a> {
    producer: &'a str,
    id: &'a str,
    point: &'a str,
    question: &'a str,
    model: &'a str,
    mode: Mode,
    #[serde(skip_serializing_if = "Option::is_none")]
    probability: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<Numbers<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    action: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    asked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure: Option<FailureOut<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    subject: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<&'a Map<String, Value>>,
}

#[derive(Serialize)]
struct FailureOut<'a> {
    kind: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'a str>,
}

/// A choice's options, written as numbers.
struct Numbers<'a>(&'a BTreeMap<String, Probability>);

impl Serialize for Numbers<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_map(
            self.0
                .iter()
                .map(|(name, probability)| (name, probability.get())),
        )
    }
}

impl Serialize for JudgementRecorded {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut out = RecordedOut {
            producer: self.producer.as_str(),
            id: self.id.as_str(),
            point: self.point.as_str(),
            question: self.question.as_str(),
            model: &self.model,
            mode: self.mode,
            probability: None,
            options: None,
            top: None,
            action: None,
            asked: None,
            failure: None,
            subject: self.subject.as_ref().map(Subject::as_str),
            detail: self.detail.as_ref(),
        };
        match &self.answer {
            Answer::Given {
                probability,
                options,
                top,
                action,
                asked,
            } => {
                out.probability = Some(probability.get());
                out.options = Some(Numbers(options)).filter(|options| !options.0.is_empty());
                out.top = top.as_deref();
                out.action = action.as_deref();
                out.asked = Some(*asked);
            }
            Answer::Failed { kind, reason } => {
                out.failure = Some(FailureOut {
                    kind,
                    reason: reason.as_deref(),
                });
            }
        }
        out.serialize(serializer)
    }
}

#[derive(Serialize)]
struct LabelledOut<'a> {
    producer: &'a str,
    judgement: &'a str,
    outcome: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    by: Option<&'a LabelBy>,
}

impl Serialize for JudgementLabelled {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        LabelledOut {
            producer: self.producer.as_str(),
            judgement: self.judgement.as_str(),
            outcome: self.outcome.as_str(),
            by: self.by.as_ref(),
        }
        .serialize(serializer)
    }
}

#[cfg(test)]
mod tests;
