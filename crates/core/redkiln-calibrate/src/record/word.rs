//! The short words a record names things with (CA-02): 1 to 64 of `[A-Za-z0-9_.:-]`, the
//! charset Bench's decision ids and outcome labels already use.

use std::fmt;
use std::str::FromStr;

/// The most bytes in a word.
const MAX_WORD_BYTES: usize = 64;

/// Text that is not 1 to 64 of `[A-Za-z0-9_.:-]`. Holds the text.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0:?} is not 1 to 64 of [A-Za-z0-9_.:-]")]
pub struct WordError(String);

/// `text` when it is a word.
fn word(text: String) -> Result<String, WordError> {
    let fits = (1..=MAX_WORD_BYTES).contains(&text.len())
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b':' | b'-'));
    if fits { Ok(text) } else { Err(WordError(text)) }
}

macro_rules! word_type {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            /// The text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// The text, owned.
            #[must_use]
            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = WordError;

            fn try_from(text: String) -> Result<Self, Self::Error> {
                word(text).map(Self)
            }
        }

        impl FromStr for $name {
            type Err = WordError;

            fn from_str(text: &str) -> Result<Self, Self::Err> {
                Self::try_from(text.to_owned())
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

word_type! {
    /// A judgement's id, unique per producer; labels name it (`d1760000000000-1-0`).
    JudgementId
}

word_type! {
    /// What turned out right, in the producer's vocabulary (`yes`, an option's name).
    Outcome
}

word_type! {
    /// A decision point or question type, in the producer's vocabulary (`D4`, `noul`).
    Vocab
}
