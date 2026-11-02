//! `key:value` tags (SPEC EV-11): validated whole, never truncated to fit; and Core's registry
//! of unprefixed keys (EV-12).

use std::fmt;
use std::str::FromStr;

use happenstance_core::MAX_TAG_LEN;

use crate::envelope::{Namespace, is_dotted_name};
use crate::ids::{ContentId, Digest, IdPrefix, SessionId, WorktreeId};
use crate::trust::Level;

/// An unprefixed tag key in Core's registry (EV-12): each means one thing for every writer.
/// Every other key is `<namespace>.<key>` ([`Tag::namespaced`]). Adding one is a SPEC minor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoreKey {
    /// The type's namespace; on every event, set by Core.
    Plugin,
    /// The trust level; on every event, set by Core.
    Trust,
    /// The envelope's `session`; set by Core.
    Session,
    /// The envelope's `worktree`; set by Core.
    Worktree,
    /// A Weigh-In item id (`wi-…`).
    Item,
    /// A subject, `<method>:<hash>` (ID-09).
    Subject,
    /// A pull request number.
    Pr,
    /// A Loop run id.
    Run,
    /// A task or ticket key.
    Task,
}

impl CoreKey {
    /// Every key in the registry.
    pub const ALL: [Self; 9] = [
        Self::Plugin,
        Self::Trust,
        Self::Session,
        Self::Worktree,
        Self::Item,
        Self::Subject,
        Self::Pr,
        Self::Run,
        Self::Task,
    ];

    /// The key as written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Plugin => "plugin",
            Self::Trust => "trust",
            Self::Session => "session",
            Self::Worktree => "worktree",
            Self::Item => "item",
            Self::Subject => "subject",
            Self::Pr => "pr",
            Self::Run => "run",
            Self::Task => "task",
        }
    }

    /// Whether Core sets this key from the envelope, so a caller never passes it.
    #[must_use]
    pub const fn set_by_core(self) -> bool {
        matches!(
            self,
            Self::Plugin | Self::Trust | Self::Session | Self::Worktree
        )
    }

    /// Whether `value` is what this key holds. `run` and `task` take any valid tag value.
    fn accepts(self, value: &str) -> bool {
        match self {
            Self::Plugin => value.parse::<Namespace>().is_ok(),
            Self::Trust => value.parse::<Level>().is_ok(),
            Self::Session => SessionId::parse(value).is_ok(),
            Self::Worktree => WorktreeId::parse(value).is_ok(),
            Self::Item => ContentId::parse(value).is_ok_and(|id| id.prefix() == &IdPrefix::Item),
            Self::Subject => Digest::parse(value).is_ok(),
            Self::Pr => {
                !value.starts_with('0')
                    && !value.is_empty()
                    && value.bytes().all(|b| b.is_ascii_digit())
            }
            Self::Run | Self::Task => true,
        }
    }
}

impl FromStr for CoreKey {
    type Err = TagError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|key| key.as_str() == text)
            .ok_or(TagError::Unregistered)
    }
}

impl fmt::Display for CoreKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Checks a tag a caller adds to an envelope's event: an unprefixed key from Core's registry
/// that Core does not set itself, with a value of its kind, or `<namespace>.<key>` with a
/// registered namespace and a key in EV-03's name grammar.
pub(crate) fn check_caller_tag(tag: &Tag) -> Result<(), TagError> {
    let (Some(key), Some(value)) = (tag.key(), tag.value()) else {
        return Err(TagError::Unregistered);
    };
    if let Ok(core) = key.parse::<CoreKey>() {
        if core.set_by_core() {
            return Err(TagError::CoreSet { key: core });
        }
        return if core.accepts(value) {
            Ok(())
        } else {
            Err(TagError::Value { key: core })
        };
    }
    match key.split_once('.') {
        Some((namespace, rest))
            if namespace.parse::<Namespace>().is_ok() && is_dotted_name(rest) =>
        {
            Ok(())
        }
        _ => Err(TagError::Unregistered),
    }
}

/// A happenstance `key:value` tag that meets EV-11: a non-empty key with no colon, a non-empty
/// value, no control characters in either, and at most [`MAX_TAG_LEN`] bytes with the colon.
///
/// A value that does not fit is an error, never shortened: weigh-in 1.x truncated tags, which
/// let two items share one (EV-11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag(happenstance_core::Tag);

