//! Pins `bench_contract::cli`, the subcommands and value options other crates may name (xtask's
//! check that every subcommand a prompt names exists, M2-C), against clap's own tree, so the
//! contract's lists cannot drift from what `bench` accepts.

use bench::cli::Cli;
use bench_contract::cli;
use clap::{Command, CommandFactory};

/// Every subcommand path under `command`, each prefixed by `prefix`: a node is listed itself only
/// when its own subcommand is optional, and clap's generated `help` is left out.
fn paths(command: &Command, prefix: &str, into: &mut Vec<String>) {
    for sub in command.get_subcommands().filter(|s| s.get_name() != "help") {
        let path = format!("{prefix}{}", sub.get_name());
        let has_children = sub.get_subcommands().any(|s| s.get_name() != "help");
        paths(sub, &format!("{path} "), into);
        if !has_children || !sub.is_subcommand_required_set() {
            into.push(path);
        }
    }
}

#[test]
fn the_contract_lists_every_subcommand_clap_accepts() {
    let mut found = Vec::new();
    paths(&Cli::command(), "", &mut found);
    found.sort();
    assert_eq!(found, cli::COMMANDS, "bench_contract::cli::COMMANDS");
}

#[test]
fn the_contract_gives_an_output_id_to_every_subcommand_clap_accepts_but_hooks_and_journal() {
    let mut found = Vec::new();
    paths(&Cli::command(), "", &mut found);
    let mut ids: Vec<String> = found
        .iter()
        .filter(|path| {
            !path.starts_with("hook ") && !matches!(path.as_str(), "statusline" | "ledger journal")
        })
        .map(|path| cli::output_id(path))
        .collect();
    ids.sort();
    assert_eq!(ids, cli::OUTPUTS, "bench_contract::cli::OUTPUTS");
}

#[test]
fn the_contract_lists_every_global_option_that_takes_a_value() {
    let command = Cli::command();
    let mut found: Vec<String> = command
        .get_arguments()
        .filter(|a| a.is_global_set() && a.get_action().takes_values())
        .filter_map(|a| a.get_long().map(|long| format!("--{long}")))
        .collect();
    found.sort();
    assert_eq!(
        found,
        cli::VALUE_OPTIONS,
        "bench_contract::cli::VALUE_OPTIONS"
    );
}

#[test]
fn the_contract_names_this_binary() {
    assert_eq!(
        Cli::command().get_name(),
        cli::BINARY,
        "bench_contract::cli::BINARY"
    );
}
