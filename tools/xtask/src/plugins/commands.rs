//! Check (a) of the plugin rules: every subcommand a skill, agent or command names after a binary
//! the plugin ships is in that binary's command list (root `AGENTS.md`, CLI first, harness second).
//!
//! Only code is read: the lines of fenced blocks and the inline code spans of other lines, so prose
//! such as "weighin did not run here" is never taken for an invocation. In a span, whitespace
//! splits tokens and surrounding quotes are stripped. An invocation is the binary's name, a path
//! ending in `libexec/<binary>`, or an alias: a one-word span followed by ` means ` or
//! ` below means ` and a span holding an invocation (the first such definition in a file wins, and
//! holds in the whole file). After the invocation, a value option and the token after it, and any
//! other token starting with `-`, are skipped; the next token, if it is a lowercase word, is the
//! command. When the list has paths under that word and the token after it is a word too, the two
//! words must be a listed path; otherwise the one word must be.
//!
//! Not read: the mod's argv arrays in TypeScript, which its own plugin tests pin.

use super::{Plugin, Violation, has_extension, is_prompt, launchers};

/// The prompt files read: Markdown and text, not a skill's HTML or JSON assets.
const PROMPT_EXTENSIONS: [&str; 2] = ["md", "txt"];

/// What a binary accepts, as far as the check needs it.
#[derive(Debug, Clone, Copy)]
pub struct Cli<'a> {
    /// The binary's name, as its launcher copy `libexec/<binary>` names it.
    pub binary: &'a str,
    /// Every subcommand path it accepts, words separated by one space.
    pub commands: &'a [&'a str],
    /// The options that take a value and may come before the subcommand.
    pub value_options: &'a [&'a str],
}

/// Checks that every subcommand the prompts of `plugins` name after a shipped binary is in that
/// binary's list in `clis`.
///
/// Pure: violations come in plugin order. Within a plugin, a missing list comes first for each
/// launcher copy with none, then unknown commands in file and mention order. A binary the plugin
/// does not ship is left to [`check`](super::check)'s `UnknownBinary`, and an entry that is not a
/// folder to its `NotAFolder`.
#[must_use]
pub fn check_commands(plugins: &[Plugin], clis: &[Cli<'_>]) -> Vec<Violation> {
    plugins
        .iter()
        .filter(|plugin| plugin.folder)
        .flat_map(|plugin| check_plugin(plugin, clis))
        .collect()
}

fn check_plugin(plugin: &Plugin, clis: &[Cli<'_>]) -> Vec<Violation> {
    let mut found = Vec::new();
    let mut shipped = Vec::new();
    for binary in launchers(plugin) {
        match clis.iter().find(|cli| cli.binary == binary) {
            Some(cli) => shipped.push(*cli),
            None => found.push(Violation::NoCommandList {
                plugin: plugin.name.clone(),
                binary: binary.to_owned(),
            }),
        }
    }
    let prompts = plugin
        .files
        .iter()
        .filter(|f| is_prompt(&f.path) && has_extension(&f.path, &PROMPT_EXTENSIONS));
    for file in prompts {
        let Some(text) = &file.text else { continue };
        for (cli, command) in unknown_commands(text, &shipped) {
            found.push(Violation::UnknownCommand {
                plugin: plugin.name.clone(),
                path: file.path.clone(),
                binary: cli.binary.to_owned(),
                command,
            });
        }
    }
    found
}

/// Each command `text` names that its binary does not have, with that binary.
fn unknown_commands<'c>(text: &str, shipped: &[Cli<'c>]) -> Vec<(Cli<'c>, String)> {
    let lines = code_lines(text);
    let alias = alias(&lines, shipped);
    lines
        .iter()
        .flatten()
        .flat_map(|span| unknown_in(span.code, shipped, alias))
        .collect()
}

/// A piece of code on one line, and the text after it up to the next span on the line.
struct Span<'t> {
    code: &'t str,
    after: &'t str,
}

/// The code of each line of `text`: one span for a line inside a fenced block, the inline spans of
/// any other line. Fence lines themselves hold no code.
fn code_lines(text: &str) -> Vec<Vec<Span<'_>>> {
    let mut lines = Vec::new();
    let mut fence = None;
    for line in text.lines() {
        match (fence, fence_char(line)) {
            (None, Some(opened)) => fence = Some(opened),
            (Some(open), Some(closed)) if open == closed => fence = None,
            (Some(_), _) => lines.push(vec![Span {
                code: line,
                after: "",
            }]),
            (None, None) => lines.push(inline_spans(line)),
        }
    }
    lines
}

/// The character of the fence `line` opens or closes, if it is one.
fn fence_char(line: &str) -> Option<char> {
    let line = line.trim_start();
    ['`', '~']
        .into_iter()
        .find(|&c| line.chars().take(3).filter(|&d| d == c).count() == 3)
}

