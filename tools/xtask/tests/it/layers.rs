//! The layer rules of `AGENTS.md`, checked against synthetic workspaces and this one.

use std::path::Path;

use xtask::layers::{Dependency, Kind, Package, Violation, check, read_workspace};

fn library(name: &str, dir: &str, dependencies: &[(&str, Kind)]) -> Package {
    Package {
        name: name.to_owned(),
        dir: dir.to_owned(),
        publishable: false,
        has_binary: false,
        dependencies: dependencies
            .iter()
            .map(|&(dependency, kind)| Dependency {
                name: dependency.to_owned(),
                kind,
            })
            .collect(),
    }
}

fn binary(name: &str, dir: &str, dependencies: &[(&str, Kind)]) -> Package {
    Package {
        has_binary: true,
        ..library(name, dir, dependencies)
    }
}

/// One crate of every kind, wired with every edge the rules allow.
fn fully_wired() -> Vec<Package> {
    vec![
        library(
            "redkiln-core",
            "crates/core/redkiln-core",
            &[("serde", Kind::Normal)],
        ),
        library(
            "redkiln-calibrate",
            "crates/core/redkiln-calibrate",
            &[("redkiln-core", Kind::Normal)],
        ),
        library(
            "redkiln-guard",
            "crates/kits/redkiln-guard",
            &[("redkiln-core", Kind::Normal)],
        ),
        library(
            "weighin-contract",
            "crates/weigh-in/weighin-contract",
            &[("redkiln-core", Kind::Normal)],
        ),
        library(
            "review-contract",
            "crates/review/review-contract",
            &[("redkiln-core", Kind::Normal)],
        ),
        library(
            "weighin-core",
            "crates/weigh-in/weighin-core",
            &[
                ("redkiln-core", Kind::Normal),
                ("weighin-contract", Kind::Normal),
            ],
        ),
        binary(
            "weighin",
            "crates/weigh-in/weighin",
            &[
                ("redkiln-core", Kind::Normal),
                ("redkiln-guard", Kind::Normal),
                ("weighin-core", Kind::Normal),
                ("weighin-contract", Kind::Normal),
                ("review-contract", Kind::Normal),
                ("anyhow", Kind::Normal),
            ],
        ),
        binary(
            "xtask",
            "tools/xtask",
            &[
                ("redkiln-core", Kind::Normal),
                ("redkiln-guard", Kind::Normal),
                ("weighin-contract", Kind::Dev),
            ],
        ),
    ]
}

/// `workspace` with the member named like `replacement` swapped for it, or with it appended.
fn with(mut workspace: Vec<Package>, replacement: Package) -> Vec<Package> {
    match workspace
        .iter_mut()
        .find(|member| member.name == replacement.name)
    {
        Some(slot) => *slot = replacement,
        None => workspace.push(replacement),
    }
    workspace
}

#[test]
fn accepts_every_allowed_edge() {
    let workspace = fully_wired();

    assert_eq!(check(&workspace), Vec::new(), "allowed edges were rejected");
}

#[test]
fn rejects_a_kit_depending_on_another_kit() {
    let workspace = with(
        with(
            fully_wired(),
            library("redkiln-github", "crates/kits/redkiln-github", &[]),
        ),
        library(
            "redkiln-guard",
            "crates/kits/redkiln-guard",
            &[("redkiln-github", Kind::Normal)],
        ),
    );

    assert_eq!(
        check(&workspace),
        vec![Violation::ForbiddenEdge {
            from: "redkiln-guard",
            to: "redkiln-github"
        }],
        "kits depend only on Core"
    );
}

#[test]
fn rejects_core_depending_on_a_kit() {
    let workspace = with(
        fully_wired(),
        library(
            "redkiln-core",
            "crates/core/redkiln-core",
            &[("redkiln-guard", Kind::Normal)],
        ),
    );

    assert_eq!(
        check(&workspace),
        vec![Violation::ForbiddenEdge {
            from: "redkiln-core",
            to: "redkiln-guard"
        }],
        "Core depends on nothing outside Core"
    );
}

#[test]
fn rejects_a_contract_depending_on_a_kit() {
    let workspace = with(
        fully_wired(),
        library(
            "review-contract",
            "crates/review/review-contract",
            &[("redkiln-guard", Kind::Normal)],
        ),
    );

    assert_eq!(
        check(&workspace),
        vec![Violation::ForbiddenEdge {
            from: "review-contract",
            to: "redkiln-guard"
        }],
        "contract crates depend only on Core"
    );
}

#[test]
fn rejects_a_contract_depending_on_its_own_plugins_internals() {
    let workspace = with(
        fully_wired(),
        library(
            "weighin-contract",
            "crates/weigh-in/weighin-contract",
            &[("weighin-core", Kind::Normal)],
        ),
    );

    assert_eq!(
        check(&workspace),
        vec![Violation::ForbiddenEdge {
            from: "weighin-contract",
            to: "weighin-core"
        }],
        "a contract crate must not reach into the plugin behind it"
    );
}

#[test]
fn rejects_a_plugin_reaching_into_another_plugins_internals_even_for_tests() {
    let workspace = with(
        fully_wired(),
        library(
            "review-domain",
            "crates/review/review-domain",
            &[
                ("review-contract", Kind::Normal),
                ("weighin-contract", Kind::Normal),
                ("weighin-core", Kind::Dev),
            ],
        ),
    );

    assert_eq!(
        check(&workspace),
        vec![Violation::ForbiddenEdge {
            from: "review-domain",
            to: "weighin-core"
        }],
        "another plugin is reached through its contract crate, never its internals"
    );
}

