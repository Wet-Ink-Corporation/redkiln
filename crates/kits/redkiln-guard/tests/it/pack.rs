//! A two-entry pack: the program `wipe`, and a write to the protected file `secret.cfg`,
//! judged through the public engine however a command reaches them.

use redkiln_guard::engine::{self, Call, Cwd, Denial, MAX_DEPTH, MAX_LEXED_MIB, Pack, Program};
use redkiln_guard::protect::{Protected, ProtectedPaths};
use redkiln_guard::shell::{SimpleCommand, Word};

/// The pack's rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rule {
    /// The program `wipe`.
    Wipe,
    /// A write to a protected path.
    Protected,
    /// What the engine cannot read.
    Unreadable,
}

/// The pack's protected paths: one file, wherever it is.
const PATHS: [Protected; 1] = [Protected::file("secret.cfg")];

/// No directory holds the protected file.
const CONTAINERS: [(&str, &str); 0] = [];

const TABLE: ProtectedPaths<'static> =
    ProtectedPaths::new(&PATHS, &CONTAINERS, "a directory above the start");

/// The two-entry pack.
#[derive(Debug)]
struct TwoEntries;

impl Pack for TwoEntries {
    type Rule = Rule;
    type Mark = ();
    const UNREADABLE: Rule = Rule::Unreadable;

    fn enter_command(
        &mut self,
        _command: &SimpleCommand,
        _words: &[Word],
    ) -> Result<(), Denial<Rule>> {
        Ok(())
    }

    fn leave_command(&mut self, _words: &[Word], (): ()) {}

    fn redirect(&mut self, target: &Word, cwd: &Cwd) -> Option<Denial<Rule>> {
        let label = TABLE.protected(target.text(), target.shape(), cwd)?;
        Some(Denial::new(
            Rule::Protected,
            format!("a redirection writes {label}"),
        ))
    }

    fn command(&mut self, call: &Call<'_>) -> Option<Denial<Rule>> {
        matches!(call.program, Program::Named("wipe")).then(|| Denial::new(Rule::Wipe, "wipe runs"))
    }

    fn mark(&self) {}
}

fn rule_for(command: &str) -> Option<Rule> {
    engine::check(command, &mut TwoEntries).map(|denial| denial.rule)
}

#[test]
fn a_denied_program_is_denied_behind_wrappers_shells_eval_variables_and_here_strings() {
    let reached = [
        "env FOO=1 wipe",
        "sudo -u root wipe",
        "bash -c 'wipe'",
        "eval wipe",
        "p=wipe; $p",
        "bash <<< wipe",
        "nice env bash -c \"eval 'sudo wipe'\"",
    ];
    for command in reached {
        assert_eq!(rule_for(command), Some(Rule::Wipe), "{command:?}");
    }
}

#[test]
fn a_redirection_writing_a_protected_path_is_denied_wherever_it_is_written_from() {
    let writes = [
        "echo x > secret.cfg",
        "echo x >> conf/secret.cfg",
        "bash -c 'echo x > secret.cfg'",
        "cd conf && echo x > secret.cfg",
    ];
    for command in writes {
        assert_eq!(rule_for(command), Some(Rule::Protected), "{command:?}");
    }
}

#[test]
fn commands_no_entry_names_are_left_undecided() {
    for command in [
        "ls -la",
        "echo wipe",
        "cat secret.cfg",
        "echo x > notes.cfg",
    ] {
        assert_eq!(rule_for(command), None, "{command:?}");
    }
}

#[test]
fn nesting_past_max_depth_is_the_packs_unreadable_rule() {
    let within = format!("{}true", "eval ".repeat(MAX_DEPTH));
    let past = format!("{}true", "eval ".repeat(MAX_DEPTH + 1));
    assert_eq!(rule_for(&within), None, "{MAX_DEPTH} evals deep is read");
    assert_eq!(
        rule_for(&past),
        Some(Rule::Unreadable),
        "one eval deeper is refused"
    );
}

#[test]
fn input_past_max_lexed_mib_is_the_packs_unreadable_rule() {
    let budget = MAX_LEXED_MIB * 1024 * 1024;
    let within = format!("echo {}", "a".repeat(budget - "echo ".len()));
    let past = format!("echo {}", "a".repeat(budget));
    assert_eq!(
        within.len(),
        budget,
        "the within case is exactly the budget"
    );
    assert_eq!(
        rule_for(&within),
        None,
        "a command of exactly the budget is read"
    );
    let denial = engine::check(&past, &mut TwoEntries);
    assert!(
        denial
            .as_ref()
            .is_some_and(|denial| denial.rule == Rule::Unreadable
                && denial.reason.contains(&format!("{MAX_LEXED_MIB} MiB"))),
        "a command past the lexing budget is refused for its size: {denial:?}"
    );
}
