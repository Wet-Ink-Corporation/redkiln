use super::{Protected, ProtectedPaths};
use crate::engine::Cwd;
use crate::shell::Shape;

/// A file under a two-component parent, and both its directories as containers.
const PATHS: [Protected; 1] = [Protected::exact(
    ".tool/pack/config.json",
    Some(".tool/pack"),
    "config.json",
)];

const CONTAINERS: [(&str, &str); 2] = [(".tool", ".tool/"), (".tool/pack", ".tool/pack/")];

const TABLE: ProtectedPaths<'static> = ProtectedPaths::new(&PATHS, &CONTAINERS, "above");

fn protected(path: &str, cwd: &Cwd) -> Option<&'static str> {
    TABLE.protected(path, Shape::Literal, cwd)
}

fn removed(path: &str, cwd: &Cwd) -> Option<&'static str> {
    TABLE.contains_protected(path, Shape::Literal, cwd)
}

#[test]
fn a_two_component_parent_protects_its_file_only_under_both_components() {
    let label = Some(".tool/pack/config.json");

    assert_eq!(
        [
            protected(".tool/pack/config.json", &Cwd::Start),
            protected("x/.TOOL/Pack/config.json", &Cwd::Start),
            protected(".tool/x/../pack/config.json", &Cwd::Start),
        ],
        [label, label, label]
    );
    assert_eq!(
        [
            protected("pack/config.json", &Cwd::Start),
            protected(".tool/config.json", &Cwd::Start),
            protected("other/pack/config.json", &Cwd::Start),
            protected(".tool/pack/other.json", &Cwd::Start),
        ],
        [None, None, None, None]
    );
}

#[test]
fn above_the_start_a_two_component_parent_may_be_any_suffix_of_it() {
    let label = Some(".tool/pack/config.json");

    assert_eq!(
        [
            protected("../config.json", &Cwd::Start),
            protected("../pack/config.json", &Cwd::Start),
            protected("pack/config.json", &Cwd::Unknown),
            protected("config.json", &Cwd::Unknown),
        ],
        [label, label, label, label]
    );
    assert_eq!(protected("../other/config.json", &Cwd::Start), None);
}

#[test]
fn a_cd_into_the_parent_is_followed() {
    let cwd = Cwd::Dir(".tool/pack".to_owned(), Shape::Literal);

    assert_eq!(
        protected("config.json", &cwd),
        Some(".tool/pack/config.json")
    );
}

#[test]
fn a_two_component_container_is_matched_on_its_last_components() {
    assert_eq!(
        [
            removed(".tool/pack", &Cwd::Start),
            removed("x/.tool/pack/", &Cwd::Start),
            removed(".tool", &Cwd::Start),
            removed("../pack", &Cwd::Start),
        ],
        [
            Some(".tool/pack/"),
            Some(".tool/pack/"),
            Some(".tool/"),
            Some(".tool/pack/")
        ]
    );
    assert_eq!(
        [
            removed("pack", &Cwd::Start),
            removed("other/pack", &Cwd::Start),
            removed("../other/pack", &Cwd::Start),
        ],
        [None, None, None]
    );
}
