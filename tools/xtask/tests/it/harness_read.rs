//! The source reader of check (b): what it reads, its limits (H-13), each pinned at its boundary,
//! and its symlink handling.

use std::fs;
use std::io;
use std::num::TryFromIntError;
use std::path::Path;

use xtask::harness::{MAX_DEPTH, MAX_FILES, MAX_TEXT, ReadError, SourceFile, read_sources};

use crate::support::{Scratch, many, scratch, tree};

fn paths(files: &[SourceFile]) -> Vec<&str> {
    files.iter().map(|f| f.path.as_str()).collect()
}

#[test]
fn the_reader_loads_rs_files_under_crates_in_path_order() {
    let crates = scratch("harness-read").expect("a scratch folder");
    tree(
        &crates,
        &[
            ("weigh-in/weighin/src/main.rs", "fn main() {}\n"),
            ("weigh-in/weighin/src/hooks.rs", "fn run() {}\n"),
            ("weigh-in/weighin/Cargo.toml", "[package]\n"),
            ("core/redkiln-core/tests/it/main.rs", "mod x;\n"),
        ],
    )
    .expect("write the tree");

    let files = read_sources(&crates).expect("read it");

    assert_eq!(
        files,
        [
            SourceFile {
                path: "crates/core/redkiln-core/tests/it/main.rs".to_owned(),
                text: "mod x;\n".to_owned(),
            },
            SourceFile {
                path: "crates/weigh-in/weighin/src/hooks.rs".to_owned(),
                text: "fn run() {}\n".to_owned(),
            },
            SourceFile {
                path: "crates/weigh-in/weighin/src/main.rs".to_owned(),
                text: "fn main() {}\n".to_owned(),
            },
        ]
    );
}

#[test]
fn an_absent_crates_folder_holds_no_sources() {
    let root = scratch("harness-read-absent").expect("a scratch folder");

    let files = read_sources(&root.join("crates")).expect("read it");

    assert_eq!(files, []);
}

#[test]
fn a_target_folder_is_skipped() {
    let crates = scratch("harness-read-target").expect("a scratch folder");
    tree(
        &crates,
        &[
            ("a/b/src/lib.rs", "pub fn f() {}\n"),
            ("a/b/target/debug/build/out.rs", "\"systemMessage\"\n"),
        ],
    )
    .expect("write the tree");

    let files = read_sources(&crates).expect("read it");

    assert_eq!(paths(&files), ["crates/a/b/src/lib.rs"]);
}

#[test]
fn a_crates_folder_that_is_a_file_is_refused() {
    let root = scratch("harness-read-root-file").expect("a scratch folder");
    let crates = root.join("crates");
    fs::write(&crates, b"").expect("write the file");

    let found = read_sources(&crates);

    assert!(
        matches!(found, Err(ReadError::RootNotFolder { .. })),
        "{found:?}"
    );
}

fn max_text() -> Result<usize, TryFromIntError> {
    usize::try_from(MAX_TEXT)
}

fn with_source(name: &str, contents: &[u8]) -> io::Result<Scratch> {
    let crates = scratch(name)?;
    fs::create_dir_all(crates.join("a/b/src"))?;
    fs::write(crates.join("a/b/src/lib.rs"), contents)?;
    Ok(crates)
}

#[test]
fn a_file_at_the_size_limit_is_read() {
    let max = max_text().expect("the limit fits in usize");
    let crates = with_source("harness-size-at", &vec![b'a'; max]).expect("write it");

    let files = read_sources(&crates).expect("read it");

    assert_eq!(
        files.iter().map(|f| f.text.len()).collect::<Vec<_>>(),
        [max]
    );
}

#[test]
fn the_reader_refuses_a_file_past_max_text() {
    let max = max_text().expect("the limit fits in usize");
    let crates = with_source("harness-size-over", &vec![b'a'; max + 1]).expect("write it");

    let found = read_sources(&crates);

    assert!(
        matches!(found, Err(ReadError::TooLarge { limit, .. }) if limit == MAX_TEXT),
        "{found:?}"
    );
}

#[test]
fn a_file_that_is_not_utf8_is_refused() {
    let crates = with_source("harness-not-utf8", b"ok\xff").expect("write it");

    let found = read_sources(&crates);

    assert!(
        matches!(found, Err(ReadError::NonUtf8Text { valid_up_to: 2, .. })),
        "{found:?}"
    );
}

/// A source file `folders` folders below `crates/`.
fn nested(folders: usize) -> String {
    format!("{}a.rs", "d/".repeat(folders))
}

