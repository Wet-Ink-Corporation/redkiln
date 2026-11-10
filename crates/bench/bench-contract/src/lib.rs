//! Bench's contract: what other crates may name of Bench without depending on its internals
//! (M2-C). Today that is [`cli`], the `bench` command line's binary, subcommands and value
//! options, and [`plugin`], the `rk-bench` plugin's name that its skills are namespaced under.

pub mod cli;
pub mod plugin;
