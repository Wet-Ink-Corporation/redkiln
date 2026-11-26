//! `weighin-hook` (#168, SPEC TR-09 and HK-12): a Bash call may not run `weighin hook`.
//!
//! `weighin hook` is the harness's entry point: `hooks.json` runs it with the hook's own
//! payload on stdin, and its `PostToolUse` path records an `AskUserQuestion` answer as
//! `witnessed`. A model that runs it from Bash can feed it a forged payload, which #125 D2 A
//! (Ryan, 2026-11-20) accepted as the hook's surface, with this entry as the tripwire.
//!
//! The launcher copy run as a script (`sh "${CLAUDE_PLUGIN_ROOT}/libexec/weighin" hook`, the way
//! Weigh-In's skill runs weighin) is judged through the engine's script-file call
//! ([`script_files`], `Pack::script_files`, wi-22281f307b): a script file whose last path
//! component is weighin, known before the command runs, has the words after it read as
//! weighin's, in every reading the engine follows, so `sudo sh`, `env sh`, `exec sh`, `xargs sh`,
//! `source`, shell options read several ways and a path brace expansion builds
//! (`sh {/x/libexec/weighin,hook}`, expanded under the engine's cap) are all seen.
//!
//! It is a tripwire, not a wall (HK-12): it judges the words as the engine forms them (wrappers,
//! `bash -c`, `eval`, variables the command sets, braces), and a command built where the
//! engine cannot read it escapes it: a script path known only at run time (`sh "$SCRIPT"
//! hook`), a script written and then run, an interpreter's inline code, a copy of the binary
//! under another name. It runs only where Bench's guard runs; nothing else detects a forged
//! payload, a residual #125 D2 A accepted (`docs/weigh-in/hooks.md`).
//!
//! The words are read as clap reads weighin's: global options before the subcommand, each of
//! [`VALUE_OPTIONS`] taking the next word as its value (`--dir hook open` runs `open`), any
//! other option word standing alone, and `--` ending them. The first operand is the
//! subcommand, denied when it may be `hook`: exactly `hook` when read as written, else any word
//! whose written start allows it (`"$sub"`, `h"$rest"`). A word that may become several words
//! (`"$@"`, an unquoted expansion), as an option, a value or the subcommand, may supply `hook`
//! and is denied (`weighin --dir $DIR open`; quote it). An option-looking word not read as
//! written that may be a value-taking option (`--di"$r"`) is read both ways, with and without
//! a value. Arguments `xargs` appends at run time may be anything, so with no subcommand
//! before them the call is denied.

use std::borrow::Borrow;

use super::names;
use crate::hook::guard::{Denial, Rule};
use redkiln_guard::engine::{ScriptFiles, braces};
use redkiln_guard::shell::Word;
use weighin_contract::cli::{BINARY, VALUE_OPTIONS};

/// The subcommand `hooks.json` runs and a model may not.
const HOOK: &str = "hook";

/// The reason every `weighin-hook` denial gives.
const REASON: &str = "weighin hook is the harness's entry point, run by hooks.json with the \
                      hook's own payload; a payload fed from Bash could forge a witnessed answer \
                      (#125 D2), so run another weighin command or write the subcommand out";

/// Whether the program name `name` (a last path component) is weighin, in any ASCII case, with
/// or without `.exe`.
pub(super) fn is_weighin(name: &str) -> bool {
    names::is(name, BINARY)
}

/// Whether the program `word` may be weighin: its last path component is weighin, or it is
/// not known before the command runs.
pub(super) fn may_name_weighin(word: &Word) -> bool {
    names::may_name(word, is_weighin)
}

/// The denial for `weighin <args>` when it may run `hook`; `appended` when `xargs` adds
/// arguments known only at run time.
pub(super) fn check(args: &[Word], appended: bool) -> Option<Denial> {
    may_run_hook(args, appended).then(denial)
}

/// The denial for the script files one shell or `source` runs ([`ScriptFiles`]) when one is
/// the launcher copy, named before the command runs, and its arguments may run `hook`
/// (`sudo sh libexec/weighin hook`). Every script is judged from one pass over the shell's
/// words ([`hook_from`]), so a shell whose readings find many is still linear (H-13).
pub(super) fn script_files(files: &ScriptFiles<'_>) -> Option<Denial> {
    let mut launchers = files
        .scripts
        .iter()
        .filter(|at| {
            files
                .words
                .get(**at)
                .is_some_and(|script| names::surely_names(script, is_weighin))
        })
        .peekable();
    launchers.peek()?;
    let from = hook_from(files.words, files.appended);
    launchers
        .any(|at| from.get(at.saturating_add(1)).is_some_and(|hook| *hook))
        .then(denial)
}

fn denial() -> Denial {
    Denial::new(Rule::WeighinHook, REASON)
}

