//! The closed set of top-level `skillcheck` commands and what a green result from each claims.
//!
//! The claims are word for word those Bench gave for `lint` and `eval`, so an envelope differs
//! from Bench's only in its `schema` id.

use std::fmt::{self, Display, Formatter};

/// A top-level `skillcheck` command. Adding one means adding a variant, and every exhaustive
/// `match` (exit codes, claims, dispatch) then fails to compile until it is handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CommandName {
    /// `skillcheck lint`
    Lint,
    /// `skillcheck eval`
    Eval,
}

/// What a green (exit 0) result from a command establishes, and what it does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Claim {
    /// What a green result proves.
    pub proves: &'static str,
    /// What a green result does NOT prove.
    pub does_not_prove: &'static str,
}

impl CommandName {
    /// Every command, in the order `--help` lists them.
    pub const ALL: [Self; 2] = [Self::Lint, Self::Eval];

    /// The name as typed on the command line.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lint => "lint",
            Self::Eval => "eval",
        }
    }

    /// What a green result from this command proves and does not prove.
    #[must_use]
    pub const fn claim(self) -> Claim {
        match self {
            Self::Lint => Claim {
                proves: "the checked files have no error under anneal's lint rules",
                does_not_prove: "that the content is right, or that its warnings and review \
                                 items were addressed",
            },
            Self::Eval => Claim {
                proves: "skill routing accuracy, or each skill's behaviour uplift over its \
                         baseline, is at or above its bound on the eval set",
                does_not_prove: "routing accuracy or behaviour on prompts outside the eval set",
            },
        }
    }
}

impl Display for CommandName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
