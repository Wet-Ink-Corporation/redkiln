//! Check (b) of `cargo xtask plugins`: Claude Code's hook protocol is read and written in one
//! adapter module per binary, and nowhere else (SPEC CLI-13; root `AGENTS.md`, CLI first,
//! harness second).
//!
//! The protocol is recognised by its tokens: payload and response keys only Claude Code uses
//! ([`TOKENS`]) and any `CLAUDE_` variable (`CLAUDE_` then an uppercase letter). Generic names
//! such as `session_id`, `cwd`, `source`, `tool_name` and `agent_id` are left out, since events
//! and other domains use them too.
//!
//! Every `*.rs` file under `crates/` is checked, except:
//!
//! - a crate's integration tests (`crates/<layer>/<crate>/tests/**`), which act as the harness
//!   does;
//! - in a crate with `src/main.rs`, its adapter: `src/harness.rs` and `src/harness/**`;
//! - lines whose trimmed start is `//` (docs may name the protocol; a trailing comment is still
//!   checked, which fails closed);
//! - a unit test module as rustfmt writes it at the top level of a file: a line `#[cfg(test)]`,
//!   then `mod <name> {`, through the next line that is `}` alone. A test module written any
//!   other way is checked.

mod read;

use std::collections::BTreeSet;
use std::fmt;

pub use read::{MAX_DEPTH, MAX_FILES, MAX_TEXT, ReadError, read_sources};

/// Payload and response keys of Claude Code's hook protocol, and its Cowork entry point.
pub const TOKENS: [&str; 12] = [
    "hook_event_name",
    "last_assistant_message",
    "tool_input",
    "tool_response",
    "stop_hook_active",
    "transcript_path",
    "hookSpecificOutput",
    "hookEventName",
    "additionalContext",
    "systemMessage",
    "permissionDecision",
    "remote_cowork",
];

/// The prefix of Claude Code's environment variables; a token when an uppercase letter follows.
const VARIABLE_PREFIX: &str = "CLAUDE_";

/// One Rust source file under `crates/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    /// The path from the repository root, starting `crates/`, with `/` separators.
    pub path: String,
    /// The file's contents.
    pub text: String,
}

/// A way the sources break the rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Violation {
    /// A protocol token outside a harness adapter.
    OutsideAdapter {
        /// The file.
        path: String,
        /// The line, counted from 1.
        line: usize,
        /// The first token on the line.
        token: String,
    },
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutsideAdapter { path, line, token } => write!(
                f,
                "{path}:{line}: names {token}, Claude Code's hook protocol, outside the crate's \
                 harness adapter (src/harness/); CLI-13"
            ),
        }
    }
}

/// Checks `files` against the rule, returning one violation per offending line, in path then
/// line order.
#[must_use]
pub fn check(files: &[SourceFile]) -> Vec<Violation> {
    let binaries: BTreeSet<&str> = files
        .iter()
        .filter_map(|file| crate_and_rest(&file.path))
        .filter(|&(_, rest)| rest == "src/main.rs")
        .map(|(root, _)| root)
        .collect();
    let mut sorted: Vec<&SourceFile> = files.iter().collect();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));

    let mut violations = Vec::new();
    for file in sorted {
        if exempt(&file.path, &binaries) {
            continue;
        }
        let lines: Vec<&str> = file.text.lines().collect();
        let tests = test_module_lines(&lines);
        for (index, line) in lines.iter().enumerate() {
            if tests.contains(&index) || line.trim_start().starts_with("//") {
                continue;
            }
            if let Some(token) = first_token(line) {
                violations.push(Violation::OutsideAdapter {
                    path: file.path.clone(),
                    line: index + 1,
                    token: token.to_owned(),
                });
            }
        }
    }
    violations
}

/// `crates/<layer>/<crate>` and the path below it, for a path at least that deep.
fn crate_and_rest(path: &str) -> Option<(&str, &str)> {
    if !path.starts_with("crates/") {
        return None;
    }
    let (end, _) = path.match_indices('/').nth(2)?;
    Some((path.get(..end)?, path.get(end + 1..)?))
}

/// Whether `path` is a crate's integration test, or the harness adapter of a crate in
/// `binaries`.
fn exempt(path: &str, binaries: &BTreeSet<&str>) -> bool {
    let Some((root, rest)) = crate_and_rest(path) else {
        return false;
    };
    rest.starts_with("tests/")
        || ((rest == "src/harness.rs" || rest.starts_with("src/harness/"))
            && binaries.contains(root))
}

/// The indexes of the lines of each top-level unit test module, as rustfmt writes one.
fn test_module_lines(lines: &[&str]) -> BTreeSet<usize> {
    let mut skipped = BTreeSet::new();
    let mut index = 0;
    while let Some(&line) = lines.get(index) {
        let opens =
            line == "#[cfg(test)]" && lines.get(index + 1).is_some_and(|&l| opens_module(l));
        let close = opens
            .then(|| lines.iter().skip(index + 2).position(|&l| l == "}"))
            .flatten()
            .map(|offset| index + 2 + offset);
        match close {
            Some(close) => {
                skipped.extend(index..=close);
                index = close + 1;
            }
            // No closing line: the module is checked as code, which fails closed.
            None => index += 1,
        }
    }
    skipped
}

/// Whether `line` is `mod <name> {` at the top level.
fn opens_module(line: &str) -> bool {
    line.strip_prefix("mod ")
        .and_then(|rest| rest.strip_suffix(" {"))
        .is_some_and(|name| !name.is_empty() && name.chars().all(is_identifier_char))
}

fn is_identifier_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// The first protocol token on `line`, matched as a whole identifier.
fn first_token(line: &str) -> Option<&str> {
    line.split(|c: char| !is_identifier_char(c)).find(|word| {
        TOKENS.contains(word)
            || word
                .strip_prefix(VARIABLE_PREFIX)
                .and_then(|rest| rest.chars().next())
                .is_some_and(|c| c.is_ascii_uppercase())
    })
}
