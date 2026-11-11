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

/// The version every output id has today: `bench` was never released before SPEC CLI-02, so no
/// output keeps an older shape under its id (CLI-04).
pub const OUTPUT_VERSION: u32 = 1;

/// The output id (SPEC CLI-02) of every subcommand path that writes a result envelope, sorted.
///
/// Each is `bench.<command>[-<subcommand>]/<n>`, as [`output_id`] builds it. Every path in
/// [`COMMANDS`] is here but the hooks and `statusline`, which answer Claude Code instead, and
/// `ledger journal`, whose output is the journal file itself.
pub const OUTPUTS: &[&str] = &[
    "bench.brief-build/1",
    "bench.calibrate/1",
    "bench.dag-check/1",
    "bench.dag-frontier/1",
    "bench.decide-label/1",
    "bench.decide/1",
    "bench.eval-routing/1",
    "bench.eval-uplift/1",
    "bench.floor-guard/1",
    "bench.grill-classify/1",
    "bench.grill-confirm/1",
    "bench.grill-frontier/1",
    "bench.ledger-append/1",
    "bench.ledger-query/1",
    "bench.ledger-tail/1",
    "bench.lint-brief/1",
    "bench.lint-plan/1",
    "bench.lint-rules/1",
    "bench.lint-skills/1",
    "bench.lint-spec/1",
    "bench.lint-terms/1",
    "bench.migrate-contract/1",
    "bench.migrate-done/1",
    "bench.migrate-start/1",
    "bench.mode-build/1",
    "bench.mode-diagnose/1",
    "bench.mode-off/1",
    "bench.orchestrate-fan-out/1",
    "bench.orchestrate-park/1",
    "bench.orchestrate-rule/1",
    "bench.perf-noise/1",
    "bench.perf-sample/1",
    "bench.plan-approve/1",
    "bench.plan-check/1",
    "bench.playbook-list/1",
    "bench.playbook-show/1",
    "bench.repro-leftovers/1",
    "bench.repro-record/1",
    "bench.rollout-decide/1",
    "bench.round-start/1",
    "bench.round-status/1",
    "bench.ship-skip-rule/1",
    "bench.snapshot-write/1",
    "bench.status/1",
    "bench.task-done/1",
    "bench.verdict-set/1",
    "bench.verdict-valid/1",
    "bench.verify-risk/1",
    "bench.watch-ci/1",
    "bench.watch-deploy/1",
    "bench.watch-pr/1",
];

/// The output id of the subcommand path `command` (`"dag check"` gives `"bench.dag-check/1"`):
/// the binary's namespace, the path's words joined by `-`, then [`OUTPUT_VERSION`].
#[must_use]
pub fn output_id(command: &str) -> String {
    format!(
        "{BINARY}.{}/{OUTPUT_VERSION}",
        command.split(' ').collect::<Vec<_>>().join("-")
    )
}

#[cfg(test)]
mod tests {
    use super::{COMMANDS, OUTPUTS, VALUE_OPTIONS, output_id};

    #[test]
    fn an_output_id_joins_the_path_words_with_a_hyphen() {
        assert_eq!(output_id("dag check"), "bench.dag-check/1", "two words");
        assert_eq!(output_id("floor-guard"), "bench.floor-guard/1", "one word");
        assert_eq!(
            output_id("orchestrate fan-out"),
            "bench.orchestrate-fan-out/1",
            "a hyphenated subcommand"
        );
    }

    #[test]
    fn outputs_are_sorted_unique_and_each_the_id_of_a_command() {
        assert!(
            OUTPUTS.windows(2).all(|pair| pair[0] < pair[1]),
            "OUTPUTS is sorted and unique"
        );
        let ids: Vec<String> = COMMANDS.iter().map(|command| output_id(command)).collect();
        for output in OUTPUTS {
            assert!(
                ids.iter().any(|id| id == output),
                "{output} is the id of a command"
            );
        }
    }

    #[test]
    fn every_command_but_the_hooks_statusline_and_the_journal_has_an_output() {
        let mut expected: Vec<String> = COMMANDS
            .iter()
            .filter(|c| !c.starts_with("hook ") && !matches!(**c, "statusline" | "ledger journal"))
            .map(|command| output_id(command))
            .collect();
        expected.sort();
        assert_eq!(expected, OUTPUTS, "OUTPUTS");
    }

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
