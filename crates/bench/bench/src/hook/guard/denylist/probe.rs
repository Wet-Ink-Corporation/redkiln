//! The merge gate's probe, a rule pack that collects `gh pr merge` calls rather than denying
//! them, and notes as it walks a command: each `gh pr merge`, a [`GH_TARGET_VARIABLES`] the
//! command sets, and how it changed directory before gh. The denylist's entries are not
//! applied; what the engine cannot read still denies, as does a redirection writing lint or
//! hook config ([`config::redirect`], as in the guard).
//!
//! These notes are defence in depth: the gate's invariant rests on `--repo`, which gh reads
//! before `GH_REPO`, `GH_HOST` and the working directory.

use super::braces;
use super::config;
use super::cwd::Cwd;
use super::gh::{self, PrMerge};
use super::seam::{Call, Pack};
use super::wrappers::{self, Program};
use crate::hook::guard::{Denial, Rule};
use redkiln_guard::shell::{SimpleCommand, Stdin, Word};

/// The variables that point gh at another repository or host; the probe notes a command that
/// sets or names one (`GH_REPO=o/x gh ...`, `env`, `export`), wherever it does.
const GH_TARGET_VARIABLES: [&str; 2] = ["GH_REPO", "GH_HOST"];

/// The prefix every gh variable name has.
const GH_PREFIX: &str = "GH_";

/// The most `gh pr merge` calls one command may make before the probe refuses it (H-13).
const MAX_MERGES: usize = 8;

/// A gh call whose first operands are known only at run time (`gh "$@"`).
const RUN_TIME_GH_CALL: &str = "a gh call whose subcommand is known only at run time";

/// A program known only at run time (`${GH:-gh}`, `g$x`).
const RUN_TIME_PROGRAM: &str = "a program known only at run time";

/// A function, alias or `hash -p` definition, or a `BASH_FUNC_` export.
const DEFINITION: &str = "a function, alias or hash -p definition";

/// A word naming gh where the probe reads no gh call with it (`xargs gh`, `parallel gh`,
/// `ln -s /usr/bin/gh`).
const GH_WORD: &str = "gh where the probe cannot read its arguments";

/// A `gh api` call, which may call the merge endpoint (`gh api -X PUT .../pulls/7/merge`).
const GH_API: &str = "gh api, which may call the merge endpoint";

/// How a gh word or call of a command may hide a merge from the probe, ranked: a later
/// variant outranks an earlier one, and the strongest seen is kept.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
enum GhHides {
    /// Every word naming gh is a call read in full, and none is `gh api`.
    #[default]
    Nothing,
    /// A simple command has more words naming gh than gh calls read in full ([`GH_WORD`]).
    UnreadWord,
    /// A `gh api` call ([`GH_API`]).
    Api,
}

impl GhHides {
    /// How the command may run a merge the probe cannot read, if it may.
    fn how(self) -> Option<&'static str> {
        match self {
            Self::Nothing => None,
            Self::UnreadWord => Some(GH_WORD),
            Self::Api => Some(GH_API),
        }
    }
}

/// The word whose presence in the command's words, in any case, arms the tripwire.
const MERGE_WORD: &str = "merge";

/// The names gh's program file has, compared without ASCII case.
const GH_NAMES: [&str; 2] = ["gh", "gh.exe"];

/// Whether `text` holds [`MERGE_WORD`] in any case.
fn holds_merge(text: &str) -> bool {
    text.to_lowercase().contains(MERGE_WORD)
}

/// Whether the last path component of `text` (after `/` or `\`) is one of [`GH_NAMES`].
fn names_gh(text: &str) -> bool {
    let name = text.rsplit(['/', '\\']).next().unwrap_or(text);
    GH_NAMES.iter().any(|gh| name.eq_ignore_ascii_case(gh))
}

/// The one of [`GH_TARGET_VARIABLES`] `text` sets or names: `NAME`, `NAME=...` or `NAME+=...`.
fn gh_target(text: &str) -> Option<&'static str> {
    GH_TARGET_VARIABLES.into_iter().find(|name| {
        text.strip_prefix(name).is_some_and(|after| {
            after.is_empty() || after.starts_with('=') || after.starts_with("+=")
        })
    })
}

/// Whether `word` holds a brace sequence (`{a..b}`, which brace expansion here does not
/// expand) that may build a gh variable's name: the text before its first `{` starts with
/// `GH_`, or begins `GH_` (`G{H..H}_REPO`), or is empty in an assignment (`{G..G}H_REPO=`).
fn sequence_may_name_gh_variable(word: &Word) -> bool {
    // An unquoted brace whatever else the word holds (`GH_REP{O..O}"$x"=`).
    if !word.has_pattern() {
        return false;
    }
    let text = word.text.as_str();
    let Some((prefix, group)) = text.split_once('{') else {
        return false;
    };
    let sequence = group
        .split_once("..")
        .is_some_and(|(_, after)| after.contains('}'));
    let names = if prefix.is_empty() {
        text.contains('=')
    } else {
        prefix.starts_with(GH_PREFIX) || GH_PREFIX.starts_with(prefix)
    };
    sequence && names
}

