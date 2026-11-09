//! The plugin reader's limits and symlink handling (H-13), each pinned at its boundary.

use std::fs;
use std::io;
use std::num::TryFromIntError;
use std::path::PathBuf;

use xtask::plugins::{MAX_DEPTH, MAX_FILES, MAX_TEXT, Plugin, ReadError, read_plugins};

use crate::support::{Scratch, many, scratch, tree};

/// A `plugins/` folder for one test, holding one plugin with a manifest.
fn plugins_root(name: &str) -> io::Result<Scratch> {
    let root = scratch(name)?;
    tree(&root, &[("rk-weigh-in/.claude-plugin/plugin.json", "{}")])?;
    Ok(root)
}

/// A `plugins/` folder whose plugin has one skill holding `contents`.
fn with_skill(name: &str, contents: &[u8]) -> io::Result<Scratch> {
    let root = plugins_root(name)?;
    let folder = root.join("rk-weigh-in/skills/x");
    fs::create_dir_all(&folder)?;
    fs::write(folder.join("SKILL.md"), contents)?;
    Ok(root)
}

fn read_skill(name: &str, contents: &[u8]) -> Result<Vec<Plugin>, ReadError> {
    let root = with_skill(name, contents).map_err(|source| ReadError::Io {
        path: PathBuf::from(name),
        source,
    })?;
    read_plugins(&root)
}

fn max_text() -> Result<usize, TryFromIntError> {
    usize::try_from(MAX_TEXT)
}

#[test]
fn a_text_file_at_the_size_limit_is_read() {
    let max = max_text().expect("the limit fits in usize");
    let plugins = read_skill("size-at", &vec![b'a'; max]).expect("read it");
    let skill = plugins
        .iter()
        .flat_map(|p| &p.files)
        .find(|f| f.path == "skills/x/SKILL.md")
        .expect("the skill is listed");
    assert_eq!(skill.text.as_ref().map(String::len), Some(max));
}

#[test]
fn a_text_file_one_byte_over_the_limit_is_refused() {
    let max = max_text().expect("the limit fits in usize");
    let found = read_skill("size-over", &vec![b'a'; max + 1]);
    assert!(
        matches!(found, Err(ReadError::TooLarge { .. })),
        "{found:?}"
    );
}

#[test]
fn an_oversized_file_cut_inside_a_character_is_refused_by_size() {
    let max = max_text().expect("the limit fits in usize");
    let mut contents = vec![b'a'; max - 1];
    contents.extend_from_slice("é".as_bytes());
    let found = read_skill("size-multibyte", &contents);
    assert!(
        matches!(found, Err(ReadError::TooLarge { .. })),
        "{found:?}"
    );
}

#[test]
fn a_text_file_that_is_not_utf8_is_refused() {
    let found = read_skill("not-utf8", b"ok\xff");
    assert!(
        matches!(found, Err(ReadError::NonUtf8Text { valid_up_to: 2, .. })),
        "{found:?}"
    );
}

/// A file `folders` folders below the plugin, relative to the plugin.
fn nested(folders: usize) -> String {
    format!("{}file.md", "d/".repeat(folders))
}

#[test]
fn files_at_the_depth_limit_are_read() {
    let root = plugins_root("depth-at").expect("a plugins folder");
    let deepest = nested(MAX_DEPTH);
    tree(&root.join("rk-weigh-in"), &[(&deepest, "deep")]).expect("write the file");
    let plugins = read_plugins(&root).expect("read it");
    assert!(
        plugins
            .iter()
            .flat_map(|p| &p.files)
            .any(|f| f.path == deepest),
        "{plugins:?}"
    );
}

#[test]
fn files_one_folder_past_the_depth_limit_are_refused() {
    let root = plugins_root("depth-over").expect("a plugins folder");
    tree(
        &root.join("rk-weigh-in"),
        &[(&nested(MAX_DEPTH + 1), "deeper")],
    )
    .expect("write the file");
    let found = read_plugins(&root);
    assert!(matches!(found, Err(ReadError::TooDeep { .. })), "{found:?}");
}

