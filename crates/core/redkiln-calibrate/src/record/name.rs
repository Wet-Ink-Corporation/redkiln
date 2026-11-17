//! The two names a record carries whose rules come from elsewhere in the SPEC: its producer
//! (ST-17, one plain folder name) and its subject (ID-09, `<method>:<hash>`).

use std::fmt;
use std::str::FromStr;

/// The ID-09 hash methods a subject may name.
const METHODS: [&str; 5] = [
    "sha256",
    "git-commit",
    "git-tree",
    "patch-id",
    "rust-fingerprint",
];

/// The producer of every anneal v1 row: anneal's rows became Bench's (CA-04).
const V1: &str = "bench";

/// The namespace whose judgement a record is, also its user-scope folder name (ST-17):
/// lowercase letters, digits and `-`, not first.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Producer(String);

/// Text that is not a producer. Holds the text.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0:?} is not a producer (lowercase letters, digits and `-`, not first)")]
pub struct ProducerError(String);

/// The content a judgement binds to, `<method>:<lowercase hex>` of an ID-09 method.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Subject(String);

/// Text that is not a subject. Holds the text.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0:?} is not <method>:<hash>")]
pub struct SubjectError(String);

impl Producer {
    /// The producer of every anneal v1 row.
    pub(crate) fn v1() -> Self {
        Self(V1.to_owned())
    }

    /// The name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The name, owned.
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl TryFrom<String> for Producer {
    type Error = ProducerError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        if crate::scope::is_plain(&text) {
            Ok(Self(text))
        } else {
            Err(ProducerError(text))
        }
    }
}

impl FromStr for Producer {
    type Err = ProducerError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::try_from(text.to_owned())
    }
}

impl fmt::Display for Producer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl ProducerError {
    /// The text refused, handed back.
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl Subject {
    /// The subject's text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The subject's text, owned.
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl TryFrom<String> for Subject {
    type Error = SubjectError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        let well_formed = text.split_once(':').is_some_and(|(method, hash)| {
            METHODS.contains(&method)
                && !hash.is_empty()
                && hash
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        });
        if well_formed {
            Ok(Self(text))
        } else {
            Err(SubjectError(text))
        }
    }
}

impl FromStr for Subject {
    type Err = SubjectError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::try_from(text.to_owned())
    }
}

impl fmt::Display for Subject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl SubjectError {
    /// The text refused, handed back.
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::{Producer, V1};

    #[test]
    fn the_v1_producer_is_a_plain_name() {
        assert_eq!(Producer::try_from(V1.to_owned()), Ok(Producer::v1()));
    }
}