/// A denial for a command the probe cannot read.
fn unreadable(reason: String) -> Denial {
    Denial::new(Rule::Unparseable, reason)
}

/// What the probe saves for the engine to hand back: the gh calls read in full in the simple
/// command being checked, and how many merges were found.
#[derive(Debug, Clone, Copy)]
pub(super) struct ProbeMark {
    calls: usize,
    merges: usize,
}

/// The merge gate's probe for one check: the `gh pr merge` calls found and the notes on how
/// the command may run one it cannot read.
#[derive(Debug, Default)]
pub(super) struct MergeProbe {
    merges: Vec<PrMerge>,
    /// The first of `GH_REPO` and `GH_HOST` a word of the command sets or names.
    gh_env: Option<&'static str>,
    /// The first `cd`, `pushd` or `popd` the command ran, so far.
    moved: Option<&'static str>,
    /// The first way the command may run a call the probe cannot read, so far: a program
    /// known only at run time, or a function or alias definition.
    unseen: Option<&'static str>,
    /// Whether the command calls gh with a first or second operand known only at run time
    /// (`gh "$@"`).
    unseen_gh_call: bool,
    /// Whether a word of the command, or a here-document or here-string it reads, holds
    /// `merge` in any case, after quote removal.
    mentions_merge: bool,
    /// The strongest way a gh word or call of the command may hide a merge, so far.
    gh_hides: GhHides,
    /// How many gh calls the probe read in full in the simple command it is checking: run as
    /// the program itself or under a known wrapper, with no arguments appended at run time and
    /// not found by scanning an unknown program's arguments ([`Call::in_arguments`]: the
    /// unknown program may pass it other arguments, `parallel gh ::: pr`).
    gh_calls_here: usize,
}

impl MergeProbe {
    /// The `gh pr merge` calls found, in order, each once, each noting a [`GH_TARGET_VARIABLES`]
    /// the command sets anywhere and a `cd`, `pushd` or `popd` it runs anywhere: a function or
    /// trap body is read where it is defined but may run after a later change.
    pub(super) fn into_merges(self) -> Vec<PrMerge> {
        let (env, moved) = (self.gh_env, self.moved);
        self.merges
            .into_iter()
            .map(|merge| PrMerge {
                env: env.map(str::to_owned),
                moved: merge.moved.or(moved),
                ..merge
            })
            .collect()
    }

    /// How the command may run a merge the probe cannot read, if it may: [`RUN_TIME_GH_CALL`]
    /// whenever it makes one; else, when a word holds `merge`, the first of
    /// [`RUN_TIME_PROGRAM`] and [`DEFINITION`], then [`GH_API`] when it runs `gh api`, then
    /// [`GH_WORD`] when a simple command has more words naming gh than the probe read gh calls
    /// in it in full (`gh pr merge 3 --disable-auto` is read; `xargs gh`, `parallel gh` and
    /// `ln -s /usr/bin/gh` are not).
    pub(super) fn unseen(&self) -> Option<&'static str> {
        if self.unseen_gh_call {
            return Some(RUN_TIME_GH_CALL);
        }
        if !self.mentions_merge {
            return None;
        }
        self.unseen.or_else(|| self.gh_hides.how())
    }

    /// Notes the first word of `words` that sets or names one of [`GH_TARGET_VARIABLES`]
    /// (`GH_REPO`, `GH_REPO=...`, `GH_REPO+=...`). A word holding a brace group is read as
    /// each word it expands to (`export GH_RE{PO,X}=o/x` sets `GH_REPO`).
    ///
    /// # Errors
    /// The unreadable denial when one word expands to more than the brace cap, or holds a
    /// brace sequence that may build a gh variable's name.
    fn note_gh_env(&mut self, words: &[Word]) -> Option<Denial> {
        if self.gh_env.is_some() {
            return None;
        }
        for word in words {
            if sequence_may_name_gh_variable(word) {
                return Some(unreadable(format!(
                    "{} may build a gh variable's name by a brace sequence; write the name out",
                    word.text
                )));
            }
            let expanded = match braces::expand(std::slice::from_ref(word)) {
                Ok(expanded) => expanded,
                Err(error) => return Some(unreadable(error.to_string())),
            };
            self.gh_env = expanded.iter().find_map(|word| gh_target(&word.text));
            if self.gh_env.is_some() {
                break;
            }
        }
        None
    }

    /// Notes whether `command`'s words (after quote removal, so `'mer'ge` and `me\rge` read
    /// `merge`) or the here-document or here-string it reads hold `merge`. Substitutions and
    /// `bash -c` strings are lexed into commands of their own, so their words are noted too.
    fn note_mentions(&mut self, command: &SimpleCommand) {
        for word in command.words() {
            self.mentions_merge |= holds_merge(&word.text);
        }
        if let Stdin::Text(body) = command.stdin() {
            self.mentions_merge |= holds_merge(&body.text);
        }
    }

    /// Notes `how` the command may run a call the probe cannot read ([`RUN_TIME_PROGRAM`],
    /// [`DEFINITION`]); the first one is kept.
    fn note_unseen(&mut self, how: &'static str) {
        self.unseen.get_or_insert(how);
    }

    /// Records the `gh pr merge` that `gh <args>` runs in `call`, if any, with how the command
    /// changed directory before it; past [`MAX_MERGES`] denies. A call whose first operands are
    /// known only at run time is noted ([`RUN_TIME_GH_CALL`]), as is `gh api` ([`GH_API`]), and
    /// one read in full is counted against the words naming gh in its simple command (see
    /// [`Pack::leave_command`]).
    fn gh(&mut self, call: &Call<'_>) -> Option<Denial> {
        let args = call.args;
        self.unseen_gh_call |= gh::call_unseen(args);
        if gh::calls_api(args) {
            self.gh_hides = GhHides::Api;
        }
        // A bare `gh` reads as no call: `command -v gh` unwraps to it, though it only names gh.
        if !call.appended && !call.in_arguments && !args.is_empty() {
            self.gh_calls_here = self.gh_calls_here.saturating_add(1);
        }
        let merge = PrMerge {
            moved: call.moved.or(self.moved),
            ..gh::pr_merge(args)?
        };
        if !self.merges.contains(&merge) {
            self.merges.push(merge);
        }
        (self.merges.len() > MAX_MERGES).then(|| {
            unreadable(format!(
                "the command runs more than {MAX_MERGES} gh pr merge calls; merge one pull \
                 request at a time"
            ))
        })
    }
}

