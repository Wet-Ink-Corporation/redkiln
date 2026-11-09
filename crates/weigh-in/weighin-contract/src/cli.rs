//! What another crate may name of the `weighin` command line: its binary, every subcommand path it
//! accepts and the global options that take a value.
//!
//! `cargo xtask plugins` reads these lists to check that every subcommand a skill, agent or
//! command names exists (root `AGENTS.md`, CLI first, harness second), and weighin's `cli_pin`
//! test pins them against clap's own tree, so they change with the CLI or that test fails.

/// The binary's name, as a plugin's launcher copy (`libexec/weighin`) names it.
pub const BINARY: &str = "weighin";

/// Every subcommand path `weighin` accepts, sorted, words separated by one space.
///
/// A command with subcommands under it is listed itself only when it runs without one, so
/// `journal` is listed and `board` is not. clap's generated `help` is not listed; xtask's check
/// lets it through for every binary.
pub const COMMANDS: &[&str] = &[
    "batch",
    "board apply",
    "board export",
    "ci-check",
    "context",
    "decide",
    "decisions",
    "defer",
    "digest",
    "dismiss",
    "hook",
    "init",
    "journal",
    "journal checkpoint",
    "journal import",
    "journal pre-push",
    "journal relay",
    "open",
    "prime",
    "raise",
    "reopen",
    "scan",
    "show",
    "stats",
    "status",
    "trailer",
];

/// The global options that take a value, sorted: an invocation may put them, each followed by its
/// value, before the subcommand.
pub const VALUE_OPTIONS: &[&str] = &["--dir", "--session", "--store"];

#[cfg(test)]
mod tests {
    use super::{COMMANDS, VALUE_OPTIONS};

    #[test]
    fn commands_are_sorted_and_unique() {
        assert!(COMMANDS.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn value_options_are_sorted_unique_long_options() {
        assert!(VALUE_OPTIONS.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(VALUE_OPTIONS.iter().all(|o| o.starts_with("--")));
    }

    #[test]
    fn each_command_is_lowercase_words_separated_by_one_space() {
        for command in COMMANDS {
            assert!(
                command.split(' ').all(|word| {
                    word.starts_with(|c: char| c.is_ascii_lowercase())
                        && word
                            .chars()
                            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                }),
                "{command}"
            );
        }
    }
}
