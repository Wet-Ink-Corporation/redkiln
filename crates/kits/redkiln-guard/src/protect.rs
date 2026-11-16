//! Protected paths, matched on their components.
//!
//! Whether a path a command writes or removes is one of a pack's protected paths
//! ([`ProtectedPaths`], a table of [`Protected`] entries), matched on its components
//! after `..` is resolved lexically, read from where the `cd` commands seen so far say
//! relative paths start ([`Cwd`]). The table is the pack's; this module only matches.

use std::borrow::Cow;
use std::iter;

use crate::engine::{Cwd, is_absolute};
use crate::shell::{EXTGLOB_OPENERS, Shape};

/// The most components an extended glob span is read as beyond one (see `without_extglobs`).
const MAX_SPANNED: usize = 32;

/// A path component's name.
#[derive(Debug, Clone, Copy)]
enum Name {
    Exact(&'static str),
    /// Starts with `prefix` and ends with `suffix`; a glob is tried against `samples`.
    Affix {
        prefix: &'static str,
        suffix: &'static str,
        samples: &'static [&'static str],
    },
}

impl Name {
    /// Whether `component` is this name; when `glob` and the component holds a wildcard,
    /// whether it could match it. Letter case is ignored: on case-insensitive file systems
    /// (APFS, NTFS by default) `.CLAUDE/settings.json` is `.claude/settings.json`.
    fn matches(self, component: &str, glob: bool) -> bool {
        let wildcard = glob && component.contains(['*', '?', '[']);
        // The names are lower case; the component is lowered to compare.
        let component = component.to_ascii_lowercase();
        match self {
            Self::Exact(name) if wildcard => glob_match(&component, name),
            Self::Exact(name) => component == name,
            Self::Affix { samples, .. } if wildcard => {
                samples.iter().any(|sample| glob_match(&component, sample))
            }
            Self::Affix { prefix, suffix, .. } => component
                .strip_prefix(prefix)
                .is_some_and(|rest| rest.ends_with(suffix)),
        }
    }
}

/// A protected path: a component, directly under `parent` when that is set.
///
/// A `parent` may be several `/`-separated components (`.redkiln/bench`), all of which must
/// lead to the component; above the start, any of its leading components may be unknown.
/// Names are written in lower case and matched ignoring ASCII case.
#[derive(Debug)]
pub struct Protected {
    /// How a denial names it.
    label: &'static str,
    parent: Option<&'static str>,
    name: Name,
}

impl Protected {
    /// A file protected wherever it is, named by its name.
    #[must_use]
    pub const fn file(name: &'static str) -> Self {
        Self {
            label: name,
            parent: None,
            name: Name::Exact(name),
        }
    }

    /// The component `name`, directly under `parent` when that is set, named `label`.
    #[must_use]
    pub const fn exact(
        label: &'static str,
        parent: Option<&'static str>,
        name: &'static str,
    ) -> Self {
        Self {
            label,
            parent,
            name: Name::Exact(name),
        }
    }

    /// Any component that starts with `prefix` and ends with `suffix`, directly under `parent`
    /// when that is set, named `label`; a component holding a glob is tried against `samples`.
    #[must_use]
    pub const fn affix(
        label: &'static str,
        parent: Option<&'static str>,
        (prefix, suffix): (&'static str, &'static str),
        samples: &'static [&'static str],
    ) -> Self {
        Self {
            label,
            parent,
            name: Name::Affix {
                prefix,
                suffix,
                samples,
            },
        }
    }

    fn matches(&self, path: &Resolved<'_>, glob: bool) -> bool {
        let components = path.components.as_slice();
        match self.parent {
            None => components.iter().any(|c| self.name.matches(c, glob)),
            Some(parent) => {
                let parents = parent.split('/').count();
                let named = |window: &[&str]| {
                    window.split_last().is_some_and(|(last, above)| {
                        self.name.matches(last, glob) && ends_with(above, parent, glob)
                    })
                };
                let under_parent = components.windows(parents.saturating_add(1)).any(named);
                // Above the start, the parent (or the part of it above the first component)
                // is unknown and may be the protected one: the path may start inside it.
                let under_unknown = path.escaped
                    && (0..=parents).any(|known| {
                        components.get(..=known).is_some_and(|start| {
                            start.split_last().is_some_and(|(last, above)| {
                                self.name.matches(last, glob) && is_tail_of(above, parent, glob)
                            })
                        })
                    });
                under_parent || under_unknown
            }
        }
    }
}

