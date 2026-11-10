//! What another crate may name of the `bench` command line: its binary, every subcommand path it
//! accepts and the global options that take a value.
//!
//! `cargo xtask plugins` reads these lists to check that every subcommand a skill names exists
//! (root `AGENTS.md`, CLI first, harness second), and bench's `cli_pin` test pins them against
//! clap's own tree, so they change with the CLI or that test fails.

/// The binary's name, as a plugin's launcher copy (`libexec/bench`) names it.
pub const BINARY: &str = "bench";

/// Every subcommand path `bench` accepts, sorted, words separated by one space.
///
/// A command with subcommands under it is listed itself only when it runs without one. clap's
/// generated `help` is not listed; xtask's check lets it through for every binary.
pub const COMMANDS: &[&str] = &[
    "brief build",
    "calibrate",
    "dag check",
    "dag frontier",
    "decide",
    "decide label",
    "eval routing",
    "eval uplift",
    "floor-guard",
    "grill classify",
    "grill confirm",
    "grill frontier",
    "hook bootstrap",
    "hook brief",
    "hook completeness",
    "hook context-monitor",
    "hook fast-check",
    "hook guard",
    "hook handoff",
    "hook injection-scan",
    "hook merge-gate",
    "hook preconditions",
    "hook route",
    "hook ruled-before",
    "hook standing-orders",
    "hook status",
    "ledger append",
    "ledger journal",
    "ledger query",
    "ledger tail",
    "lint brief",
    "lint plan",
    "lint rules",
    "lint skills",
    "lint spec",
    "lint terms",
    "migrate contract",
    "migrate done",
    "migrate start",
    "mode build",
    "mode diagnose",
    "mode off",
    "orchestrate fan-out",
    "orchestrate park",
    "orchestrate rule",
    "perf noise",
    "perf sample",
    "plan approve",
    "plan check",
    "playbook list",
    "playbook show",
    "repro leftovers",
    "repro record",
    "rollout decide",
    "round start",
    "round status",
    "ship skip-rule",
    "snapshot write",
    "status",
    "statusline",
    "task done",
    "verdict set",
    "verdict valid",
    "verify risk",
    "watch ci",
    "watch deploy",
    "watch pr",
];

/// The global options that take a value, sorted: an invocation may put them, each followed by its
/// value, before the subcommand.
pub const VALUE_OPTIONS: &[&str] = &[];

#[cfg(test)]
mod tests {
    use super::{COMMANDS, VALUE_OPTIONS};

    #[test]
    fn commands_are_sorted_and_unique() {
        assert!(
            COMMANDS.windows(2).all(|pair| pair[0] < pair[1]),
            "COMMANDS is sorted and unique"
        );
    }

    #[test]
    fn value_options_are_sorted_unique_long_options() {
        assert!(
            VALUE_OPTIONS.windows(2).all(|pair| pair[0] < pair[1]),
            "VALUE_OPTIONS is sorted and unique"
        );
        assert!(
            VALUE_OPTIONS.iter().all(|o| o.starts_with("--")),
            "every value option is long"
        );
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
