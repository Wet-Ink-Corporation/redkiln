//! Check (a) of the plugin rules: every subcommand a skill, agent or command names after a shipped
//! binary exists, checked against synthetic plugins and command lists, and this repository against
//! `weighin_contract::cli`.

use std::path::Path;

use xtask::plugins::{
    Cli, FileKind, Plugin, PluginFile, ReadError, Violation, check_commands, read_plugins,
};

const SKILL: &str = "skills/weigh-in/SKILL.md";

/// A command list written for these tests, not the contract's, so no expectation is copied from
/// the list under test.
const WEIGHIN: Cli<'static> = Cli {
    binary: "weighin",
    commands: &[
        "board apply",
        "board export",
        "journal",
        "journal import",
        "open",
        "scan",
    ],
    value_options: &["--dir", "--session"],
};

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
    let mut all = vec![
        file(".claude-plugin/plugin.json"),
        file("libexec/weighin"),
        file("libexec/weighin.cmd"),
    ];
    all.extend(files);
    Plugin {
        name: "rk-weigh-in".to_owned(),
        folder: true,
        files: all,
    }
}

/// The violations of a plugin whose skill is `skill`.
fn skill(skill: &str) -> Vec<Violation> {
    check_commands(&[plugin(vec![text(SKILL, skill)])], &[WEIGHIN])
}

fn unknown(path: &str, command: &str) -> Violation {
    Violation::UnknownCommand {
        plugin: "rk-weigh-in".to_owned(),
        path: path.to_owned(),
        binary: "weighin".to_owned(),
        command: command.to_owned(),
    }
}

#[test]
fn a_known_command_passes() {
    assert_eq!(skill("Run `libexec/weighin open --json`."), vec![]);
}

#[test]
fn an_unknown_command_fails() {
    let found = skill("Run `${CLAUDE_PLUGIN_ROOT}/libexec/weighin frobnicate --json`.");

    assert_eq!(found, vec![unknown(SKILL, "frobnicate")]);
    assert_eq!(
        found.first().map(ToString::to_string).as_deref(),
        Some(
            "rk-weigh-in/skills/weigh-in/SKILL.md: names `weighin frobnicate`, which weighin does \
             not have"
        )
    );
}

#[test]
fn the_binary_by_name_is_an_invocation_too() {
    assert_eq!(skill("`weighin digest`"), vec![unknown(SKILL, "digest")]);
}

#[test]
fn a_two_word_command_must_be_listed() {
    assert_eq!(skill("`weighin board export`"), vec![]);
    assert_eq!(skill("`weighin board`"), vec![unknown(SKILL, "board")]);
    assert_eq!(
        skill("`weighin board frobnicate <dir>`"),
        vec![unknown(SKILL, "board frobnicate")]
    );
}

#[test]
fn an_optional_parent_alone_passes() {
    assert_eq!(
        skill("`weighin journal` or `weighin journal --json`"),
        vec![]
    );
    assert_eq!(skill("`weighin journal import`"), vec![]);
    assert_eq!(
        skill("`weighin journal relay`"),
        vec![unknown(SKILL, "journal relay")]
    );
}

#[test]
fn a_command_without_children_ignores_the_word_after_it() {
    assert_eq!(skill("`weighin scan notes`"), vec![]);
}

#[test]
fn global_options_and_their_values_are_skipped() {
    let line = r#"!`sh "${CLAUDE_PLUGIN_ROOT}/libexec/weighin" --session "${CLAUDE_SESSION_ID}" --dir "${CLAUDE_PROJECT_DIR}" open --with-context --top 8 2>&1 || echo "(weighin did not run here)"`"#;
    assert_eq!(skill(line), vec![]);

    let unknown_after_options = "`weighin --session s --verbose --dir=d --dir x stats`";
    assert_eq!(skill(unknown_after_options), vec![unknown(SKILL, "stats")]);
}

