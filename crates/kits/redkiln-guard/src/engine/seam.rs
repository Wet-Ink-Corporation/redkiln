//! The seam between the engine and a rule pack.
//!
//! The engine ([`check`](super::check)) lexes a command, unwraps every simple command it
//! runs ([`wrappers`](super::wrappers)), reads the strings shells, `eval` and git's settings
//! run, follows `cd` and the variables a command sets, and refuses what it cannot read or what
//! passes its budgets. What a command may not do is a pack's: the engine calls a [`Pack`] at
//! fixed points of its walk and stops at the first [`Denial`] it returns. Bench has two packs,
//! the guard's entries and the merge gate's probe; nothing here names a plugin's type.

use super::cwd::Cwd;
use super::program::Program;
use crate::shell::{SimpleCommand, Word};

/// Why a command is denied: the pack's rule, and one line naming what was denied and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Denial<R> {
    /// The rule that denied it: a pack's own, or the pack's [`Pack::UNREADABLE`] for a command
    /// the engine cannot read.
    pub rule: R,
    /// One line naming what was denied and why.
    pub reason: String,
}

impl<R> Denial<R> {
    /// The denial under `rule` for `reason`.
    pub fn new(rule: R, reason: impl Into<String>) -> Self {
        Self {
            rule,
            reason: reason.into(),
        }
    }
}

/// One reading of a program's arguments as the engine resolved it: wrappers removed, brace
/// groups expanded, the variables the command set put in.
#[derive(Debug, Clone, Copy)]
pub struct Call<'a> {
    /// The program: a name without a path or `.exe`, or a word known only at run time, against
    /// which a pack applies every entry it has.
    pub program: Program<'a>,
    /// Its arguments.
    pub args: &'a [Word],
    /// Whether operands are appended at run time (`xargs`, `find -exec`).
    pub appended: bool,
    /// How a wrapper of this command, or of one it nests in, changed the working directory
    /// first (`env --chdir`, `sudo --chdir`, `find -execdir`).
    pub moved: Option<&'static str>,
    /// Whether the call was found by reading an unknown program's arguments as a command, so
    /// that program may pass it other arguments (`parallel gh ::: pr`).
    pub in_arguments: bool,
    /// Where relative paths start, as far as the `cd` commands seen so far say.
    pub cwd: &'a Cwd,
}

/// The script files one shell runs, which the engine does not open.
///
/// That is `sh FILE ARGS...` (and `bash`, `dash`, ... ; su's shell), or `source FILE ARGS...`
/// and `. FILE ARGS...`, which run it in the current shell with the same positional
/// arguments. Found in every reading the engine follows: wrappers removed (`sudo sh FILE`),
/// inside a `bash -c` string or a `find` action, brace groups expanded and the variables the
/// command set put in. A `-c` string, a script read from standard input (`sh -s`,
/// `sh /dev/stdin`) and one from a process substitution or a descriptor are the engine's own
/// and never come here.
///
/// A shell's options may be read several ways, each leaving a different word as the script
/// (`sh $o a b c`: `$o`, `a`, `b` or `c`), so every script the readings find comes in one call,
/// as positions in one list of words. A pack that judges each script's arguments with a walk
/// of its own is quadratic in the words; judge them in one pass over [`ScriptFiles::words`]
/// instead (H-13).
#[derive(Debug, Clone, Copy)]
pub struct ScriptFiles<'a> {
    /// What runs the files: the shell's name, `su` for su's shell, or `source` (for `.` too).
    pub runner: &'a str,
    /// The words the runner got: each script and, after it, its positional arguments.
    /// References, since su's shell gets words su permuted rather than one run of the
    /// command's words.
    pub words: &'a [&'a Word],
    /// Where a script stands in [`ScriptFiles::words`], in ascending order, one per position
    /// some reading takes as the script file.
    pub scripts: &'a [usize],
    /// Whether arguments are appended after the words at run time (`xargs`,
    /// `find -exec ... +`).
    pub appended: bool,
    /// Where relative paths start, as far as the `cd` commands seen so far say.
    pub cwd: &'a Cwd,
}

