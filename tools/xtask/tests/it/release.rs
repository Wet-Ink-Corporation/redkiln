//! `xtask::release`: changelog sections, the Cowork stamp, packaging and the release repository.

use std::fs;
use std::path::Path;

use serde_json::{Value, json};
use xtask::release::{self, Download, ReleaseError};

use crate::support::{scratch, tree};

const CHANGELOG: &str = "# Changelog\n\nIntro.\n\n## Unreleased\n\n- next\n\n\
## 2.0.0 (2026-11-10)\n\n- moved\n- renamed\n\n## 1.2.0\n\n- older\n";

#[test]
fn a_changelog_section_is_the_lines_under_its_heading() {
    assert_eq!(
        release::changelog_section(CHANGELOG, "2.0.0"),
        Some("- moved\n- renamed")
    );
    assert_eq!(
        release::changelog_section(CHANGELOG, "1.2.0"),
        Some("- older")
    );
}

#[test]
fn a_missing_or_empty_changelog_section_is_none() {
    assert_eq!(release::changelog_section(CHANGELOG, "3.0.0"), None);
    assert_eq!(release::changelog_section(CHANGELOG, "2.0"), None);
    assert_eq!(
        release::changelog_section("## 2.0.0\n\n## 1.0.0\n- x\n", "2.0.0"),
        None
    );
}

#[test]
fn the_stamp_takes_the_version_and_the_checksum() {
    let script = "ls\nV=dev; S=none;\nB=\"$V\"\n";
    assert_eq!(
        release::stamp(script, "2.0.0", Some("abc")).unwrap(),
        "ls\nV=2.0.0; S=abc;\nB=\"$V\"\n"
    );
    assert_eq!(
        release::stamp(script, "2.0.0", None).unwrap(),
        "ls\nV=2.0.0; S=none;\nB=\"$V\"\n"
    );
}

#[test]
fn a_script_without_exactly_one_stamp_is_refused() {
    for (script, count) in [
        ("ls\n", 0),
        ("V=a; S=b;\nV=c; S=d;\n", 2),
        ("V=a;\nS=b;\n", 0),
    ] {
        assert!(
            matches!(
                release::stamp(script, "2.0.0", None),
                Err(ReleaseError::Stamp { found, .. }) if found == count
            ),
            "{script:?}"
        );
    }
}

const HOOKS: &str = r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"${CLAUDE_PLUGIN_ROOT}/libexec/demo","args":["hook","Stop"]}]}]}}"#;
const MOD_HOOKS: &str = r#"{"modules":["./hooks/mod/register.tsx"],"hooks":{}}"#;

/// A `plugins/` with a skill plugin carrying two builds and a mod plugin carrying none.
fn plugins(root: &Path, mod_version: &str) -> std::io::Result<()> {
    tree(
        root,
        &[
            (
                "demo/.claude-plugin/plugin.json",
                r#"{"name":"rk-demo","version":"2.0.0","description":"The demo."}"#,
            ),
            ("demo/hooks/hooks.json", HOOKS),
            ("demo/skills/demo/SKILL.md", "Run `demo open`.\n"),
            ("demo/libexec/demo", "#!/bin/sh\n"),
            ("demo/libexec/demo.cmd", "@echo off\r\n"),
            ("demo/libexec/demo.exe", "stale"),
            ("demo/libexec/demo-cowork-fetch.sh", "V=dev; S=none;\n"),
            ("demo/libexec/x86_64-unknown-linux-musl/demo", "musl build"),
            ("demo/libexec/x86_64-pc-windows-msvc/demo.exe", "msvc build"),
            (
                "demo-mod/.claude-plugin/plugin.json",
                &format!(
                    r#"{{"name":"rk-demo-mod","version":"{mod_version}","description":"The mod."}}"#
                ),
            ),
            ("demo-mod/hooks/hooks.json", MOD_HOOKS),
            ("demo-mod/hooks/mod/register.tsx", "export {};\n"),
            ("demo-mod/hooks/mod/mod.test.ts", "test\n"),
            ("demo-mod/libexec/demo", "#!/bin/sh\n"),
            ("demo-mod/libexec/demo.cmd", "@echo off\r\n"),
        ],
    )
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| format!("unreadable: {error}"))
}