/// Whether `weighin <args>`, with any arguments `appended` at run time, may run `hook`.
fn may_run_hook(args: &[Word], appended: bool) -> bool {
    hook_from(args, appended).first().is_some_and(|hook| *hook)
}

/// For each position of `args`, and the end past them, whether a reading of weighin's global
/// options that reads its next word there, with any arguments `appended` at run time, may run
/// `hook`; `weighin <args>` may run it when the first does.
///
/// A word moves a reading only forward and only by what it and the word after it are
/// ([`step`]), so each position is answered from later ones, in one pass from the end: linear
/// in the words, whichever position a reading starts at.
fn hook_from<W: Borrow<Word>>(args: &[W], appended: bool) -> Vec<bool> {
    let end = args.len();
    let mut from = vec![appended; end.saturating_add(1)];
    for at in (0..end).rev() {
        let Some(word) = args.get(at).map(Borrow::borrow) else {
            continue;
        };
        let next = at.saturating_add(1);
        let at_or_end = |position: usize| from.get(position.min(end)).is_some_and(|hook| *hook);
        let hook = match step(word, args.get(next).map(Borrow::borrow)) {
            Step::Hook => true,
            Step::Subcommand => false,
            Step::Skip(words) => at_or_end(at.saturating_add(words)),
            Step::SkipOneOrTwo => at_or_end(next) || at_or_end(at.saturating_add(2)),
            Step::Ended => args
                .get(next)
                .map(Borrow::borrow)
                .map_or(appended, |subcommand| names::may_be(subcommand, HOOK)),
        };
        if let Some(slot) = from.get_mut(at) {
            *slot = hook;
        }
    }
    from
}

/// What one word before weighin's subcommand does to a reading.
enum Step {
    /// It may be, or may supply, `hook` as the subcommand.
    Hook,
    /// It is another subcommand: this reading runs no hook.
    Subcommand,
    /// An option standing for this many words, itself and any value.
    Skip(usize),
    /// Not read as written, it may be an option taking a value, or one standing alone.
    SkipOneOrTwo,
    /// `--`: the next word is the subcommand.
    Ended,
}

/// How `word`, read where weighin's global options may stand, with `next` after it, moves a
/// reading. A word that may become several words (`"$@"`, an unquoted expansion) may supply
/// any words, `hook` among them, and so is [`Step::Hook`] whatever it starts with; so is a
/// value-taking option's value that may.
fn step(word: &Word, next: Option<&Word>) -> Step {
    if names::may_be_several(word) {
        return Step::Hook;
    }
    if !braces::reads_as_written(word) {
        if !names::written_start(word).starts_with('-') {
            return if names::may_be(word, HOOK) {
                Step::Hook
            } else {
                Step::Subcommand
            };
        }
        let takes_value = VALUE_OPTIONS
            .iter()
            .any(|option| names::may_be(word, option));
        return if takes_value {
            value_step(next, Step::SkipOneOrTwo)
        } else {
            Step::Skip(1)
        };
    }
    let text = word.text();
    if text == "--" {
        return Step::Ended;
    }
    if !text.starts_with('-') {
        return if text == HOOK {
            Step::Hook
        } else {
            Step::Subcommand
        };
    }
    if VALUE_OPTIONS.contains(&text) {
        // Its value, which clap never reads as the subcommand.
        return value_step(next, Step::Skip(2));
    }
    Step::Skip(1)
}

/// `skip` for an option whose value is `value`, unless that value may become several words,
/// which may supply `hook` after it.
fn value_step(value: Option<&Word>, skip: Step) -> Step {
    if value.is_some_and(names::may_be_several) {
        Step::Hook
    } else {
        skip
    }
}

#[cfg(test)]
mod tests {
    use super::hook_from;
    use redkiln_guard::shell;

    /// The pass over a whole list answers, at each position, what a pass over the words from
    /// there answers at its start: a script's arguments are judged in the shell's one pass.
    #[test]
    fn each_position_of_one_pass_answers_as_a_pass_over_the_words_from_there() {
        let lines = [
            "--dir weighin --dir x hook",
            "--session s -- hook open",
            "--di\"$r\" /x hook",
            "a --store b hook --dir",
            "open hook",
            "--dir $DIR hook",
        ];
        for line in lines {
            let commands = shell::parse(line).expect("the line lexes");
            let words = commands.first().expect("one command").words();
            for appended in [false, true] {
                let whole = hook_from(words, appended);
                for start in 0..=words.len() {
                    let tail = words.get(start..).expect("start is in range");
                    assert_eq!(
                        whole.get(start).copied(),
                        hook_from(tail, appended).first().copied(),
                        "{line:?} from {start}, appended {appended}"
                    );
                }
            }
        }
    }
}
