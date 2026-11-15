//! Renders the exit-code table for `--help`.

use std::fmt::{self, Display, Formatter};

use super::{Code, CommandCodes, GLOBAL};
use crate::command::CommandName;

/// `after_help` for `skillcheck --help`: the global codes, then one line per command.
#[must_use]
pub fn overview_help() -> String {
    format!(
        "Exit codes (all commands):\n{}\nExit codes per command:\n{}\nOnly success exits 0.",
        Rows(GLOBAL),
        Overview
    )
}

/// `after_help` for `skillcheck <command> --help`: its codes, the global ones, and its claim.
#[must_use]
pub fn command_help(command: CommandName) -> String {
    let table = CommandCodes::of(command);
    let claim = command.claim();
    format!(
        "Exit codes ({command}):\n{}\nExit codes (all commands):\n{}\n\
         A green result proves: {}.\nIt does NOT prove: {}.",
        Rows(table.codes),
        Rows(GLOBAL),
        claim.proves,
        claim.does_not_prove
    )
}

/// One indented `code meaning` line per code.
struct Rows<'a>(&'a [Code]);

impl Display for Rows<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        for code in self.0 {
            writeln!(f, "  {:<3} {}", code.value(), code.meaning())?;
        }
        Ok(())
    }
}

/// One line per command: its name, then its codes joined by ` · `.
struct Overview;

impl Display for Overview {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        for command in CommandName::ALL {
            write!(f, "  {:<5}", command.as_str())?;
            for (index, code) in CommandCodes::of(command).codes.iter().enumerate() {
                let separator = if index == 0 { " " } else { " · " };
                write!(f, "{separator}{} {}", code.value(), code.meaning())?;
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{command_help, overview_help};
    use crate::command::CommandName;

    #[test]
    fn command_help_shows_its_codes_and_what_it_does_not_prove() {
        let help = command_help(CommandName::Lint);
        assert!(
            help.starts_with("Exit codes (lint):\n  0   no errors\n  1   error findings\n"),
            "got {help}"
        );
        assert!(help.ends_with(&format!(
            "It does NOT prove: {}.",
            CommandName::Lint.claim().does_not_prove
        )));
    }

    #[test]
    fn the_overview_lists_every_command() {
        let help = overview_help();
        for command in CommandName::ALL {
            assert!(
                help.lines()
                    .any(|l| l.trim_start().starts_with(command.as_str())),
                "{command} has a line"
            );
        }
    }
}
