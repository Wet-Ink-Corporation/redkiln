//! Writers that unpack a tree into a destination (#117): `rsync` (its own module), whose
//! destination is its last operand as `cp`'s is, `tar` extracting into `-C DIR`, and
//! `unzip -d DIR`. Each destination is judged as `cp`'s is: a protected file, or a config folder
//! the tree may bring its own config into, since the guard cannot tell whether the folder
//! exists. What lands in the working directory itself (`rsync -a x/ .`, `tar -xf x.tar`,
//! `unzip x.zip`) is a limit, as `cp -r x/. .` is: the guard does not read an archive's member
//! names.

mod rsync;

pub(super) use rsync::rsync;

use super::{creates_protected, deny};
use crate::hook::guard::Denial;
use redkiln_guard::engine::Cwd;
use redkiln_guard::engine::wrappers::{Long, is_option, skip_one};
use redkiln_guard::shell::{Shape, Word};

/// Short options of GNU tar and bsdtar that take a value.
const TAR_SHORT_VALUES: [char; 13] = [
    'f', 'C', 'b', 'F', 'g', 'H', 'I', 'K', 'L', 'N', 'T', 'V', 'X',
];

/// Short options of `unzip` that take a value: `-d EXDIR` and `-P PASSWORD`.
const UNZIP_SHORT_VALUES: [char; 2] = ['d', 'P'];

/// A path as a destination is judged: its text and shape.
type Dest<'a> = (&'a str, Shape);

/// What a short-option cluster (`-xzf`, `-C.github`) holds: the flags before its first
/// value-taking option, that option with its value (the rest of the cluster, else the next
/// word), and whether the next word was taken.
#[derive(Debug)]
struct Cluster<'a> {
    flags: &'a str,
    value: Option<(char, Option<Dest<'a>>)>,
    takes_next: bool,
}

impl<'a> Cluster<'a> {
    /// The cluster `word` (its leading `-` included in `skip`), followed by `tail`, whose
    /// options in `values` take a value.
    fn read(word: &'a Word, skip: usize, tail: &'a [Word], values: &[char]) -> Self {
        let text = word.text();
        let found = text
            .char_indices()
            .skip(skip)
            .find(|(_, flag)| values.contains(flag));
        let Some((at, flag)) = found else {
            return Self {
                flags: text,
                value: None,
                takes_next: false,
            };
        };
        let flags = text.get(..at).unwrap_or(text);
        let attached = text
            .get(at.saturating_add(flag.len_utf8())..)
            .filter(|rest| !rest.is_empty());
        let (value, takes_next) = match attached {
            Some(rest) => (Some((rest, word.shape())), false),
            None => (tail.first().map(|next| (next.text(), next.shape())), true),
        };
        Self {
            flags,
            value: Some((flag, value)),
            takes_next,
        }
    }

    /// The value of option `flag`, if this cluster gives it one.
    fn value_of(&self, flag: char) -> Option<Dest<'a>> {
        self.value
            .and_then(|(found, value)| (found == flag).then_some(value))
            .flatten()
    }
}

/// Whether `tar` extracts, and every directory it changes to (`-C DIR`, `--directory DIR`).
#[derive(Debug, Default)]
struct TarArgs<'a> {
    extract: bool,
    dirs: Vec<Dest<'a>>,
}

impl<'a> TarArgs<'a> {
    fn parse(args: &'a [Word]) -> Self {
        let mut tar = Self::default();
        let mut rest = args;
        // The old style: a first word without `-` is a cluster whose value-taking options take
        // the next words in order (`tar xfC x.tar DIR`).
        if let [first, tail @ ..] = args
            && !first.text().starts_with('-')
        {
            rest = tail;
            for flag in first.text().chars() {
                tar.extract |= flag == 'x';
                if TAR_SHORT_VALUES.contains(&flag) {
                    if flag == 'C'
                        && let Some(dir) = rest.first()
                    {
                        tar.dirs.push((dir.text(), dir.shape()));
                    }
                    rest = skip_one(rest);
                }
            }
        }
        while let [first, tail @ ..] = rest {
            rest = tail;
            let text = first.text();
            if text == "--" {
                break;
            }
            if !is_option(first) {
                continue;
            }
            if let Some(long) = Long::parse(text) {
                // `--extract`, `--get` and `--directory` (also its alias `--cd`), abbreviated
                // or not.
                tar.extract |= long.is("--extract") || long.is("--get");
                if long.is("--directory") || long.is("--cd") {
                    if let Some(dir) = long.value() {
                        tar.dirs.push((dir, first.shape()));
                    } else {
                        tar.dirs
                            .extend(tail.first().map(|dir| (dir.text(), dir.shape())));
                        rest = skip_one(rest);
                    }
                }
            } else if !text.starts_with("--") {
                let cluster = Cluster::read(first, 1, tail, &TAR_SHORT_VALUES);
                tar.extract |= cluster.flags.contains('x');
                tar.dirs.extend(cluster.value_of('C'));
                if cluster.takes_next {
                    rest = skip_one(rest);
                }
            }
        }
        tar
    }
}

/// `tar` extracting (`-x`, `--extract`, `--get`) into a directory it changes to: a config
/// folder there may receive whatever config the archive holds.
pub(super) fn tar(program: &str, args: &[Word], cwd: &Cwd) -> Option<Denial> {
    let tar = TarArgs::parse(args);
    if !tar.extract {
        return None;
    }
    tar.dirs.iter().find_map(|(dir, shape)| {
        let label = creates_protected(dir, *shape, cwd)?;
        Some(deny(&format!("{program} extracts into {dir}"), label))
    })
}

/// Every directory `unzip` extracts into (`-d EXDIR`, `-dEXDIR`), wherever the option stands.
fn unzip_dirs(args: &[Word]) -> Vec<Dest<'_>> {
    let mut dirs = Vec::new();
    let mut rest = args;
    while let [first, tail @ ..] = rest {
        rest = tail;
        if !is_option(first) || first.text() == "--" {
            continue;
        }
        let cluster = Cluster::read(first, 1, tail, &UNZIP_SHORT_VALUES);
        dirs.extend(cluster.value_of('d'));
        if cluster.takes_next {
            rest = skip_one(rest);
        }
    }
    dirs
}

/// `unzip -d EXDIR`: a config folder there may receive whatever config the archive holds.
pub(super) fn unzip(args: &[Word], cwd: &Cwd) -> Option<Denial> {
    unzip_dirs(args).iter().find_map(|(dir, shape)| {
        let label = creates_protected(dir, *shape, cwd)?;
        Some(deny(&format!("unzip extracts into {dir}"), label))
    })
}
