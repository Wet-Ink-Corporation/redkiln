//! A line's `tags` (SPEC JR-02): the event's caller tags, such as `item`, so an import restores
//! them. Core's own tags (`plugin`, `trust`, `session`, `worktree`) are never in it: they are set
//! from the envelope, on export and import alike. A line with no `tags` key has no caller tags,
//! as every line written before the key existed.

use serde::Deserialize;

use super::line::LineError;
use crate::store::{CoreKey, Tag, check_line_tag};

/// The most tags a line carries: happenstance's guaranteed tags per event.
pub(crate) const MAX_LINE_TAGS: usize = happenstance_core::MIN_SUPPORTED_TAGS_PER_EVENT;

/// The member `"tags":[...]` for a line of an event stored with `tags`: those a caller set, in
/// stored order; `None` when there are none, so such a line has no `tags` key.
///
/// # Errors
/// [`LineError::Envelope`] when the list cannot be written as JSON.
pub(crate) fn member(tags: &[Tag]) -> Result<Option<Vec<u8>>, LineError> {
    let core_set = |tag: &&Tag| {
        tag.key()
            .and_then(|key| key.parse::<CoreKey>().ok())
            .is_some_and(CoreKey::set_by_core)
    };
    let callers: Vec<&str> = tags
        .iter()
        .filter(|tag| !core_set(tag))
        .map(Tag::as_str)
        .collect();
    if callers.is_empty() {
        return Ok(None);
    }
    let mut out = b"\"tags\":".to_vec();
    serde_json::to_writer(&mut out, &callers)
        .map_err(|e| LineError::Envelope(crate::envelope::Unusable::Json(e)))?;
    Ok(Some(out))
}

/// Only the `tags` key of a line; every other key is read elsewhere.
#[derive(Deserialize)]
struct LineTags {
    tags: Option<Vec<String>>,
}

/// The caller tags the line `text` carries; none when it has no `tags` key.
///
/// # Errors
/// [`LineError::Envelope`] when `tags` is not a list of strings; [`LineError::TooManyTags`] past
/// [`MAX_LINE_TAGS`]; [`LineError::CallerTag`] for a tag that is not `key:value`, names a key
/// Core sets (a line never sets its own trust), or a registered key with a value not its kind.
pub(crate) fn read(text: &[u8]) -> Result<Vec<Tag>, LineError> {
    let LineTags { tags } = serde_json::from_slice(text)
        .map_err(|e| LineError::Envelope(crate::envelope::Unusable::Json(e)))?;
    let tags = tags.unwrap_or_default();
    if tags.len() > MAX_LINE_TAGS {
        return Err(LineError::TooManyTags { count: tags.len() });
    }
    tags.iter()
        .map(|text| {
            let (key, value) = text.split_once(':').unwrap_or((text, ""));
            let tag = Tag::new(key, value).map_err(LineError::CallerTag)?;
            check_line_tag(&tag).map_err(LineError::CallerTag)?;
            Ok(tag)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::TagError;

    fn tag(text: &str) -> Tag {
        let (key, value) = text.split_once(':').expect("key:value");
        Tag::new(key, value).expect("a valid tag")
    }

    #[test]
    fn only_a_callers_tags_go_in_the_member() {
        let stored = [
            tag("plugin:weighin"),
            tag("trust:asserted"),
            tag("session:a1"),
            tag("item:wi-3fa9c01b2e"),
            tag("weighin.channel:chat"),
        ];
        assert_eq!(
            member(&stored).expect("written").as_deref(),
            Some(&br#""tags":["item:wi-3fa9c01b2e","weighin.channel:chat"]"#[..])
        );
        assert_eq!(member(&stored[..3]).expect("written"), None, "no key");
    }

    #[test]
    fn a_line_without_tags_has_none_and_one_with_them_reads_them_back() {
        assert!(
            read(br#"{"type":"weighin.item_raised"}"#)
                .expect("read")
                .is_empty()
        );
        let read = read(br#"{"tags":["item:wi-3fa9c01b2e","later.key:v"]}"#).expect("read");
        let read: Vec<&str> = read.iter().map(Tag::as_str).collect();
        assert_eq!(read, ["item:wi-3fa9c01b2e", "later.key:v"]);
    }

    #[test]
    fn a_line_cannot_carry_a_tag_core_sets_or_a_malformed_one() {
        assert!(matches!(
            read(br#"{"tags":["trust:witnessed"]}"#),
            Err(LineError::CallerTag(TagError::CoreSet {
                key: CoreKey::Trust
            }))
        ));
        assert!(matches!(
            read(br#"{"tags":["item:not-an-item"]}"#),
            Err(LineError::CallerTag(TagError::Value { key: CoreKey::Item }))
        ));
        for bad in [&br#"{"tags":["no-colon"]}"#[..], br#"{"tags":["Upper:x"]}"#] {
            assert!(
                matches!(read(bad), Err(LineError::CallerTag(_))),
                "{}",
                String::from_utf8_lossy(bad)
            );
        }
        assert!(matches!(
            read(br#"{"tags":"item:wi-3fa9c01b2e"}"#),
            Err(LineError::Envelope(_))
        ));
        let many: Vec<String> = (0..=MAX_LINE_TAGS)
            .map(|n| format!("\"task:{n}\""))
            .collect();
        let many = format!("{{\"tags\":[{}]}}", many.join(","));
        assert!(matches!(
            read(many.as_bytes()),
            Err(LineError::TooManyTags { count }) if count == MAX_LINE_TAGS + 1
        ));
    }
}