#[test]
fn a_placeholder_is_not_a_command() {
    assert_eq!(skill("`weighin <sub>` and `weighin`"), vec![]);
    assert_eq!(skill("`weighin --session`"), vec![]);
}

#[test]
fn prose_outside_code_spans_is_not_read() {
    assert_eq!(
        skill("If weighin did not run here, libexec/weighin frobnicate is not a command."),
        vec![]
    );
}

#[test]
fn an_unpaired_backtick_opens_no_span() {
    assert_eq!(skill("a ` then weighin frobnicate"), vec![]);
}

#[test]
fn a_double_backtick_span_is_read() {
    assert_eq!(
        skill("``weighin frobnicate `x` ``"),
        vec![unknown(SKILL, "frobnicate")]
    );
}

#[test]
fn every_line_of_a_fenced_block_is_read() {
    let found = skill("```sh\nweighin open\nweighin frobnicate\n```\nweighin digest\n");

    assert_eq!(found, vec![unknown(SKILL, "frobnicate")]);
}

#[test]
fn a_shorter_fence_inside_a_longer_one_does_not_close_it() {
    let found = skill("````md\n```sh\nweighin one\n```\nweighin two\n```` \nweighin three\n");

    assert_eq!(found, vec![unknown(SKILL, "one"), unknown(SKILL, "two")]);
}

#[test]
fn a_fence_closes_only_on_its_own_character_with_nothing_after() {
    let found = skill("~~~\n```\nweighin one\n~~~ sh\nweighin two\n~~~~\nweighin three\n");

    assert_eq!(found, vec![unknown(SKILL, "one"), unknown(SKILL, "two")]);
}

#[test]
fn a_one_line_triple_backtick_span_opens_no_fence() {
    let found = skill("```weighin one```\n`weighin two`\n");

    assert_eq!(found, vec![unknown(SKILL, "one"), unknown(SKILL, "two")]);
}

#[test]
fn an_alias_names_its_binary() {
    let found = skill(
        "`W` below means `sh \"${CLAUDE_PLUGIN_ROOT}/libexec/weighin\" --session \"${CLAUDE_SESSION_ID}\"`. \
         Run `W open`, then `W frobnicate <id>`.",
    );

    assert_eq!(found, vec![unknown(SKILL, "frobnicate")]);
}

#[test]
fn an_alias_holds_in_the_whole_file() {
    assert_eq!(
        skill("First `W digest`.\n\n`W` means `libexec/weighin`."),
        vec![unknown(SKILL, "digest")]
    );
}

#[test]
fn the_first_alias_definition_wins() {
    let bench = Cli {
        binary: "bench",
        commands: &["digest"],
        value_options: &[],
    };
    let files = vec![
        file("libexec/bench"),
        text(
            SKILL,
            "`W` means `weighin --dir d`. Later, `W` means `libexec/bench`. `W digest`.",
        ),
    ];

    let found = check_commands(&[plugin(files)], &[WEIGHIN, bench]);

    assert_eq!(found, vec![unknown(SKILL, "digest")]);
}

#[test]
fn a_word_that_is_not_defined_is_no_alias() {
    assert_eq!(skill("`W` is `weighin`. `W frobnicate`"), vec![]);
}

#[test]
fn agents_commands_and_skill_text_files_are_read_but_assets_are_not() {
    let files = vec![
        text("agents/reviewer.md", "`weighin one`"),
        text("commands/sit.md", "`weighin two`"),
        text("skills/weigh-in/references/notes.txt", "`weighin three`"),
        text("skills/weigh-in/assets/board.html", "`weighin four`"),
        text("skills/weigh-in/references/cards.json", "`weighin five`"),
    ];

    let found = check_commands(&[plugin(files)], &[WEIGHIN]);

    assert_eq!(
        found,
        vec![
            unknown("agents/reviewer.md", "one"),
            unknown("commands/sit.md", "two"),
            unknown("skills/weigh-in/references/notes.txt", "three"),
        ]
    );
}