/// Why a tag was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TagError {
    /// The key or the value is empty.
    #[error("a tag's key and value must both be non-empty")]
    Empty,
    /// The key holds a colon, which would make the split ambiguous.
    #[error("a tag key must not contain a colon")]
    ColonInKey,
    /// The key or the value holds a control character.
    #[error("a tag must not contain control characters")]
    ControlCharacter,
    /// `key:value` is longer than happenstance allows.
    #[error("the tag is {len} bytes, more than the {MAX_TAG_LEN} allowed")]
    TooLong {
        /// The length of `key:value` in bytes.
        len: usize,
    },
    /// happenstance refused it for another reason (such as a bidirectional formatting control).
    #[error("happenstance refused the tag")]
    Refused(#[source] happenstance_core::InvalidTag),
    /// An unprefixed key outside Core's registry, or a prefix that is not a registered
    /// namespace (EV-12).
    #[error("the tag key is neither in Core's registry nor <namespace>.<key>")]
    Unregistered,
    /// A namespaced key outside EV-03's name grammar.
    #[error("a namespaced tag key matches [a-z][a-z0-9_]*(.[a-z][a-z0-9_]*)*")]
    Key,
    /// A value that is not what its registered key holds.
    #[error("the value is not a valid {key} tag value")]
    Value {
        /// The key.
        key: CoreKey,
    },
    /// A key Core sets from the envelope itself.
    #[error("the {key} tag is set by Core from the envelope")]
    CoreSet {
        /// The key.
        key: CoreKey,
    },
}

impl Tag {
    /// The tag `key:value`.
    ///
    /// # Errors
    /// [`TagError`] naming the first rule the tag breaks.
    pub fn new(key: &str, value: &str) -> Result<Self, TagError> {
        if key.is_empty() || value.is_empty() {
            return Err(TagError::Empty);
        }
        if key.contains(':') {
            return Err(TagError::ColonInKey);
        }
        if key.chars().chain(value.chars()).any(char::is_control) {
            return Err(TagError::ControlCharacter);
        }
        let len = key.len().saturating_add(1).saturating_add(value.len());
        if len > MAX_TAG_LEN {
            return Err(TagError::TooLong { len });
        }
        happenstance_core::Tag::key_value(key, value)
            .map(Self)
            .map_err(TagError::Refused)
    }

    /// The tag `key:value` of a key in Core's registry, its value checked against what the key
    /// holds (EV-12).
    ///
    /// # Errors
    /// [`TagError::Value`] for a value of the wrong kind, then as [`Tag::new`].
    pub fn core(key: CoreKey, value: &str) -> Result<Self, TagError> {
        if key.accepts(value) {
            Self::new(key.as_str(), value)
        } else {
            Err(TagError::Value { key })
        }
    }

    /// The tag `<namespace>.<key>:value` (EV-12): `weighin.channel:chat`.
    ///
    /// # Errors
    /// [`TagError::Key`] for a key outside EV-03's name grammar, then as [`Tag::new`].
    pub fn namespaced(namespace: Namespace, key: &str, value: &str) -> Result<Self, TagError> {
        if !is_dotted_name(key) {
            return Err(TagError::Key);
        }
        Self::new(&format!("{namespace}.{key}"), value)
    }

    /// A tag read back from the store, which happenstance validated when it was written.
    pub(crate) const fn stored(tag: happenstance_core::Tag) -> Self {
        Self(tag)
    }

    /// The tag as stored, `key:value`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// The part before the first colon.
    #[must_use]
    pub fn key(&self) -> Option<&str> {
        self.0.key()
    }

    /// The part after the first colon.
    #[must_use]
    pub fn value(&self) -> Option<&str> {
        self.0.value()
    }

    pub(crate) const fn as_happenstance(&self) -> &happenstance_core::Tag {
        &self.0
    }

