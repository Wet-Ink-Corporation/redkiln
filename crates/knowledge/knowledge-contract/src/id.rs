//! Atom ids and the paths they set (Knowledge calls B and C, SPEC ID-04).

use std::fmt;

use sha2::{Digest as _, Sha256};

use crate::kind::Kind;
use crate::slug::slug;

/// How many hex characters a hashed id takes: 10, extended to 12, then 16, when the same id
/// already names a different producer key (ID-04).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashLen {
    /// 10 hex characters, every new id's length.
    Ten,
    /// 12, after a collision at 10.
    Twelve,
    /// 16, after a collision at 12.
    Sixteen,
}

impl HashLen {
    /// The number of hex characters.
    #[must_use]
    pub const fn get(self) -> usize {
        match self {
            Self::Ten => 10,
            Self::Twelve => 12,
            Self::Sixteen => 16,
        }
    }

    /// The length to try after a collision at this one, if there is one.
    #[must_use]
    pub const fn extended(self) -> Option<Self> {
        match self {
            Self::Ten => Some(Self::Twelve),
            Self::Twelve => Some(Self::Sixteen),
            Self::Sixteen => None,
        }
    }
}

/// An atom id, `kb-<kind>-<key>`.
///
/// The key is either a producer's own key kept as it is (today's `kb-decision-wi-4dfe38`, whose
/// key is the 6-hex item id) or a content hash of the producer's key (`kb-decision-<10 hex>`).
/// Ids already written are kept forever; new ones are hashed (Knowledge call C).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AtomId {
    kind: Kind,
    key: String,
}

impl AtomId {
    /// The id that keeps `key` as written: `kb-<kind>-<key>`. Weigh-In uses it for items whose
    /// `wi-` id is shorter than 10 hex, so atoms and citations made before the hashed form keep
    /// their ids.
    ///
    /// Returns `None` unless `key` is lowercase ASCII letters, digits and single dashes, starting
    /// and ending with a letter or digit, so the id is also a safe file-name stem.
    ///
    /// ```
    /// use knowledge_contract::{AtomId, Kind};
    /// let id = AtomId::kept(Kind::Decision, "wi-4dfe38").map(|id| id.to_string());
    /// assert_eq!(id.as_deref(), Some("kb-decision-wi-4dfe38"));
    /// assert_eq!(AtomId::kept(Kind::Decision, "../x"), None);
    /// ```
    #[must_use]
    pub fn kept(kind: Kind, key: &str) -> Option<Self> {
        let word = |w: &str| {
            !w.is_empty()
                && w.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        };
        key.split('-').all(word).then(|| Self {
            kind,
            key: key.to_owned(),
        })
    }

    /// The hashed id of `producer_key`: `kb-<kind>-` and the first `len` hex characters of the
    /// sha256 of ID-04's input, the domain `redkiln.id/knowledge.<kind>/1` and then
    /// `producer_key` as its UTF-8 byte length (64-bit big-endian) and its bytes.
    ///
    /// ```
    /// use knowledge_contract::{AtomId, HashLen, Kind};
    /// let id = AtomId::hashed(Kind::Decision, "wi-4dfe38a1b2", HashLen::Ten);
    /// assert_eq!(id.key().len(), 10);
    /// assert_eq!(id, AtomId::hashed(Kind::Decision, "wi-4dfe38a1b2", HashLen::Ten));
    /// ```
    #[must_use]
    pub fn hashed(kind: Kind, producer_key: &str, len: HashLen) -> Self {
        let domain = format!("redkiln.id/knowledge.{}/1", kind.as_str());
        // A `usize` length always fits in 64 bits on the targets Rust supports.
        let field_len = u64::try_from(producer_key.len()).unwrap_or(u64::MAX);
        let mut hasher = Sha256::new();
        hasher.update(domain.as_bytes());
        hasher.update(field_len.to_be_bytes());
        hasher.update(producer_key.as_bytes());
        let key = hex(&hasher.finalize()).take(len.get()).collect();
        Self { kind, key }
    }

