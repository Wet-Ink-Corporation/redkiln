//! The tag vocabulary the CLI and Bench's readers name: `key:value` tags the store indexes
//! events by, in Core's tag registry (EV-12).
//!
//! Every event carries `session:` and `worktree:` when its envelope names them (Core sets both),
//! plus one subject tag chosen by its type. The CLI's short keys map to Core's: `session`,
//! `worktree`, `task` and `pr` are Core's own keys, and `plan`, `point`, `agent` and `migration`
//! are written `bench.<key>` (EV-12). A value is never shortened (EV-11).

use std::fmt;
use std::str::FromStr;

use bench_contract::event::SubjectKey;
use redkiln_core::envelope::Namespace;
use redkiln_core::store::{CoreKey, Tag, TagError as CoreTagError};

/// A tag key in the CLI's vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TagKey {
    /// `session:<session_id>`
    Session,
    /// `worktree:<worktree id>`
    Worktree,
    /// `task:<task id>`
    Task,
    /// `bench.plan:<plan id>`
    Plan,
    /// `pr:<number>`
    Pr,
    /// `bench.point:<gate or decision point>`
    Point,
    /// `bench.agent:<subagent type>`
    Agent,
    /// `bench.migration:<migration id>`
    Migration,
}

impl TagKey {
    /// Every key, in documentation order.
    pub const ALL: [Self; 8] = [
        Self::Session,
        Self::Worktree,
        Self::Task,
        Self::Plan,
        Self::Pr,
        Self::Point,
        Self::Agent,
        Self::Migration,
    ];

    /// The short key, as the CLI's `--tag` names it, before the `:`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::Worktree => "worktree",
            Self::Task => "task",
            Self::Plan => "plan",
            Self::Pr => "pr",
            Self::Point => "point",
            Self::Agent => "agent",
            Self::Migration => "migration",
        }
    }

    /// The store's tag `key:value` for this key: Core's key, or `bench.<key>`.
    fn tag(self, value: &str) -> Result<Tag, CoreTagError> {
        let core = |key| Tag::core(key, value);
        match self {
            Self::Session => core(CoreKey::Session),
            Self::Worktree => core(CoreKey::Worktree),
            Self::Task => core(CoreKey::Task),
            Self::Pr => core(CoreKey::Pr),
            Self::Plan | Self::Point | Self::Agent | Self::Migration => {
                Tag::namespaced(Namespace::Bench, self.as_str(), value)
            }
        }
    }
}

impl From<SubjectKey> for TagKey {
    /// The CLI key of an event's subject: the same short key.
    fn from(key: SubjectKey) -> Self {
        match key {
            SubjectKey::Task => Self::Task,
            SubjectKey::Plan => Self::Plan,
            SubjectKey::Pr => Self::Pr,
            SubjectKey::Point => Self::Point,
            SubjectKey::Agent => Self::Agent,
            SubjectKey::Migration => Self::Migration,
            SubjectKey::Worktree => Self::Worktree,
        }
    }
}

impl fmt::Display for TagKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why a tag could not be built.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TagError {
    /// The text has no `:` separating key and value.
    #[error("a tag is written key:value, got {text:?}")]
    NotKeyValue {
        /// The text that was given.
        text: String,
    },
    /// The key is not in the vocabulary.
    #[error(
        "unknown tag key {key:?}: expected session, worktree, task, plan, pr, point, agent or migration"
    )]
    UnknownKey {
        /// The key that was given.
        key: String,
    },
    /// The value is not one the store's key holds (empty, a control character, too long, a
    /// pull request 0, a worktree that is not a `wt-` id).
    #[error("invalid {key} tag value {value:?}")]
    Value {
        /// The tag's key.
        key: TagKey,
        /// The value that was refused.
        value: String,
        /// Why Core refuses it.
        #[source]
        source: CoreTagError,
    },
}

/// A validated tag from the CLI's vocabulary, as the store writes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerTag {
    key: TagKey,
    tag: Tag,
}

impl LedgerTag {
    /// The tag of `key` with `value`.
    ///
    /// # Errors
    /// [`TagError::Value`] when `value` is not one the store's key holds.
    pub fn new(key: TagKey, value: &str) -> Result<Self, TagError> {
        key.tag(value)
            .map(|tag| Self { key, tag })
            .map_err(|source| TagError::Value {
                key,
                value: value.to_owned(),
                source,
            })
    }

    /// The tag's key.
    #[must_use]
    pub const fn key(&self) -> TagKey {
        self.key
    }

    /// The whole tag as the store keeps it: `task:T1`, `bench.plan:p.md`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.tag.as_str()
    }

    /// The store's tag.
    pub(super) const fn as_tag(&self) -> &Tag {
        &self.tag
    }
}

impl fmt::Display for LedgerTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for LedgerTag {
    type Err = TagError;

    /// Parses `key:value`, splitting at the first `:`. The key is a short one (`plan`) or, for
    /// Bench's own keys, as stored (`bench.plan`).
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (key, value) = text.split_once(':').ok_or_else(|| TagError::NotKeyValue {
            text: text.to_owned(),
        })?;
        let found = TagKey::ALL.into_iter().find(|candidate| {
            candidate.as_str() == key
                || (matches!(
                    candidate,
                    TagKey::Plan | TagKey::Point | TagKey::Agent | TagKey::Migration
                ) && key.strip_prefix("bench.") == Some(candidate.as_str()))
        });
        let key = found.ok_or_else(|| TagError::UnknownKey {
            key: key.to_owned(),
        })?;
        Self::new(key, value)
    }
}

#[cfg(test)]
mod tests;
