//! Knowledge's atom contract (`references/docs/knowledge.md`, The atom model).
//!
//! An atom is one Markdown file holding one fact: typed frontmatter that Knowledge owns, then a
//! body that is the producer's rendering. Producers write atoms through this crate, so every
//! atom carries the same keys in the same order and lives at the path its kind sets; readers
//! decode frontmatter through [`read`], so every reader sees the same fields.
//!
//! The crate does no I/O: it builds strings and reads strings. Writing the file, and checking
//! what is already in `.kb/`, is the producer's.

mod frontmatter;
mod id;
mod kind;
pub mod read;
mod slug;
mod yaml;

pub use frontmatter::{AuthorityTier, Field, Frontmatter, Reversibility, Status, Value};
pub use id::{AtomId, HashLen};
pub use kind::Kind;
pub use slug::slug;
pub use yaml::{yaml_list, yaml_str};
