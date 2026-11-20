//! Bash writes to lint or hook config: redirections, `tee`, `truncate`, `sed -i`, `cp`, `mv`,
//! `ln`, `install`, `rm` and `chmod` aimed at a protected path, and hard links to one (`ln`
//! without `-s`, `link`, `cp -l`), since writing the link writes the file. `rsync`, `tar -x`
//! and `unzip -d` destinations are read in [`unpack`], inline interpreter code in [`inline`]
//! (#117).
//!
//! Paths are matched on their components by the guard kit ([`redkiln_guard::protect`]) against
//! Bench's table ([`PROTECTED`]), so `docs/hooks.md` and `myhooks/x` are not `hooks/`. A
//! component with an unquoted glob matches when it could match a protected name
//! (`.cl*/settings.json`). `..` is resolved lexically; a relative path that climbs above where
//! it starts may land under any parent, so `../settings.json` counts as `.claude/settings.json`.
//! A relative path after a `cd` is also read from the directory the `cd` named, and from
//! anywhere after a `cd` the guard cannot follow.

mod inline;
mod unpack;

use crate::hook::guard::{Denial, Rule};
use redkiln_guard::engine::Cwd;
use redkiln_guard::engine::wrappers::{Long, is_option, skip_one};
use redkiln_guard::protect::{Protected, ProtectedPaths, last_component};
use redkiln_guard::shell::{Shape, Word};

/// The protected paths.
const PROTECTED: [Protected; 14] = [
    Protected::exact("hooks/", None, "hooks"),
    Protected::affix(
        ".claude/settings*.json",
        Some(".claude"),
        ("settings", ".json"),
        &["settings.json", "settings.local.json"],
    ),
    Protected::affix(
        ".cargo/config*",
        Some(".cargo"),
        ("config", ""),
        &["config", "config.toml"],
    ),
    Protected::exact(".github/workflows/", Some(".github"), "workflows"),
    Protected::affix(
        ".eslintrc*",
        None,
        (".eslintrc", ""),
        &[
            ".eslintrc",
            ".eslintrc.json",
            ".eslintrc.js",
            ".eslintrc.cjs",
            ".eslintrc.yml",
        ],
    ),
    Protected::file("clippy.toml"),
    Protected::file(".clippy.toml"),
    Protected::file("rustfmt.toml"),
    Protected::file(".rustfmt.toml"),
    Protected::file("deny.toml"),
    Protected::file(".pre-commit-config.yaml"),
    Protected::file("lefthook.yml"),
    // The gates' own settings: the orchestrate leads the brief hook trusts, the merge gate's
    // review switch and sensitive globs, `check_fast`. Bench reads `.redkiln/bench/` and, as a
    // 0.x fallback, anneal's `.anneal/` (`repo_file`); both are protected.
    Protected::exact(
        ".redkiln/bench/config.json",
        Some(".redkiln/bench"),
        "config.json",
    ),
    Protected::exact(".anneal/config.json", Some(".anneal"), "config.json"),
];

/// Directories that hold protected config: removing, moving or re-permissioning one changes it.
/// `.redkiln/bench` is matched on its last two components.
const CONTAINERS: [(&str, &str); 6] = [
    (".redkiln/bench", ".redkiln/bench/"),
    (".redkiln", ".redkiln/"),
    (".anneal", ".anneal/"),
    (".claude", ".claude/"),
    (".github", ".github/"),
    (".cargo", ".cargo/"),
];

/// How a denial names a directory above the start, which may hold any of [`CONTAINERS`].
const ABOVE: &str = "a directory above the working directory, which may hold .claude/";

/// Bench's protected config, as the kit matches it.
const CONFIG: ProtectedPaths<'static> = ProtectedPaths::new(&PROTECTED, &CONTAINERS, ABOVE);

/// The protected path writing `path` (seen from `cwd`) changes, if any.
fn protected(path: &str, shape: Shape, cwd: &Cwd) -> Option<&'static str> {
    CONFIG.protected(path, shape, cwd)
}