#[test]
fn files_at_the_depth_limit_are_read() {
    let crates = scratch("harness-depth-at").expect("a scratch folder");
    let deepest = nested(MAX_DEPTH);
    tree(&crates, &[(&deepest, "fn f() {}\n")]).expect("write the file");

    let files = read_sources(&crates).expect("read it");

    assert_eq!(paths(&files), [format!("crates/{deepest}").as_str()]);
}

#[test]
fn the_reader_stops_at_max_depth() {
    let crates = scratch("harness-depth-over").expect("a scratch folder");
    tree(&crates, &[(&nested(MAX_DEPTH + 1), "fn f() {}\n")]).expect("write the file");

    let found = read_sources(&crates);

    assert!(
        matches!(found, Err(ReadError::TooDeep { limit, .. }) if limit == MAX_DEPTH),
        "{found:?}"
    );
}

/// Writes `count` empty Rust files named `0.rs` to `<count - 1>.rs` into `dir`.
fn many_rs(dir: &Path, count: usize) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    for n in 0..count {
        fs::write(dir.join(format!("{n}.rs")), b"")?;
    }
    Ok(())
}

/// A `crates/` holding `count` Rust files, split over two folders so no folder passes the entry
/// limit.
fn with_files(name: &str, count: usize) -> io::Result<Scratch> {
    let crates = scratch(name)?;
    let half = count / 2;
    many_rs(&crates.join("a"), half)?;
    many_rs(&crates.join("b"), count - half)?;
    Ok(crates)
}

#[test]
fn crates_at_the_file_limit_are_read() {
    let crates = with_files("harness-files-at", MAX_FILES).expect("write the files");

    let files = read_sources(&crates).expect("read it");

    assert_eq!(files.len(), MAX_FILES);
}

#[test]
fn the_reader_stops_at_max_files() {
    let crates = with_files("harness-files-over", MAX_FILES + 1).expect("write the files");

    let found = read_sources(&crates);

    assert!(
        matches!(&found, Err(ReadError::TooMany { path, limit }) if path.as_path() == &*crates && *limit == MAX_FILES),
        "{found:?}"
    );
}

#[test]
fn a_folder_at_the_entry_limit_is_read() {
    let crates = scratch("harness-entries-at").expect("a scratch folder");
    // Entries that are not Rust files, so the total-file limit stays out of the way.
    many(&crates.join("a"), MAX_FILES).expect("write the files");

    let files = read_sources(&crates).expect("read it");

    assert_eq!(files, []);
}

#[test]
fn a_folder_with_too_many_entries_is_refused() {
    let crates = scratch("harness-entries-over").expect("a scratch folder");
    many(&crates.join("a"), MAX_FILES + 1).expect("write the files");

    let found = read_sources(&crates);

    assert!(
        matches!(&found, Err(ReadError::TooManyEntries { path, .. }) if path.ends_with("a")),
        "{found:?}"
    );
}

/// Symlink cases; only unix lets a test create a symlink without extra rights.
#[cfg(unix)]
mod symlinks {
    use std::fs;
    use std::os::unix::fs::symlink;

    use xtask::harness::{ReadError, read_sources};

    use super::with_source;
    use crate::support::scratch;

    #[test]
    fn a_symlinked_source_is_refused() {
        let crates = with_source("harness-file-link", b"fn f() {}\n").expect("write it");
        symlink(
            crates.join("a/b/src/lib.rs"),
            crates.join("a/b/src/other.rs"),
        )
        .expect("create the symlink");

        let found = read_sources(&crates);

        assert!(
            matches!(&found, Err(ReadError::NotRegular { path }) if path.ends_with("a/b/src/other.rs")),
            "{found:?}"
        );
    }

    #[test]
    fn a_symlinked_folder_is_refused() {
        let crates = with_source("harness-folder-link", b"").expect("write it");
        let other = scratch("harness-folder-target").expect("a scratch folder");
        fs::write(other.join("c.rs"), b"\"systemMessage\"\n").expect("write the file");
        symlink(&other, crates.join("a/b/src/linked")).expect("create the symlink");

        let found = read_sources(&crates);

        assert!(
            matches!(&found, Err(ReadError::NotRegular { path }) if path.ends_with("a/b/src/linked")),
            "{found:?}"
        );
    }

    #[test]
    fn a_symlinked_crates_folder_is_refused() {
        let real = with_source("harness-root-real", b"").expect("write it");
        let link = scratch("harness-root-link").expect("a scratch folder");
        let crates = link.join("crates");
        symlink(&real, &crates).expect("create the symlink");

        let found = read_sources(&crates);

        assert!(
            matches!(found, Err(ReadError::RootNotFolder { .. })),
            "{found:?}"
        );
    }
}
