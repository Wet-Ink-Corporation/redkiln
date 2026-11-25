//! `rsync [OPTION]... SRC... DEST`, whose destination is its last operand as `cp`'s is. Its
//! options are read wherever they stand, after the operands too (a trailing `--delete`, #135),
//! and a long option the guard does not list may take the next word whatever it looks like
//! (#160).

use super::super::{CopyArgs, CopyMode, TrailingSlash, copy_into};
use super::Cluster;
use crate::hook::guard::Denial;
use redkiln_guard::engine::Cwd;
use redkiln_guard::engine::wrappers::is_option;
use redkiln_guard::shell::Word;

/// Short options of `rsync` that take a value: the rest of the cluster, else the next word.
const RSYNC_SHORT_VALUES: [char; 6] = ['e', 'f', 'T', 'B', 'M', '@'];

/// Long options of `rsync` that take a value (`--name VALUE` or `--name=VALUE`). popt reads
/// long names whole, never abbreviated, so they are matched exactly. Checked against rsync
/// v3.5.2 (commit `41bbd895`), `options.c`: every `POPT_ARG_STRING` and `POPT_ARG_INT` entry of
/// `long_options[]` and `long_daemon_options[]` (#160).
const RSYNC_LONG_VALUES: [&str; 65] = [
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
    "--log-format",
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
    "--outbuf",
    "--stderr",
    "--copy-as",
    "--confine-root",
    "--compress-threads",
    "--zt",
    "--time-limit",
    "--nice",
    // Both switch rsync to daemon-mode parsing.
    "--config",
    "--dparam",
];

/// The name of the long option `text`, without any `=VALUE`.
fn long_name(text: &str) -> &str {
    text.split_once('=').map_or(text, |(name, _)| name)
}

/// How `rsync` reads one word while it still reads options. popt takes a value's next word
/// whole, even one starting with `-` (rsync v3.5.2 `popt/popt.c`, `poptGetNextOpt`), and reads
/// options wherever they stand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    /// `--`: every later word is an operand.
    EndOfOptions,
    /// A word that is not an option.
    Operand,
    /// An option that takes no later word: a flag, `--name=VALUE`, a cluster holding its value.
    Flag,
    /// An option whose value is the next word.
    Valued,
    /// A long option [`RSYNC_LONG_VALUES`] does not list, written without `=`: a flag, or an
    /// option whose value is the next word, whatever that word looks like.
    Unlisted,
}

impl Step {
    /// How `rsync` reads `word`, followed by `tail`.
    fn of(word: &Word, tail: &[Word]) -> Self {
        let text = word.text();
        if text == "--" {
            Self::EndOfOptions
        } else if !is_option(word) {
            Self::Operand
        } else if text.starts_with("--") {
            if text.contains('=') {
                Self::Flag
            } else if RSYNC_LONG_VALUES.contains(&text) {
                Self::Valued
            } else {
                Self::Unlisted
            }
        } else if Cluster::read(word, 1, tail, &RSYNC_SHORT_VALUES).takes_next {
            Self::Valued
        } else {
            Self::Flag
        }
    }

    /// How many words on the next one read as an option or an operand stands: one, two (this
    /// one takes the next word), or either. After [`Step::EndOfOptions`] every word is an
    /// operand, so readers stop there.
    fn strides(self) -> &'static [usize] {
        match self {
            Self::EndOfOptions | Self::Operand | Self::Flag => &[1],
            Self::Valued => &[2],
            Self::Unlisted => &[1, 2],
        }
    }
}

/// The value of `flags` at `at`, or `past` beyond the words.
fn flag_at(flags: &[bool], at: usize, past: bool) -> bool {
    flags.get(at).copied().unwrap_or(past)
}

/// Every way `rsync` may read its words: each long option the table does not list either a
/// flag or taking the next word. Kept as two flags per word, so it is linear in the words
/// rather than one entry per reading.
#[derive(Debug)]
struct Readings<'a> {
    words: &'a [Word],
    steps: Vec<Step>,
    /// `reached[i]`: some reading reads word `i` while it still reads options.
    reached: Vec<bool>,
    /// `clear[i]`: some reading reads from word `i` (reading options) to the end with no
    /// further operand.
    clear: Vec<bool>,
    /// The first `--` some reading reads as the end of options.
    end_of_options: Option<usize>,
}

