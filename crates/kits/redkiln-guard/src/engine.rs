//! The denylist engine: it reads a Bash command as the shell would and asks a rule pack what
//! may not run. It denies a command or leaves it undecided, never allows.
//!
//! [`check`] lexes the command with [`shell::parse`](crate::shell::parse) into every simple
//! command it runs, and walks each in order. Each one is unwrapped ([`wrappers`]: leading
//! assignments and keywords, `sudo`, `env`, `command`, `exec`, `nohup`, `time`, `nice`,
//! `timeout`, `setsid`, `stdbuf`, `ionice`, `chrt`, `taskset`, `busybox`, `flock`, `xargs`,
//! `find`'s actions, `function`, `coproc`, path prefixes) and the strings that shells, `eval`,
//! `env -S`, `su -c`, `script -c`, `trap`, `watch`, `alias` and git's command settings
//! ([`git_settings`]) run are lexed and walked in turn, as is a script a shell reads from a
//! static heredoc, here-string or `echo` pipe. Brace expansion is applied to arguments
//! ([`braces`]), the values of variables the command sets are put in, a word holding both an
//! expansion and a pattern is also judged with the expansion empty ([`readings`]), and `cd` is
//! followed ([`Cwd`]). What may not run is a rule pack's, reached through [`Pack`] at fixed
//! points of the walk. What the engine cannot model it denies under the pack's
//! [`Pack::UNREADABLE`]; every input is bounded by [`MAX_DEPTH`] and [`MAX_LEXED_MIB`].

pub mod braces;
mod checker;
mod cwd;
mod feed;
pub mod git_settings;
mod program;
pub mod readings;
mod seam;
mod variables;
pub mod wrappers;

pub use checker::{MAX_DEPTH, MAX_LEXED_MIB, check};
pub use cwd::Cwd;
pub(crate) use cwd::is_absolute;
pub use program::Program;
pub use seam::{Call, Denial, Pack, ScriptFile};

#[cfg(test)]
mod tests;
