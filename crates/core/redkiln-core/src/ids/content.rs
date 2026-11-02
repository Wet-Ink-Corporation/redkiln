//! Content-hash ids (SPEC ID-04).

use std::fmt;

use sha2::{Digest as _, Sha256};

use super::{IdError, is_lower_hex, to_hex};

/// A registered content-id prefix (ID-04). The third, `wt-`, has its own derivation and type,
/// [`super::WorktreeId`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IdPrefix {
    /// `wi-`: a Weigh-In item.
    Item,
    /// `kb-<kind>-`: a Knowledge atom of `kind`.
    Atom(AtomKind),
}

impl IdPrefix {
    /// The prefix as written, dash included.
    #[must_use]
    pub fn to_text(&self) -> String {
        match self {
            Self::Item => "wi-".to_owned(),
            Self::Atom(kind) => format!("kb-{kind}-"),
        }
    }
}

/// A Knowledge atom kind (`decision` in `kb-decision-…`): a lowercase ASCII word,
/// `[a-z][a-z0-9]*`, so the dash after it ends it. Knowledge defines the kinds (its call C).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AtomKind(String);

impl AtomKind {
    /// The kind `kind`.
    ///
    /// # Errors
    /// [`IdError::AtomKind`] unless `kind` is a lowercase ASCII word.
    pub fn new(kind: &str) -> Result<Self, IdError> {
        let mut bytes = kind.bytes();
        let starts = bytes.next().is_some_and(|b| b.is_ascii_lowercase());
        if starts && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()) {
            Ok(Self(kind.to_owned()))
        } else {
            Err(IdError::AtomKind)
        }
    }

    /// The kind as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AtomKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// How many hex characters a new id takes: 10, extended to 12, then 16, when the same id
/// already names a different input (ID-04).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HexLen {
    /// 10 hex characters, every new id's length.
    Ten,
    /// 12, after a collision at 10.
    Twelve,
    /// 16, after a collision at 12.
    Sixteen,
}

impl HexLen {
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

/// The hex lengths ids are read at: today's 6 and 8, and the 10, 12 and 16 of new ids.
const READ_LENGTHS: [usize; 5] = [6, 8, 10, 12, 16];

/// A content-hash id: a registered prefix and the first hex characters of a sha256 (ID-04).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContentId {
    prefix: IdPrefix,
    text: String,
}

impl ContentId {
    /// Mints the id of `fields` under `domain` (`redkiln.id/weighin.item/1`): the prefix and the
    /// first `len` hex characters of the sha256 of [`content_input`].
    ///
    /// # Errors
    /// [`IdError::Domain`] for a domain outside printable ASCII.
    pub fn mint(
        prefix: IdPrefix,
        domain: &str,
        fields: &[&str],
        len: HexLen,
    ) -> Result<Self, IdError> {
        let digest: [u8; 32] = Sha256::digest(content_input(domain, fields)?).into();
        Ok(Self::from_digest(prefix, &digest, len))
    }

    /// The id of an input hashed elsewhere: the prefix and the first `len` hex characters of
    /// `digest`. Weigh-In's captured items take this path, since their input keeps its old form
    /// (ID-04's exception).
    #[must_use]
    pub fn from_digest(prefix: IdPrefix, digest: &[u8; 32], len: HexLen) -> Self {
        let mut text = prefix.to_text();
        text.extend(to_hex(digest).chars().take(len.get()));
        Self { prefix, text }
    }

    /// Reads an id at the length it was written: 6, 8, 10, 12 or 16 hex characters.
    ///
    /// # Errors
    /// [`IdError::Prefix`] for an unregistered prefix, [`IdError::AtomKind`] for a bad atom
    /// kind, [`IdError::ContentHex`] for a hash part that is not lowercase hex of a read length.
    pub fn parse(text: &str) -> Result<Self, IdError> {
        let (prefix, hex) = if let Some(hex) = text.strip_prefix("wi-") {
            (IdPrefix::Item, hex)
        } else if let Some(rest) = text.strip_prefix("kb-") {
            let (kind, hex) = rest.split_once('-').ok_or(IdError::Prefix)?;
            (IdPrefix::Atom(AtomKind::new(kind)?), hex)
        } else {
            return Err(IdError::Prefix);
        };
        if !is_lower_hex(hex) || !READ_LENGTHS.contains(&hex.len()) {
            return Err(IdError::ContentHex);
        }
        Ok(Self {
            prefix,
            text: text.to_owned(),
        })
    }

    /// The id as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Its prefix.
    #[must_use]
    pub const fn prefix(&self) -> &IdPrefix {
        &self.prefix
    }

    /// Its hash part, after the prefix.
    #[must_use]
    pub fn hex(&self) -> &str {
        self.text
            .rsplit_once('-')
            .map_or(self.text.as_str(), |(_, hex)| hex)
    }
}

impl fmt::Display for ContentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

