//! The knowledge-base reader's limits, errors and symlink handling (H-13), each limit pinned at
//! its boundary.

use std::fs;
use std::io;
use std::num::TryFromIntError;
use std::path::{Path, PathBuf};

use xtask::ids::{AtomFile, MAX_DEPTH, MAX_FILES, MAX_TEXT, ReadError, read_kb};

use crate::support::{scratch, tree};

/// A `.kb/` folder for one test holding one atom with `contents`.
fn with_atom(name: &str, contents: &[u8]) -> io::Result<PathBuf> {
    let kb = scratch(name)?;
    fs::create_dir_all(kb.join("decision"))?;
    fs::write(kb.join("decision/a.md"), contents)?;
    Ok(kb)
}

fn read_atom(name: &str, contents: &[u8]) -> Result<Vec<AtomFile>, ReadError> {
    let kb = with_atom(name, contents).map_err(|source| ReadError::Io {
        path: PathBuf::from(name),
        source,
    })?;
    read_kb(&kb)
}

fn max_text() -> Result<usize, TryFromIntError> {
    usize::try_from(MAX_TEXT)
}

#[test]
fn a_file_at_the_size_limit_is_read() {
    let max = max_text().expect("the limit fits in usize");
    let atoms = read_atom("ids-size-at", &vec![b'a'; max]).expect("read it");
    assert_eq!(
        atoms.iter().map(|a| a.text.len()).collect::<Vec<_>>(),
        [max]
    );
}

#[test]
fn a_file_one_byte_over_the_limit_is_refused() {
    let max = max_text().expect("the limit fits in usize");
    let found = read_atom("ids-size-over", &vec![b'a'; max + 1]);
    assert!(
        matches!(found, Err(ReadError::TooLarge { limit, .. }) if limit == MAX_TEXT),
        "{found:?}"
    );
}

#[test]
fn a_file_that_is_not_utf8_is_refused() {
    let found = read_atom("ids-not-utf8", b"ok\xff");
    assert!(
        matches!(found, Err(ReadError::NonUtf8Text { valid_up_to: 2, .. })),
        "{found:?}"
    );
}

/// A Markdown file `folders` folders below `.kb/`.
fn nested(folders: usize) -> String {
    format!("{}a.md", "d/".repeat(folders))
}

#[test]
fn files_at_the_depth_limit_are_read() {
    let kb = scratch("ids-depth-at").expect("a scratch folder");
    let deepest = nested(MAX_DEPTH);
    tree(&kb, &[(&deepest, "deep")]).expect("write the file");
    let atoms = read_kb(&kb).expect("read it");
    assert_eq!(
        atoms.iter().map(|a| a.path.as_str()).collect::<Vec<_>>(),
        [deepest.as_str()]
    );
}

#[test]
fn files_one_folder_past_the_depth_limit_are_refused() {
    let kb = scratch("ids-depth-over").expect("a scratch folder");
    tree(&kb, &[(&nested(MAX_DEPTH + 1), "deeper")]).expect("write the file");
    let found = read_kb(&kb);
    assert!(
        matches!(found, Err(ReadError::TooDeep { limit, .. }) if limit == MAX_DEPTH),
        "{found:?}"
    );
}

/// Writes `count` empty Markdown files named `0.md` to `<count - 1>.md` into `dir`.
fn many_md(dir: &Path, count: usize) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    for n in 0..count {
        fs::write(dir.join(format!("{n}.md")), b"")?;
    }
    Ok(())
}

/// A `.kb/` holding `count` Markdown files, split over two folders so no folder passes the entry
/// limit.
fn with_files(name: &str, count: usize) -> io::Result<PathBuf> {
    let kb = scratch(name)?;
    let half = count / 2;
    many_md(&kb.join("a"), half)?;
    many_md(&kb.join("b"), count - half)?;
    Ok(kb)
}

#[test]
fn a_knowledge_base_at_the_file_limit_is_read() {
    let kb = with_files("ids-files-at", MAX_FILES).expect("write the files");
    let atoms = read_kb(&kb).expect("read it");
    assert_eq!(atoms.len(), MAX_FILES);
}

#[test]
fn a_knowledge_base_one_file_over_the_limit_is_refused_naming_its_root() {
    let kb = with_files("ids-files-over", MAX_FILES + 1).expect("write the files");
    let found = read_kb(&kb);
    assert!(
        matches!(&found, Err(ReadError::TooMany { path, limit }) if *path == kb && *limit == MAX_FILES),
        "{found:?}"
    );
}

#[test]
fn a_folder_with_too_many_entries_is_refused() {
    let kb = scratch("ids-entries-over").expect("a scratch folder");
    let dir = kb.join("decision");
    fs::create_dir_all(&dir).expect("a folder");
    for n in 0..=MAX_FILES {
        fs::write(dir.join(n.to_string()), b"").expect("write a file");
    }
    let found = read_kb(&kb);
    assert!(
        matches!(&found, Err(ReadError::TooManyEntries { path, .. }) if path.ends_with("decision")),
        "{found:?}"
    );
}

#[test]
fn a_knowledge_base_that_is_a_file_is_refused() {
    let root = scratch("ids-root-file").expect("a scratch folder");
    let kb = root.join(".kb");
    fs::write(&kb, b"").expect("write the file");
    let found = read_kb(&kb);
    assert!(
        matches!(found, Err(ReadError::RootNotFolder { .. })),
        "{found:?}"
    );
}

/// Symlink cases; only unix lets a test create a symlink without extra rights.
#[cfg(unix)]
mod symlinks {
    use std::fs;
    use std::os::unix::fs::symlink;

    use xtask::ids::{ReadError, read_kb};

    use super::with_atom;
    use crate::support::scratch;

    #[test]
    fn a_symlinked_knowledge_base_is_refused() {
        let real = with_atom("ids-root-real", b"").expect("a knowledge base");
        let kb = scratch("ids-root-link")
            .expect("a scratch folder")
            .join(".kb");
        symlink(&real, &kb).expect("create the symlink");
        let found = read_kb(&kb);
        assert!(
            matches!(found, Err(ReadError::RootNotFolder { .. })),
            "{found:?}"
        );
    }

    #[test]
    fn a_symlinked_atom_is_refused() {
        let kb = with_atom("ids-file-link", b"---\nid: \"kb-decision-a\"\n---\n")
            .expect("a knowledge base");
        symlink(kb.join("decision/a.md"), kb.join("decision/b.md")).expect("create the symlink");
        let found = read_kb(&kb);
        assert!(
            matches!(&found, Err(ReadError::NotRegular { path }) if path.ends_with("decision/b.md")),
            "{found:?}"
        );
    }

    #[test]
    fn a_symlinked_folder_is_refused() {
        let kb = with_atom("ids-folder-link", b"").expect("a knowledge base");
        let other = scratch("ids-folder-target").expect("a scratch folder");
        fs::write(other.join("c.md"), b"").expect("write the file");
        symlink(&other, kb.join("decision/linked")).expect("create the symlink");
        let found = read_kb(&kb);
        assert!(
            matches!(&found, Err(ReadError::NotRegular { path }) if path.ends_with("decision/linked")),
            "{found:?}"
        );
    }
}
