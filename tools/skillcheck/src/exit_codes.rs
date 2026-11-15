//! The exit-code table of `lint skills` and `eval`: the one place their exit codes are
//! declared. [`render`] puts the table in `--help`.
//!
//! The rule behind every row: "2 never reads as 0". Only success exits 0.

pub mod render;

use crate::command::CommandName;

/// One documented exit code and what it means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Code {
    value: u8,
    meaning: &'static str,
}

impl Code {
    /// Declares a code. Private: every code is a constant in this module.
    #[must_use]
    const fn new(value: u8, meaning: &'static str) -> Self {
        Self { value, meaning }
    }

    /// The process exit status.
    #[must_use]
    pub const fn value(self) -> u8 {
        self.value
    }

    /// What the code means, as documented.
    #[must_use]
    pub const fn meaning(self) -> &'static str {
        self.meaning
    }

    /// True only for exit status 0.
    #[must_use]
    pub const fn is_success(self) -> bool {
        self.value == 0
    }
}

impl From<Code> for std::process::ExitCode {
    fn from(code: Code) -> Self {
        Self::from(code.value)
    }
}

/// A command's exit-code table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandCodes {
    /// The command the table belongs to.
    pub command: CommandName,
    /// The codes, in ascending order.
    pub codes: &'static [Code],
}

/// Usage error: bad arguments or an unknown command. Nothing is written on stdout.
pub const USAGE: Code = Code::new(2, "usage error: bad arguments; nothing on stdout");
/// The command could not run: git missing, unreadable snapshot, and the like.
pub const INTERNAL: Code = Code::new(70, "internal error: the command could not run");
/// stdout or stderr could not be written, so the result was not delivered.
pub const OUTPUT: Code = Code::new(74, "output error: stdout or stderr could not be written");

/// Codes that mean the same for every command. Command tables never reuse 70 or 74.
pub const GLOBAL: &[Code] = &[USAGE, INTERNAL, OUTPUT];

/// Success, for commands whose 0 needs no more specific wording.
pub const OK: Code = Code::new(0, "ok");

/// `lint`: no error findings (warnings and review items are listed but do not fail).
pub const LINT_CLEAN: Code = Code::new(0, "no errors");
/// `lint`: at least one error finding.
pub const LINT_FINDINGS: Code = Code::new(1, "error findings");
/// `eval routing`: accuracy is below its bound; `eval uplift`: a skill missed a bar.
pub const EVAL_BELOW_BOUND: Code = Code::new(1, "below bound");
/// `eval routing`: a fixture or skill is invalid or missing, so accuracy cannot be trusted;
/// `eval uplift`: a suite or case is invalid.
pub const EVAL_INVALID: Code = Code::new(5, "invalid fixtures, suites or skills");
/// `eval routing --router claude`: the cost cap, the time limit or a failed later batch
/// stopped the run while a prompt was still unsettled; `eval uplift`: they stopped it before
/// every batch ran.
pub const EVAL_INCOMPLETE: Code = Code::new(
    3,
    "incomplete: cost cap, time limit or a failed batch stopped the run short",
);
/// `eval routing --router claude` and `eval uplift`: `claude plugin eval` could not run its
/// first batch (`claude` missing or failing, no credentials, the plugin not loaded, an
/// unreadable result).
pub const EVAL_ROUTER_FAILED: Code = Code::new(4, "the real router could not run");

impl CommandCodes {
    /// The exit-code table of `command`.
    #[must_use]
    pub const fn of(command: CommandName) -> Self {
        let codes: &'static [Code] = match command {
            CommandName::Lint => &[LINT_CLEAN, LINT_FINDINGS],
            CommandName::Eval => &[
                OK,
                EVAL_BELOW_BOUND,
                EVAL_INCOMPLETE,
                EVAL_ROUTER_FAILED,
                EVAL_INVALID,
            ],
        };
        Self { command, codes }
    }
}

#[cfg(test)]
mod tests {
    use super::{CommandCodes, GLOBAL, USAGE};
    use crate::command::CommandName;

    #[test]
    fn each_table_has_one_success_code_ascends_and_reuses_no_global_code_but_usage() {
        for command in CommandName::ALL {
            let codes = CommandCodes::of(command).codes;
            let values: Vec<u8> = codes.iter().map(|c| c.value()).collect();
            assert_eq!(
                codes.iter().filter(|c| c.is_success()).count(),
                1,
                "{command} has one 0 row"
            );
            assert!(values.is_sorted(), "{command} ascends: {values:?}");
            assert!(
                codes
                    .iter()
                    .all(|c| *c == USAGE || !GLOBAL.iter().any(|g| g.value() == c.value())),
                "{command} reuses no global code"
            );
        }
    }
}
