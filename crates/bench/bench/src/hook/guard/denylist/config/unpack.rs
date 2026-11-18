//! Writers that unpack a tree into a destination (#117): `rsync`, whose destination is its last
//! operand as `cp`'s is. Each destination is judged as `cp`'s is: a protected file, or a config
//! folder the tree may bring its own config into, since the guard cannot tell whether the
//! folder exists. What lands in the working directory itself (`rsync -a x/ .`) is a limit,
//! as `cp -r x/. .` is.

use super::{CopyArgs, copy_into};
use crate::hook::guard::Denial;
use redkiln_guard::engine::Cwd;
use redkiln_guard::engine::wrappers::{is_option, skip_one};
use redkiln_guard::shell::Word;

/// Short options of `rsync` that take a value: the rest of the cluster, else the next word.
const RSYNC_SHORT_VALUES: [char; 6] = ['e', 'f', 'T', 'B', 'M', '@'];

/// Long options of `rsync` that take a value (`--name VALUE` or `--name=VALUE`). popt reads
/// long names whole, never abbreviated, so they are matched exactly.
const RSYNC_LONG_VALUES: [&str; 54] = [
    "--rsh",
    "--rsync-path",
    "--filter",
    "--exclude",
    "--exclude-from",
    "--include",
    "--include-from",
    "--files-from",
    "--backup-dir",
    "--suffix",
    "--chmod",
    "--chown",
    "--usermap",
    "--groupmap",
    "--block-size",
    "--max-delete",
    "--max-size",
    "--min-size",
    "--max-alloc",
    "--partial-dir",
    "--temp-dir",
    "--compare-dest",
    "--copy-dest",
    "--link-dest",
    "--compress-choice",
    "--zc",
    "--compress-level",
    "--zl",
    "--skip-compress",
    "--checksum-choice",
    "--cc",
    "--checksum-seed",
    "--modify-window",
    "--timeout",
    "--contimeout",
    "--address",
    "--port",
    "--sockopts",
    "--out-format",
    "--log-file",
    "--log-file-format",
    "--password-file",
    "--early-input",
    "--bwlimit",
    "--stop-after",
    "--stop-at",
    "--write-batch",
    "--only-write-batch",
    "--read-batch",
    "--protocol",
    "--iconv",
    "--remote-option",
    "--info",
    "--debug",
];

/// The name of the long option `text`, without any `=VALUE`.
fn long_name(text: &str) -> &str {
    text.split_once('=').map_or(text, |(name, _)| name)
}

/// `rsync`'s operands, read past its options wherever they stand (popt permutes them), and
/// whether it removes its sources (`--remove-source-files`).
fn rsync_operands(args: &[Word]) -> (Vec<&Word>, bool) {
    let mut operands = Vec::new();
    let mut removes = false;
    let mut rest = args;
    while let [first, tail @ ..] = rest {
        rest = tail;
        let text = first.text();
        if text == "--" {
            operands.extend(tail);
            break;
        }
        if !is_option(first) {
            operands.push(first);
        } else if let Some(long) = text.strip_prefix("--") {
            let name = long_name(text);
            removes |= name == "--remove-source-files";
            if !long.contains('=') && RSYNC_LONG_VALUES.contains(&name) {
                rest = skip_one(rest);
            }
        } else if let Some((at, _)) = text.char_indices().skip(1).find(|(_, flag)| {
            RSYNC_SHORT_VALUES.contains(flag)
        }) && at.saturating_add(1) == text.len()
        {
            // The value-taking option ends the cluster: its value is the next word.
            rest = skip_one(rest);
        }
    }
    (operands, removes)
}

/// `rsync [OPTION]... SRC... DEST`: the destination and joined sources judged as `cp`'s are,
/// and with `--remove-source-files` each source as `mv` judges what it moves away.
pub(super) fn rsync(args: &[Word], cwd: &Cwd) -> Option<Denial> {
    let (operands, removes) = rsync_operands(args);
    let copy = CopyArgs {
        operands,
        slash_copies_contents: true,
        ..CopyArgs::default()
    };
    copy_into("rsync", &copy, removes, cwd)
}
