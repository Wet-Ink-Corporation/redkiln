//! A git object name: 40 (SHA-1) or 64 (SHA-256) lowercase hex digits.

use std::fmt;

use super::RefsError;

/// A git object name, as git printed it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Oid(String);

impl Oid {
    /// `text` when it is 40 or 64 lowercase hex digits.
    pub(crate) fn parse(text: &str) -> Result<Self, RefsError> {
        let hex = text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        if hex && matches!(text.len(), 40 | 64) {
            Ok(Self(text.to_owned()))
        } else {
            Err(RefsError::Answer {
                step: "an object name",
                got: text.to_owned(),
            })
        }
    }

    /// The one name git printed, its line end trimmed.
    pub(crate) fn from_output(stdout: &[u8]) -> Result<Self, RefsError> {
        Self::parse(String::from_utf8_lossy(stdout).trim())
    }

    /// The name.
    #[must_use]
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Oid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_full_lowercase_hex_names_parse() {
        let sha1 = "a".repeat(40);
        let sha256 = "0".repeat(64);
        assert_eq!(Oid::parse(&sha1).expect("sha-1").as_str(), sha1);
        assert_eq!(Oid::parse(&sha256).expect("sha-256").as_str(), sha256);
        for bad in ["", "abc", &"A".repeat(40), &"g".repeat(40), &"a".repeat(41)] {
            assert!(Oid::parse(bad).is_err(), "{bad}");
        }
        assert_eq!(
            Oid::from_output(format!("{sha1}\n").as_bytes()).expect("trimmed"),
            Oid::parse(&sha1).expect("sha-1")
        );
    }
}
