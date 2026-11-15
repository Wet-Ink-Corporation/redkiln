//! The rule-pack seam: a pack of its own, with its own rules, driven through the engine.

use crate::hook::guard::denylist::checker;
use crate::hook::guard::denylist::cwd::Cwd;
use crate::hook::guard::denylist::seam::{Call, Denial, Pack};
use crate::hook::guard::denylist::wrappers::Program;
use redkiln_guard::shell::{SimpleCommand, Word};

/// The test pack's rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestRule {
    /// The one entry: a program named `boom`.
    Boom,
    /// What the engine cannot read.
    Unreadable,
}

/// A pack with one entry, denying the program `boom` however the command reaches it.
#[derive(Debug)]
struct BoomPack;

impl Pack for BoomPack {
    type Rule = TestRule;
    type Mark = ();
    const UNREADABLE: TestRule = TestRule::Unreadable;

    fn enter_command(
        &mut self,
        _command: &SimpleCommand,
        _words: &[Word],
    ) -> Result<(), Denial<TestRule>> {
        Ok(())
    }

    fn leave_command(&mut self, _words: &[Word], (): ()) {}

    fn redirect(&mut self, _target: &Word, _cwd: &Cwd) -> Option<Denial<TestRule>> {
        None
    }

    fn command(&mut self, call: &Call<'_>) -> Option<Denial<TestRule>> {
        matches!(call.program, Program::Named("boom"))
            .then(|| Denial::new(TestRule::Boom, "boom runs"))
    }

    fn mark(&self) {}
}

fn rule_for(command: &str) -> Option<TestRule> {
    checker::check(command, &mut BoomPack).map(|denial| denial.rule)
}

#[test]
fn a_packs_entry_judges_every_command_the_engine_reaches() {
    let wrapped = [
        "boom",
        "sudo boom",
        "env -i nice -n 5 boom",
        "bash -c 'boom'",
        "eval 'boom now'",
        "echo boom | sh",
        "sh <<< 'boom'",
        "x=boom; $x",
        "'bo'om",
        "xargs boom < list",
    ];
    for command in wrapped {
        assert_eq!(rule_for(command), Some(TestRule::Boom), "{command:?}");
    }
    assert_eq!(rule_for("echo boom"), None, "an argument is no program");
}

#[test]
fn the_engines_own_refusals_carry_the_packs_unreadable_rule() {
    let too_deep = format!("{}boom", "eval ".repeat(10));
    for command in ["boom 'unclosed", too_deep.as_str(), "{boom,x}"] {
        assert_eq!(rule_for(command), Some(TestRule::Unreadable), "{command:?}");
    }
}
