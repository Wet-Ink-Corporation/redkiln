//! The launcher copy run as a shell script: `sh <path>/libexec/weighin hook ...`, the way a
//! skill runs weighin (`sh "${CLAUDE_PLUGIN_ROOT}/libexec/weighin" ...`).
//!
//! The engine reads a shell's script-file operand as a file it does not open and gives packs no
//! call for it, so the pack reads one simple command's words itself
//! ([`Pack::words`](redkiln_guard::engine::Pack::words)): leading assignments and reserved
//! words, a shell named as written, its
//! options as far as they are plain, then a script file whose last path component is weighin,
//! and the words after it as weighin's arguments. It reads only that plain shape: a wrapper
//! before the shell (`sudo sh`, `env sh`, `exec sh`), an option word not read as written, or a
//! script path known only at run time (`sh "$SCRIPT" hook`) is not followed. A shell's `-c`
//! string and a script on standard input are the engine's, which reads them as commands.

use redkiln_guard::engine::braces;
use redkiln_guard::shell::Word;

use super::super::names;

/// The shells whose script-file operand is read, by program name.
const SHELLS: [&str; 8] = ["sh", "bash", "dash", "zsh", "ksh", "mksh", "ash", "posh"];

/// Long shell options that take the next word as their value.
const LONG_VALUES: [&str; 2] = ["--rcfile", "--init-file"];

/// The words after the script file when `words` run a shell on a script file `named` accepts
/// (`sh libexec/weighin hook` gives `hook`); `None` for any other command.
pub(super) fn arguments(words: &[Word], named: impl FnOnce(&str) -> bool) -> Option<&[Word]> {
    let start = words
        .iter()
        .position(|word| !is_assignment(word) && !is_keyword(word))?;
    let (shell, rest) = words.get(start..)?.split_first()?;
    let shell_named = braces::reads_as_written(shell)
        && names::surely_names(shell, |name| {
            SHELLS.iter().any(|known| names::is(name, known))
        });
    if !shell_named {
        return None;
    }
    let (script, after) = script_operand(rest)?.split_first()?;
    names::surely_names(script, named).then_some(after)
}

/// Whether `word` is a reserved word the lexer leaves before a command (`{ sh ...; }`,
/// `if sh ...`, `! sh ...`).
fn is_keyword(word: &Word) -> bool {
    braces::reads_as_written(word) && KEYWORDS.contains(&word.text())
}

/// Reserved words that may come before a command.
const KEYWORDS: [&str; 10] = [
    "!", "{", "if", "then", "else", "elif", "do", "while", "until", "time",
];

/// Whether `word` is an assignment written before a command (`CI=1`).
fn is_assignment(word: &Word) -> bool {
    word.text().split_once('=').is_some_and(|(name, _)| {
        name.chars()
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    })
}

/// The shell's arguments from its script-file operand on, once its plain options are passed;
/// `None` when it runs a `-c` string or reads its script from standard input (`-s`), or an
/// option is not read as written.
fn script_operand(args: &[Word]) -> Option<&[Word]> {
    let mut rest = args;
    loop {
        let (word, after) = rest.split_first()?;
        if !braces::reads_as_written(word) {
            // An option the shell reads only at run time is not followed; any other such word
            // is the script operand.
            let may_be_option = names::written_start(word).starts_with(['-', '+']);
            return (!may_be_option).then_some(rest);
        }
        let text = word.text();
        if text == "--" || text == "-" {
            return Some(after);
        }
        if let Some(long) = text.strip_prefix("--") {
            rest = if LONG_VALUES
                .iter()
                .any(|option| option.get(2..) == Some(long))
            {
                after.get(1..)?
            } else {
                after
            };
            continue;
        }
        let Some(cluster) = text.strip_prefix(['-', '+']).filter(|c| !c.is_empty()) else {
            return Some(rest);
        };
        if cluster.contains(['c', 's']) {
            return None;
        }
        rest = if cluster.ends_with(['o', 'O']) {
            after.get(1..)?
        } else {
            after
        };
    }
}
