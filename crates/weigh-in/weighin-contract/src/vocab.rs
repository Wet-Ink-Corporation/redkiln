//! The words every item is classified by: what kind of item it is, how reversible the choice is
//! and whose call it is.
//!
//! The `weigh-in` marker, the ledger's events and the decision request (SPEC DR-02) spell them
//! the same way, in kebab-case. `parse` is the marker's forgiving reader; serde's is strict, so a
//! stored event or a request with an unknown word does not decode.

use serde::{Deserialize, Serialize};

/// What the agent is telling the owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// The agent could not decide and asks. The default for an unrecognised value.
    #[default]
    Question,
    /// The agent proceeded on an assumption the owner should confirm.
    Assumption,
    /// The agent left something undone on purpose, or a to-do comment.
    Deferral,
    /// The agent made an unexamined call and logs it.
    Call,
}

impl Kind {
    /// Read a `kind:` value, ignoring case and surrounding space. Infallible: anything
    /// unrecognised is [`Kind::Question`].
    #[must_use]
    pub fn parse(v: &str) -> Self {
        match v.trim().to_ascii_lowercase().as_str() {
            "assumption" | "assumed" => Self::Assumption,
            "deferral" | "deferred" | "defer" | "todo" => Self::Deferral,
            "call" | "decision" => Self::Call,
            _ => Self::Question,
        }
    }

    /// The kebab-case name used in the marker block and in the ledger.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Question => "question",
            Self::Assumption => "assumption",
            Self::Deferral => "deferral",
            Self::Call => "call",
        }
    }
}

/// How reversible the choice is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Door {
    /// Hard or impossible to undo.
    OneWay,
    /// Cheap to undo.
    TwoWay,
    /// The agent could not say. The default for an unrecognised value.
    #[default]
    Unsure,
}

impl Door {
    /// Read a `door:` value, ignoring case and treating `_` and spaces as `-`. Infallible:
    /// anything unrecognised is [`Door::Unsure`].
    #[must_use]
    pub fn parse(v: &str) -> Self {
        let v = v.trim().to_ascii_lowercase().replace(['_', ' '], "-");
        match v.as_str() {
            "one-way" | "oneway" | "1-way" | "irreversible" => Self::OneWay,
            "two-way" | "twoway" | "2-way" | "reversible" => Self::TwoWay,
            _ => Self::Unsure,
        }
    }

    /// The kebab-case name used in the marker block and in the ledger.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OneWay => "one-way",
            Self::TwoWay => "two-way",
            Self::Unsure => "unsure",
        }
    }
}

/// Whose call the question is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Authority {
    /// Needs the human's authority: security, money, data retention, migrations, product calls,
    /// or intent only the user knows.
    #[default]
    Authority,
    /// A judgment the agent is entitled to make and log.
    Judgment,
}

impl Authority {
    /// Read an `authority:` value, ignoring case and surrounding space. Infallible: anything
    /// unrecognised is [`Authority::Authority`], erring towards asking the owner.
    #[must_use]
    pub fn parse(v: &str) -> Self {
        match v.trim().to_ascii_lowercase().as_str() {
            "judgment" | "judgement" | "agent" => Self::Judgment,
            _ => Self::Authority,
        }
    }

    /// The kebab-case name used in the marker block and in the ledger.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Authority => "authority",
            Self::Judgment => "judgment",
        }
    }
}
