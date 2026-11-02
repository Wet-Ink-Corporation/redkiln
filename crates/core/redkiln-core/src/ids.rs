//! Ids and digests (SPEC ID-01 to ID-08).
//!
//! - Stores and events (ID-01 to ID-03): happenstance's [`StoreId`] and [`EventId`], whose text
//!   forms are read here ([`parse_store_id`], [`parse_event_ref`]) and written by their
//!   `Display`.
//! - Content-hash ids (ID-04): [`ContentId`], under a registered [`IdPrefix`].
//! - Session ids (ID-05): [`SessionId`].
//! - Worktree ids (ID-06): [`WorktreeId`], from a top-level path the caller found.
//! - Digests (ID-07): [`Digest`]. The canonical JSON digest (ID-08) is [`crate::canonical`].

mod content;
mod digest;
mod session;
mod worktree;

use happenstance_core::{EventId, SequencePosition, StoreId};

pub use content::{AtomKind, ContentId, HexLen, IdPrefix, content_input};
pub use digest::Digest;
pub use session::SessionId;
pub use worktree::{Host, WorktreeId};

/// Why a text is not the id it was read as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum IdError {
    /// Not a store id: 32 lowercase hex characters, no dashes (ID-01).
    #[error("a store id is 32 lowercase hex characters with no dashes")]
    StoreId,
    /// Not an event ref: `<store-id>:<position>`, the position a decimal from 1 with no sign or
    /// leading zero (ID-02, ID-03).
    #[error("an event ref is <store-id>:<position>, the position a decimal from 1")]
    EventRef,
    /// Not under a registered content-id prefix (`wi-`, `kb-<kind>-`; `wt-` is a [`WorktreeId`]).
    #[error("not a registered content-id prefix (wi-, kb-<kind>-)")]
    Prefix,
    /// A knowledge atom kind that is not a lowercase ASCII word (`[a-z][a-z0-9]*`).
    #[error("an atom kind is a lowercase ASCII word")]
    AtomKind,
    /// A content id's hash part is not lowercase hex of a length ids are read at (6, 8, 10, 12
    /// or 16).
    #[error("a content id's hash is 6, 8, 10, 12 or 16 lowercase hex characters")]
    ContentHex,
    /// A derivation domain that is empty or holds a character outside printable ASCII.
    #[error("a derivation domain is non-empty printable ASCII with no spaces")]
    Domain,
    /// A session id with a control character, or too long to be a tag value.
    #[error("a session id holds no control characters and fits in a tag")]
    Session,
    /// A worktree's path is not UTF-8.
    #[error("the worktree path is not UTF-8")]
    NotUtf8,
    /// A worktree's path is not absolute.
    #[error("the worktree path is not absolute")]
    NotAbsolute,
    /// Not a worktree id: `wt-` and 12 lowercase hex characters (ID-06).
    #[error("a worktree id is wt- and 12 lowercase hex characters")]
    Worktree,
    /// Not a digest: `<method>:<lowercase hex>`, 64 hex for `sha256` (ID-07).
    #[error("a digest is <method>:<lowercase hex>")]
    Digest,
}

/// Reads a store id written as ID-01 says: 32 lowercase hex characters, no dashes.
///
/// # Errors
/// [`IdError::StoreId`] for any other text, uppercase and UUID spellings included.
pub fn parse_store_id(text: &str) -> Result<StoreId, IdError> {
    let Some(bytes) = decode_hex::<16>(text) else {
        return Err(IdError::StoreId);
    };
    Ok(StoreId::from_bytes(bytes))
}

/// Reads an event ref, `<store-id>:<position>` (ID-02, ID-03), the form [`EventId`]'s `Display`
/// writes. One value has one spelling: no sign, no leading zero, no position 0.
///
/// # Errors
/// [`IdError::EventRef`] for any other text.
pub fn parse_event_ref(text: &str) -> Result<EventId, IdError> {
    let (store, position) = text.split_once(':').ok_or(IdError::EventRef)?;
    let store = decode_hex::<16>(store)
        .map(StoreId::from_bytes)
        .ok_or(IdError::EventRef)?;
    let canonical = !position.starts_with('0') && position.bytes().all(|b| b.is_ascii_digit());
    let position = canonical
        .then(|| position.parse::<u64>().ok())
        .flatten()
        .and_then(SequencePosition::new)
        .ok_or(IdError::EventRef)?;
    Ok(EventId::new(store, position))
}

/// `N` bytes from exactly `2 * N` lowercase hex characters.
fn decode_hex<const N: usize>(text: &str) -> Option<[u8; N]> {
    if text.len() != N.checked_mul(2)? || !is_lower_hex(text) {
        return None;
    }
    let mut bytes = [0u8; N];
    let (pairs, _) = text.as_bytes().as_chunks::<2>();
    for (slot, pair) in bytes.iter_mut().zip(pairs) {
        let pair = std::str::from_utf8(pair).ok()?;
        *slot = u8::from_str_radix(pair, 16).ok()?;
    }
    Some(bytes)
}

/// Non-empty, and only `0-9a-f`.
pub(crate) fn is_lower_hex(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Lowercase hex of `bytes`.
pub(crate) fn to_hex(bytes: &[u8]) -> String {
    // Every nibble is below 16, so `from_digit` always gives a (lowercase) digit.
    bytes
        .iter()
        .flat_map(|b| [b >> 4, b & 0x0f])
        .filter_map(|nibble| char::from_digit(u32::from(nibble), 16))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const STORE: &str = "0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f";

    #[test]
    fn a_store_id_reads_from_its_text_and_writes_it_back() {
        let id = parse_store_id(STORE).expect("valid");
        assert_eq!(id.to_string(), STORE);
    }

    #[test]
    fn hex_is_written_lowercase() {
        assert_eq!(to_hex(&[0x0a, 0xff, 0x00]), "0aff00");
    }

    #[test]
    fn a_store_id_in_any_other_spelling_is_refused() {
        for text in [
            "",
            "0F3C9A1E6B7D4C2A8E5F1B3D7C9A2E4F",
            "0f3c9a1e-6b7d-4c2a-8e5f-1b3d7c9a2e4f",
            "0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4",
            "0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f0",
            "0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4g",
        ] {
            assert_eq!(parse_store_id(text), Err(IdError::StoreId), "{text:?}");
        }
    }

    #[test]
    fn an_event_ref_is_the_store_id_a_colon_and_the_position() {
        let id = parse_event_ref(&format!("{STORE}:41")).expect("valid");
        assert_eq!(id.store(), parse_store_id(STORE).expect("valid"));
        assert_eq!(id.position(), SequencePosition::new(41).expect("non-zero"));
        assert_eq!(
            id.to_string(),
            format!("{STORE}:41"),
            "written as it is read"
        );
    }

    #[test]
    fn an_event_ref_with_a_non_canonical_position_is_refused() {
        for position in [
            "",
            "0",
            "041",
            "+41",
            "-1",
            "4 1",
            "18446744073709551616",
            "x",
        ] {
            assert_eq!(
                parse_event_ref(&format!("{STORE}:{position}")),
                Err(IdError::EventRef),
                "{position:?}"
            );
        }
        assert_eq!(parse_event_ref(STORE), Err(IdError::EventRef), "no colon");
        assert_eq!(
            parse_event_ref("0F3C9A1E6B7D4C2A8E5F1B3D7C9A2E4F:1"),
            Err(IdError::EventRef),
            "uppercase store id"
        );
    }
}