#[test]
fn rejects_a_tool_depending_on_plugin_internals() {
    let workspace = with(
        fully_wired(),
        binary("xtask", "tools/xtask", &[("weighin-core", Kind::Build)]),
    );

    assert_eq!(
        check(&workspace),
        vec![Violation::ForbiddenEdge {
            from: "xtask",
            to: "weighin-core"
        }],
        "dev tools use Core, kits and contracts only"
    );
}

#[test]
fn rejects_a_contract_depending_on_another_contract() {
    let workspace = with(
        fully_wired(),
        library(
            "review-contract",
            "crates/review/review-contract",
            &[("weighin-contract", Kind::Normal)],
        ),
    );

    assert_eq!(
        check(&workspace),
        vec![Violation::ForbiddenEdge {
            from: "review-contract",
            to: "weighin-contract"
        }],
        "contract crates depend only on Core, not on each other"
    );
}

#[test]
fn rejects_a_kit_depending_on_a_contract() {
    let workspace = with(
        fully_wired(),
        library(
            "redkiln-guard",
            "crates/kits/redkiln-guard",
            &[("weighin-contract", Kind::Normal)],
        ),
    );

    assert_eq!(
        check(&workspace),
        vec![Violation::ForbiddenEdge {
            from: "redkiln-guard",
            to: "weighin-contract"
        }],
        "kits depend only on Core"
    );
}

#[test]
fn rejects_a_tool_depending_on_another_tool() {
    let workspace = with(
        fully_wired(),
        binary("skillcheck", "tools/skillcheck", &[("xtask", Kind::Normal)]),
    );

    assert_eq!(
        check(&workspace),
        vec![Violation::ForbiddenEdge {
            from: "skillcheck",
            to: "xtask"
        }],
        "dev tools use Core, kits and contracts only"
    );
}

#[test]
fn rejects_a_member_nested_below_its_layer_folder() {
    let workspace = with(
        fully_wired(),
        library("deep", "crates/core/group/deep", &[]),
    );

    assert_eq!(
        check(&workspace),
        vec![Violation::Unplaced {
            package: "deep",
            dir: "crates/core/group/deep"
        }],
        "members sit directly under their layer folder"
    );
}

#[test]
fn rejects_a_member_outside_the_layout() {
    let workspace = with(fully_wired(), library("stray", "misc/stray", &[]));

    assert_eq!(
        check(&workspace),
        vec![Violation::Unplaced {
            package: "stray",
            dir: "misc/stray"
        }],
        "every member sits under crates/core, crates/kits, crates/<plugin> or tools"
    );
}

#[test]
fn rejects_a_crate_under_an_unknown_plugin_folder() {
    let workspace = with(
        fully_wired(),
        library("lens-core", "crates/lens/lens-core", &[]),
    );

    assert_eq!(
        check(&workspace),
        vec![Violation::Unplaced {
            package: "lens-core",
            dir: "crates/lens/lens-core"
        }],
        "a new plugin folder is a crate-boundary change made in the policy first"
    );
}

#[test]
fn rejects_an_edge_into_a_member_outside_the_layout() {
    let workspace = with(
        with(fully_wired(), library("stray", "misc/stray", &[])),
        library(
            "redkiln-core",
            "crates/core/redkiln-core",
            &[("stray", Kind::Normal)],
        ),
    );

    assert_eq!(
        check(&workspace),
        vec![
            Violation::ForbiddenEdge {
                from: "redkiln-core",
                to: "stray"
            },
            Violation::Unplaced {
                package: "stray",
                dir: "misc/stray"
            },
        ],
        "an unplaced member is reachable from nowhere"
    );
}

#[test]
fn rejects_anyhow_in_a_library() {
    let workspace = with(
        fully_wired(),
        library(
            "weighin-core",
            "crates/weigh-in/weighin-core",
            &[("anyhow", Kind::Normal)],
        ),
    );

    assert_eq!(
        check(&workspace),
        vec![Violation::AnyhowInLibrary {
            package: "weighin-core"
        }],
        "anyhow belongs only in binaries (H-09)"
    );
}

#[test]
fn accepts_anyhow_as_a_library_dev_dependency() {
    let workspace = with(
        fully_wired(),
        library(
            "weighin-core",
            "crates/weigh-in/weighin-core",
            &[("anyhow", Kind::Dev)],
        ),
    );

    assert_eq!(
        check(&workspace),
        Vec::new(),
        "tests may use anyhow; only the library's own code may not"
    );
}

#[test]
fn rejects_a_publishable_member() {
    let workspace = with(
        fully_wired(),
        Package {
            publishable: true,
            ..library("redkiln-guard", "crates/kits/redkiln-guard", &[])
        },
    );

    assert_eq!(
        check(&workspace),
        vec![Violation::Publishable {
            package: "redkiln-guard"
        }],
        "turning publish on is an H-16 decision"
    );
}

#[test]
fn describes_a_violation_in_one_line() {
    let violation = Violation::ForbiddenEdge {
        from: "redkiln-guard",
        to: "redkiln-github",
    };

    assert_eq!(
        violation.to_string(),
        "redkiln-guard depends on redkiln-github, which its layer may not use",
        "the CLI prints violations through Display"
    );
}

#[test]
#[cfg_attr(miri, ignore = "spawns `cargo metadata`; Miri cannot run processes")]
fn this_workspace_follows_the_layer_rules() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml");
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());

    let packages = read_workspace(&cargo, &manifest)?;

    assert!(
        packages.iter().any(|package| package.name == "xtask"
            && package.dir == "tools/xtask"
            && package.has_binary
            && !package.publishable),
        "cargo metadata was read into packages: {packages:?}"
    );
    assert_eq!(
        check(&packages),
        Vec::new(),
        "the workspace breaks the layer rules in AGENTS.md"
    );
    Ok(())
}
