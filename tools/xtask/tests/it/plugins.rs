//! The plugin rules of `AGENTS.md` (CLI first, harness second), checked against synthetic
//! plugins, a plugin tree on disk and this repository.

use std::path::Path;

use xtask::plugins::{FileKind, Plugin, PluginFile, Violation, check, read_plugins};

use crate::support::{scratch, tree};

const HOOKS: &str = r#"{"hooks":{"SessionStart":[{"hooks":[
  {"type":"command","command":"${CLAUDE_PLUGIN_ROOT}/libexec/weighin","args":["hook","SessionStart"]}
]}]}}"#;

const MOD_HOOKS: &str = r#"{"modules":["./mod/register.tsx"]}"#;

fn file(path: &str) -> PluginFile {
    PluginFile {
        path: path.to_owned(),
        kind: FileKind::Regular,
        text: None,
    }
}

fn text(path: &str, contents: &str) -> PluginFile {
    PluginFile {
        text: Some(contents.to_owned()),
        ..file(path)
    }
}

fn plugin(files: Vec<PluginFile>) -> Plugin {
    Plugin {
        name: "rk-weigh-in".to_owned(),
        folder: true,
        files,
    }
}

/// A plugin with everything the rules allow a main plugin to carry.
fn full() -> Vec<PluginFile> {
    vec![
        file(".claude-plugin/plugin.json"),
        file("README.md"),
        file("CHANGELOG.md"),
        text("hooks/hooks.json", HOOKS),
        text(
            "skills/weigh-in/SKILL.md",
            "Run `${CLAUDE_PLUGIN_ROOT}/libexec/weighin open --json`; builds sit in \
             `libexec/<target>/` behind `libexec/<binary>`.",
        ),
        file("skills/weigh-in/assets/board.html"),
        file("skills/weigh-in/references/cards.json"),
        text(
            "agents/reviewer.md",
            "Read `libexec/weighin status --json`.",
        ),
        text("commands/sit.md", "Calls libexec/weighin.cmd on Windows."),
        file("libexec/weighin"),
        file("libexec/weighin.cmd"),
        file("libexec/x86_64-unknown-linux-musl/weighin"),
        file("libexec/x86_64-pc-windows-msvc/weighin.exe"),
    ]
}

fn violations(files: Vec<PluginFile>) -> Vec<Violation> {
    check(&[plugin(files)])
}

fn with(extra: PluginFile) -> Vec<Violation> {
    let mut files = full();
    files.push(extra);
    violations(files)
}

fn replacing_hooks(hooks: &str) -> Vec<Violation> {
    let mut files: Vec<PluginFile> = full()
        .into_iter()
        .filter(|f| f.path != "hooks/hooks.json")
        .collect();
    files.push(text("hooks/hooks.json", hooks));
    violations(files)
}

#[test]
fn a_plugin_with_every_allowed_part_passes() {
    assert_eq!(violations(full()), vec![]);
}

#[test]
fn a_plugin_with_only_a_manifest_passes() {
    assert_eq!(violations(vec![file(".claude-plugin/plugin.json")]), vec![]);
}

#[test]
fn no_plugins_pass() {
    assert_eq!(check(&[]), vec![]);
}

#[test]
fn a_plugin_without_a_manifest_fails() {
    assert_eq!(
        violations(vec![file("README.md")]),
        vec![Violation::NoManifest {
            plugin: "rk-weigh-in".to_owned()
        }]
    );
}

#[test]
fn a_script_in_the_plugin_fails() {
    for path in [
        "hooks/guard.sh",
        "scripts/build.py",
        "skills/weigh-in/helper.mjs",
        "mod/register.tsx",
    ] {
        assert_eq!(
            with(file(path)),
            vec![Violation::Unlisted {
                plugin: "rk-weigh-in".to_owned(),
                path: path.to_owned()
            }],
            "{path}"
        );
    }
}

#[test]
fn policy_data_beside_the_skills_fails() {
    assert_eq!(
        with(file("playbooks/clef-thresholds.json")),
        vec![Violation::Unlisted {
            plugin: "rk-weigh-in".to_owned(),
            path: "playbooks/clef-thresholds.json".to_owned()
        }]
    );
}

#[test]
fn a_nested_agent_or_command_fails() {
    assert_eq!(
        with(file("agents/deep/reviewer.md")),
        vec![Violation::Unlisted {
            plugin: "rk-weigh-in".to_owned(),
            path: "agents/deep/reviewer.md".to_owned()
        }]
    );
}