impl<'a> Readings<'a> {
    fn new(words: &'a [Word]) -> Self {
        let steps: Vec<Step> = words
            .iter()
            .enumerate()
            .map(|(at, word)| Step::of(word, words.get(at.saturating_add(1)..).unwrap_or(&[])))
            .collect();
        let mut reached = vec![false; words.len()];
        if let Some(first) = reached.first_mut() {
            *first = true;
        }
        for (at, step) in steps.iter().enumerate() {
            if flag_at(&reached, at, false) && *step != Step::EndOfOptions {
                for stride in step.strides() {
                    if let Some(next) = reached.get_mut(at.saturating_add(*stride)) {
                        *next = true;
                    }
                }
            }
        }
        let mut clear = vec![false; words.len()];
        for (at, step) in steps.iter().enumerate().rev() {
            let after = |stride: &usize| flag_at(&clear, at.saturating_add(*stride), true);
            let value = match step {
                Step::EndOfOptions => at.saturating_add(1) >= words.len(),
                Step::Operand => false,
                Step::Flag | Step::Valued | Step::Unlisted => step.strides().iter().any(after),
            };
            if let Some(slot) = clear.get_mut(at) {
                *slot = value;
            }
        }
        let end_of_options = steps
            .iter()
            .enumerate()
            .find(|(at, step)| **step == Step::EndOfOptions && flag_at(&reached, *at, false))
            .map(|(at, _)| at);
        Self {
            words,
            steps,
            reached,
            clear,
            end_of_options,
        }
    }

    /// Whether some reading reads `--remove-source-files` as an option (fail closed).
    fn removes(&self) -> bool {
        self.words.iter().enumerate().any(|(at, word)| {
            flag_at(&self.reached, at, false) && long_name(word.text()) == "--remove-source-files"
        })
    }

    /// Where the operands stand when each unlisted long option is read as a flag.
    fn plain_operands(&self) -> Vec<usize> {
        let mut operands = Vec::new();
        let mut at = 0;
        while let Some(step) = self.steps.get(at) {
            match step {
                Step::EndOfOptions => {
                    operands.extend(at.saturating_add(1)..self.words.len());
                    break;
                }
                Step::Operand => operands.push(at),
                Step::Flag | Step::Valued | Step::Unlisted => {}
            }
            at = at.saturating_add(step.strides().first().copied().unwrap_or(1));
        }
        operands
    }

    /// Whether some reading makes word `at` the last operand: read as an operand with nothing
    /// read as one after it, or the last word after a `--` some reading reaches.
    fn may_be_destination(&self, at: usize) -> bool {
        let operand = flag_at(&self.reached, at, false)
            && self.steps.get(at) == Some(&Step::Operand)
            && flag_at(&self.clear, at.saturating_add(1), true);
        let after_end = at.saturating_add(1) == self.words.len()
            && self.end_of_options.is_some_and(|end| end < at);
        operand || after_end
    }
}

/// `rsync [OPTION]... SRC... DEST`: the destination and joined sources judged as `cp`'s are
/// (a source ending in `/` copies only its contents), and with `--remove-source-files` each
/// source as `mv` judges what it moves away. A long option the table does not list, written
/// without `=`, may be a flag or take the next word, whatever it looks like, so another word may
/// stand where the destination is read (`rsync -a x/ .claude/ --new-option L`, #135; `rsync x/
/// L --new-option --exclude .claude/`, #160). Every word some reading leaves as the last operand
/// is a candidate destination, judged with the operands before it (unlisted options read as
/// flags) as sources, and any candidate's denial stands (fail closed; an option before a source
/// never drops that source).
pub(in crate::hook::guard::denylist::config) fn rsync(args: &[Word], cwd: &Cwd) -> Option<Denial> {
    let readings = Readings::new(args);
    let plain = readings.plain_operands();
    let mode = CopyMode {
        moves: readings.removes(),
        slash: TrailingSlash::Contents,
    };
    (0..args.len())
        .rev()
        .filter(|at| readings.may_be_destination(*at))
        .find_map(|dest| {
            let sources = plain.iter().take_while(|at| **at < dest);
            let copy = CopyArgs {
                operands: sources
                    .chain(std::iter::once(&dest))
                    .filter_map(|at| args.get(*at))
                    .collect(),
                ..CopyArgs::default()
            };
            copy_into("rsync", &copy, mode, cwd)
        })
}
