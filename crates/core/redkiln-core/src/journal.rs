//! Journals (SPEC JR-01 to JR-14): the store's durable facts as JSON lines, exported so they
//! outlive the clone, and imported idempotently into any other store.
//!
//! [`refs`] exports them into journal refs and cloud inbox branches, and relays an inbox into
//! its ref (JR-07, JR-08). [`checkpoint`] writes a directory of journal files instead, and
//! [`import`] and [`import_file`] read one back (JR-10 to JR-14).

mod export;
mod import;
mod line;
mod members;
mod merge;
mod name;
pub mod refs;
mod tags;

pub use export::{Checkpoint, Exported, JournalError, MAX_JOURNAL_BYTES, checkpoint};
pub use import::{
    ImportCompleted, ImportError, ImportReport, ImportSource, MAX_PROBLEMS, Problem, Recording,
    import, import_file,
};
pub use line::{Line, LineError, MAX_LINE_BYTES, encode_line, lines};
pub use name::{JOURNAL_SUFFIX, JournalName, MAX_NAME_BYTES, NameError};