/// The inline code spans of `line`: a run of backticks opens one and the next run of the same
/// length closes it; a run with no such partner is plain text.
fn inline_spans(line: &str) -> Vec<Span<'_>> {
    let runs = backtick_runs(line);
    let run = |&(start, end): &(usize, usize)| line.get(start..end);
    // (where the opening run starts, where the code starts and ends, where the closing run ends)
    let mut pairs = Vec::new();
    let mut rest = runs.as_slice();
    while let Some((open, tail)) = rest.split_first() {
        let closing = tail
            .iter()
            .position(|close| run(close) == run(open))
            .and_then(|at| tail.get(at..))
            .and_then(<[_]>::split_first);
        rest = match closing {
            Some((&(close_start, close_end), after)) => {
                pairs.push((open.0, open.1, close_start, close_end));
                after
            }
            None => tail,
        };
    }
    let mut spans = Vec::with_capacity(pairs.len());
    for (index, &(_, code_start, code_end, close_end)) in pairs.iter().enumerate() {
        let next_open = pairs
            .get(index.saturating_add(1))
            .map_or(line.len(), |next| next.0);
        // Every offset comes from `char_indices` or is the line's length, so each range is on a
        // character boundary and `get` always gives the text.
        spans.push(Span {
            code: line.get(code_start..code_end).unwrap_or_default(),
            after: line.get(close_end..next_open).unwrap_or_default(),
        });
    }
    spans
}

/// The byte ranges of each run of backticks in `line`.
fn backtick_runs(line: &str) -> Vec<(usize, usize)> {
    let mut runs = Vec::new();
    let mut open = None;
    for (at, c) in line.char_indices() {
        match (c == '`', open) {
            (true, None) => open = Some(at),
            (false, Some(start)) => {
                runs.push((start, at));
                open = None;
            }
            _ => {}
        }
    }
    if let Some(start) = open {
        runs.push((start, line.len()));
    }
    runs
}

/// The file's alias, if a one-word span is defined as an invocation: the first definition wins.
fn alias<'t, 'c>(lines: &[Vec<Span<'t>>], shipped: &[Cli<'c>]) -> Option<(&'t str, Cli<'c>)> {
    lines
        .iter()
        .flat_map(|spans| spans.windows(2))
        .find_map(|pair| {
            let [word, definition] = pair else {
                return None;
            };
            let name = word.code.trim();
            let defines = matches!(word.after, " means " | " below means ");
            if !defines || name.is_empty() || name.contains(char::is_whitespace) {
                return None;
            }
            tokens(definition.code)
                .into_iter()
                .find_map(|token| invoked(token, shipped))
                .map(|cli| (name, cli))
        })
}

fn tokens(code: &str) -> Vec<&str> {
    code.split_whitespace()
        .map(|token| token.trim_matches(['"', '\'']))
        .collect()
}

/// The shipped binary `token` runs by name or by its `libexec/` path.
fn invoked<'c>(token: &str, shipped: &[Cli<'c>]) -> Option<Cli<'c>> {
    shipped.iter().copied().find(|cli| {
        token
            .strip_suffix(cli.binary)
            .is_some_and(|head| head.is_empty() || head.ends_with("libexec/"))
    })
}

/// Each command `code` names after an invocation that its binary does not have.
fn unknown_in<'c>(
    code: &str,
    shipped: &[Cli<'c>],
    alias: Option<(&str, Cli<'c>)>,
) -> Vec<(Cli<'c>, String)> {
    let tokens = tokens(code);
    let mut found = Vec::new();
    let mut rest = tokens.as_slice();
    while let Some((&token, tail)) = rest.split_first() {
        rest = tail;
        let by_alias = alias.filter(|&(word, _)| word == token).map(|(_, cli)| cli);
        let Some(cli) = invoked(token, shipped).or(by_alias) else {
            continue;
        };
        let (named, after) = command_after(tail, &cli);
        rest = after;
        if let Some(named) = named.filter(|named| !named.is_listed(cli.commands)) {
            found.push((cli, named.to_string()));
        }
    }
    found
}

/// The command one or two words name.
#[derive(Clone, Copy)]
enum Named<'t> {
    One(&'t str),
    Two(&'t str, &'t str),
}

impl Named<'_> {
    fn is_listed(self, commands: &[&str]) -> bool {
        commands.iter().any(|&listed| match self {
            Self::One(word) => listed == word,
            Self::Two(first, second) => listed.split_once(' ') == Some((first, second)),
        })
    }
}

impl std::fmt::Display for Named<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::One(word) => f.write_str(word),
            Self::Two(first, second) => write!(f, "{first} {second}"),
        }
    }
}

/// The command `tokens`, which follow an invocation of `cli`, name (if any), and the tokens after
/// it.
fn command_after<'t, 's>(
    tokens: &'s [&'t str],
    cli: &Cli<'_>,
) -> (Option<Named<'t>>, &'s [&'t str]) {
    let mut rest = tokens;
    while let Some((&token, tail)) = rest.split_first() {
        if cli.value_options.contains(&token) {
            rest = tail.split_first().map_or(tail, |(_, after)| after);
        } else if token.starts_with('-') {
            rest = tail;
        } else if !is_word(token) {
            return (None, tail);
        } else {
            let is_parent = cli.commands.iter().any(|listed| {
                listed
                    .split_once(' ')
                    .is_some_and(|(parent, _)| parent == token)
            });
            return match tail.split_first() {
                Some((&next, after)) if is_parent && is_word(next) => {
                    (Some(Named::Two(token, next)), after)
                }
                _ => (Some(Named::One(token)), tail),
            };
        }
    }
    (None, rest)
}

/// A command word: a lowercase letter, then lowercase letters, digits and `-`.
fn is_word(token: &str) -> bool {
    let mut chars = token.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}
