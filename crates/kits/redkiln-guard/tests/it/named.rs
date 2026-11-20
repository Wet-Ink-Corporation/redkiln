//! `ProtectedPaths::named_in` through the public API: which entry or container of a table a
//! piece of text (inline code the guard cannot read as commands) names.

use redkiln_guard::protect::{Protected, ProtectedPaths};

/// A file under a container, a parentless file, a parentless component and an affix.
const PATHS: [Protected; 4] = [
    Protected::exact("the settings file", Some(".claude"), "settings.json"),
    Protected::file("clippy.toml"),
    Protected::exact("the hooks directory", None, "hooks"),
    Protected::affix(".eslintrc*", None, (".eslintrc", ""), &[".eslintrc"]),
];

/// `.redkiln/bench` holds nothing in this table, but is a container named by two components.
const CONTAINERS: [(&str, &str); 1] = [(".redkiln/bench", "the bench folder")];

const TABLE: ProtectedPaths<'static> = ProtectedPaths::new(&PATHS, &CONTAINERS, "above");

#[test]
fn an_entry_under_a_parent_is_named_by_its_parent() {
    let label = TABLE.named_in("open('.claude/x', 'w')");

    assert_eq!(label, Some("the settings file"));
}

#[test]
fn a_container_is_named_by_its_first_component() {
    let label = TABLE.named_in("shutil.copytree(src, '.redkiln')");

    assert_eq!(label, Some("the bench folder"));
}

#[test]
fn a_parentless_file_is_named_by_its_name() {
    let label = TABLE.named_in("fs.writeFileSync('clippy.toml', s)");

    assert_eq!(label, Some("clippy.toml"));
}

#[test]
fn a_parentless_component_is_named_as_a_path_word() {
    let label = TABLE.named_in("open('.git/hooks/pre-commit', 'w')");

    assert_eq!(label, Some("the hooks directory"));
}

#[test]
fn an_affix_is_named_by_its_prefix_with_any_suffix() {
    let label = TABLE.named_in("open('.eslintrc.json', 'w')");

    assert_eq!(label, Some(".eslintrc*"));
}

#[test]
fn a_name_is_matched_ignoring_ascii_case() {
    let label = TABLE.named_in("open('.CLAUDE/settings.json', 'w')");

    assert_eq!(label, Some("the settings file"));
}

#[test]
fn a_table_name_written_in_upper_case_is_matched_ignoring_ascii_case() {
    const UPPER: [Protected; 1] = [Protected::exact("upper", Some(".Claude"), "settings.json")];
    let table = ProtectedPaths::new(&UPPER, &[], "above");

    let label = table.named_in("open('.claude/x', 'w')");

    assert_eq!(label, Some("upper"));
}

#[test]
fn a_dotless_name_counts_only_beside_a_path_separator() {
    let texts = ["import hooks", "hooks = 1", "print(hooks)"];

    let named: Vec<(&str, Option<&str>)> = texts
        .iter()
        .map(|text| (*text, TABLE.named_in(text)))
        .filter(|(_, label)| label.is_some())
        .collect();

    assert_eq!(named, Vec::new());
}

#[test]
fn a_dotless_name_before_a_separator_is_named() {
    let label = TABLE.named_in("open('hooks/x', 'w')");

    assert_eq!(label, Some("the hooks directory"));
}

#[test]
fn a_bare_container_name_is_labelled_as_the_container() {
    const PATHS: [Protected; 1] = [Protected::exact(
        "the settings file",
        Some(".claude"),
        "settings.json",
    )];
    const CONTAINERS: [(&str, &str); 1] = [(".claude", "the .claude directory")];
    let table = ProtectedPaths::new(&PATHS, &CONTAINERS, "above");

    let label = table.named_in("shutil.rmtree('.claude')");

    assert_eq!(label, Some("the .claude directory"));
}

#[test]
fn a_name_inside_a_longer_word_is_not_named() {
    let texts = [
        "print('webhooks')",
        "print('my.claude')",
        "print('.claudette')",
        "print('.claude.bak')",
        "print('hooks_dir')",
        "print(1)",
    ];

    let named: Vec<(&str, Option<&str>)> = texts
        .iter()
        .map(|text| (*text, TABLE.named_in(text)))
        .filter(|(_, label)| label.is_some())
        .collect();

    assert_eq!(named, Vec::new());
}
