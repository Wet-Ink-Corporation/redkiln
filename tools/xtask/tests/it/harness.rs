//! Check (b): Claude Code's hook protocol is named only in a binary's harness adapter (SPEC
//! CLI-13; root `AGENTS.md`, CLI first, harness second), against synthetic sources and this
//! repository.

use std::path::Path;

use xtask::harness::{SourceFile, Violation, check, read_sources};

fn source(path: &str, text: &str) -> SourceFile {
    SourceFile {
        path: path.to_owned(),
        text: text.to_owned(),
    }
}

/// A binary crate's root, which makes its `src/harness` an adapter.
fn main_of(krate: &str) -> SourceFile {
    source(&format!("crates/{krate}/src/main.rs"), "fn main() {}\n")
}

fn outside(path: &str, line: usize, token: &str) -> Violation {
    Violation::OutsideAdapter {
        path: path.to_owned(),
        line,
        token: token.to_owned(),
    }
}

#[test]
fn a_token_outside_an_adapter_fails() {
    let files = [
        main_of("weigh-in/weighin"),
        source(
            "crates/weigh-in/weighin/src/hooks.rs",
            "fn run() {}\nlet key = \"hookSpecificOutput\";\n",
        ),
    ];

    let found = check(&files);

    assert_eq!(
        found,
        [outside(
            "crates/weigh-in/weighin/src/hooks.rs",
            2,
            "hookSpecificOutput"
        )]
    );
}

#[test]
fn the_violation_names_the_file_line_token_and_rule() {
    let violation = outside("crates/a/b/src/hooks.rs", 7, "systemMessage");

    assert_eq!(
        violation.to_string(),
        "crates/a/b/src/hooks.rs:7: names systemMessage, Claude Code's hook protocol, outside \
         the crate's harness adapter (src/harness/); CLI-13"
    );
}

#[test]
fn the_adapter_module_and_its_folder_pass() {
    let files = [
        main_of("weigh-in/weighin"),
        source(
            "crates/weigh-in/weighin/src/harness.rs",
            "const KEY: &str = \"hook_event_name\";\n",
        ),
        source(
            "crates/weigh-in/weighin/src/harness/claude_code.rs",
            "let v = std::env::var(\"CLAUDE_CODE_ENTRYPOINT\");\n",
        ),
        source(
            "crates/weigh-in/weighin/src/harness/claude_code/tests.rs",
            "let k = \"additionalContext\";\n",
        ),
    ];

    assert_eq!(check(&files), []);
}

#[test]
fn an_adapter_path_in_a_crate_without_main_fails() {
    let files = [
        source("crates/core/redkiln-core/src/lib.rs", "pub mod harness;\n"),
        source(
            "crates/core/redkiln-core/src/harness.rs",
            "const KEY: &str = \"systemMessage\";\n",
        ),
    ];

    let found = check(&files);

    assert_eq!(
        found,
        [outside(
            "crates/core/redkiln-core/src/harness.rs",
            1,
            "systemMessage"
        )]
    );
}

#[test]
fn another_crates_main_does_not_make_an_adapter() {
    let files = [
        main_of("weigh-in/weighin"),
        source(
            "crates/weigh-in/weighin-core/src/harness.rs",
            "let k = \"tool_input\";\n",
        ),
    ];

    let found = check(&files);

    assert_eq!(
        found,
        [outside(
            "crates/weigh-in/weighin-core/src/harness.rs",
            1,
            "tool_input"
        )]
    );
}

#[test]
fn integration_tests_and_comment_lines_pass() {
    let files = [
        source(
            "crates/weigh-in/weighin/tests/it/cli.rs",
            "let p = json!({\"hook_event_name\": \"Stop\"});\n",
        ),
        source(
            "crates/core/redkiln-core/src/location.rs",
            "//! never `CLAUDE_PROJECT_DIR`\n    /// `CLAUDE_SESSION_ID` gives it\n  // \
             systemMessage\nfn f() {}\n",
        ),
    ];

    assert_eq!(check(&files), []);
}

#[test]
fn a_trailing_comment_is_still_checked() {
    let files = [source(
        "crates/core/redkiln-core/src/ids.rs",
        "fn f() {} // reads CLAUDE_SESSION_ID\n",
    )];

    let found = check(&files);

    assert_eq!(
        found,
        [outside(
            "crates/core/redkiln-core/src/ids.rs",
            1,
            "CLAUDE_SESSION_ID"
        )]
    );
}

