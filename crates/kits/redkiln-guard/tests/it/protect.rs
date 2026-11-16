//! `ProtectedPaths::contains_protected` through the public API: what removing a path changes,
//! for a table with one protected file under one container directory.

use redkiln_guard::engine::Cwd;
use redkiln_guard::protect::{Protected, ProtectedPaths};
use redkiln_guard::shell::Shape;

/// The protected file `settings.json`, directly under `.claude`.
const PATHS: [Protected; 1] = [Protected::exact(
    "the settings file",
    Some(".claude"),
    "settings.json",
)];

/// `.claude` holds the protected file.
const CONTAINERS: [(&str, &str); 1] = [(".claude", "the .claude directory")];

const ABOVE: &str = "a directory above the start";

const TABLE: ProtectedPaths<'static> = ProtectedPaths::new(&PATHS, &CONTAINERS, ABOVE);

#[test]
fn removing_the_protected_file_itself_is_reported_with_its_label() {
    let label = TABLE.contains_protected(".claude/settings.json", Shape::Literal, &Cwd::Start);

    assert_eq!(label, Some("the settings file"));
}

#[test]
fn removing_a_directory_that_holds_the_protected_file_is_reported_with_its_container_label() {
    let label = TABLE.contains_protected(".claude", Shape::Literal, &Cwd::Start);

    assert_eq!(label, Some("the .claude directory"));
}

#[test]
fn a_container_is_not_itself_a_protected_path() {
    let written = TABLE.protected(".claude", Shape::Literal, &Cwd::Start);

    assert_eq!(written, None);
}

#[test]
fn a_container_is_matched_ignoring_ascii_case() {
    let label = TABLE.contains_protected("work/.CLAUDE", Shape::Literal, &Cwd::Start);

    assert_eq!(label, Some("the .claude directory"));
}

#[test]
fn a_glob_that_could_name_a_container_is_reported_with_its_label() {
    let label = TABLE.contains_protected(".clau*", Shape::Pattern, &Cwd::Start);

    assert_eq!(label, Some("the .claude directory"));
}

#[test]
fn removing_the_current_directory_after_a_cd_into_a_container_is_reported() {
    let cwd = Cwd::Dir(".claude".to_owned(), Shape::Literal);

    let label = TABLE.contains_protected(".", Shape::Literal, &cwd);

    assert_eq!(label, Some("the .claude directory"));
}

#[test]
fn removing_a_directory_above_the_start_is_reported_with_the_above_label() {
    let label = TABLE.contains_protected("..", Shape::Literal, &Cwd::Start);

    assert_eq!(label, Some(ABOVE));
}

#[test]
fn a_path_that_holds_no_protected_file_is_none() {
    let label = TABLE.contains_protected("src/main.rs", Shape::Literal, &Cwd::Start);

    assert_eq!(label, None);
}

#[test]
fn the_protected_name_under_another_directory_is_none() {
    let label = TABLE.contains_protected("other/settings.json", Shape::Literal, &Cwd::Start);

    assert_eq!(label, None);
}
