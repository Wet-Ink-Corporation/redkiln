//! `weighin-hook` (#168, the #125 D2 tripwire): a model's Bash call may not run `weighin hook`,
//! however the shell forms the words; weighin's other subcommands, and `hook` as data, stay
//! allowed.

use super::forms::{forms, still_runs};
use super::{ALLOW, RM_RF, assert_table, rule_for};
use crate::harness::claude_code::PLUGIN_ROOT_VAR;
use crate::hook::guard::Rule;

const WEIGHIN_HOOK: Option<Rule> = Some(Rule::WeighinHook);

/// The hook run as `hooks.json` runs it, which a model's Bash call may not.
const BASE: &str = "weighin hook PostToolUse";

#[test]
fn a_weighin_hook_run_is_denied_in_every_wrapping_form() {
    let missed: Vec<String> = forms()
        .into_iter()
        .map(|(name, form)| (name, form(BASE)))
        .filter_map(|(name, command)| {
            let actual = rule_for(&command);
            (actual != WEIGHIN_HOOK).then(|| format!("{name}: {command:?} -> {actual:?}"))
        })
        .collect();

    assert!(missed.is_empty(), "misjudged:\n{}", missed.join("\n"));
}

#[test]
fn a_weighin_hook_run_stays_denied_in_every_pair_of_forms() {
    let forms = forms();
    let mut missed = Vec::new();
    for (inner_name, inner) in &forms {
        let inner = inner(BASE);
        for (outer_name, outer) in &forms {
            if !still_runs(outer_name, &inner) {
                continue;
            }
            let command = outer(&inner);
            if rule_for(&command).is_none() {
                missed.push(format!("{outer_name} of {inner_name}: {command:?}"));
            }
        }
    }

    assert!(missed.is_empty(), "allowed:\n{}", missed.join("\n"));
}

#[test]
fn a_weighin_hook_run_is_denied_however_the_program_and_options_are_written() {
    let rows: Vec<String> = vec![
        "weighin hook".to_owned(),
        "weighin hook PostToolUse < payload.json".to_owned(),
        "echo '{}' | weighin hook PostToolUse".to_owned(),
        "weighin --session s1 --dir /work/proj hook PostToolUse".to_owned(),
        "weighin --store=/tmp/s.db hook".to_owned(),
        "weighin --dir=/work/proj --session \"$S\" hook Stop".to_owned(),
        "weighin --version hook".to_owned(),
        "weighin -- hook".to_owned(),
        "/opt/plugin/libexec/weighin hook PostToolUse".to_owned(),
        format!("\"${{{PLUGIN_ROOT_VAR}}}/libexec/weighin\" hook PostToolUse"),
        "./libexec/weighin.exe hook PostToolUse".to_owned(),
        "weighin.exe hook".to_owned(),
        "WeighIn.EXE hook".to_owned(),
        "C:/plugins/weigh-in/libexec/x86_64-pc-windows-msvc/weighin.exe hook".to_owned(),
        "\"$WEIGHIN\" hook PostToolUse".to_owned(),
        "$(command -v weighin) hook PostToolUse".to_owned(),
        "weighin \"$sub\" PostToolUse".to_owned(),
        "weighin --dir d h\"$rest\"".to_owned(),
        "weighin ho{o,}k".to_owned(),
        "sub=hook; weighin $sub PostToolUse".to_owned(),
        "cat args | xargs weighin".to_owned(),
        "cat args | xargs weighin --dir /work/proj".to_owned(),
        "caffeinate weighin hook PostToolUse".to_owned(),
        "env -i PATH=/bin weighin hook PostToolUse".to_owned(),
        "f() { weighin \"$@\"; }; f hook PostToolUse".to_owned(),
        "bash -c 'weighin --session s hook PostToolUse'".to_owned(),
    ];
    let rows: Vec<(&str, Option<Rule>)> = rows
        .iter()
        .map(|command| (command.as_str(), WEIGHIN_HOOK))
        .collect();

    assert_table(&rows);
}

#[test]
fn weighins_other_subcommands_and_hook_as_data_stay_allowed() {
    assert_table(&[
        ("weighin open", ALLOW),
        ("weighin open --with-context --top 8", ALLOW),
        ("weighin scan", ALLOW),
        ("weighin --session s1 --dir /work/proj open", ALLOW),
        ("weighin --dir hook open", ALLOW),
        ("weighin --store hook status --json", ALLOW),
        ("weighin show hook", ALLOW),
        ("weighin decide wi-123 -- hook", ALLOW),
        ("weighin raise \"$question\" hook", ALLOW),
        ("weighin o\"$x\" hook", ALLOW),
        ("weighin HOOK", ALLOW),
        ("weighin hooks", ALLOW),
        ("cat args | xargs weighin open", ALLOW),
        ("echo weighin hook", ALLOW),
        ("grep -r 'weighin hook' docs", ALLOW),
        ("git commit -m 'weighin hook PostToolUse'", ALLOW),
        ("weighin-other hook", ALLOW),
        ("bench hook guard", ALLOW),
        ("\"$EDITOR\" notes.md", ALLOW),
        // After `--` the next word is the subcommand, so `--version` is no option here.
        ("weighin -- --version hook", ALLOW),
        ("weighin --dir \"$DIR\" open", ALLOW),
    ]);
}

