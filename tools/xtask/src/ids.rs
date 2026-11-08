//! The knowledge-base id rules (Knowledge call C; Ryan's call 7 of Move 1 step 4) and a check of
//! `.kb/` against them.
//!
//! Every atom is a Markdown file at `.kb/<kind>/` whose frontmatter `id` is `kb-<kind>-<key>`
//! for one of the eight kinds, and no two files share an id. Content-hash keys can collide, and
//! so can the shorter `wi-` ids of Weigh-In 1.x, so the check also fails when one `wi-` id stands
//! for two questions: a decision names its item in `weighin_item` and its question in
//! `question`; an open question names its item in `tracks` and its question in `title`.
//!
//! The ledger folder `.kb/_ledger/` holds no atoms and is not read. A Markdown file outside a
//! kind's folder without an `id` is not an atom (a README, say) and is passed over.

mod read;

use std::collections::BTreeMap;
use std::fmt;

use knowledge_contract::Kind;
use knowledge_contract::read::frontmatter;

pub use read::{MAX_DEPTH, MAX_FILES, MAX_TEXT, ReadError, read_kb};

/// One Markdown file under `.kb/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtomFile {
    /// The path relative to `.kb/`, with `/` separators.
    pub path: String,
    /// The file's contents.
    pub text: String,
}

/// A way `.kb/` breaks the id rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Violation {
    /// Two or more files carry one id.
    DuplicateId {
        /// The id.
        id: String,
        /// Every file that carries it, in path order.
        paths: Vec<String>,
    },
    /// One `wi-` id stands for two different questions.
    ItemCollision {
        /// The `wi-` id.
        item: String,
        /// The first file, in path order, that names the item.
        first: String,
        /// A later file that names it with another question.
        second: String,
    },
    /// An atom outside its kind's folder.
    Misplaced {
        /// The file.
        path: String,
        /// Its id.
        id: String,
        /// The folder its kind's atoms live in.
        folder: String,
    },
    /// An id that is not `kb-<kind>-<key>` for one of the eight kinds.
    UnknownId {
        /// The file.
        path: String,
        /// Its id.
        id: String,
    },
    /// A file in a kind's folder with no frontmatter `id`.
    NoId {
        /// The file.
        path: String,
    },
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateId { id, paths } => {
                write!(f, "{id} is the id of {} files:", paths.len())?;
                for path in paths {
                    write!(f, " .kb/{path}")?;
                }
                Ok(())
            }
            Self::ItemCollision {
                item,
                first,
                second,
            } => write!(
                f,
                "{item} stands for two questions, in .kb/{first} and .kb/{second}"
            ),
            Self::Misplaced { path, id, folder } => {
                write!(f, ".kb/{path}: {id} belongs in {folder}/")
            }
            Self::UnknownId { path, id } => {
                write!(
                    f,
                    ".kb/{path}: {id} is not kb-<kind>-<key> for a known kind"
                )
            }
            Self::NoId { path } => write!(f, ".kb/{path}: no frontmatter id"),
        }
    }
}

/// Checks `atoms` against the id rules, returning every violation: those of single files in path
/// order, then duplicate ids in id order, then colliding items in the order they are met.
#[must_use]
pub fn check(atoms: &[AtomFile]) -> Vec<Violation> {
    let mut sorted: Vec<&AtomFile> = atoms.iter().collect();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));

    let mut violations = Vec::new();
    // Each id's files, in path order.
    let mut ids: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    // Each `wi-` id's question and the first file that names it.
    let mut items: BTreeMap<String, (String, &str)> = BTreeMap::new();
    let mut collisions = Vec::new();

    for file in sorted {
        let path = file.path.as_str();
        let fm = frontmatter(&file.text);
        let in_folder = folder_kind(path);
        let Some(id) = fm.as_ref().and_then(|fm| fm.get("id")) else {
            if in_folder.is_some() {
                violations.push(Violation::NoId {
                    path: path.to_owned(),
                });
            }
            continue;
        };
        violations.extend(placement(path, &id, in_folder));
        ids.entry(id.into_owned()).or_default().push(path);

        let named = fm.as_ref().and_then(|fm| {
            fm.get("weighin_item")
                .zip(fm.get("question"))
                .or_else(|| fm.get("tracks").zip(fm.get("title")))
        });
        if let Some((item, question)) = named {
            match items.get(item.as_ref()) {
                None => {
                    items.insert(item.into_owned(), (question.into_owned(), path));
                }
                Some((known, first)) if *known != question => {
                    collisions.push(Violation::ItemCollision {
                        item: item.into_owned(),
                        first: (*first).to_owned(),
                        second: path.to_owned(),
                    });
                }
                Some(_) => {}
            }
        }
    }

    for (id, paths) in ids {
        if paths.len() > 1 {
            violations.push(Violation::DuplicateId {
                id,
                paths: paths.into_iter().map(str::to_owned).collect(),
            });
        }
    }
    violations.extend(collisions);
    violations
}

/// The violation of the file at `path`, directly in the folder of `in_folder` if any, whose id is
/// `id`: an id of no known kind, or an atom outside its kind's folder.
fn placement(path: &str, id: &str, in_folder: Option<Kind>) -> Option<Violation> {
    match id_kind(id) {
        None => Some(Violation::UnknownId {
            path: path.to_owned(),
            id: id.to_owned(),
        }),
        Some(kind) if in_folder != Some(kind) => Some(Violation::Misplaced {
            path: path.to_owned(),
            id: id.to_owned(),
            folder: kind.dir(),
        }),
        Some(_) => None,
    }
}

/// The kind whose folder holds `path` directly (`<kind>/<file>`), if any.
fn folder_kind(path: &str) -> Option<Kind> {
    let (folder, file) = path.split_once('/')?;
    if file.contains('/') {
        return None;
    }
    Kind::parse(folder)
}

/// The kind an id `kb-<kind>-<key>` names, when `<key>` is not empty.
fn id_kind(id: &str) -> Option<Kind> {
    let rest = id.strip_prefix("kb-")?;
    Kind::ALL.into_iter().find(|kind| {
        rest.strip_prefix(kind.as_str())
            .and_then(|tail| tail.strip_prefix('-'))
            .is_some_and(|key| !key.is_empty())
    })
}
