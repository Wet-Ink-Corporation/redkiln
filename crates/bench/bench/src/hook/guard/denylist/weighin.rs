//! `weighin-hook` (#168, SPEC TR-09 and HK-12): a Bash call may not run `weighin hook`.
//!
//! `weighin hook` is the harness's entry point: `hooks.json` runs it with the hook's own
//! payload on stdin, and its `PostToolUse` path records an `AskUserQuestion` answer as
//! `witnessed`. A model that runs it from Bash can feed it a forged payload, which #125 D2 A
//! (Ryan, 2026-11-20) accepted as the hook's surface, with this entry as the tripwire.
//!
//! It is a tripwire, not a wall (HK-12): it judges the words as the engine forms them (wrappers,
//! `bash -c`, `eval`, variables the command sets, braces), and a command built where the
//! engine cannot read it escapes it: a script file (the launcher copy run as
//! `sh .../libexec/weighin hook`, which the engine reads as a file it does not open, or one
//! written and then run), an interpreter's inline code, a copy of the binary under another
//! name. It runs only where Bench's guard runs; nothing else detects a forged payload,
//! a residual #125 D2 A accepted (`docs/weigh-in/hooks.md`).
//!
//! The words are read as clap reads weighin's: global options before the subcommand, each of
//! [`VALUE_OPTIONS`] taking the next word as its value (`--dir hook open` runs `open`), any
//! other option word standing alone, and `--` ending them. The first operand is the
//! subcommand, denied when it may be `hook`: exactly `hook` when read as written, else any word
//! whose written start allows it (`"$sub"`, `h"$rest"`). An option-looking word not read as
//! written is passed over, so the word after it is still judged. Arguments `xargs` appends at
//! run time may be anything, so with no subcommand before them the call is denied.

use super::names;
use crate::hook::guard::{Denial, Rule};
use redkiln_guard::engine::braces;
use redkiln_guard::shell::Word;
use weighin_contract::cli::{BINARY, VALUE_OPTIONS};

/// The subcommand `hooks.json` runs and a model may not.
const HOOK: &str = "hook";

/// The reason every `weighin-hook` denial gives.
const REASON: &str = "weighin hook is the harness's entry point, run by hooks.json with the \
                      hook's own payload; a payload fed from Bash could forge a witnessed answer \
                      (#125 D2), so run another weighin command";

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
    may_run_hook(args, appended).then(|| Denial::new(Rule::WeighinHook, REASON))
}

/// Whether `weighin <args>`, with any arguments `appended` at run time, may run `hook`.
fn may_run_hook(args: &[Word], appended: bool) -> bool {
    let mut words = args.iter();
    while let Some(word) = words.next() {
        if !braces::reads_as_written(word) {
            if names::written_start(word).starts_with('-') {
                // Maybe an option, maybe its value: the next word is judged either way.
                continue;
            }
            return names::may_be(word, HOOK);
        }
        let text = word.text();
        if text == "--" {
            return words
                .next()
                .map_or(appended, |word| names::may_be(word, HOOK));
        }
        if !text.starts_with('-') {
            return text == HOOK;
        }
        if VALUE_OPTIONS.contains(&text) {
            // Its value, which clap never reads as the subcommand.
            words.next();
        }
    }
    appended
}
