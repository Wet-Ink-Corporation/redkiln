//! Journals (SPEC JR-01 to JR-14): the store's durable facts as JSON lines, exported so they
//! outlive the clone, and imported idempotently into any other store.
//!
//! [`checkpoint`] writes a directory of journal files (JR-08, JR-09), [`import`] and
//! [`import_file`] read one back (JR-10 to JR-14). The journal refs and the cloud inbox (JR-07)
//! build on these lines in a later step.

mod export;
mod import;
mod line;
mod members;
mod name;
mod tags;

pub use export::{Checkpoint, Exported, JournalError, MAX_JOURNAL_BYTES, checkpoint};
pub use import::{
    ImportCompleted, ImportError, ImportReport, ImportSource, MAX_PROBLEMS, Problem, Recording,
    import, import_file,
};
pub use line::{Line, LineError, MAX_LINE_BYTES, encode_line, lines};
pub use name::{JOURNAL_SUFFIX, JournalName};