/// The protected path removing (or re-permissioning) `path` changes, if any: a protected path,
/// a directory holding one, or a directory above the start.
fn contains_protected(path: &str, shape: Shape, cwd: &Cwd) -> Option<&'static str> {
    CONFIG.contains_protected(path, shape, cwd)
}

/// The protected path a copy, move or link creating `path` may write, if any: a protected file,
/// or a directory holding one, which the new entry brings its own config into when the
/// directory does not exist yet. A directory above the start is not one: `..` cannot be
/// created, and a copy into it lands at a joined path, which is checked on its own.
fn creates_protected(path: &str, shape: Shape, cwd: &Cwd) -> Option<&'static str> {
    contains_protected(path, shape, cwd).filter(|label| *label != ABOVE)
}

fn deny(action: &str, label: &str) -> Denial {
    Denial::new(
        Rule::ConfigWrite,
        format!(
            "{action}: {label} is lint or hook config, which a Bash command may not change; \
             change it in an edit a human reviews"
        ),
    )
}

/// A redirection writing `target`, from `cwd`.
pub(super) fn redirect(target: &Word, cwd: &Cwd) -> Option<Denial> {
    let label = protected(target.text(), target.shape(), cwd)?;
    Some(deny(
        &format!("a redirection writes {}", target.text()),
        label,
    ))
}

/// Checks `<program> <args>`, run from `cwd`, for writes to protected config.
pub(super) fn check(program: &str, args: &[Word], cwd: &Cwd) -> Option<Denial> {
    match program {
        "tee" | "truncate" => args.iter().find_map(|arg| {
            let label = protected(arg.text(), arg.shape(), cwd)?;
            Some(deny(&format!("{program} writes {}", arg.text()), label))
        }),
        "rm" | "unlink" | "rmdir" | "shred" | "chmod" | "chown" | "chgrp" => {
            args.iter().find_map(|arg| {
                let label = contains_protected(arg.text(), arg.shape(), cwd)?;
                Some(deny(&format!("{program} changes {}", arg.text()), label))
            })
        }
        "sed" => sed(args, cwd),
        "cp" | "install" | "ln" | "link" => copy(program, args, false, cwd),
        "mv" => copy(program, args, true, cwd),
        "rsync" => unpack::rsync(args, cwd),
        "tar" | "gtar" | "bsdtar" => unpack::tar(program, args, cwd),
        "unzip" => unpack::unzip(args, cwd),
        _ => inline::check(program, args, cwd),
    }
}

/// The parts of a `sed` command line that matter here.
#[derive(Debug, Default)]
struct SedArgs<'a> {
    in_place: bool,
    /// The script came from `-e` or `-f`, so every operand is a file.
    script_given: bool,
    operands: Vec<&'a Word>,
}

impl<'a> SedArgs<'a> {
    fn parse(args: &'a [Word]) -> Self {
        let mut sed = Self::default();
        let mut rest = args;
        while let [first, tail @ ..] = rest {
            rest = tail;
            let text = first.text();
            if text == "--" {
                sed.operands.extend(tail);
                break;
            }
            if !is_option(first) {
                sed.operands.push(first);
            } else if let Some(long) = Long::parse(text) {
                // `--in-place[=SUFFIX]`, `--expression SCRIPT`, `--file FILE`, abbreviated or not.
                sed.in_place |= long.is("--in-place");
                if long.is("--expression") || long.is("--file") {
                    sed.script_given = true;
                    if long.value().is_none() {
                        rest = skip_one(rest);
                    }
                }
            } else if !text.starts_with("--") && sed.cluster(text) {
                rest = skip_one(rest);
            }
        }
        sed
    }

    /// Reads a short-option cluster; true when its last option takes the next word.
    fn cluster(&mut self, cluster: &str) -> bool {
        for (at, flag) in cluster.char_indices().skip(1) {
            let last = at.saturating_add(1) == cluster.len();
            match flag {
                // `-i[SUFFIX]`: the rest of the cluster is the suffix.
                'i' => {
                    self.in_place = true;
                    return false;
                }
                'e' | 'f' => {
                    self.script_given = true;
                    return last;
                }
                'l' => return last,
                _ => {}
            }
        }
        false
    }
}

