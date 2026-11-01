//! Item ids: `wi-` plus lowercase hex of `sha256(session \n normalised question)`.
//!
//! Hash ids instead of sequence numbers because parallel sessions and worktrees mint ids with
//! no coordinator; sequential ids are what collided in beads, `beads_rust` and Backlog.md. Six hex
//! characters by default; a writer that finds a different question under the same id extends to
//! eight ([`ItemId::mint_with_len`]).

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

/// Default number of hex characters after the `wi-` prefix.
pub const DEFAULT_HEX_LEN: usize = 6;
/// Length used after a collision is detected.
pub const EXTENDED_HEX_LEN: usize = 8;

/// A ledger item id: `wi-` plus lowercase hex. The same session and question always mint the
/// same id (see the [module docs](self)); [`ItemId::parse`] accepts one typed by a person or read
/// from the store.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ItemId(String);

impl ItemId {
    /// Mint the default-length id for a question raised in a session.
    ///
    /// # Examples
    ///
    /// ```
    /// use weighin_core::ItemId;
    ///
    /// let id = ItemId::mint("session-1", "Should retention attach per record?");
    /// assert_eq!(id.as_str().len(), "wi-".len() + 6);
    ///
    /// // Spacing, case and trailing punctuation do not change the question.
    /// assert_eq!(id, ItemId::mint("session-1", "  should retention  attach per record "));
    /// // Another session mints another id for the same question.
    /// assert_ne!(id, ItemId::mint("session-2", "Should retention attach per record?"));
    /// ```
    #[must_use]
    pub fn mint(session: &str, question: &str) -> Self {
        Self::mint_with_len(session, question, DEFAULT_HEX_LEN)
    }

    /// Mint an id with `hex_len` hex characters (clamped to 4..=16).
    #[must_use]
    pub fn mint_with_len(session: &str, question: &str, hex_len: usize) -> Self {
        let digest =
            Sha256::digest(format!("{session}\n{}", normalise_question(question)).as_bytes());
        Self(format!("wi-{}", hex_prefix(&digest, hex_len.clamp(4, 16))))
    }

    /// Accept an id typed by a person or read from the store. Returns `None` when the shape is wrong.
    ///
    /// # Examples
    ///
    /// ```
    /// use weighin_core::ItemId;
    ///
    /// let minted = ItemId::mint("session-1", "Use Postgres?");
    /// let parsed = ItemId::parse(minted.as_str()).ok_or("a minted id parses")?;
    /// assert_eq!(parsed, minted);
    ///
    /// // Uppercase hex, a missing prefix and non-hex characters are rejected.
    /// assert!(ItemId::parse("wi-3FA9C1").is_none());
    /// assert!(ItemId::parse("3fa9c1").is_none());
    /// assert!(ItemId::parse("wi-xyz123").is_none());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        let v = value.trim();
        let hex = v.strip_prefix("wi-")?;
        if (4..=16).contains(&hex.len())
            && hex
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        {
            Some(Self(v.to_owned()))
        } else {
            None
        }
    }

    /// The id as written, `wi-` included.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ItemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Lowercase, collapse whitespace, drop trailing punctuation. Two raises of the same question
/// with different spacing or a missing `?` are one question.
///
/// A trailing `#`, `+` or `%` is kept, because it carries meaning (`C#`, `C++`, `100%`).
#[must_use]
pub fn normalise_question(question: &str) -> String {
    let collapsed = question
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    collapsed
        .trim_end_matches(|c: char| {
            (c.is_ascii_punctuation() && !matches!(c, '#' | '+' | '%')) || c.is_whitespace()
        })
        .to_owned()
}

/// Hash of the normalised question alone, used to spot the same question raised in two sessions.
#[must_use]
pub fn question_key(question: &str) -> String {
    let digest = Sha256::digest(normalise_question(question).as_bytes());
    hex_prefix(&digest, 16)
}

/// The first `len` lowercase hex characters of `bytes`, two per byte.
fn hex_prefix(bytes: &[u8], len: usize) -> String {
    bytes
        .iter()
        .flat_map(|b| [b >> 4, b & 0x0f])
        .filter_map(|nibble| char::from_digit(u32::from(nibble), 16))
        .take(len)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_question_same_session_same_id() {
        let a = ItemId::mint("s1", "Should retention attach per record?");
        let b = ItemId::mint("s1", "  should   retention attach per record ");
        assert_eq!(a, b);
        assert_eq!(a.as_str().len(), 9);
    }

    #[test]
    fn different_session_different_id() {
        assert_ne!(ItemId::mint("s1", "q?"), ItemId::mint("s2", "q?"));
    }

    #[test]
    fn extended_ids_share_a_prefix() {
        let short = ItemId::mint("s", "q");
        let long = ItemId::mint_with_len("s", "q", EXTENDED_HEX_LEN);
        assert!(long.as_str().starts_with(short.as_str()));
    }

    #[test]
    fn parse_rejects_bad_shapes() {
        assert!(ItemId::parse("wi-3fa9c1").is_some());
        assert!(ItemId::parse("wi-3FA9C1").is_none());
        assert!(ItemId::parse("wi-xyz123").is_none());
        assert!(ItemId::parse("3fa9c1").is_none());
    }

    #[test]
    fn minted_ids_are_pinned() {
        let q = "Should retention attach per record?";
        assert_eq!(ItemId::mint("s1", q).as_str(), "wi-1b5df8");
        assert_eq!(ItemId::mint_with_len("s1", q, 4).as_str(), "wi-1b5d");
        assert_eq!(
            ItemId::mint_with_len("s1", q, EXTENDED_HEX_LEN).as_str(),
            "wi-1b5df8ca"
        );
        assert_eq!(
            ItemId::mint_with_len("s1", q, 16).as_str(),
            "wi-1b5df8ca7d8de40d"
        );
    }

    #[test]
    fn requested_length_is_clamped() {
        let q = "Should retention attach per record?";
        assert_eq!(
            ItemId::mint_with_len("s1", q, 0),
            ItemId::mint_with_len("s1", q, 4)
        );
        assert_eq!(
            ItemId::mint_with_len("s1", q, 99),
            ItemId::mint_with_len("s1", q, 16)
        );
    }

    #[test]
    fn question_key_is_pinned() {
        assert_eq!(question_key("Use Postgres?"), "b39477a7169707a1");
    }

    #[test]
    fn parse_round_trips_minted_ids() {
        let id = ItemId::mint("s1", "Should retention attach per record?");
        assert_eq!(ItemId::parse(id.as_str()), Some(id.clone()));
        assert_eq!(ItemId::parse(&format!("  {id} ")), Some(id));
    }

    #[test]
    fn question_key_ignores_session() {
        assert_eq!(question_key("Use Postgres?"), question_key("use postgres"));
    }

    #[test]
    fn trailing_hash_plus_and_percent_carry_meaning() {
        assert_eq!(
            normalise_question("Port the client to C#?"),
            "port the client to c#"
        );
        assert_eq!(
            normalise_question("Port the client to C++?"),
            "port the client to c++"
        );
        assert_eq!(normalise_question("Raise it to 100%?"), "raise it to 100%");
        assert_ne!(
            question_key("Port the client to C#?"),
            question_key("Port the client to C++?")
        );
        assert_eq!(question_key("Which one?"), question_key("Which one"));
        assert_eq!(normalise_question("Which one?!.) "), "which one");
    }
}
