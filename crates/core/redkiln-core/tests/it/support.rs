//! Shared helpers: raw views of a store file that never change it.

use std::path::{Path, PathBuf};

use happenstance_sqlite::rusqlite::Connection;

/// `<path><suffix>`.
pub(crate) fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

/// `user_version` from the header (bytes 60..64), without opening a connection.
pub(crate) fn header_user_version(path: &Path) -> i32 {
    let bytes = std::fs::read(path).expect("read the store");
    i32::from_be_bytes(bytes[60..64].try_into().expect("four bytes"))
}

/// The file change counter (header bytes 24..28), moved by every write transaction.
pub(crate) fn change_counter(path: &Path) -> [u8; 4] {
    std::fs::read(path).expect("read the store")[24..28]
        .try_into()
        .expect("four bytes")
}

/// Whether the file is in WAL (header bytes 18 and 19 both 2).
pub(crate) fn in_wal(path: &Path) -> bool {
    let bytes = std::fs::read(path).expect("read the store");
    bytes[18] == 2 && bytes[19] == 2
}

/// The names in `dir`, sorted, so a test can say nothing was added or removed beside a file.
pub(crate) fn listing(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("read the directory")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// The store's identity as happenstance keeps it, read through a TRUNCATE connection.
pub(crate) fn raw_store_id(path: &Path) -> Vec<u8> {
    let conn = Connection::open(path).expect("open");
    conn.query_row("PRAGMA journal_mode = truncate", [], |r| {
        r.get::<_, String>(0)
    })
    .expect("truncate");
    conn.query_row("SELECT v FROM store_meta WHERE k = 'store_id'", [], |r| {
        r.get(0)
    })
    .expect("store id")
}

/// The store has TRUNCATE's layout: an empty journal, no WAL, no shared memory.
pub(crate) fn assert_truncate_layout(path: &Path) {
    let journal = sibling(path, "-journal");
    assert!(
        journal.exists(),
        "TRUNCATE keeps the journal instead of deleting it"
    );
    assert_eq!(std::fs::metadata(&journal).expect("journal").len(), 0);
    assert!(!sibling(path, "-wal").exists(), "no WAL beside the store");
    assert!(
        !sibling(path, "-shm").exists(),
        "no shared memory beside the store"
    );
}

/// A copy of the named fixture in `dir`.
pub(crate) fn fixture(name: &str, dir: &Path) -> PathBuf {
    let from = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    let to = dir.join(name);
    std::fs::copy(&from, &to).expect("copy the fixture");
    to
}