impl<'a> ScriptFiles<'a> {
    /// Each script file with its arguments, the words after it.
    pub fn files(&self) -> impl Iterator<Item = (&'a Word, &'a [&'a Word])> + use<'a> {
        let (words, scripts) = (self.words, self.scripts);
        scripts.iter().filter_map(move |&at| {
            let (script, args) = words.get(at..)?.split_first()?;
            Some((*script, args))
        })
    }
}

/// A rule pack: what the engine asks at each point of its walk.
///
/// A hook that denies stops the
/// check; one that returns nothing lets the walk go on. A pack never allows: what no hook
/// denies is left undecided.
pub trait Pack {
    /// The pack's rules.
    type Rule;
    /// What the pack saves of its own state per simple command and per reading, for
    /// [`Pack::leave_command`] and [`Pack::reread`] to restore.
    type Mark: Copy;
    /// The rule a command the engine cannot read (unclosed quotes, nesting past its depth,
    /// input past its budget, a brace expansion past its cap) is denied under.
    const UNREADABLE: Self::Rule;

    /// One simple command starts: `command` as lexed, `words` its words with the variables the
    /// check knows put in.
    ///
    /// # Errors
    /// The pack's denial, which stops the check before the command is read.
    fn enter_command(
        &mut self,
        command: &SimpleCommand,
        words: &[Word],
    ) -> Result<Self::Mark, Denial<Self::Rule>>;

    /// The simple command [`Pack::enter_command`] started, with its `words`, is read; `outer`
    /// is what that call returned.
    fn leave_command(&mut self, words: &[Word], outer: Self::Mark);

    /// A redirection writes `target`, one reading of its word, from `cwd`.
    fn redirect(&mut self, target: &Word, cwd: &Cwd) -> Option<Denial<Self::Rule>>;

    /// A list of words is about to be unwrapped: a simple command's, an `-exec` action's, or
    /// the command an unknown program's arguments may run.
    fn words(&mut self, _words: &[Word]) -> Option<Denial<Self::Rule>> {
        None
    }

    /// Every reading of a program's arguments, whatever the program, before the engine
    /// follows it (git's settings, `cd`) or asks [`Pack::command`].
    fn read(&mut self, _call: &Call<'_>) -> Option<Denial<Self::Rule>> {
        None
    }

    /// A shell, or `source`, runs script files the engine does not open ([`ScriptFiles`]):
    /// every one its readings find, in one call per shell.
    fn script_files(&mut self, _files: &ScriptFiles<'_>) -> Option<Denial<Self::Rule>> {
        None
    }

    /// The pack's entries for the program of `call`; `cd`, `pushd` and `popd` are the
    /// engine's and never come here.
    fn command(&mut self, call: &Call<'_>) -> Option<Denial<Self::Rule>>;

    /// Whether the pack reads the arguments of the program `name` itself, so the engine does
    /// not also read them as a command.
    fn reads_arguments(&self, _name: &str) -> bool {
        false
    }

    /// Whether an argument naming the program `name` starts a command the engine reads, besides
    /// the programs it checks, shells and wrappers (`task done -- <cmd>`).
    fn watches(&self, _name: &str) -> bool {
        false
    }

    /// The command ran `how` (`cd`, `pushd`, `popd`), changing the working directory.
    fn changed_directory(&mut self, _how: &'static str) {}

    /// The pack's state now, for [`Pack::reread`].
    fn mark(&self) -> Self::Mark;

    /// A second reading of the same arguments, with their expansions emptied, ended: `first`
    /// was marked before the first reading and `second` before the second. Both read the same
    /// call, so a pack drops what it would count twice.
    fn reread(&mut self, _first: Self::Mark, _second: Self::Mark) {}
}