#[test]
fn a_symlink_fails_even_on_an_allowed_path() {
    let mut files = full();
    files.push(PluginFile {
        kind: FileKind::Other,
        ..file("skills/linked/SKILL.md")
    });
    assert_eq!(
        violations(files),
        vec![Violation::NotRegularFile {
            plugin: "rk-weigh-in".to_owned(),
            path: "skills/linked/SKILL.md".to_owned()
        }]
    );
}

#[test]
fn a_mod_plugin_carries_its_module_and_types() {
    let files = vec![
        file(".claude-plugin/plugin.json"),
        text("hooks/hooks.json", MOD_HOOKS),
        file("hooks/mod/register.tsx"),
        file("hooks/mod/band.ts"),
        file("types/plugin-api.d.ts"),
        file("libexec/weighin"),
    ];
    assert_eq!(violations(files), vec![]);
}

#[test]
fn a_mod_plugin_carries_no_skills() {
    let files = vec![
        file(".claude-plugin/plugin.json"),
        text("hooks/hooks.json", MOD_HOOKS),
        file("hooks/mod/register.tsx"),
        file("skills/weigh-in/SKILL.md"),
    ];
    assert_eq!(
        violations(files),
        vec![Violation::ModWithSkills {
            plugin: "rk-weigh-in".to_owned(),
            path: "skills/weigh-in/SKILL.md".to_owned()
        }]
    );
}

#[test]
fn a_hook_that_is_not_a_command_fails() {
    let found = replacing_hooks(
        r#"{"hooks":{"Stop":[{"hooks":[{"type":"prompt","prompt":"Is the work done?"}]}]}}"#,
    );
    assert_eq!(
        found,
        vec![Violation::HookNotBinary {
            plugin: "rk-weigh-in".to_owned(),
            event: "Stop".to_owned(),
            problem: "a `prompt` hook, not a command".to_owned()
        }]
    );
}

#[test]
fn a_shell_form_command_fails() {
    let found = replacing_hooks(
        r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"sh ${CLAUDE_PLUGIN_ROOT}/libexec/weighin hook"}]}]}}"#,
    );
    assert_eq!(
        found,
        vec![Violation::HookNotBinary {
            plugin: "rk-weigh-in".to_owned(),
            event: "Stop".to_owned(),
            problem: "`sh ${CLAUDE_PLUGIN_ROOT}/libexec/weighin hook` is not \
                      `${CLAUDE_PLUGIN_ROOT}/libexec/<binary>` with a launcher copy"
                .to_owned()
        }]
    );
}

#[test]
fn a_hook_calling_a_binary_without_a_launcher_fails() {
    let found = replacing_hooks(
        r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"${CLAUDE_PLUGIN_ROOT}/libexec/bench","args":["hook"]}]}]}}"#,
    );
    assert_eq!(
        found,
        vec![Violation::HookNotBinary {
            plugin: "rk-weigh-in".to_owned(),
            event: "Stop".to_owned(),
            problem: "`${CLAUDE_PLUGIN_ROOT}/libexec/bench` is not \
                      `${CLAUDE_PLUGIN_ROOT}/libexec/<binary>` with a launcher copy"
                .to_owned()
        }]
    );
}

#[test]
fn a_hook_that_does_not_run_the_hook_subcommand_fails() {
    let found = replacing_hooks(
        r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"${CLAUDE_PLUGIN_ROOT}/libexec/weighin","args":["batch"]}]}]}}"#,
    );
    assert_eq!(
        found,
        vec![Violation::HookNotBinary {
            plugin: "rk-weigh-in".to_owned(),
            event: "Stop".to_owned(),
            problem: "its arguments do not start with `hook`".to_owned()
        }]
    );
}

#[test]
fn unreadable_hooks_fail() {
    let found = replacing_hooks("{not json");
    assert!(
        matches!(found.as_slice(), [Violation::HooksUnreadable { plugin, .. }] if plugin == "rk-weigh-in"),
        "{found:?}"
    );
}

#[test]
fn a_skill_naming_a_binary_the_plugin_does_not_ship_fails() {
    assert_eq!(
        with(text(
            "skills/plan/SKILL.md",
            "Run `${CLAUDE_PLUGIN_ROOT}/libexec/bench plan check`."
        )),
        vec![Violation::UnknownBinary {
            plugin: "rk-weigh-in".to_owned(),
            path: "skills/plan/SKILL.md".to_owned(),
            binary: "bench".to_owned()
        }]
    );
}