impl Pack for MergeProbe {
    type Rule = Rule;
    type Mark = ProbeMark;
    const UNREADABLE: Rule = Rule::Unparseable;

    /// Notes a function definition and a mention of `merge` in `command`, and starts reading
    /// its gh calls; the count of the command it nests in is returned, for
    /// [`Pack::leave_command`] to restore.
    fn enter_command(
        &mut self,
        command: &SimpleCommand,
        _words: &[Word],
    ) -> Result<ProbeMark, Denial> {
        if command.defines_function() {
            self.note_unseen(DEFINITION);
        }
        self.note_mentions(command);
        Ok(ProbeMark {
            calls: std::mem::take(&mut self.gh_calls_here),
            merges: self.merges.len(),
        })
    }

    /// Notes a word naming gh that no gh call read in full in the simple command pairs with
    /// (`$G pr view 1` with `G=gh` is a call; `xargs gh` is not), and restores the count
    /// `outer` of the command it nests in.
    fn leave_command(&mut self, words: &[Word], outer: ProbeMark) {
        let named = words.iter().filter(|word| names_gh(&word.text)).count();
        if named > self.gh_calls_here {
            self.gh_hides = self.gh_hides.max(GhHides::UnreadWord);
        }
        self.gh_calls_here = outer.calls;
    }

    fn redirect(&mut self, target: &Word, cwd: &Cwd) -> Option<Denial> {
        config::redirect(target, cwd)
    }

    /// Notes a gh variable `words` set or name ([`MergeProbe::note_gh_env`]), and a definition
    /// ([`wrappers::defines`]).
    fn words(&mut self, words: &[Word]) -> Option<Denial> {
        if let Some(denial) = self.note_gh_env(words) {
            return Some(denial);
        }
        if wrappers::defines(words) {
            self.note_unseen(DEFINITION);
        }
        None
    }

    /// Notes a program known only at run time ([`RUN_TIME_PROGRAM`]).
    fn read(&mut self, call: &Call<'_>) -> Option<Denial> {
        if let Program::Unknown(_) = call.program {
            self.note_unseen(RUN_TIME_PROGRAM);
        }
        None
    }

    fn command(&mut self, call: &Call<'_>) -> Option<Denial> {
        match call.program {
            Program::Named("gh") => self.gh(call),
            Program::Named(_) | Program::Unknown(_) => None,
        }
    }

    /// gh, whose calls the probe reads ([`MergeProbe::gh`]).
    fn reads_arguments(&self, name: &str) -> bool {
        name == "gh"
    }

    /// gh, whose arguments an unknown program may run (`xargs gh pr merge 1`).
    fn watches(&self, name: &str) -> bool {
        name == "gh"
    }

    fn changed_directory(&mut self, how: &'static str) {
        self.moved.get_or_insert(how);
    }

    fn mark(&self) -> ProbeMark {
        ProbeMark {
            calls: self.gh_calls_here,
            merges: self.merges.len(),
        }
    }

    /// Restores the gh calls counted before the second reading, and drops the merges it found
    /// when the first reading found one: it is the same call (`gh pr merge 1$x` is one merge,
    /// its pull request known only at run time), while a merge only the emptied reading shows
    /// is kept.
    fn reread(&mut self, first: ProbeMark, second: ProbeMark) {
        self.gh_calls_here = second.calls;
        if second.merges > first.merges {
            self.merges.truncate(second.merges);
        }
    }
}