#[test]
fn packaging_gives_every_plugin_the_builds_of_its_binaries() {
    let dir = scratch("release-package").unwrap();
    plugins(&dir.join("plugins"), "2.0.0").unwrap();
    let out = dir.join("out");

    let packaged = release::package(&dir.join("plugins"), &out, Download::Off).unwrap();

    let names: Vec<_> = packaged.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["rk-demo", "rk-demo-mod"]);
    for plugin in ["rk-demo", "rk-demo-mod"] {
        let libexec = out.join(plugin).join("libexec");
        assert_eq!(
            read(&libexec.join("x86_64-unknown-linux-musl/demo")),
            "musl build"
        );
        assert_eq!(
            read(&libexec.join("x86_64-pc-windows-msvc/demo.exe")),
            "msvc build"
        );
        assert_eq!(
            read(&libexec.join("demo.exe")),
            "msvc build",
            "the root exe is the msvc build"
        );
    }
    assert!(!out.join("rk-demo-mod/hooks/mod/mod.test.ts").exists());
    assert_eq!(
        read(&out.join("rk-demo-mod/hooks/mod/register.tsx")),
        "export {};\n"
    );
    assert_eq!(
        read(&out.join("rk-demo/libexec/demo-cowork-fetch.sh")),
        "V=2.0.0; S=none;\n"
    );
    assert_eq!(
        read(&dir.join("plugins/demo/libexec/demo-cowork-fetch.sh")),
        "V=dev; S=none;\n",
        "the source stays unstamped"
    );
}

#[test]
fn a_download_stamp_pins_the_musl_builds_checksum() {
    let dir = scratch("release-package-stamp").unwrap();
    plugins(&dir.join("plugins"), "2.0.0").unwrap();
    let out = dir.join("out");

    release::package(&dir.join("plugins"), &out, Download::Stamp).unwrap();

    // `printf 'musl build' | sha256sum`
    let sha = "8391935894f72d0dbe4ecf940f1102fa151cdb22f038b43ae980c7ad1281c393";
    assert_eq!(
        read(&out.join("rk-demo/libexec/demo-cowork-fetch.sh")),
        format!("V=2.0.0; S={sha};\n")
    );
}

#[cfg(unix)]
#[test]
fn packaged_launchers_and_builds_are_executable() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("release-package-mode").unwrap();
    plugins(&dir.join("plugins"), "2.0.0").unwrap();
    let out = dir.join("out");

    release::package(&dir.join("plugins"), &out, Download::Off).unwrap();

    for file in [
        "libexec/demo",
        "libexec/x86_64-unknown-linux-musl/demo",
        "libexec/demo.exe",
    ] {
        let mode = fs::metadata(out.join("rk-demo-mod").join(file))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111, "{file}: {mode:o}");
    }
}

#[test]
fn packaging_refuses_an_existing_output() {
    let dir = scratch("release-package-exists").unwrap();
    plugins(&dir.join("plugins"), "2.0.0").unwrap();
    fs::create_dir_all(dir.join("out")).unwrap();

    let refused = release::package(&dir.join("plugins"), &dir.join("out"), Download::Off);

    assert!(
        matches!(refused, Err(ReleaseError::OutExists { .. })),
        "{refused:?}"
    );
}

#[test]
fn packaging_refuses_plugins_whose_versions_differ() {
    let dir = scratch("release-package-versions").unwrap();
    plugins(&dir.join("plugins"), "1.9.0").unwrap();

    let refused = release::package(&dir.join("plugins"), &dir.join("out"), Download::Off);

    assert!(
        matches!(refused, Err(ReleaseError::VersionsDiffer { .. })),
        "{refused:?}"
    );
}

#[test]
fn packaging_refuses_a_binary_with_no_build() {
    let dir = scratch("release-package-nobuild").unwrap();
    plugins(&dir.join("plugins"), "2.0.0").unwrap();
    fs::remove_dir_all(dir.join("plugins/demo/libexec/x86_64-unknown-linux-musl")).unwrap();
    fs::remove_dir_all(dir.join("plugins/demo/libexec/x86_64-pc-windows-msvc")).unwrap();

    let refused = release::package(&dir.join("plugins"), &dir.join("out"), Download::Off);

    assert!(
        matches!(&refused, Err(ReleaseError::NoBuild { for_target, .. }) if for_target.is_empty()),
        "{refused:?}"
    );
}

#[test]
fn a_download_stamp_needs_the_musl_build() {
    let dir = scratch("release-package-nomusl").unwrap();
    plugins(&dir.join("plugins"), "2.0.0").unwrap();
    fs::remove_dir_all(dir.join("plugins/demo/libexec/x86_64-unknown-linux-musl")).unwrap();

    let refused = release::package(&dir.join("plugins"), &dir.join("out"), Download::Stamp);

    assert!(
        matches!(&refused, Err(ReleaseError::NoBuild { for_target, .. })
            if for_target.contains("x86_64-unknown-linux-musl")),
        "{refused:?}"
    );
}