#[test]
fn violations_come_in_plugin_order() {
    let other = Plugin {
        name: "rk-bench".to_owned(),
        folder: true,
        files: vec![file("run.sh")],
    };
    let found = check(&[plugin(vec![file("run.sh")]), other]);
    let plugins: Vec<&str> = found
        .iter()
        .map(|violation| match violation {
            Violation::NoManifest { plugin } | Violation::Unlisted { plugin, .. } => {
                plugin.as_str()
            }
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    assert_eq!(
        plugins,
        ["rk-weigh-in", "rk-weigh-in", "rk-bench", "rk-bench"]
    );
}

#[test]
fn a_mod_folder_in_a_main_plugin_fails() {
    for path in ["hooks/mod/register.tsx", "types/plugin-api.d.ts"] {
        assert_eq!(
            with(file(path)),
            vec![Violation::Unlisted {
                plugin: "rk-weigh-in".to_owned(),
                path: path.to_owned()
            }],
            "{path}"
        );
    }
}

#[test]
fn only_a_shipped_binary_is_a_build() {
    for path in [
        "libexec/x/run.py",
        "libexec/x86_64-unknown-linux-musl/run.py",
        "libexec/x86_64-unknown-linux-musl/bench",
        "libexec/x86_64-unknown-linux-musl/nested/weighin",
    ] {
        assert_eq!(
            with(file(path)),
            vec![Violation::Unlisted {
                plugin: "rk-weigh-in".to_owned(),
                path: path.to_owned()
            }],
            "{path}"
        );
    }
}

#[test]
fn skill_data_naming_an_unknown_binary_fails() {
    assert_eq!(
        with(text(
            "skills/weigh-in/references/steps.json",
            r#"{"run":"libexec/bench plan check"}"#
        )),
        vec![Violation::UnknownBinary {
            plugin: "rk-weigh-in".to_owned(),
            path: "skills/weigh-in/references/steps.json".to_owned(),
            binary: "bench".to_owned()
        }]
    );
}

#[test]
fn an_entry_that_is_not_a_folder_fails_alone() {
    let entry = Plugin {
        folder: false,
        ..plugin(vec![])
    };
    assert_eq!(
        check(&[entry]),
        vec![Violation::NotAFolder {
            plugin: "rk-weigh-in".to_owned()
        }]
    );
}

#[test]
fn the_reader_loads_a_plugin_tree() {
    let root = scratch("reader").expect("a scratch directory");
    tree(
        &root,
        &[
            ("rk-weigh-in/.claude-plugin/plugin.json", "{}"),
            ("rk-weigh-in/hooks/hooks.json", HOOKS),
            (
                "rk-weigh-in/skills/weigh-in/SKILL.md",
                "libexec/weighin open",
            ),
            ("rk-weigh-in/libexec/weighin", "#!/bin/sh"),
            (
                "rk-weigh-in/libexec/x86_64-unknown-linux-musl/weighin",
                "\0elf",
            ),
            ("rk-weigh-in/tools/run.sh", "echo"),
        ],
    )
    .expect("write the tree");

    let plugins = read_plugins(&root).expect("read the tree");

    assert_eq!(
        check(&plugins),
        vec![Violation::Unlisted {
            plugin: "rk-weigh-in".to_owned(),
            path: "tools/run.sh".to_owned()
        }]
    );
    let skill = plugins
        .iter()
        .flat_map(|p| &p.files)
        .find(|f| f.path == "skills/weigh-in/SKILL.md")
        .expect("the skill was read");
    assert_eq!(skill.text.as_deref(), Some("libexec/weighin open"));
}

#[test]
fn the_reader_finds_nothing_when_there_is_no_plugins_folder() {
    let root = scratch("absent")
        .expect("a scratch directory")
        .join("plugins");
    assert_eq!(
        read_plugins(&root).expect("an absent folder is empty"),
        vec![]
    );
}

#[cfg(unix)]
#[test]
fn the_reader_reports_a_symlink_as_not_regular() {
    let outside = scratch("symlink").expect("a scratch directory");
    let root = outside.join("plugins");
    tree(
        &outside,
        &[
            ("plugins/rk-weigh-in/.claude-plugin/plugin.json", "{}"),
            ("plugins/rk-weigh-in/skills/x/README.md", "the skill folder"),
            ("elsewhere.md", "outside the plugin"),
        ],
    )
    .expect("write the tree");
    std::os::unix::fs::symlink(
        outside.join("elsewhere.md"),
        root.join("rk-weigh-in/skills/x/SKILL.md"),
    )
    .expect("create the symlink");
    let plugins = read_plugins(&root).expect("read the tree");
    assert_eq!(
        check(&plugins),
        vec![Violation::NotRegularFile {
            plugin: "rk-weigh-in".to_owned(),
            path: "skills/x/SKILL.md".to_owned()
        }]
    );
}

#[test]
fn this_repository_follows_the_plugin_rules() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    let plugins = read_plugins(&root).expect("read this repository's plugins");
    assert_eq!(check(&plugins), vec![]);
}