#[test]
fn a_global_option_whose_words_may_split_or_take_a_value_may_hide_hook() {
    assert_table(&[
        // The function's arguments are `--dir`'s value and whatever follows it.
        (
            "f() { weighin --dir \"$@\"; }; f /work/proj hook PostToolUse",
            WEIGHIN_HOOK,
        ),
        // An unquoted value may split into the value and `hook`.
        ("weighin --dir $DIR PostToolUse", WEIGHIN_HOOK),
        ("weighin --store=$S PostToolUse", WEIGHIN_HOOK),
        // A word not read as written may be `--dir`, taking `/work/proj` as its value.
        ("weighin --di\"$r\" /work/proj hook", WEIGHIN_HOOK),
        (
            "weighin \"--$opt\" /work/proj hook PostToolUse",
            WEIGHIN_HOOK,
        ),
    ]);
}

#[test]
fn the_launcher_copy_run_as_a_shell_script_may_not_run_hook() {
    let skill_form = format!(
        "sh \"${{{PLUGIN_ROOT_VAR}}}/libexec/weighin\" --session s --dir /work/proj hook \
         PostToolUse"
    );
    assert_table(&[
        (
            "sh /opt/plugin/libexec/weighin hook PostToolUse",
            WEIGHIN_HOOK,
        ),
        (skill_form.as_str(), WEIGHIN_HOOK),
        ("bash -e ./libexec/weighin hook", WEIGHIN_HOOK),
        ("sh -o posix libexec/weighin hook", WEIGHIN_HOOK),
        ("dash -- libexec/weighin hook", WEIGHIN_HOOK),
        ("CI=1 sh libexec/weighin hook", WEIGHIN_HOOK),
        (
            "bash -c 'sh libexec/weighin hook PostToolUse'",
            WEIGHIN_HOOK,
        ),
        ("f() { sh libexec/weighin hook; }; f", WEIGHIN_HOOK),
        ("sh libexec/weighin open", ALLOW),
        ("sh libexec/weighin --dir hook open", ALLOW),
        ("sh script.sh weighin hook", ALLOW),
        ("sh -c 'echo weighin hook'", ALLOW),
        ("sh \"$SCRIPT\" hook", ALLOW),
    ]);
}

#[test]
fn the_launcher_copy_run_as_a_script_behind_a_wrapper_may_not_run_hook() {
    assert_table(&[
        (
            "sudo sh /opt/plugin/libexec/weighin hook PostToolUse",
            WEIGHIN_HOOK,
        ),
        (
            "env sh /opt/plugin/libexec/weighin hook PostToolUse",
            WEIGHIN_HOOK,
        ),
        (
            "exec sh /opt/plugin/libexec/weighin hook PostToolUse",
            WEIGHIN_HOOK,
        ),
        ("sudo -u root bash -e libexec/weighin hook", WEIGHIN_HOOK),
        ("env -i PATH=/bin sh libexec/weighin hook", WEIGHIN_HOOK),
        ("nice -n 5 sh libexec/weighin hook", WEIGHIN_HOOK),
        ("timeout 5 sh libexec/weighin hook", WEIGHIN_HOOK),
        ("command sh libexec/weighin hook", WEIGHIN_HOOK),
        ("find . -exec sh libexec/weighin hook {} +", WEIGHIN_HOOK),
        // xargs appends the subcommand at run time.
        ("cat args | xargs sh libexec/weighin", WEIGHIN_HOOK),
        // A shell option word read as options in one reading leaves the launcher the script.
        ("sh \"$o\" libexec/weighin hook", WEIGHIN_HOOK),
        // `source` hands the launcher its arguments as a shell does.
        ("source libexec/weighin hook PostToolUse", WEIGHIN_HOOK),
        (". libexec/weighin hook", WEIGHIN_HOOK),
        // A shell the engine does not unwrap has its arguments read as a command.
        ("posh libexec/weighin hook", WEIGHIN_HOOK),
        ("sudo sh libexec/weighin open", ALLOW),
        ("env sh libexec/weighin --dir hook open", ALLOW),
        ("cat args | xargs sh libexec/weighin open", ALLOW),
        ("source libexec/weighin open", ALLOW),
        ("sudo sh -c 'echo weighin hook'", ALLOW),
    ]);
}

#[test]
fn the_launcher_path_built_by_brace_expansion_may_not_run_hook() {
    assert_table(&[
        (
            "sh {/opt/plugin/libexec/weighin,hook} PostToolUse",
            WEIGHIN_HOOK,
        ),
        ("sudo sh {/opt/plugin/libexec/weighin,hook}", WEIGHIN_HOOK),
        ("sh /opt/plugin/libexec/weighin {hook,open}", WEIGHIN_HOOK),
        ("sh {/opt/plugin/libexec/weighin,open}", ALLOW),
    ]);
}

#[test]
fn a_shell_whose_readings_find_the_launcher_many_times_is_judged_whole() {
    // `$o` may be an option taking any number of values, so every `weighin` below is the
    // script in some reading, each with the rest as its arguments (#181 review, H-13).
    let pairs = vec!["libexec/weighin --dir"; 2_000].join(" ");
    let then_rm = format!("sh $o {pairs} x; rm -rf /");
    let with_hook = format!("sh $o {pairs} libexec/weighin hook");
    let without = format!("sh $o {pairs} x");
    assert_table(&[
        (then_rm.as_str(), RM_RF),
        (with_hook.as_str(), WEIGHIN_HOOK),
        (without.as_str(), ALLOW),
    ]);
}