#[test]
fn without_an_msvc_build_the_root_exe_is_the_gnu_build() {
    let dir = scratch("release-package-gnu").unwrap();
    plugins(&dir.join("plugins"), "2.0.0").unwrap();
    fs::remove_dir_all(dir.join("plugins/demo/libexec/x86_64-pc-windows-msvc")).unwrap();
    tree(
        &dir.join("plugins/demo/libexec"),
        &[("x86_64-pc-windows-gnu/demo.exe", "gnu build")],
    )
    .unwrap();
    let out = dir.join("out");

    release::package(&dir.join("plugins"), &out, Download::Off).unwrap();

    assert_eq!(read(&out.join("rk-demo/libexec/demo.exe")), "gnu build");
}

#[test]
fn without_a_windows_build_no_root_exe_ships() {
    let dir = scratch("release-package-nowin").unwrap();
    plugins(&dir.join("plugins"), "2.0.0").unwrap();
    fs::remove_dir_all(dir.join("plugins/demo/libexec/x86_64-pc-windows-msvc")).unwrap();
    let out = dir.join("out");

    release::package(&dir.join("plugins"), &out, Download::Off).unwrap();

    assert!(
        !out.join("rk-demo/libexec/demo.exe").exists(),
        "the stale source exe must not ship"
    );
}

#[test]
fn packaging_refuses_a_tree_that_breaks_the_plugin_rules() {
    let dir = scratch("release-package-rules").unwrap();
    plugins(&dir.join("plugins"), "2.0.0").unwrap();
    tree(
        &dir.join("plugins/demo"),
        &[("scripts/helper.py", "print()\n")],
    )
    .unwrap();

    let refused = release::package(&dir.join("plugins"), &dir.join("out"), Download::Off);

    assert!(
        matches!(&refused, Err(ReleaseError::Rules { violations })
            if violations.len() == 1 && violations.iter().all(|v| v.contains("scripts/helper.py"))),
        "{refused:?}"
    );
}

fn marketplace(repo: &Path) -> std::io::Result<()> {
    let existing = json!({
        "name": "weigh-in",
        "owner": {"name": "Ryan Britton"},
        "plugins": [
            {"name": "weigh", "source": "./plugin", "description": "1.x"},
            {"name": "rk-demo", "source": "./old", "description": "stale", "category": "dev"}
        ]
    });
    tree(
        repo,
        &[
            (".claude-plugin/marketplace.json", &existing.to_string()),
            ("plugin/keep.txt", "1.x stays"),
            ("plugins/rk-demo/stale.txt", "gone after assembly"),
        ],
    )
}

#[test]
fn assembly_replaces_each_packaged_plugin_and_keeps_the_rest() {
    let dir = scratch("release-assemble").unwrap();
    plugins(&dir.join("plugins"), "2.0.0").unwrap();
    release::package(&dir.join("plugins"), &dir.join("out"), Download::Off).unwrap();
    let repo = dir.join("release-repo");
    marketplace(&repo).unwrap();

    let names = release::assemble(&dir.join("out"), &repo).unwrap();

    assert_eq!(names, ["rk-demo", "rk-demo-mod"]);
    assert!(!repo.join("plugins/rk-demo/stale.txt").exists());
    assert_eq!(
        read(&repo.join("plugins/rk-demo-mod/libexec/x86_64-unknown-linux-musl/demo")),
        "musl build"
    );
    assert_eq!(read(&repo.join("plugin/keep.txt")), "1.x stays");
    let written: Value =
        serde_json::from_str(&read(&repo.join(".claude-plugin/marketplace.json"))).unwrap();
    assert_eq!(written["name"], "weigh-in");
    assert_eq!(
        written["plugins"],
        json!([
            {"name": "weigh", "source": "./plugin", "description": "1.x"},
            {"name": "rk-demo", "source": "./plugins/rk-demo", "description": "The demo.", "category": "dev"},
            {"name": "rk-demo-mod", "source": "./plugins/rk-demo-mod", "description": "The mod."}
        ])
    );
}

