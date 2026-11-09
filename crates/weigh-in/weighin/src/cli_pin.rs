//! Pins `weighin_contract::cli`, the subcommands and value options other crates may name (xtask's
//! check that every subcommand a prompt names exists), against clap's own tree, so the contract's
//! lists cannot drift from what `weighin` accepts.

mod tests {
    use clap::{Command, CommandFactory};
    use weighin_contract::cli;

    use crate::Cli;

    /// Every subcommand path under `command`, each prefixed by `prefix`: a node is listed itself
    /// only when its own subcommand is optional, and clap's generated `help` is left out.
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
        assert_eq!(found, cli::COMMANDS);
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
        assert_eq!(found, cli::VALUE_OPTIONS);
    }

    #[test]
    fn the_contract_names_this_binary() {
        assert_eq!(Cli::command().get_name(), cli::BINARY);
    }
}
