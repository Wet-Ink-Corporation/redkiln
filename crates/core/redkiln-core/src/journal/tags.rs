//! A line's `tags` (SPEC JR-02): the event's caller tags, such as `item`, so an import restores
//! them. Core's own tags (`plugin`, `trust`, `session`, `worktree`) are never in it: they are set
//! from the envelope, on export and import alike. A line with no `tags` key has no caller tags,
//! as every line written before the key existed.

use serde::Deserialize;

use super::line::LineError;
use crate::store::{MAX_CALLER_TAGS, Tag, check_line_tag, is_core_set};

/// The most caller tags a line carries, the cap a write holds too ([`MAX_CALLER_TAGS`]).
pub(crate) const MAX_LINE_TAGS: usize = MAX_CALLER_TAGS;

/// The member `"tags":[...]` for a line of an event stored with `tags`: those a caller set,
/// sorted, each once, so one fact has one line in every journal; `None` when there are none, so
/// such a line has no `tags` key.
///
/// # Errors
/// What an import would refuse, so the event is not exported (it counts as unexportable):
/// [`LineError::TooManyTags`] past [`MAX_LINE_TAGS`], [`LineError::CallerTag`] for a tag
/// [`check_line_tag`] refuses. [`LineError::Envelope`] when the list cannot be written as JSON.
pub(crate) fn member(tags: &[Tag]) -> Result<Option<Vec<u8>>, LineError> {
    let mut callers: Vec<&Tag> = tags.iter().filter(|tag| !is_core_set(tag)).collect();
    callers.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    callers.dedup_by(|a, b| a.as_str() == b.as_str());
    if callers.len() > MAX_LINE_TAGS {
        return Err(LineError::TooManyTags {
            count: callers.len(),
        });
    }
    for tag in &callers {
        check_line_tag(tag).map_err(LineError::CallerTag)?;
    }
    if callers.is_empty() {
        return Ok(None);
    }
    let callers: Vec<&str> = callers.into_iter().map(Tag::as_str).collect();
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
    use crate::store::{CoreKey, TagError};

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
    fn the_member_is_sorted_each_once_and_refuses_what_no_import_reads() {
        let stored = [tag("task:b"), tag("item:wi-3fa9c01b2e"), tag("task:b")];
        assert_eq!(
            member(&stored).expect("written").as_deref(),
            Some(&br#""tags":["item:wi-3fa9c01b2e","task:b"]"#[..])
        );
        let many: Vec<Tag> = (0..=MAX_LINE_TAGS)
            .map(|n| tag(&format!("task:{n}")))
            .collect();
        assert!(matches!(
            member(&many),
            Err(LineError::TooManyTags { count }) if count == MAX_LINE_TAGS + 1
        ));
        assert!(matches!(
            member(&[tag("Upper:x")]),
            Err(LineError::CallerTag(TagError::Key))
        ));
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
