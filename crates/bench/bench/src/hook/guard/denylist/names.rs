//! How the denylist's entries recognise the program they are about, and what a word may be.
//!
//! A program is matched by its last path component, in any ASCII case, with or without `.exe`
//! (a case-insensitive file system, APFS or NTFS, finds `ANNEAL` or `weighin.EXE` as `anneal`
//! or `weighin`), or by a program word not known before the command runs, which may name it.
//! A word not read as written may be any text its written start allows ([`may_be`]).

use redkiln_guard::engine::braces;
use redkiln_guard::shell::Word;

/// The extension a Windows program name may end in.
const EXE: &str = ".exe";

/// Characters after which a pattern word's text is no longer what it matches.
pub(super) const PATTERN: [char; 9] = ['*', '?', '[', '{', '(', '~', '@', '+', '!'];

/// Whether the program name `name` (a last path component) is `binary`, in any ASCII case,
/// with or without `.exe` in any case.
pub(super) fn is(name: &str, binary: &str) -> bool {
    let stem = name
        .len()
        .checked_sub(EXE.len())
        .and_then(|at| name.split_at_checked(at))
        .filter(|(_, extension)| extension.eq_ignore_ascii_case(EXE))
        .map_or(name, |(stem, _)| stem);
    stem.eq_ignore_ascii_case(binary)
}

/// Whether the program `word` may be one `named` accepts: its last path component is
/// (`"${CLAUDE_PLUGIN_ROOT}/libexec/weighin"`), or it is not known before the command runs
/// (`"$W"`, `$(command -v weighin)`, a pattern).
pub(super) fn may_name(word: &Word, named: impl FnOnce(&str) -> bool) -> bool {
    known_tail(word).is_none_or(named)
}

/// Whether the path `word` names a file `named` accepts by its last path component, known
/// before the command runs (`"${CLAUDE_PLUGIN_ROOT}/libexec/weighin"`); a word whose last
/// component is not known (`"$SCRIPT"`) names none.
pub(super) fn surely_names(word: &Word, named: impl FnOnce(&str) -> bool) -> bool {
    known_tail(word).is_some_and(named)
}

/// The last path component of `word` when it is known before the command runs: no expansion
/// reaches into it and, in a word not read as written, it holds no pattern character.
fn known_tail(word: &Word) -> Option<&str> {
    let start = word
        .text()
        .rfind(['/', '\\'])
        .map_or(0, |at| at.saturating_add(1));
    let tail = word.text().get(start..)?;
    let unknown = word
        .expansions()
        .iter()
        .any(|expansion| expansion.range.end > start)
        || (!braces::reads_as_written(word) && tail.contains(PATTERN));
    (!unknown).then_some(tail)
}

/// Whether `word` may become several words: it holds an unquoted expansion, which the shell
/// field-splits (`$x`, `$*`, `${x[*]}`, `$(cmd)`), or a quoted one whose source text holds an
/// `@`, which may expand to one word per element: `"$@"`, `"${x[@]}"`, `"${!prefix@}"`, and
/// `$@` nested in another expansion (`"${x:-$@}"`, `"${x-"$@"}"`). A quoted `@` that gives one
/// word (`"${x@Q}"`) is counted too, which only lets the word stand for more.
pub(super) fn may_be_several(word: &Word) -> bool {
    word.expansions().iter().any(|expansion| {
        !expansion.quoted
            || word
                .text()
                .get(expansion.range.clone())
                .is_none_or(|source| source.contains('@'))
    })
}

/// Whether `word` may be `text` once the shell has expanded it: exactly `text` when it is read
/// as written, else when `text` starts with what is written before its first expansion or
/// pattern character.
pub(super) fn may_be(word: &Word, text: &str) -> bool {
    if braces::reads_as_written(word) {
        return word.text() == text;
    }
    text.starts_with(written_start(word))
}

/// What is written of `word` before its first expansion or pattern character: all of it when
/// it is read as written, and nothing when the cut is not on a character boundary (so the word
/// may be any text). A quoted pattern character is cut at too: the written start is then
/// shorter, which only lets the word be more texts.
pub(super) fn written_start(word: &Word) -> &str {
    let text = word.text();
    if braces::reads_as_written(word) {
        return text;
    }
    let end = word
        .expansions()
        .iter()
        .map(|expansion| expansion.range.start)
        .chain(text.find(PATTERN))
        .min()
        .unwrap_or(text.len());
    text.get(..end).unwrap_or_default()
}
