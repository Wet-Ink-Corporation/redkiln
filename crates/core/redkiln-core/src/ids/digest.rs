//! Digests (SPEC ID-07).

use std::fmt;

use sha2::{Digest as _, Sha256};

use super::{IdError, is_lower_hex, to_hex};

/// The method name of a file's bytes, and of the canonical JSON digest (ID-08).
const SHA256: &str = "sha256";

/// The hex length of a sha256.
const SHA256_HEX: usize = 64;

/// A digest, `<method>:<lowercase hex>` (ID-07): `sha256:<64 hex>` for a file's bytes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Digest {
    /// Always one colon or more, the first after a valid method name.
    text: String,
}

impl Digest {
    /// The `sha256` digest of `bytes`.
    #[must_use]
    pub fn sha256(bytes: &[u8]) -> Self {
        Self {
            text: format!("{SHA256}:{}", to_hex(&Sha256::digest(bytes))),
        }
    }

    /// Reads a digest: a method name (`[a-z][a-z0-9-]*`), a colon, and lowercase hex, 64
    /// characters of it for `sha256`.
    ///
    /// # Errors
    /// [`IdError::Digest`] for any other text.
    pub fn parse(text: &str) -> Result<Self, IdError> {
        let (method, hex) = text.split_once(':').ok_or(IdError::Digest)?;
        let mut name = method.bytes();
        let named = name.next().is_some_and(|b| b.is_ascii_lowercase())
            && name.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        let sized = method != SHA256 || hex.len() == SHA256_HEX;
        if named && sized && is_lower_hex(hex) {
            Ok(Self {
                text: text.to_owned(),
            })
        } else {
            Err(IdError::Digest)
        }
    }

    /// The digest as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// The method, before the colon.
    #[must_use]
    pub fn method(&self) -> &str {
        self.text.split_once(':').map_or("", |(method, _)| method)
    }

    /// The lowercase hex, after the colon.
    #[must_use]
    pub fn hex(&self) -> &str {
        self.text.split_once(':').map_or("", |(_, hex)| hex)
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_digest_is_sha256_and_64_lowercase_hex() {
        let digest = Digest::sha256(b"abc");
        assert_eq!(
            digest.as_str(),
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(digest.method(), "sha256");
        assert_eq!(digest.hex().len(), 64);
    }

    #[test]
    fn a_digest_reads_back_from_its_text() {
        let text = "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert_eq!(Digest::parse(text), Ok(Digest::sha256(b"abc")));
        let other =
            Digest::parse("git-commit:bd234009c1e5a7d4f1c3b2a0e9f8d7c6b5a49382").expect("a method");
        assert_eq!(other.method(), "git-commit");
    }

    #[test]
    fn malformed_digests_are_refused() {
        for text in [
            "",
            "sha256",
            "sha256:",
            "sha256:BA7816BF",
            "sha256:ba7816bf",
            "SHA256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            ":ab",
            "1x:ab",
            "git commit:ab",
        ] {
            assert_eq!(Digest::parse(text), Err(IdError::Digest), "{text:?}");
        }
    }
}
