//! The 100-byte SQLite file header, read without SQLite: probing a file through a connection
//! could create a journal or WAL beside it, and a refused file must be left untouched (ST-07).

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use super::OpenError;

/// The length of a SQLite file header.
const HEADER_LEN: usize = 100;

/// The first 16 bytes of every SQLite database file.
const HEADER_MAGIC: &[u8; 16] = b"SQLite format 3\0";

/// What Core reads from a file header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Header {
    /// `PRAGMA user_version`: the big-endian 32-bit integer at offset 60.
    pub(crate) user_version: i64,
    /// The file is in WAL: the read and write format versions (bytes 18 and 19) are both 2.
    pub(crate) wal: bool,
    /// The file change counter (offset 24), which every write transaction moves.
    pub(crate) change_counter: u32,
    /// The schema cookie (offset 40), which every schema change moves: 0 for a database that
    /// has never held a table or index.
    pub(crate) schema_cookie: u32,
}

impl Header {
    /// The header in `bytes`, or `None` when they are not a SQLite header.
    fn parse(bytes: &[u8; HEADER_LEN]) -> Option<Self> {
        if bytes.first_chunk::<16>()? != HEADER_MAGIC {
            return None;
        }
        let formats = bytes.get(18..)?.first_chunk::<2>()?;
        let counter = bytes.get(24..)?.first_chunk::<4>()?;
        let cookie = bytes.get(40..)?.first_chunk::<4>()?;
        let user_version = bytes.get(60..)?.first_chunk::<4>()?;
        Some(Self {
            user_version: i64::from(i32::from_be_bytes(*user_version)),
            wal: formats == &[2, 2],
            change_counter: u32::from_be_bytes(*counter),
            schema_cookie: u32::from_be_bytes(*cookie),
        })
    }
}

/// What is at a path, as far as its first bytes tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Found {
    /// No file.
    Missing,
    /// A file of zero bytes, which SQLite treats as an empty database.
    Empty,
    /// A file that is not a SQLite database: shorter than a header, or without its magic.
    NotSqlite,
    /// A SQLite database and its header.
    Sqlite(Header),
}

/// What is at `path`.
///
/// # Errors
/// [`OpenError::Header`] when the file exists and cannot be read.
pub(crate) fn read(path: &Path) -> Result<Found, OpenError> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(missing) if missing.kind() == io::ErrorKind::NotFound => return Ok(Found::Missing),
        Err(error) => return Err(OpenError::Header(error)),
    };
    if file.metadata().map_err(OpenError::Header)?.len() == 0 {
        return Ok(Found::Empty);
    }
    let mut bytes = [0_u8; HEADER_LEN];
    match file.read_exact(&mut bytes) {
        Ok(()) => Ok(Header::parse(&bytes).map_or(Found::NotSqlite, Found::Sqlite)),
        Err(short) if short.kind() == io::ErrorKind::UnexpectedEof => Ok(Found::NotSqlite),
        Err(error) => Err(OpenError::Header(error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(formats: u8, counter: u32, version: i32) -> [u8; HEADER_LEN] {
        let mut bytes = [0_u8; HEADER_LEN];
        bytes[..16].copy_from_slice(HEADER_MAGIC);
        bytes[18] = formats;
        bytes[19] = formats;
        bytes[24..28].copy_from_slice(&counter.to_be_bytes());
        bytes[40..44].copy_from_slice(&3_u32.to_be_bytes());
        bytes[60..64].copy_from_slice(&version.to_be_bytes());
        bytes
    }

    #[test]
    fn the_stamp_the_journal_format_and_the_counters_are_read() {
        assert_eq!(
            Header::parse(&header(2, 7, 1000)),
            Some(Header {
                user_version: 1000,
                wal: true,
                change_counter: 7,
                schema_cookie: 3,
            })
        );
        assert_eq!(
            Header::parse(&header(1, 0, 1)).map(|h| (h.wal, h.user_version)),
            Some((false, 1))
        );
    }

    #[test]
    fn a_file_without_the_magic_is_not_a_header() {
        let mut bytes = header(1, 0, 1000);
        bytes[0] = b'X';
        assert_eq!(Header::parse(&bytes), None);
    }

    #[test]
    fn missing_empty_short_and_foreign_files_are_told_apart() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(matches!(
            read(&dir.path().join("absent")),
            Ok(Found::Missing)
        ));
        let empty = dir.path().join("empty");
        std::fs::write(&empty, b"").expect("write");
        assert!(matches!(read(&empty), Ok(Found::Empty)));
        let short = dir.path().join("short");
        std::fs::write(&short, b"SQLite format 3\0").expect("write");
        assert!(matches!(read(&short), Ok(Found::NotSqlite)));
        let text = dir.path().join("text");
        std::fs::write(&text, [b'x'; 200]).expect("write");
        assert!(matches!(read(&text), Ok(Found::NotSqlite)));
    }
}