    pub(crate) fn into_happenstance(self) -> happenstance_core::Tag {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_valid_tag_keeps_its_key_and_value() {
        let tag = Tag::new("item", "wi-3fa9c01b2e").expect("valid");
        assert_eq!(tag.as_str(), "item:wi-3fa9c01b2e");
        assert_eq!(tag.key(), Some("item"));
        assert_eq!(tag.value(), Some("wi-3fa9c01b2e"));
    }

    #[test]
    fn empty_parts_colons_in_keys_and_control_characters_are_refused() {
        assert_eq!(Tag::new("", "v"), Err(TagError::Empty));
        assert_eq!(Tag::new("k", ""), Err(TagError::Empty));
        assert_eq!(Tag::new("a:b", "v"), Err(TagError::ColonInKey));
        assert_eq!(Tag::new("k", "a\nb"), Err(TagError::ControlCharacter));
        assert_eq!(Tag::new("k\t", "v"), Err(TagError::ControlCharacter));
    }

    #[test]
    fn a_value_one_byte_too_long_is_refused_not_truncated() {
        let fits = "v".repeat(MAX_TAG_LEN - "k:".len());
        assert_eq!(
            Tag::new("k", &fits).expect("fits exactly").as_str().len(),
            MAX_TAG_LEN,
            "a tag of exactly MAX_TAG_LEN bytes is kept whole"
        );
        let over = format!("{fits}v");
        assert_eq!(
            Tag::new("k", &over),
            Err(TagError::TooLong {
                len: MAX_TAG_LEN + 1
            })
        );
    }

    #[test]
    fn core_keys_check_their_values() {
        let ok = [
            (CoreKey::Plugin, "weighin"),
            (CoreKey::Trust, "attested"),
            (CoreKey::Session, "gha:1/2/3"),
            (CoreKey::Worktree, "wt-5d41402abc4b"),
            (CoreKey::Item, "wi-3fa9c0"),
            (
                CoreKey::Subject,
                "patch-id:bd234009c1e5a7d4f1c3b2a0e9f8d7c6b5a49382",
            ),
            (CoreKey::Pr, "31"),
            (CoreKey::Run, "r-7"),
            (CoreKey::Task, "WP-8"),
        ];
        for (key, value) in ok {
            let tag = Tag::core(key, value).expect(value);
            assert_eq!(tag.as_str(), format!("{}:{value}", key.as_str()));
        }
        let refused = [
            (CoreKey::Plugin, "anneal"),
            (CoreKey::Trust, "trusted"),
            (CoreKey::Worktree, "redkiln"),
            (CoreKey::Item, "kb-decision-3fa9c01b2e"),
            (CoreKey::Subject, "a commit"),
            (CoreKey::Pr, "031"),
            (CoreKey::Pr, "0"),
            (CoreKey::Pr, "#31"),
        ];
        for (key, value) in refused {
            assert_eq!(
                Tag::core(key, value),
                Err(TagError::Value { key }),
                "{value:?}"
            );
        }
    }

    #[test]
    fn other_keys_are_prefixed_with_their_namespace() {
        let tag = Tag::namespaced(Namespace::Weighin, "channel", "chat").expect("valid");
        assert_eq!(tag.as_str(), "weighin.channel:chat");
        assert_eq!(check_caller_tag(&tag), Ok(()));
        assert_eq!(
            Tag::namespaced(Namespace::Bench, "Point", "1"),
            Err(TagError::Key)
        );
    }

    #[test]
    fn a_caller_tag_must_be_in_the_registry_and_not_set_by_core() {
        let raw = |key, value| Tag::new(key, value).expect("valid happenstance tag");
        assert_eq!(check_caller_tag(&raw("pr", "31")), Ok(()));
        assert_eq!(
            check_caller_tag(&raw("channel", "chat")),
            Err(TagError::Unregistered)
        );
        assert_eq!(
            check_caller_tag(&raw("anneal.wave", "1")),
            Err(TagError::Unregistered)
        );
        assert_eq!(
            check_caller_tag(&raw("weighin.Chan", "1")),
            Err(TagError::Unregistered)
        );
        assert_eq!(
            check_caller_tag(&raw("pr", "x")),
            Err(TagError::Value { key: CoreKey::Pr })
        );
        assert_eq!(
            check_caller_tag(&raw("plugin", "weighin")),
            Err(TagError::CoreSet {
                key: CoreKey::Plugin
            })
        );
    }

    #[test]
    fn a_colon_in_the_value_is_kept() {
        let tag = Tag::new("subject", "sha256:ab").expect("valid");
        assert_eq!(tag.value(), Some("sha256:ab"));
    }
}
