//! The facts about a plugin the skill rules read, chosen with `lint skills --plugin`.
//!
//! A [`Profile`] holds a plugin's binary and commands (rule 10's `` `<binary> <command>` ``
//! enforcers), its hook ids (rule 10's default enforcers) and its entry skills (rule 4). Each
//! is built only from that plugin's contract crate, so the checks name no plugin internals.

use bench_contract::{cli, plugin};

/// What the skill rules know of the plugin whose skills they lint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Profile {
    /// The binary a skill names a command by (`bench`).
    pub binary: &'static str,
    /// Every subcommand path the binary accepts; a command's first word is a command name.
    pub commands: &'static [&'static str],
    /// The hook ids that count as enforcers when no `--hooks` file is given.
    pub hook_ids: &'static [&'static str],
    /// The model-invoked entry skills; every other skill disables model invocation.
    pub entry_skills: &'static [&'static str],
    /// What a MUST line with no enforcer should do, naming commands by [`Profile::binary`]
    /// (held equal by a test).
    pub enforcer_fix: &'static str,
}

impl Profile {
    /// Whether `word` is the first word of one of the binary's commands.
    pub(crate) fn has_command(&self, word: &str) -> bool {
        self.commands
            .iter()
            .any(|path| path.split(' ').next() == Some(word))
    }
}

/// The plugin `--plugin` names when it is not given.
pub const DEFAULT_PLUGIN: &str = "bench";

/// Bench's profile, built from `bench_contract`: what [`profile`] gives for [`DEFAULT_PLUGIN`].
pub const BENCH: Profile = Profile {
    binary: cli::BINARY,
    commands: cli::COMMANDS,
    hook_ids: plugin::HOOK_IDS,
    entry_skills: &plugin::ENTRY_SKILLS,
    enforcer_fix: "name the check on the same line: `bench <command> ...` or a hook id \
                   (`bench hook --help` lists them), in backticks; or soften the MUST.",
};

/// No skill profile is known for the plugin `lint skills --plugin` named.
#[derive(Debug, thiserror::Error)]
#[error("no skill profile for plugin {0:?}")]
pub struct UnknownPlugin(pub String);

/// The profile of `plugin`; only `bench` has one.
///
/// # Errors
/// [`UnknownPlugin`] for any other name.
pub fn profile(plugin: &str) -> Result<Profile, UnknownPlugin> {
    match plugin {
        DEFAULT_PLUGIN => Ok(BENCH),
        other => Err(UnknownPlugin(other.to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use super::{BENCH, DEFAULT_PLUGIN, UnknownPlugin, profile};

    #[test]
    fn the_default_plugin_is_bench_and_reads_bench_contract() {
        let found = profile(DEFAULT_PLUGIN).expect("bench has a profile");

        assert_eq!(found, BENCH, "the default is Bench's profile");
        assert_eq!(found.binary, bench_contract::cli::BINARY, "its binary");
    }

    #[test]
    fn an_unknown_plugin_is_refused_by_name() {
        let refused = profile("weighin");

        assert!(
            matches!(&refused, Err(UnknownPlugin(name)) if name == "weighin"),
            "{refused:?}"
        );
        let message = refused.map(|_| ()).map_err(|e| e.to_string());
        assert_eq!(
            message,
            Err("no skill profile for plugin \"weighin\"".to_owned()),
            "the message names it"
        );
    }
}
