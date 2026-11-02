//! `key:value` tags (SPEC EV-11): validated whole, never truncated to fit.

use happenstance_core::MAX_TAG_LEN;

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
    fn a_colon_in_the_value_is_kept() {
        let tag = Tag::new("subject", "sha256:ab").expect("valid");
        assert_eq!(tag.value(), Some("sha256:ab"));
    }
}
