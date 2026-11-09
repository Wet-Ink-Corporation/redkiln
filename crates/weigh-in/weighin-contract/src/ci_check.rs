//! The JSON `weighin ci-check --json` wrote before #48, schema `weighin.ci-check/1`.
//!
//! assay's Weigh-In gate reads it (redkiln `docs/plan/move-1-weigh-in.md`, Consumers), so it is
//! kept byte for byte: field order is wire order. A reader deserialises [`Report`].
//!
//! `/1` is frozen and readable for Review until it moves; weighin writes [`v2`].

pub mod v2;

use crate::id::ItemId;
use crate::open::Status;
use crate::vocab::{Authority, Door, Kind};
use serde::{Deserialize, Serialize};

/// The schema name and version the report carries.
pub const SCHEMA: &str = "weighin.ci-check/1";

/// The report: what a branch leaves open and decides, and whether its committed journals were
/// only appended to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    /// Always [`SCHEMA`].
    pub schema: String,
    /// The base commit's full sha.
    pub base: String,
    /// The head commit's full sha.
    pub head: String,
    /// Items still open at head, by id.
    pub open_items: Vec<OpenItem>,
    /// The decisions the range adds, by (id, option).
    pub decisions: Vec<Decision>,
    /// The append-only check.
    pub journal: JournalCheck,
}

/// Why [`Report::from_json`] refused a document.
#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    /// The document is not this report's shape.
    #[error("not a {SCHEMA} report")]
    Shape(#[from] serde_json::Error),
    /// The document names another schema, such as a later `/2`.
    #[error("the report is {found}, not {SCHEMA}")]
    Schema {
        /// The schema the document names.
        found: String,
    },
}

impl Report {
    /// Read a report, refusing one whose `schema` is not [`SCHEMA`]. A plain deserialise takes
    /// any `schema` string, so a reader should come through here.
    ///
    /// # Errors
    /// [`ReadError::Shape`] when the JSON is not the report's shape, an unknown key included;
    /// [`ReadError::Schema`] when it names another schema.
    pub fn from_json(json: &str) -> Result<Self, ReadError> {
        let report: Self = serde_json::from_str(json)?;
        if report.schema == SCHEMA {
            Ok(report)
        } else {
            Err(ReadError::Schema {
                found: report.schema,
            })
        }
    }
}

/// An item still open at head.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenItem {
    /// The item.
    pub id: ItemId,
    /// One of the open statuses.
    pub status: Status,
    /// Its kind.
    pub kind: Kind,
    /// The triaged door, or `null` before triage; never the raiser's hint.
    pub door: Option<Door>,
    /// The triaged authority, or `null` before triage; never the raiser's hint.
    pub authority: Option<Authority>,
    /// What it holds up.
    pub blocks: Blocks,
    /// Its deadline: the deferral's when deferred, else the triage's.
    pub resolve_by: Option<String>,
    /// The question as raised.
    pub question: String,
}

/// What an open item holds up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Blocks {
    /// It must be settled before the branch merges.
    Merge,
    /// It must be settled before the next release.
    Release,
    /// It has a date deadline, which holds up neither.
    None,
    /// Nothing recorded says.
    Unknown,
}

/// One decision the branch adds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    /// The item decided.
    pub id: ItemId,
    /// The option, when a source names one (atoms do not, B-28).
    pub option: Option<String>,
    /// The commit that records it: the trailer's, else the atom's, else the journal line's.
    pub commit: Option<String>,
    /// Where it is attested, each once, in [`Source`]'s order.
    pub sources: Vec<Source>,
}

/// Where a decision is attested. The derive order is the output order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// An `item_decided` event added to a committed journal.
    Journal,
    /// A decision atom added under `.kb`.
    Atom,
    /// A `Weigh-In:` trailer in a commit message.
    Trailer,
}

/// The append-only result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JournalCheck {
    /// No violation.
    pub ok: bool,
    /// Each break, by (path, line).
    pub violations: Vec<Violation>,
}

/// One break of the append-only rule.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Violation {
    /// The journal, relative to the worktree root.
    pub path: String,
    /// The 1-based line: the base line for `missing` and `altered`, the head line for
    /// `undecodable`, and none for `removed`.
    pub line: Option<u64>,
    /// What is wrong.
    pub reason: Reason,
}

/// Why a journal breaks the append-only rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Reason {
    /// The file is gone at head.
    Removed,
    /// A complete base line's event is nowhere in the head file.
    Missing,
    /// As `missing`, and the head line at that number holds an event the base file did not.
    Altered,
    /// A complete head line that is not an event.
    Undecodable,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A report with every field set, as weighin 1.2.0 wrote it.
    const FULL: &str = r#"{"schema":"weighin.ci-check/1","base":"b","head":"h","open_items":[{"id":"wi-930e15","status":"deferred","kind":"call","door":"one-way","authority":"authority","blocks":"merge","resolve_by":"before-merge","question":"Q?"},{"id":"wi-930e16","status":"raised","kind":"question","door":null,"authority":null,"blocks":"unknown","resolve_by":null,"question":"R?"}],"decisions":[{"id":"wi-930e17","option":"a","commit":"c","sources":["journal","atom","trailer"]},{"id":"wi-930e18","option":null,"commit":null,"sources":["atom"]}],"journal":{"ok":false,"violations":[{"path":"j.jsonl","line":3,"reason":"altered"},{"path":"k.jsonl","line":null,"reason":"removed"}]}}"#;

    #[test]
    fn a_report_reads_and_writes_back_byte_for_byte() {
        let report: Report = serde_json::from_str(FULL).expect("the 1.x shape reads");
        assert_eq!(serde_json::to_string(&report).expect("writes"), FULL);
    }

    #[test]
    fn a_key_the_schema_does_not_have_is_refused() {
        let extra = FULL.replacen("\"head\":\"h\"", "\"head\":\"h\",\"tail\":1", 1);
        let err = serde_json::from_str::<Report>(&extra).expect_err("an unknown key is refused");
        assert!(err.to_string().starts_with("unknown field `tail`"), "{err}");
    }

    #[test]
    fn the_checked_reader_takes_version_one() {
        let report = Report::from_json(FULL).expect("a /1 report reads");
        assert_eq!(report.schema, SCHEMA);
    }

    #[test]
    fn the_checked_reader_refuses_another_schema() {
        let later = FULL.replacen("weighin.ci-check/1", "weighin.ci-check/2", 1);
        let err = Report::from_json(&later).expect_err("a /2 report is refused");
        assert!(
            matches!(&err, ReadError::Schema { found } if found == "weighin.ci-check/2"),
            "{err:?}"
        );
        assert_eq!(
            err.to_string(),
            "the report is weighin.ci-check/2, not weighin.ci-check/1"
        );
    }

    #[test]
    fn the_checked_reader_refuses_a_bad_shape() {
        let err = Report::from_json("{}").expect_err("an empty object is refused");
        assert!(matches!(err, ReadError::Shape(_)), "{err:?}");
    }
}