#[test]
fn a_unit_test_module_as_rustfmt_writes_it_passes() {
    let text = "pub fn f() {}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {\n        \
                let v = \"${CLAUDE_SESSION_ID}\";\n    }\n}\n";
    let files = [source("crates/core/redkiln-core/src/ids/session.rs", text)];

    assert_eq!(check(&files), []);
}

#[test]
fn code_after_a_unit_test_module_is_checked() {
    let text =
        "#[cfg(test)]\nmod tests {\n    fn t() {}\n}\n\nconst K: &str = \"systemMessage\";\n";
    let files = [source("crates/core/redkiln-core/src/a.rs", text)];

    let found = check(&files);

    assert_eq!(
        found,
        [outside(
            "crates/core/redkiln-core/src/a.rs",
            6,
            "systemMessage"
        )]
    );
}

#[test]
fn a_test_module_with_no_closing_line_is_checked_to_the_end() {
    let text = "#[cfg(test)]\nmod tests {\n    const K: &str = \"systemMessage\"; }\n";
    let files = [source("crates/core/redkiln-core/src/a.rs", text)];

    let found = check(&files);

    assert_eq!(
        found,
        [outside(
            "crates/core/redkiln-core/src/a.rs",
            3,
            "systemMessage"
        )]
    );
}

#[test]
fn an_indented_test_module_is_checked() {
    let text = "mod outer {\n    #[cfg(test)]\n    mod tests {\n        const K: &str = \
                \"systemMessage\";\n    }\n}\n";
    let files = [source("crates/core/redkiln-core/src/a.rs", text)];

    let found = check(&files);

    assert_eq!(
        found,
        [outside(
            "crates/core/redkiln-core/src/a.rs",
            4,
            "systemMessage"
        )]
    );
}

#[test]
fn each_token_is_found() {
    let tokens = [
        "hook_event_name",
        "last_assistant_message",
        "tool_input",
        "tool_response",
        "stop_hook_active",
        "transcript_path",
        "hookSpecificOutput",
        "hookEventName",
        "additionalContext",
        "systemMessage",
        "permissionDecision",
        "remote_cowork",
        "CLAUDE_X",
        "CLAUDE_CODE_ENTRYPOINT",
    ];
    for token in tokens {
        let files = [source(
            "crates/a/b/src/lib.rs",
            &format!("let k = \"{token}\";\n"),
        )];

        let found = check(&files);

        assert_eq!(
            found,
            [outside("crates/a/b/src/lib.rs", 1, token)],
            "{token}"
        );
    }
}

#[test]
fn near_misses_are_not_tokens() {
    let near = [
        "CLAUDE_",
        "MY_CLAUDE",
        "MY_CLAUDE_X",
        "CLAUDE_x",
        "tool_inputs",
        "my_tool_input",
        "systemMessages",
        "session_id",
        "cwd",
        "tool_name",
        "agent_id",
        "source",
    ];
    for text in near {
        let files = [source(
            "crates/a/b/src/lib.rs",
            &format!("let k = \"{text}\";\n"),
        )];

        assert_eq!(check(&files), [], "{text}");
    }
}

#[test]
fn the_first_token_on_a_line_is_reported_once() {
    let files = [source(
        "crates/a/b/src/lib.rs",
        "json!({\"systemMessage\": m, \"hookSpecificOutput\": o});\n",
    )];

    let found = check(&files);

    assert_eq!(
        found,
        [outside("crates/a/b/src/lib.rs", 1, "systemMessage")]
    );
}

#[test]
fn violations_come_in_path_then_line_order() {
    let files = [
        source("crates/b/c/src/lib.rs", "\"systemMessage\"\n"),
        source(
            "crates/a/c/src/lib.rs",
            "x\n\"tool_input\"\n\"tool_response\"\n",
        ),
    ];

    let found = check(&files);

    assert_eq!(
        found,
        [
            outside("crates/a/c/src/lib.rs", 2, "tool_input"),
            outside("crates/a/c/src/lib.rs", 3, "tool_response"),
            outside("crates/b/c/src/lib.rs", 1, "systemMessage"),
        ]
    );
}

#[test]
fn this_repository_keeps_harness_protocol_in_adapters() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates");
    let files = read_sources(&crates).expect("read this repository's crates");
    assert!(
        files
            .iter()
            .any(|f| f.path == "crates/weigh-in/weighin/src/main.rs"),
        "the weighin binary is read"
    );

    let found: Vec<String> = check(&files).iter().map(ToString::to_string).collect();

    assert_eq!(found, Vec::<String>::new());
}