    /// The atom's kind.
    #[must_use]
    pub const fn kind(&self) -> Kind {
        self.kind
    }

    /// The part after `kb-<kind>-`: the kept key or the hash.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Where the atom with this id and `title` lives, relative to the repository root:
    /// `.kb/<kind>/<key>-<slug of title>.md`. The path never changes once written: status is a
    /// field, not a folder (Knowledge call B).
    ///
    /// ```
    /// use knowledge_contract::{AtomId, Kind};
    /// let path = AtomId::kept(Kind::Decision, "wi-4dfe38")
    ///     .map(|id| id.path("Where should the ledger live?"));
    /// assert_eq!(
    ///     path.as_deref(),
    ///     Some(".kb/decision/wi-4dfe38-where-should-the-ledger-live.md")
    /// );
    /// ```
    #[must_use]
    pub fn path(&self, title: &str) -> String {
        format!("{}/{}-{}.md", self.kind.dir(), self.key, slug(title))
    }
}

impl fmt::Display for AtomId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "kb-{}-{}", self.kind, self.key)
    }
}

/// The lowercase hex characters of `bytes`, two per byte.
fn hex(bytes: &[u8]) -> impl Iterator<Item = char> + '_ {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    bytes.iter().flat_map(|&b| {
        [b >> 4, b & 0x0f].map(|n| DIGITS.get(usize::from(n)).map_or('0', |&d| char::from(d)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hashed_id_is_the_id_04_digest_of_the_domain_and_the_key() {
        let mut input = b"redkiln.id/knowledge.decision/1".to_vec();
        input.extend_from_slice(&9u64.to_be_bytes());
        input.extend_from_slice(b"wi-abcdef");
        let expected = Sha256::digest(&input)
            .iter()
            .fold(String::new(), |mut hex, byte| {
                fmt::Write::write_fmt(&mut hex, format_args!("{byte:02x}"))
                    .expect("writing to a String cannot fail");
                hex
            });

        let id = AtomId::hashed(Kind::Decision, "wi-abcdef", HashLen::Sixteen);

        assert_eq!(id.key(), expected.get(..16).expect("64 hex characters"));
        assert_eq!(id.to_string(), format!("kb-decision-{}", id.key()));
    }

    #[test]
    fn a_longer_hash_extends_the_shorter_one() {
        let ten = AtomId::hashed(Kind::OpenQuestion, "wi-0123456789", HashLen::Ten);
        let twelve = AtomId::hashed(Kind::OpenQuestion, "wi-0123456789", HashLen::Twelve);
        assert_eq!(ten.key().len(), 10);
        assert_eq!(twelve.key().len(), 12);
        assert!(twelve.key().starts_with(ten.key()));
        assert_eq!(HashLen::Ten.extended(), Some(HashLen::Twelve));
        assert_eq!(HashLen::Sixteen.extended(), None);
    }

    #[test]
    fn the_kind_separates_hashed_ids_of_one_key() {
        let decision = AtomId::hashed(Kind::Decision, "wi-0123456789", HashLen::Ten);
        let open = AtomId::hashed(Kind::OpenQuestion, "wi-0123456789", HashLen::Ten);
        assert_ne!(decision.key(), open.key());
    }

    #[test]
    fn a_kept_key_refuses_anything_unsafe_in_a_file_name() {
        for bad in ["", "-a", "a-", "a--b", "A", "a/b", "a.b", "a b", "é"] {
            assert_eq!(AtomId::kept(Kind::Decision, bad), None, "{bad:?}");
        }
        assert_eq!(
            AtomId::kept(Kind::OpenQuestion, "wi-4dfe38").map(|id| id.to_string()),
            Some("kb-open-question-wi-4dfe38".to_owned())
        );
    }

    #[test]
    fn a_hashed_id_lives_under_its_kind() {
        let id = AtomId::hashed(Kind::OpenQuestion, "wi-0123456789", HashLen::Ten);
        assert_eq!(
            id.path("Ship it?"),
            format!(".kb/open-question/{}-ship-it.md", id.key())
        );
    }
}
