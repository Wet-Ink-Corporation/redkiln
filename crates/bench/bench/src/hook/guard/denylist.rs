//! The deterministic denylist: it denies a command or leaves it undecided, never allows.
//!
//! The engine lexes the command with [`shell::parse`](redkiln_guard::shell::parse) into every
//! simple command it runs, and checks each in order ([`checker`]). Each one is unwrapped
//! ([`wrappers`]: leading assignments and keywords, `sudo`, `env`, `command`, `exec`, `nohup`,
//! `time`, `nice`, `timeout`, `setsid`, `stdbuf`, `ionice`, `chrt`, `taskset`, `busybox`,
//! `flock`, `xargs`, `find`'s actions, `function`, `coproc`, path prefixes) and the strings that
//! shells, `eval`, `env -S`, `su -c`, `script -c`, `trap`, `watch`, `alias` and git's command
//! settings ([`git_settings`]) run are lexed and checked in turn, as is a script a shell reads
//! from a static heredoc, here-string or `echo` pipe ([`feed`]). Brace expansion is applied to
//! arguments ([`braces`]), the values of variables the command sets are put in
//! ([`variables`]), a word holding both an expansion and a pattern is also judged with the
//! expansion empty ([`readings`]), and `cd` is followed ([`cwd`]). What may not run is a rule
//! pack's, reached through [`seam::Pack`]: the guard's ([`entries::GuardPack`]: [`git`], force
//! push to a protected branch and `git add -f`; [`rm`], recursive force delete outside temp
//! and scratch; [`config`], Bash writes to lint or hook config; [`anneal`], a human verdict or
//! label, `--actor human`, from a model's Bash call) and the merge gate's probe
//! ([`probe::MergeProbe`]). What the engine cannot model it denies. The engine and the seam
//! name no Bench type. See `docs/guard.md`.

mod anneal;
mod braces;
mod checker;
mod config;
mod cwd;
mod entries;
mod feed;
mod gh;
mod git;
mod git_settings;
mod probe;
mod readings;
mod rm;
mod seam;
mod variables;
mod wrappers;

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

use super::Denial;
use crate::harness::claude_code::PROJECT_DIR_VAR;
pub(in crate::hook::guard) use checker::MAX_LEXED_MIB;
use entries::GuardPack;
pub(crate) use gh::{PrMerge, Target, read_target};
use probe::MergeProbe;
pub(in crate::hook::guard) use seam::Denial as PackDenial;
pub(in crate::hook::guard) use wrappers::{FindActions, Layout, Role, layout, program_name};

/// Directories under which `rm -rf` of a literal absolute path is allowed, besides `$TMPDIR`
/// and `$CLAUDE_PROJECT_DIR/.scratch`.
const TEMP_ROOTS: [&str; 4] = [
    "/tmp",
    "/var/folders",
    "/private/tmp",
    "/private/var/folders",
];

/// The project's scratch directory, under `$CLAUDE_PROJECT_DIR`.
const SCRATCH_DIR: &str = ".scratch";

/// What the guard's entries need from the hook's environment.
#[derive(Debug)]
pub(crate) struct Context {
    /// Directories whose contents `rm -rf` may delete, normalized.
    safe_roots: Vec<PathBuf>,
}

impl Context {
    /// The context from the hook's environment: `$TMPDIR` and `$CLAUDE_PROJECT_DIR`, read
    /// through `lookup`. A value that is not an absolute path below the root is ignored.
    pub(crate) fn new(lookup: impl Fn(&str) -> Option<OsString>) -> Self {
        let from_env = [
            lookup("TMPDIR").map(PathBuf::from),
            lookup(PROJECT_DIR_VAR).map(|project| PathBuf::from(project).join(SCRATCH_DIR)),
        ];
        let safe_roots = TEMP_ROOTS
            .iter()
            .map(PathBuf::from)
            .chain(from_env.into_iter().flatten())
            .map(|root| normalize(&root))
            .filter(|root| root.has_root() && root.components().any(is_normal))
            .collect();
        Self { safe_roots }
    }

    /// Whether `path` (normalized) lies strictly inside a temporary or scratch directory.
    fn is_disposable(&self, path: &Path) -> bool {
        self.safe_roots
            .iter()
            .any(|root| path.starts_with(root) && path != root)
    }
}

/// The denylist's answer. It never allows: what it does not deny is for the fallback.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Verdict {
    /// On the denylist, or unreadable.
    Deny(Denial),
    /// Not on the denylist.
    Undecided,
}

/// Checks the Bash command `command` against the denylist: the engine with the guard's pack.
pub(crate) fn check(command: &str, context: &Context) -> Verdict {
    match checker::check(command, &mut GuardPack::new(context)) {
        Some(denial) => Verdict::Deny(denial),
        None => Verdict::Undecided,
    }
}

/// What the merge gate's probe found in a command.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum MergeScan {
    /// The command was read.
    Read {
        /// Every `gh pr merge` call it runs, in order (none when it runs none).
        merges: Vec<PrMerge>,
        /// How it may run a merge the probe cannot read, as a phrase, if it may: it calls gh
        /// with a first or second operand known only at run time, or a word of it holds
        /// `merge` and it runs a program known only at run time, defines a function, alias or
        /// `hash -p`, or has a word naming gh the probe read no call from. A tripwire, not a
        /// proof: see `docs/verdict.md`.
        unseen: Option<&'static str>,
    },
    /// The command cannot be read (the guard denies it too).
    Unreadable(Denial),
}

/// Every `gh pr merge` the Bash command `command` runs, read as the guard reads commands
/// (wrappers, `bash -c`, `eval`, scripts on stdin, variables): the engine with the merge
/// gate's probe in place of the denylist's entries, and how it may run one the probe cannot
/// read.
pub(crate) fn find_pr_merge(command: &str) -> MergeScan {
    let mut probe = MergeProbe::default();
    if let Some(denial) = checker::check(command, &mut probe) {
        return MergeScan::Unreadable(denial);
    }
    let unseen = probe.unseen();
    MergeScan::Read {
        merges: probe.into_merges(),
        unseen,
    }
}

/// `path` with `.` dropped and `..` applied lexically (symbolic links are not followed).
fn normalize(path: &Path) -> PathBuf {
    let mut normal = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normal.pop();
            }
            other => normal.push(other),
        }
    }
    normal
}

const fn is_normal(component: Component<'_>) -> bool {
    matches!(component, Component::Normal(_))
}

#[cfg(test)]
mod tests;
