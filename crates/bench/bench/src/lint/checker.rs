//! The one checker `bench lint rules` reports: design rule 10's `enforcer`.
//!
//! The full catalogue of skill checkers is `tools/skillcheck`'s, with `lint skills`; this enum
//! holds only the id and rule number that `lint rules` findings carry.

use std::fmt::{self, Display, Formatter};

use serde::{Deserialize, Serialize};

/// A checker: one mechanical check for one design rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Checker {
    /// Rule 10: every MUST (here: every rule of a rule document) names its enforcer.
    Enforcer,
}

impl Checker {
    /// The checker id as printed: kebab-case.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Enforcer => "enforcer",
        }
    }

    /// The skill design rule (`tools/skillcheck/docs/skill-rules.md`) this checker enforces.
    #[must_use]
    pub const fn rule(self) -> u8 {
        match self {
            Self::Enforcer => 10,
        }
    }
}

impl Display for Checker {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::Checker;

    #[test]
    fn the_enforcer_id_matches_serde_and_is_rule_10() {
        let json = serde_json::to_string(&Checker::Enforcer).expect("a unit enum serialises");
        assert_eq!(json, "\"enforcer\"", "serde id equals as_str");
        assert_eq!(Checker::Enforcer.rule(), 10, "design rule 10");
    }
}