/// The domain-separated input of a content id (ID-04): `domain`'s bytes, then each field as its
/// UTF-8 byte length (a 64-bit big-endian unsigned integer) and its bytes.
///
/// The domain is printable ASCII with no spaces, so it never holds the 0x00 a field's length
/// starts with, and the encoding is injective.
///
/// # Errors
/// [`IdError::Domain`] for an empty domain or one with a byte outside `0x21..=0x7e`.
pub fn content_input(domain: &str, fields: &[&str]) -> Result<Vec<u8>, IdError> {
    if domain.is_empty() || !domain.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(IdError::Domain);
    }
    let size = fields.iter().fold(domain.len(), |n, f| {
        n.saturating_add(8).saturating_add(f.len())
    });
    let mut input = Vec::with_capacity(size);
    input.extend_from_slice(domain.as_bytes());
    for field in fields {
        // A `usize` length always fits in 64 bits on the targets Rust supports.
        let len = u64::try_from(field.len()).unwrap_or(u64::MAX);
        input.extend_from_slice(&len.to_be_bytes());
        input.extend_from_slice(field.as_bytes());
    }
    Ok(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_input_is_the_domain_then_each_field_with_its_length() {
        let input = content_input("redkiln.id/weighin.item/1", &["weighin", "ab"]).expect("valid");
        let mut expected = b"redkiln.id/weighin.item/1".to_vec();
        expected.extend_from_slice(&7u64.to_be_bytes());
        expected.extend_from_slice(b"weighin");
        expected.extend_from_slice(&2u64.to_be_bytes());
        expected.extend_from_slice(b"ab");
        assert_eq!(input, expected);
    }

    #[test]
    fn moving_bytes_between_fields_changes_the_input() {
        let a = content_input("d", &["ab", "c"]).expect("valid");
        let b = content_input("d", &["a", "bc"]).expect("valid");
        assert_ne!(a, b, "the length prefixes keep the fields apart");
    }

    #[test]
    fn a_domain_outside_printable_ascii_is_refused() {
        for domain in ["", "a b", "a\0b", "caf\u{e9}"] {
            assert_eq!(
                content_input(domain, &[]),
                Err(IdError::Domain),
                "{domain:?}"
            );
        }
    }

    #[test]
    fn a_new_id_is_the_prefix_and_the_first_ten_hex_of_the_sha256() {
        let id = ContentId::mint(
            IdPrefix::Item,
            "redkiln.id/weighin.item/1",
            &["x"],
            HexLen::Ten,
        )
        .expect("valid");
        let input = content_input("redkiln.id/weighin.item/1", &["x"]).expect("valid");
        let hex = to_hex(&Sha256::digest(&input));
        assert_eq!(
            id.as_str(),
            format!("wi-{}", hex.get(..10).expect("64 hex"))
        );
        assert_eq!(id.hex().len(), 10);
        assert_eq!(id.prefix(), &IdPrefix::Item);
    }

    #[test]
    fn a_collision_extends_the_id_to_twelve_then_sixteen() {
        assert_eq!(HexLen::Ten.extended(), Some(HexLen::Twelve));
        assert_eq!(HexLen::Twelve.extended(), Some(HexLen::Sixteen));
        assert_eq!(HexLen::Sixteen.extended(), None);
        let digest = [0xab; 32];
        let long = ContentId::from_digest(IdPrefix::Item, &digest, HexLen::Sixteen);
        assert_eq!(long.as_str(), "wi-abababababababab");
    }

    #[test]
    fn a_weighin_captured_item_keeps_its_own_input() {
        // ID-04's exception: sha256(session \n normalised question), no domain, no lengths, so a
        // 10-hex id extends the 6-hex id it always was.
        let digest: [u8; 32] = Sha256::digest(b"s1\nwhich store?").into();
        let ten = ContentId::from_digest(IdPrefix::Item, &digest, HexLen::Ten);
        let six = to_hex(&digest);
        assert!(
            ten.hex().starts_with(six.get(..6).expect("64 hex")),
            "{ten:?}"
        );
    }

    #[test]
    fn ids_are_read_at_the_length_they_were_written() {
        for text in [
            "wi-3fa9c0",
            "wi-3fa9c01b",
            "wi-3fa9c01b2e",
            "wi-3fa9c01b2e4d",
            "wi-3fa9c01b2e4d5f6a",
        ] {
            let id = ContentId::parse(text).expect(text);
            assert_eq!(id.as_str(), text);
            assert_eq!(id.prefix(), &IdPrefix::Item);
        }
        let atom = ContentId::parse("kb-decision-3fa9c01b2e").expect("an atom id");
        assert_eq!(
            atom.prefix(),
            &IdPrefix::Atom(AtomKind::new("decision").expect("a kind"))
        );
        assert_eq!(atom.hex(), "3fa9c01b2e");
    }

    #[test]
    fn unregistered_prefixes_and_odd_lengths_are_refused() {
        assert_eq!(ContentId::parse("xx-3fa9c01b2e"), Err(IdError::Prefix));
        assert_eq!(
            ContentId::parse("wt-3fa9c01b2e4d"),
            Err(IdError::Prefix),
            "a WorktreeId"
        );
        assert_eq!(
            ContentId::parse("kb-3fa9c01b2e"),
            Err(IdError::Prefix),
            "no kind"
        );
        assert_eq!(
            ContentId::parse("kb-Decision-3fa9c01b2e"),
            Err(IdError::AtomKind)
        );
        for text in [
            "wi-3fa9c",
            "wi-3fa9c01",
            "wi-3FA9C01B2E",
            "wi-",
            "wi-3fa9c01b2e4d5f6a7b",
        ] {
            assert_eq!(ContentId::parse(text), Err(IdError::ContentHex), "{text:?}");
        }
    }

    #[test]
    fn a_minted_atom_id_carries_its_kind() {
        let kind = AtomKind::new("decision").expect("a kind");
        let id = ContentId::from_digest(IdPrefix::Atom(kind), &[0u8; 32], HexLen::Ten);
        assert_eq!(id.as_str(), "kb-decision-0000000000");
        assert_eq!(AtomKind::new("de-cision"), Err(IdError::AtomKind));
        assert_eq!(AtomKind::new(""), Err(IdError::AtomKind));
    }
}