fn sed(args: &[Word], cwd: &Cwd) -> Option<Denial> {
    let sed = SedArgs::parse(args);
    if !sed.in_place {
        return None;
    }
    let files = if sed.script_given {
        &sed.operands[..]
    } else {
        sed.operands
            .split_first()
            .map_or(&[][..], |(_, files)| files)
    };
    files.iter().find_map(|file| {
        let label = protected(file.text(), file.shape(), cwd)?;
        Some(deny(&format!("sed -i edits {}", file.text()), label))
    })
}

/// Short options of `cp`, `mv`, `ln` and `install` that take the next word as a value.
const COPY_VALUES: [char; 4] = ['S', 'm', 'o', 'g'];

/// Every short option of `cp`, `mv`, `ln` and `install` that takes a value: in a cluster, the
/// rest of the word after one is its value, not more options.
const COPY_SHORT_VALUES: [char; 5] = ['t', 'S', 'm', 'o', 'g'];

/// How a denial names a hard-linked source that is the working directory or the root, either of
/// which may hold any of [`CONTAINERS`].
const WHOLE_TREE: &str = "a directory that may hold .claude/ or .anneal/";

/// The destination and sources of `cp`, `mv`, `ln` or `install`.
#[derive(Debug, Default)]
struct CopyArgs<'a> {
    /// `-t DIR` or `--target-directory=DIR`.
    target_dir: Option<(&'a str, Shape)>,
    operands: Vec<&'a Word>,
    /// `-s` or `--symbolic` (`ln`): a symbolic link rather than a hard one. An abbreviation
    /// `--suffix` shares (`--s`) is not one, so it reads as the hard link the guard denies.
    symbolic: bool,
    /// `-l` or `--link` (`cp`): hard links instead of copies.
    link: bool,
}

/// What a source ending in `/` copies into a directory destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrailingSlash {
    /// The source itself, at `<dest>/<its name>` (`cp`, `mv`, `ln`, `install`).
    Itself,
    /// Only its contents, so it lands at no joined path (`rsync`).
    Contents,
}

impl<'a> CopyArgs<'a> {
    fn parse(args: &'a [Word]) -> Self {
        let mut copy = Self::default();
        let mut rest = args;
        while let [first, tail @ ..] = rest {
            rest = tail;
            let text = first.text();
            if text == "--" {
                copy.operands.extend(tail);
                break;
            }
            if !is_option(first) {
                copy.operands.push(first);
            } else if let Some(long) = Long::parse(text) {
                // `--target-directory DIR` and `--suffix SUFFIX`, abbreviated or not.
                if long.is("--target-directory") {
                    copy.target_dir = match long.value() {
                        Some(dir) => Some((dir, first.shape())),
                        None => tail.first().map(|dir| (dir.text(), dir.shape())),
                    };
                }
                let valued = long.is("--target-directory") || long.is("--suffix");
                if valued && long.value().is_none() {
                    rest = skip_one(rest);
                }
                copy.symbolic |= long.is("--symbolic") && !long.is("--suffix");
                copy.link |= long.is("--link");
            } else if text.starts_with("--") {
            } else if let Some((_, dir)) = copy.cluster(text) {
                if dir.is_empty() {
                    copy.target_dir = tail.first().map(|dir| (dir.text(), dir.shape()));
                    rest = skip_one(rest);
                } else {
                    copy.target_dir = Some((dir, first.shape()));
                }
            } else if text.ends_with(COPY_VALUES) {
                rest = skip_one(rest);
            }
        }
        copy
    }

    /// Reads the flags of the short-option cluster `cluster` before its first value-taking
    /// option; returns the cluster split at its `t` (the target directory follows it).
    fn cluster(&mut self, cluster: &'a str) -> Option<(&'a str, &'a str)> {
        let flags = cluster.strip_prefix('-').unwrap_or(cluster);
        let flags = flags.split(COPY_SHORT_VALUES).next().unwrap_or(flags);
        self.symbolic |= flags.contains('s');
        self.link |= flags.contains('l');
        cluster.split_once('t')
    }

