//! The facts about a plugin the skill rules read: its binary and commands (rule 10's
//! `` `<binary> <command>` `` enforcers), its hook ids (rule 10's default enforcers) and its
//! entry skills (rule 4). Built only from `bench_contract`, so the checks name no Bench
//! internals.

use bench_contract::{cli, plugin};

/// What the skill rules know of the plugin whose skills they lint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Profile {
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

/// Bench's own profile, the one `lint skills` checks against.
pub(crate) const BENCH: Profile = Profile {
    binary: cli::BINARY,
    commands: cli::COMMANDS,
    hook_ids: plugin::HOOK_IDS,
    entry_skills: &plugin::ENTRY_SKILLS,
    enforcer_fix: "name the check on the same line: `bench <command> ...` or a hook id \
                   (`bench hook --help` lists them), in backticks; or soften the MUST.",
};