#[test]
fn a_shipped_binary_without_a_list_fails() {
    let found = check_commands(&[plugin(vec![text(SKILL, "`weighin frobnicate`")])], &[]);

    assert_eq!(
        found,
        vec![Violation::NoCommandList {
            plugin: "rk-weigh-in".to_owned(),
            binary: "weighin".to_owned(),
        }]
    );
    assert_eq!(
        found.first().map(ToString::to_string).as_deref(),
        Some(
            "rk-weigh-in: ships libexec/weighin, but xtask has no command list for weighin (its \
             contract crate's cli module)"
        )
    );
}

#[test]
fn an_unshipped_binary_is_left_to_unknown_binary() {
    let bench = Cli {
        binary: "bench",
        commands: &["run"],
        value_options: &[],
    };

    let found = check_commands(
        &[plugin(vec![text(SKILL, "`libexec/bench frobnicate`")])],
        &[WEIGHIN, bench],
    );

    assert_eq!(found, vec![]);
}

#[test]
fn a_mention_after_another_in_one_span_is_read() {
    assert_eq!(
        skill("`weighin open || weighin frobnicate`"),
        vec![unknown(SKILL, "frobnicate")]
    );
}

#[test]
fn an_entry_that_is_not_a_folder_is_left_to_check() {
    let entry = Plugin {
        name: "stray".to_owned(),
        folder: false,
        files: vec![],
    };

    assert_eq!(check_commands(&[entry], &[WEIGHIN]), vec![]);
}

#[test]
fn this_repository_names_only_existing_subcommands() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    let plugins = read_plugins(&root).expect("read this repository's plugins");
    let weighin = Cli {
        binary: weighin_contract::cli::BINARY,
        commands: weighin_contract::cli::COMMANDS,
        value_options: weighin_contract::cli::VALUE_OPTIONS,
    };

    assert_eq!(check_commands(&plugins, &[weighin]), vec![]);
}

/// The violations of this repository's plugins against weighin's list without `command`.
fn this_repository_without(command: &str) -> Result<Vec<Violation>, ReadError> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    let plugins = read_plugins(&root)?;
    let commands: Vec<&str> = weighin_contract::cli::COMMANDS
        .iter()
        .copied()
        .filter(|&listed| listed != command)
        .collect();
    let weighin = Cli {
        binary: weighin_contract::cli::BINARY,
        commands: &commands,
        value_options: weighin_contract::cli::VALUE_OPTIONS,
    };
    Ok(check_commands(&plugins, &[weighin]))
}

#[test]
fn this_repository_check_reads_the_libexec_path_and_the_alias() -> Result<(), ReadError> {
    let open = Violation::UnknownCommand {
        plugin: "weigh-in".to_owned(),
        path: SKILL.to_owned(),
        binary: "weighin".to_owned(),
        command: "open".to_owned(),
    };

    // SKILL.md names `open` once by its `libexec/weighin` path (the injected ledger line) and
    // twice as `W open`: a check that missed either form would find fewer.
    assert_eq!(this_repository_without("open")?, vec![open; 3]);
    Ok(())
}

#[test]
fn this_repository_check_reads_a_command_named_only_through_the_alias() -> Result<(), ReadError> {
    assert_eq!(
        this_repository_without("digest")?,
        vec![Violation::UnknownCommand {
            plugin: "weigh-in".to_owned(),
            path: SKILL.to_owned(),
            binary: "weighin".to_owned(),
            command: "digest".to_owned(),
        }]
    );
    Ok(())
}

#[test]
fn clap_s_generated_help_passes() {
    assert_eq!(skill("`weighin help` and `weighin help open`"), vec![]);
    // clap also generates `help` under every parent with subcommands.
    assert_eq!(
        skill("`weighin board help` and `weighin board help export`"),
        vec![]
    );
}