/// Whether `components` end with every component of the `/`-separated `dirs`.
fn ends_with(components: &[&str], dirs: &'static str, glob: bool) -> bool {
    let wanted: Vec<&str> = dirs.split('/').collect();
    components
        .len()
        .checked_sub(wanted.len())
        .and_then(|from| components.get(from..))
        .is_some_and(|tail| {
            tail.iter()
                .zip(&wanted)
                .all(|(component, dir)| Name::Exact(dir).matches(component, glob))
        })
}

/// Whether `components` are the last of the `/`-separated `dirs` (none counts): what a path
/// that starts inside `dirs` holds of them.
fn is_tail_of(components: &[&str], dirs: &'static str, glob: bool) -> bool {
    let wanted: Vec<&str> = dirs.split('/').collect();
    wanted
        .len()
        .checked_sub(components.len())
        .and_then(|from| wanted.get(from..))
        .is_some_and(|tail| {
            tail.iter()
                .zip(components)
                .all(|(dir, component)| Name::Exact(dir).matches(component, glob))
        })
}

/// A pack's protected paths: the entries, the directories that hold them (each a name and how
/// a denial names it), and how a denial names a directory above the start, which may hold any
/// of them.
#[derive(Debug, Clone, Copy)]
pub struct ProtectedPaths<'t> {
    paths: &'t [Protected],
    containers: &'t [(&'static str, &'static str)],
    above: &'static str,
}

impl<'t> ProtectedPaths<'t> {
    /// The table of `paths`, held in `containers` (each a directory, one or more `/`-separated
    /// components matched as a path's last ones, and its label), with `above` naming a
    /// directory above the start.
    #[must_use]
    pub const fn new(
        paths: &'t [Protected],
        containers: &'t [(&'static str, &'static str)],
        above: &'static str,
    ) -> Self {
        Self {
            paths,
            containers,
            above,
        }
    }

    /// The label of the protected path writing `path` (seen from `cwd`) changes, if any.
    #[must_use]
    pub fn protected(&self, path: &str, shape: Shape, cwd: &Cwd) -> Option<&'static str> {
        readings(path, shape, cwd).find_map(|reading| {
            let resolved = resolve(&reading.path, reading.anywhere);
            self.paths
                .iter()
                .find(|protected| protected.matches(&resolved, reading.glob))
                .map(|protected| protected.label)
        })
    }

    /// The label of the protected path removing (or re-permissioning) `path` changes, if any:
    /// a protected path, a directory holding one, or a directory above the start.
    #[must_use]
    pub fn contains_protected(&self, path: &str, shape: Shape, cwd: &Cwd) -> Option<&'static str> {
        self.protected(path, shape, cwd).or_else(|| {
            readings(path, shape, cwd).find_map(|reading| {
                let resolved = resolve(&reading.path, reading.anywhere);
                let components = resolved.components.as_slice();
                if components.is_empty() {
                    return resolved.escaped.then_some(self.above);
                }
                // A container of several components may start above the start, as a protected
                // path's parent may.
                self.containers
                    .iter()
                    .find(|(dirs, _)| {
                        ends_with(components, dirs, reading.glob)
                            || (resolved.escaped && is_tail_of(components, dirs, reading.glob))
                    })
                    .map(|(_, label)| *label)
            })
        })
    }
}

/// A path's components after `.` and `..` are resolved, and whether `..` climbed above where
/// the path starts.
#[derive(Debug)]
struct Resolved<'a> {
    components: Vec<&'a str>,
    escaped: bool,
}

/// `path` resolved lexically; `escaped` from the start when relative paths may be anywhere.
fn resolve(path: &str, escaped: bool) -> Resolved<'_> {
    let absolute = is_absolute(path);
    let mut resolved = Resolved {
        components: Vec::new(),
        escaped: escaped && !absolute,
    };
    for component in path.split(['/', '\\']) {
        match component {
            "" | "." => {}
            ".." => {
                if resolved.components.pop().is_none() && !absolute {
                    resolved.escaped = true;
                }
            }
            name => resolved.components.push(name),
        }
    }
    resolved
}

/// The last component of `path` once resolved (`.` dropped, `..` applied lexically).
#[must_use]
pub fn last_component(path: &str) -> Option<&str> {
    resolve(path, false).components.last().copied()
}