#[cfg(unix)]
#[test]
fn assembly_makes_launchers_and_builds_executable_again() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("release-assemble-mode").unwrap();
    plugins(&dir.join("plugins"), "2.0.0").unwrap();
    let out = dir.join("out");
    release::package(&dir.join("plugins"), &out, Download::Off).unwrap();
    // What an artifact download leaves: every file 0644.
    for file in [
        "libexec/demo",
        "libexec/x86_64-unknown-linux-musl/demo",
        "libexec/demo.cmd",
    ] {
        fs::set_permissions(
            out.join("rk-demo").join(file),
            fs::Permissions::from_mode(0o644),
        )
        .unwrap();
    }
    let repo = dir.join("release-repo");
    marketplace(&repo).unwrap();

    release::assemble(&out, &repo).unwrap();

    let mode = |file: &str| {
        fs::metadata(repo.join("plugins/rk-demo").join(file))
            .unwrap()
            .permissions()
            .mode()
            & 0o777
    };
    assert_eq!(mode("libexec/demo"), 0o755);
    assert_eq!(mode("libexec/x86_64-unknown-linux-musl/demo"), 0o755);
    assert_eq!(mode("libexec/demo.cmd"), 0o644);
}

#[cfg(unix)]
#[test]
fn assembly_refuses_a_symlink_where_a_plugin_folder_goes() {
    let dir = scratch("release-assemble-symlink").unwrap();
    plugins(&dir.join("plugins"), "2.0.0").unwrap();
    release::package(&dir.join("plugins"), &dir.join("out"), Download::Off).unwrap();
    let repo = dir.join("release-repo");
    marketplace(&repo).unwrap();
    let elsewhere = dir.join("elsewhere");
    tree(&elsewhere, &[("keep.txt", "not the release's")]).unwrap();
    fs::remove_dir_all(repo.join("plugins/rk-demo")).unwrap();
    std::os::unix::fs::symlink(&elsewhere, repo.join("plugins/rk-demo")).unwrap();

    let refused = release::assemble(&dir.join("out"), &repo);

    assert!(
        matches!(&refused, Err(ReleaseError::Plugin { plugin, .. }) if plugin == "rk-demo"),
        "{refused:?}"
    );
    assert_eq!(read(&elsewhere.join("keep.txt")), "not the release's");
}

#[test]
fn assembly_refuses_a_package_whose_manifest_names_another_plugin() {
    let dir = scratch("release-assemble-name").unwrap();
    plugins(&dir.join("plugins"), "2.0.0").unwrap();
    let out = dir.join("out");
    release::package(&dir.join("plugins"), &out, Download::Off).unwrap();
    fs::rename(out.join("rk-demo"), out.join("rk-other")).unwrap();
    let repo = dir.join("release-repo");
    marketplace(&repo).unwrap();

    let refused = release::assemble(&out, &repo);

    assert!(
        matches!(&refused, Err(ReleaseError::Plugin { plugin, .. }) if plugin == "rk-other"),
        "{refused:?}"
    );
    assert!(!repo.join("plugins/rk-other").exists());
}

#[test]
fn assembly_refuses_a_marketplace_without_a_plugin_list() {
    let dir = scratch("release-assemble-shape").unwrap();
    plugins(&dir.join("plugins"), "2.0.0").unwrap();
    release::package(&dir.join("plugins"), &dir.join("out"), Download::Off).unwrap();
    let repo = dir.join("release-repo");
    tree(
        &repo,
        &[(".claude-plugin/marketplace.json", r#"{"name":"x"}"#)],
    )
    .unwrap();

    let refused = release::assemble(&dir.join("out"), &repo);

    assert!(
        matches!(refused, Err(ReleaseError::Marketplace { .. })),
        "{refused:?}"
    );
}

#[test]
fn this_repositorys_release_check_agrees_with_its_own_versions() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let manifest: Value = serde_json::from_str(&read(
        &repo.join("plugins/weigh-in/.claude-plugin/plugin.json"),
    ))
    .unwrap();
    let version = manifest["version"].as_str().unwrap();

    let refused = release::release_check(&repo, &cargo, "weigh-in", "0.0.0-not-this");
    assert!(
        matches!(refused, Err(ReleaseError::VersionMismatch { .. })),
        "{refused:?}"
    );
    // The changelog may not have a section for the current version yet, which is the only other
    // refusal: every version site agrees.
    match release::release_check(&repo, &cargo, "weigh-in", version) {
        Ok(notes) => assert!(!notes.is_empty()),
        Err(error) => assert!(
            matches!(error, ReleaseError::NoChangelogSection { .. }),
            "{error:?}"
        ),
    }
}
