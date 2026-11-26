//! The guard's rule pack: the denylist's entries, [`git`](super::git) (force push to a
//! protected branch, `git add -f`, settings it cannot follow), [`rm`](super::rm) (recursive
//! force delete outside temp and scratch), [`config`](super::config) (Bash writes to lint or
//! hook config), [`anneal`](super::anneal) (a human verdict or label from a model's call) and
//! [`weighin`](super::weighin) (`weighin hook`, the harness's entry point, from a model's call).

use super::Context;
use super::anneal;
use super::config;
use super::git;
use super::rm;
use super::weighin;
use crate::hook::guard::{Denial, Rule};
use redkiln_guard::engine::{Call, Cwd, Pack, Program, ScriptFiles};
use redkiln_guard::shell::{SimpleCommand, Word};

/// The guard's pack: the denylist's entries for one check, with the `rm` entry's safe roots.
#[derive(Debug)]
pub(super) struct GuardPack<'c> {
    context: &'c Context,
    /// Whether the command, so far, has an anneal call the guard cannot read
    /// ([`anneal::unread_call`]: `anneal "$@"`, as in a function body, or `bench decide label
    /// "$@"`) and the two halves of a human actor in any simple command ([`anneal::Halves`]):
    /// together, in any order, they deny (`f() { anneal "$@"; }; f verdict set --actor human`,
    /// `set -- d1 yes --actor; set -- "$@" human; bench decide label "$@"`).
    anneal_seen: anneal::Seen,
}

impl<'c> GuardPack<'c> {
    /// The pack for one check, under `context`.
    pub(super) fn new(context: &'c Context) -> Self {
        Self {
            context,
            anneal_seen: anneal::Seen::default(),
        }
    }
}

impl Pack for GuardPack<'_> {
    type Rule = Rule;
    type Mark = ();
    const UNREADABLE: Rule = Rule::Unparseable;

    /// The `human-only` denial once the command, so far, holds both halves of a human actor
    /// ([`anneal::Halves`]) and an anneal call the guard cannot read ([`anneal::Seen`]); notes
    /// the halves one simple command's words (`words`, its variables put in) hold. Every simple
    /// command of the call passes here: a shell's positional arguments, `set --` operands, the
    /// words of an array assignment (listed as a command of their own), an assignment of one
    /// element and a nested `bash -c` string.
    fn enter_command(&mut self, _command: &SimpleCommand, words: &[Word]) -> Result<(), Denial> {
        self.anneal_seen.words(words).map_or(Ok(()), Err)
    }

    fn leave_command(&mut self, _words: &[Word], (): ()) {}

    /// The `weighin-hook` denial for a script file that is the launcher copy run with
    /// arguments that may run `hook` (`sudo sh libexec/weighin hook`; [`weighin::script_files`]).
    fn script_files(&mut self, files: &ScriptFiles<'_>) -> Option<Denial> {
        weighin::script_files(files)
    }

    fn redirect(&mut self, target: &Word, cwd: &Cwd) -> Option<Denial> {
        config::redirect(target, cwd)
    }

    /// The `human-only` denial once a reading of a program's arguments (unwrapped and
    /// brace-expanded) completes, with what the command held before, both halves of a human
    /// actor and an anneal call the guard cannot read ([`anneal::unread_call`],
    /// [`anneal::Seen`]); notes what it holds.
    fn read(&mut self, call: &Call<'_>) -> Option<Denial> {
        if let Some(denial) = self.anneal_seen.words(call.args) {
            return Some(denial);
        }
        let runs_anneal = match call.program {
            Program::Named(name) => anneal::is_anneal(name),
            Program::Unknown(word) => anneal::may_name_anneal(word),
        };
        if runs_anneal && anneal::unread_call(call.args) {
            return self.anneal_seen.call_unread();
        }
        None
    }

    /// The entries for the program of `call`: its own for `git` and `rm`, `anneal` and the
    /// config writers for any other name, and every entry for a program known only at run time.
    fn command(&mut self, call: &Call<'_>) -> Option<Denial> {
        let Call {
            args,
            appended,
            cwd,
            ..
        } = *call;
        match call.program {
            Program::Named("git") => git::check(args),
            Program::Named("rm") => {
                rm::check(args, self.context, appended).or_else(|| config::check("rm", args, cwd))
            }
            Program::Named(name) => anneal::is_anneal(name)
                .then(|| anneal::check(args, appended))
                .flatten()
                .or_else(|| {
                    weighin::is_weighin(name)
                        .then(|| weighin::check(args, appended))
                        .flatten()
                })
                .or_else(|| config::check(name, args, cwd)),
            Program::Unknown(word) => git::check(args)
                .or_else(|| rm::check(args, self.context, appended))
                .or_else(|| config::check("cp", args, cwd))
                .or_else(|| {
                    anneal::may_name_anneal(word)
                        .then(|| anneal::check(args, appended))
                        .flatten()
                })
                .or_else(|| {
                    weighin::may_name_weighin(word)
                        .then(|| weighin::check(args, appended))
                        .flatten()
                }),
        }
    }

    /// `anneal`, whose own arguments are still read as a command, as `task done -- <cmd>` runs
    /// them ([`anneal::is_anneal`]), and `weighin`, so that an unknown program's arguments that
    /// run it (`caffeinate weighin hook`) are read ([`weighin::is_weighin`]).
    fn watches(&self, name: &str) -> bool {
        anneal::is_anneal(name) || weighin::is_weighin(name)
    }

    fn mark(&self) {}
}