/// One way to read a path: its text, whether it is a glob, and whether a relative path may
/// start anywhere.
#[derive(Debug)]
struct Reading<'a> {
    path: Cow<'a, str>,
    glob: bool,
    anywhere: bool,
}

/// The ways `path` may be read from `cwd`: as written (from anywhere after a `cd` the guard
/// cannot follow), and joined to the directory a `cd` named.
fn readings<'a>(path: &'a str, shape: Shape, cwd: &Cwd) -> impl Iterator<Item = Reading<'a>> {
    let relative = !is_absolute(path);
    let glob = shape != Shape::Literal;
    let anywhere = relative && *cwd == Cwd::Unknown;
    let as_written = Reading {
        path: Cow::Borrowed(path),
        glob,
        anywhere,
    };
    let joined = match cwd {
        Cwd::Dir(dir, dir_shape) if relative => Some(Reading {
            path: Cow::Owned(format!("{dir}/{path}")),
            glob: glob || *dir_shape != Shape::Literal,
            anywhere: false,
        }),
        Cwd::Start | Cwd::Dir(..) | Cwd::Unknown => None,
    };
    iter::once(as_written)
        .chain(joined)
        .flat_map(Reading::without_extglobs)
}

impl Reading<'_> {
    /// This reading, or for a glob each reading with its extended glob groups cut out before
    /// the path is split: a `/` inside a group is no separator (`.clau@(de|x/y)` is `.claude`).
    fn without_extglobs(self) -> impl Iterator<Item = Self> {
        let cut = if self.glob {
            without_extglobs(&self.path)
        } else {
            None
        };
        let (glob, anywhere) = (self.glob, self.anywhere);
        let whole = cut.is_none().then_some(self);
        whole
            .into_iter()
            .chain(cut.into_iter().flatten().map(move |path| Reading {
                path: Cow::Owned(path),
                glob,
                anywhere,
            }))
    }
}

/// The ways `path` may be read with its extended glob groups (`@(de)`, `!(x)`, `?(a|b)`)
/// cut out: the span from its first opener to its last `)` (or to its end when no `)`
/// follows) becomes one, two or more `*` components, up to one more than the `/`s it holds.
///
/// Quoting is not known here, so a quoted `)` could seem to close a group early
/// (`.c@(laude|l")")`), and a `/` inside a group (`.clau@(de|x/y)`) is no separator, as bash
/// matches a group within one component; the span's real `)` is never after its last one,
/// and it holds at most that many components, each of which a `*` matches, so the readings
/// only widen a match. The count is capped at [`MAX_SPANNED`]: past a few, more `*`s change a
/// match only through as many `..` after the span.
fn without_extglobs(path: &str) -> Option<Vec<String>> {
    let start = path
        .char_indices()
        .zip(path.chars().skip(1))
        .find(|((_, c), next)| EXTGLOB_OPENERS.contains(c) && *next == '(')
        .map(|((at, _), _)| at)?;
    let close = path.rfind(')').filter(|close| *close > start);
    let after = close
        .and_then(|close| path.get(close.saturating_add(1)..))
        .unwrap_or("");
    let span = close
        .and_then(|close| path.get(start..close))
        .or_else(|| path.get(start..))
        .unwrap_or("");
    let before = path.get(..start).unwrap_or("");
    let slashes = span.matches(['/', '\\']).count().min(MAX_SPANNED);
    Some(
        (0..=slashes)
            .map(|extra| format!("{before}*{}{after}", "/*".repeat(extra)))
            .collect(),
    )
}

/// Whether `text` matches the shell pattern `pattern` (`*`, `?`, and `[...]` as any one
/// character, which can only widen a match).
fn glob_match(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();
    let (mut p, mut t) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while t < text.len() {
        match pattern.get(p) {
            Some('*') => {
                star = Some((p, t));
                p += 1;
            }
            Some('[') => {
                let close = pattern.iter().skip(p).position(|c| *c == ']');
                p = close.map_or(p + 1, |close| p + close + 1);
                t += 1;
            }
            Some(c) if *c == '?' || Some(c) == text.get(t) => {
                p += 1;
                t += 1;
            }
            _ => {
                let Some((star_p, star_t)) = star else {
                    return false;
                };
                p = star_p + 1;
                t = star_t + 1;
                star = Some((star_p, t));
            }
        }
    }
    pattern.iter().skip(p).all(|c| *c == '*')
}

#[cfg(test)]
mod tests;