    /// Whether `program` run with these options makes hard links to its sources: `ln`
    /// without `-s`, `link`, and `cp -l`.
    fn hard_links(&self, program: &str) -> bool {
        match program {
            "ln" => !self.symbolic,
            "link" => true,
            "cp" => self.link,
            _ => false,
        }
    }

    /// The operands linked or copied: every one with `-t`, the only one (`ln FILE` links it
    /// into `.`), else all but the last.
    fn sources(&self) -> &[&'a Word] {
        match (self.target_dir, self.operands.as_slice()) {
            (Some(_), all) | (None, all @ [_]) => all,
            (None, all) => all.split_last().map_or(all, |(_, sources)| sources),
        }
    }
}

/// The denial for `program` hard-linking a protected source: writing the link writes the file
/// (a source that is the working directory or the root may hold one, for `cp -al`).
fn hard_link(program: &str, copy: &CopyArgs<'_>, cwd: &Cwd) -> Option<Denial> {
    copy.sources().iter().find_map(|source| {
        let label = contains_protected(source.text(), source.shape(), cwd).or_else(|| {
            last_component(source.text())
                .is_none()
                .then_some(WHOLE_TREE)
        })?;
        Some(deny(
            &format!("{program} hard-links {}", source.text()),
            label,
        ))
    })
}

/// The last component of `path`.
fn basename(path: &str) -> &str {
    path.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(path)
}

fn copy(program: &str, args: &[Word], moves: bool, cwd: &Cwd) -> Option<Denial> {
    let copy = CopyArgs::parse(args);
    if copy.hard_links(program)
        && let Some(denial) = hard_link(program, &copy, cwd)
    {
        return Some(denial);
    }
    let mode = CopyMode {
        moves,
        slash: TrailingSlash::Itself,
    };
    copy_into(program, &copy, mode, cwd)
}

/// How a copying program treats its sources.
#[derive(Debug, Clone, Copy)]
struct CopyMode {
    /// It moves them away (`mv`, `rsync --remove-source-files`).
    moves: bool,
    /// What a source ending in `/` copies.
    slash: TrailingSlash,
}

/// The denial for `program` writing its destination, or a source's joined path in it, when
/// either is protected config; when it moves its sources, also for moving one that holds
/// config away.
fn copy_into(program: &str, copy: &CopyArgs<'_>, mode: CopyMode, cwd: &Cwd) -> Option<Denial> {
    // A source lands at `<dest>/<its name>` when the destination is a directory: always for
    // `-t`, and, since the guard cannot look, when the destination ends in `/` or names a
    // directory that holds config.
    let ((dest, dest_shape), sources, into_dir) = if let Some(dir) = copy.target_dir {
        (dir, &copy.operands[..], true)
    } else {
        let (last, sources) = copy.operands.split_last()?;
        // A directory that holds config, matched as removing it would be (`.redkiln/bench` on
        // its last two components); a protected file or a directory above the start there
        // only widens which joined paths are checked.
        // `.` and `..`, with nothing left once resolved, are directories too.
        let named_dir = last.text().ends_with(['/', '\\'])
            || last_component(last.text()).is_none()
            || contains_protected(last.text(), last.shape(), cwd).is_some();
        ((last.text(), last.shape()), sources, named_dir)
    };
    if sources.is_empty() {
        return None;
    }
    // The destination itself: a protected file, or a config folder the copy may create.
    if let Some(label) = creates_protected(dest, dest_shape, cwd) {
        return Some(deny(&format!("{program} writes {dest}"), label));
    }
    sources.iter().find_map(|source| {
        if mode.moves
            && let Some(label) = contains_protected(source.text(), source.shape(), cwd)
        {
            return Some(deny(&format!("{program} moves {}", source.text()), label));
        }
        if mode.slash == TrailingSlash::Contents && source.text().ends_with(['/', '\\']) {
            return None;
        }
        let joined = format!("{dest}/{}", basename(source.text()));
        let label =
            creates_protected(&joined, dest_shape.max(source.shape()), cwd).filter(|_| into_dir)?;
        Some(deny(&format!("{program} writes {joined}"), label))
    })
}
