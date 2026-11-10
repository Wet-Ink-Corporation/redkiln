//! The bootstrap index: the stable part of what the `SessionStart` hook injects.
//!
//! It is what anneal is, the entry skills with their triggers, and the commands only the human
//! runs, in 200 words or fewer (skill rule 4). The entry skills are the linter's
//! `ENTRY_SKILLS`, in its order (a test holds them equal), so `lint skills` and the bootstrap
//! agree on them. The same text is committed as `docs/bootstrap-index.md`, which
//! `bench lint skills --index` checks.

use std::fmt::{self, Display, Formatter};

/// Each entry skill and the trigger that should load it, in `ENTRY_SKILLS` order.
const ENTRY_TRIGGERS: [(&str, &str); 7] = [
    (
        "grill",
        "new behaviour, even a precise rule, without saying how it works, or a vague request; question it until confirmed.",
    ),
    (
        "spec",
        "requirements are agreed; write the spec and its criteria.",
    ),
    (
        "plan",
        "a spec is ready; split it into tasks and get the plan approved.",
    ),
    (
        "build",
        "code to add or change, or an approved task; build it test-first.",
    ),
    (
        "diagnose",
        "told to fix wrong behaviour or a failing test; reproduce it red before any edit.",
    ),
    (
        "verify",
        "is it fixed or tested, or work looks done; prove it first.",
    ),
    (
        "ship",
        "verified work needs a pull request, verdict, merge or rollout.",
    ),
];

/// The user-invoked skills, by directory name, and what each does. Each is typed as
/// `/<plugin>:<skill>` (`/rk-bench:handoff`), the plugin being [`bench_contract::plugin::NAME`].
const USER_COMMANDS: [(&str, &str); 5] = [
    ("orchestrate", "run a plan's tasks across subagents."),
    ("handoff", "write a brief for the next session."),
    ("correct", "record a correction as a standing order."),
    ("write-skill", "author a skill that lints clean."),
    ("calibrate", "calibrate a Clef decision point."),
];

const PREAMBLE: &str = "\
# anneal

anneal turns SDLC gates into enforced checks. Skills say how to work, the `bench` CLI and its
hooks check that it was done, and the ledger records what happened.

## Entry skills

Load the matching skill before your first tool call:

";

const COMMANDS_HEADING: &str = "
## User commands

The human runs these; do not start them unasked:

";

/// The bootstrap index with every skill, as committed as `docs/bootstrap-index.md` and
/// injected when the plugin's skills cannot be listed.
#[must_use]
pub(crate) fn index() -> String {
    index_with(|_| true)
}

/// The bootstrap index listing only the entry skills and user commands whose skill `present`
/// holds, by directory name (`grill`, `handoff`), so a plugin missing a skill never sends the
/// model to load it.
#[must_use]
pub(crate) fn index_with(present: impl Fn(&str) -> bool) -> String {
    let skills = Bullets {
        pairs: &ENTRY_TRIGGERS,
        present: &present,
        form: Form::Skill,
    };
    let commands = Bullets {
        pairs: &USER_COMMANDS,
        present: &present,
        form: Form::Command,
    };
    format!("{PREAMBLE}{skills}{COMMANDS_HEADING}{commands}")
}

/// The directory name of every skill the index can list, entry skills then user commands.
pub(crate) fn skill_names() -> impl Iterator<Item = &'static str> {
    ENTRY_TRIGGERS
        .iter()
        .chain(USER_COMMANDS.iter())
        .map(|(name, _)| *name)
}

/// How a bullet writes a skill's directory name.
#[derive(Debug, Clone, Copy)]
enum Form {
    /// The name itself, `grill`.
    Skill,
    /// The slash command that runs it, as typed: `/rk-bench:handoff`.
    Command,
}

/// One ``- `name`: text`` line per pair whose skill `present` keeps, the name in its `form`.
struct Bullets<'a> {
    pairs: &'a [(&'a str, &'a str)],
    present: &'a dyn Fn(&str) -> bool,
    form: Form,
}

impl Display for Bullets<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let plugin = bench_contract::plugin::NAME;
        for (name, text) in self.pairs.iter().filter(|(name, _)| (self.present)(name)) {
            match self.form {
                Form::Skill => writeln!(f, "- `{name}`: {text}")?,
                Form::Command => writeln!(f, "- `/{plugin}:{name}`: {text}")?,
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{index, index_with};

    /// Path of the generated copy of the index, relative to the repository root.
    const DOC_PATH: &str = "docs/bench/bootstrap-index.md";
    use crate::lint::skills::checker::ENTRY_SKILLS;
    use std::path::Path;

    /// The skill names on the index's `- `name`: trigger` lines, in order.
    fn listed_skills(text: &str) -> Vec<String> {
        text.lines()
            .filter_map(|line| line.strip_prefix("- `"))
            .filter_map(|rest| rest.split_once("`: "))
            .map(|(name, _)| name.to_owned())
            .filter(|name| !name.starts_with('/'))
            .collect()
    }

    #[test]
    fn the_index_is_at_most_200_words() {
        let words = index().split_whitespace().count();
        assert!(words <= 200, "the index is {words} words, budget 200");
    }

    #[test]
    fn the_index_names_every_entry_skill_in_the_linters_order() {
        assert_eq!(
            listed_skills(&index()),
            ENTRY_SKILLS,
            "the index lists exactly the linter's entry skills, in order"
        );
    }

    #[test]
    fn the_index_lists_the_user_invoked_commands_after_the_skills() {
        let text = index();
        let commands = text
            .split_once("## User commands")
            .map(|(_, rest)| rest)
            .expect("a User commands section");
        for command in [
            "/rk-bench:orchestrate",
            "/rk-bench:handoff",
            "/rk-bench:correct",
            "/rk-bench:write-skill",
            "/rk-bench:calibrate",
        ] {
            assert!(
                commands.contains(&format!("- `{command}`: ")),
                "{command} is listed under User commands"
            );
        }
    }

    #[test]
    fn a_skill_the_plugin_lacks_is_not_listed_as_an_entry_skill_or_a_command() {
        let text = index_with(|skill| !matches!(skill, "grill" | "handoff"));

        let listed = listed_skills(&text);
        assert_eq!(
            (
                listed.contains(&"grill".to_owned()),
                listed.len(),
                text.contains("/rk-bench:handoff"),
                text.contains("- `/rk-bench:correct`: "),
            ),
            (false, ENTRY_SKILLS.len() - 1, false, true),
            "{text}"
        );
    }

    #[test]
    fn bootstrap_index_doc_matches_the_index() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .join(DOC_PATH);
        let expected = index();
        if std::env::var_os("ANNEAL_BLESS").is_some() {
            std::fs::write(&path, &expected).expect("write docs/bootstrap-index.md");
        }
        let committed = std::fs::read_to_string(&path).expect("docs/bootstrap-index.md exists");
        assert!(
            committed == expected,
            "{DOC_PATH} drifted from the index; regenerate it with \
             `ANNEAL_BLESS=1 cargo test -p bench bootstrap_index_doc`"
        );
    }
}
