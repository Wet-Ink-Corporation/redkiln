//! `bench hook standing-orders`: the `SubagentStart` hook. It fails open and never blocks
//! (`SubagentStart` cannot block).
//!
//! It reads Claude Code's `SubagentStart` input on stdin (it uses `cwd`) and adds the
//! repository's standing orders, `.redkiln/bench/standing-orders.md` at the worktree root (or
//! under `cwd` outside git; anneal's `.anneal/standing-orders.md` when only it exists, read
//! as a 0.x fallback without a warning, `repo_file`), to the subagent's context as
//! `additionalContext`. With no such file it writes nothing. A file that is not a regular
//! file, resolves outside the worktree's own folder (a symlinked folder), cannot be read or is
//! over
//! [`MAX_ORDERS_BYTES`] is injected not at all, never in part: the hook writes one `ERROR` line
//! as `systemMessage` instead. Claude Code moves `additionalContext` over 10,000 characters to
//! a file and passes a preview, so orders meant to be read whole stay under that. See
//! `docs/hooks.md`.

use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

use crate::error_chain::reason;
use crate::harness::claude_code::StopOutput;
use crate::hook::{HookEnv, StdinError, read_stdin};
use crate::paths::{self, PathError};
use crate::repo_file::{self, STANDING_ORDERS};

/// The most bytes of standing orders injected (H-13).
pub(crate) const MAX_ORDERS_BYTES: u64 = 16 * 1024;

/// The most stdin the hook reads; a `SubagentStart` input is far smaller (H-13).
const MAX_INPUT_BYTES: u64 = 64 * 1024;

/// How long git may take to find the worktree root, well inside the hook's 5 s `timeout`.
const GIT_TIMEOUT: Duration = Duration::from_secs(2);

/// The fields of Claude Code's `SubagentStart` input the hook uses; the rest are ignored.
#[derive(Debug, Deserialize)]
struct Input {
    #[serde(default)]
    cwd: Option<PathBuf>,
}

/// Why the hook input could not be used.
#[derive(Debug, thiserror::Error)]
enum InputError {
    #[error(transparent)]
    Stdin(#[from] StdinError),
    #[error("the input is not SubagentStart JSON")]
    Json(#[source] serde_json::Error),
}

/// Why the standing orders could not be injected.
#[derive(Debug, thiserror::Error)]
enum OrdersError {
    #[error("the hook input cannot be read")]
    Input(#[source] InputError),
    #[error("reading the working directory")]
    CurrentDir(#[source] io::Error),
    #[error("finding the worktree")]
    Worktree(#[source] PathError),
    #[error("{} is not a regular file", path.display())]
    NotAFile { path: PathBuf },
    #[error(
        "{} resolves to {}, not this worktree's own {own}; make {dir} a plain directory",
        path.display(),
        resolved.display(),
        dir = own.rsplit_once('/').map_or(*own, |(dir, _)| dir)
    )]
    NotOwn {
        path: PathBuf,
        resolved: PathBuf,
        own: &'static str,
    },
    #[error("{} is larger than {MAX_ORDERS_BYTES} bytes; shorten it", path.display())]
    TooLarge { path: PathBuf },
    #[error("reading {}", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

/// Runs the hook: reads the input from `env.stdin` and writes the standing orders seen from
/// its `cwd` (or `env.current_dir`) to `out` as `SubagentStart` context; nothing when there
/// are none, one `ERROR` line as `systemMessage` when they cannot be read. Pass `&mut out` to
/// keep the writer.
///
/// # Errors
/// Only the I/O error from writing `out`.
pub(crate) fn run<R, D, L, W>(env: HookEnv<R, D, L>, out: W) -> io::Result<()>
where
    R: Read,
    D: FnOnce() -> io::Result<PathBuf>,
    L: Fn(&str) -> Option<OsString>,
    W: Write,
{
    let output = StopOutput::new("SubagentStart");
    let output = match orders(env.stdin, env.current_dir) {
        Ok(None) => output,
        Ok(Some((path, text))) => output.context(format!(
            "Standing orders for every subagent ({path}):\n\n{text}"
        )),
        Err(error) => output.error(&format!("standing-orders: {}", reason(&error))),
    };
    output.write(out)
}

/// The standing orders' path and text seen from the input's working directory; `None` when
/// there is no file.
fn orders<R: Read>(
    stdin: R,
    current_dir: impl FnOnce() -> io::Result<PathBuf>,
) -> Result<Option<(String, String)>, OrdersError> {
    let input = read_stdin(stdin, MAX_INPUT_BYTES)
        .map_err(InputError::from)
        .and_then(|bytes| serde_json::from_slice::<Input>(&bytes).map_err(InputError::Json))
        .map_err(OrdersError::Input)?;
    let cwd = match input.cwd {
        Some(cwd) => cwd,
        None => current_dir().map_err(OrdersError::CurrentDir)?,
    };
    let root = match paths::worktree_root_within(&cwd, GIT_TIMEOUT) {
        Ok(root) => PathBuf::from(root),
        Err(PathError::NotARepository { .. }) => cwd,
        Err(error) => return Err(OrdersError::Worktree(error)),
    };
    let chosen = STANDING_ORDERS.choose(&root).path;
    read_orders(&root, chosen).map(|text| text.map(|text| (chosen.to_owned(), text)))
}

/// The text of the standing orders `chosen` (relative, `/`-separated) under the worktree
/// `root` (lossy UTF-8); `None` when there is no file. They must resolve to the resolved
/// root's own file, as the brief hook requires of a brief: a symlinked folder would inject a
/// file from outside the worktree.
fn read_orders(root: &Path, chosen: &'static str) -> Result<Option<String>, OrdersError> {
    let orders_path = repo_file::under(root, chosen);
    let path = orders_path.as_path();
    // Checked before opening: a FIFO would block the hook, and a directory opens on unix only.
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => {
            return Err(OrdersError::NotAFile {
                path: path.to_path_buf(),
            });
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(OrdersError::Read {
                path: path.to_path_buf(),
                source,
            });
        }
    }
    let read_error = |source| OrdersError::Read {
        path: path.to_path_buf(),
        source,
    };
    let resolved = fs::canonicalize(path).map_err(read_error)?;
    let own = repo_file::under(&fs::canonicalize(root).map_err(read_error)?, chosen);
    if resolved != own {
        return Err(OrdersError::NotOwn {
            path: path.to_path_buf(),
            resolved,
            own: chosen,
        });
    }
    let file = File::open(path).map_err(read_error)?;
    let mut bytes = Vec::new();
    file.take(MAX_ORDERS_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(read_error)?;
    if u64::try_from(bytes.len()).map_or(true, |len| len > MAX_ORDERS_BYTES) {
        return Err(OrdersError::TooLarge {
            path: path.to_path_buf(),
        });
    }
    Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
}