/// A `plugins/` folder whose plugin holds `count` files beside its manifest, in two folders.
fn with_files(name: &str, count: usize) -> io::Result<Scratch> {
    let root = plugins_root(name)?;
    let plugin = root.join("rk-weigh-in");
    let half = count / 2;
    many(&plugin.join("a"), half)?;
    many(&plugin.join("b"), count - half)?;
    Ok(root)
}

#[test]
fn a_plugin_at_the_file_limit_is_read() {
    let root = with_files("files-at", MAX_FILES - 1).expect("write the plugin");
    let plugins = read_plugins(&root).expect("read it");
    let files: usize = plugins.iter().map(|p| p.files.len()).sum();
    assert_eq!(files, MAX_FILES);
}

#[test]
fn a_plugin_one_file_over_the_limit_is_refused() {
    let root = with_files("files-over", MAX_FILES).expect("write the plugin");
    let found = read_plugins(&root);
    assert!(
        matches!(&found, Err(ReadError::TooMany { plugin, limit }) if plugin == "rk-weigh-in" && *limit == MAX_FILES),
        "{found:?}"
    );
}

#[test]
fn a_folder_with_too_many_entries_is_refused() {
    let root = plugins_root("entries-over").expect("a plugins folder");
    many(&root.join("rk-weigh-in/a"), MAX_FILES + 1).expect("write the files");
    let found = read_plugins(&root);
    assert!(
        matches!(&found, Err(ReadError::TooManyEntries { path, .. }) if path.ends_with("rk-weigh-in/a")),
        "{found:?}"
    );
}

/// Symlink cases; only unix lets a test create a symlink without extra rights.
#[cfg(unix)]
mod symlinks {
    use std::fs;
    use std::io;
    use std::path::Path;

    use xtask::plugins::{ReadError, Violation, check, read_plugins};

    use super::plugins_root;
    use crate::support::{scratch, tree};

    fn symlink(target: &Path, link: &Path) -> io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    #[test]
    fn a_symlinked_plugins_folder_is_refused() {
        let real = plugins_root("root-real").expect("a plugins folder");
        let dir = scratch("root-link").expect("a scratch directory");
        let link = dir.join("plugins");
        symlink(&real, &link).expect("create the symlink");
        let found = read_plugins(&link);
        assert!(
            matches!(found, Err(ReadError::RootNotFolder { .. })),
            "{found:?}"
        );
    }

    #[test]
    fn a_symlinked_plugin_is_not_a_folder() {
        let real = plugins_root("plugin-real")
            .expect("a plugins folder")
            .join("rk-weigh-in");
        let root = scratch("plugin-link").expect("a scratch directory");
        symlink(&real, &root.join("rk-linked")).expect("create the symlink");
        let plugins = read_plugins(&root).expect("read the tree");
        assert_eq!(
            check(&plugins),
            vec![Violation::NotAFolder {
                plugin: "rk-linked".to_owned()
            }]
        );
    }

    #[test]
    fn a_symlinked_folder_inside_a_plugin_is_not_followed() {
        let outside = scratch("folder-link-outside").expect("a scratch directory");
        tree(&outside, &[("skill/SKILL.md", "libexec/bench")]).expect("write the target");
        let root = plugins_root("folder-link").expect("a plugins folder");
        fs::create_dir_all(root.join("rk-weigh-in/skills")).expect("create skills/");
        symlink(&outside.join("skill"), &root.join("rk-weigh-in/skills/x"))
            .expect("create the symlink");
        let plugins = read_plugins(&root).expect("read the tree");
        assert_eq!(
            check(&plugins),
            vec![Violation::NotRegularFile {
                plugin: "rk-weigh-in".to_owned(),
                path: "skills/x".to_owned()
            }]
        );
    }
}
