//! `weighin-hook` (#168, SPEC TR-09 and HK-12): a Bash call may not run `weighin hook`.
//!
//! `weighin hook` is the harness's entry point: `hooks.json` runs it with the hook's own
//! payload on stdin, and its `PostToolUse` path records an `AskUserQuestion` answer as
//! `witnessed`. A model that runs it from Bash can feed it a forged payload, which #125 D2 A
//! (Ryan, 2026-11-20) accepted as the hook's surface, with this entry as the tripwire.
//!
//! It is a tripwire, not a wall (HK-12): it judges the words as the engine forms them (wrappers,
//! `bash -c`, `eval`, variables the command sets, braces), and a command built where the
//! engine cannot read it escapes it: a script file other than the plain launcher form
//! ([`script`] reads `sh <path>/libexec/weighin hook` itself, but not `sudo sh ...` or a script
//! written and then run), an interpreter's inline code, a copy of the binary under another
//! name. It runs only where Bench's guard runs; nothing else detects a forged payload,
//! a residual #125 D2 A accepted (`docs/weigh-in/hooks.md`).
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

use std::collections::BTreeSet;

use super::names;
use crate::hook::guard::{Denial, Rule};
use redkiln_guard::engine::braces;
use redkiln_guard::shell::Word;
use weighin_contract::cli::{BINARY, VALUE_OPTIONS};

mod script;

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

/// The denial for one simple command's `words` when they run the launcher copy as a shell
/// script that may run `hook` (`sh libexec/weighin hook`; [`script`]).
pub(super) fn script(words: &[Word]) -> Option<Denial> {
    script::arguments(words, is_weighin)
        .is_some_and(|args| may_run_hook(args, false))
        .then(denial)
}

fn denial() -> Denial {
    Denial::new(Rule::WeighinHook, REASON)
}

/// Whether `weighin <args>`, with any arguments `appended` at run time, may run `hook`.
///
/// Every reading of the global options is followed: `pending` holds the positions some
/// reading may next read a word at, before its subcommand. Each position is read once, and
/// a reading only moves forward, so the walk is linear in the words.
fn may_run_hook(args: &[Word], appended: bool) -> bool {
    let mut pending = BTreeSet::from([0_usize]);
    while let Some(at) = pending.pop_first() {
        let Some(word) = args.get(at) else {
            if appended {
                return true;
            }
            continue;
        };
        let next = at.saturating_add(1);
        match step(word, args.get(next)) {
            Step::Hook => return true,
            Step::Subcommand => {}
            Step::Skip(words) => {
                pending.insert(at.saturating_add(words));
            }
            Step::SkipOneOrTwo => {
                pending.insert(next);
                pending.insert(at.saturating_add(2));
            }
            Step::Ended => {
                let subcommand = args.get(next);
                if subcommand.map_or(appended, |word| names::may_be(word, HOOK)) {
                    return true;
                }
            }
        }
    }
    false
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
