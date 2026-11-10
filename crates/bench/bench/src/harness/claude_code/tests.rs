use std::fs;
use std::path::{Path, PathBuf};

/// Every `.rs` file under `dir`, recursively.
fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).expect("a readable source directory") {
        let path = entry.expect("a readable directory entry").path();
        if path.is_dir() {
            files.extend(rust_files(&path));
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
    files
}

/// The files and directories of the modules `file` declares as `#[cfg(test)] mod name;`.
fn test_module_paths(file: &Path, source: &str) -> Vec<PathBuf> {
    let dir = match file.file_stem().and_then(|stem| stem.to_str()) {
        Some("lib" | "main") => file.parent().expect("in src").to_path_buf(),
        _ => file.with_extension(""),
    };
    let lines: Vec<&str> = source.lines().map(str::trim).collect();
    lines
        .windows(2)
        .filter(|pair| pair.first() == Some(&"#[cfg(test)]"))
        .filter_map(|pair| pair.get(1)?.split("mod ").nth(1)?.strip_suffix(';'))
        .flat_map(|name| [dir.join(format!("{name}.rs")), dir.join(name)])
        .collect()
}

/// The leading whitespace of `line`.
fn indent(line: &str) -> &str {
    line.get(..line.len() - line.trim_start().len())
        .unwrap_or_default()
}

/// The 0-based line range of each inline `#[cfg(test)] mod tests { … }` in `lines`: from the
/// attribute to the `}` at the `mod` line's indentation (or the end of the file).
fn test_module_ranges(lines: &[&str]) -> Vec<std::ops::RangeInclusive<usize>> {
    let mut ranges = Vec::new();
    for (at, pair) in lines.windows(2).enumerate() {
        let is_start = pair
            .iter()
            .map(|line| line.trim())
            .eq(["#[cfg(test)]", "mod tests {"]);
        if !is_start {
            continue;
        }
        let open = pair.get(1).copied().unwrap_or_default();
        let close = lines
            .iter()
            .enumerate()
            .skip(at + 2)
            .find(|(_, line)| line.trim() == "}" && indent(line) == indent(open))
            .map_or_else(|| lines.len().saturating_sub(1), |(end, _)| end);
        ranges.push(at..=close);
    }
    ranges
}

/// The production lines of `source` that name a harness variable, numbered from 1: comment
/// lines and inline `#[cfg(test)] mod tests { … }` blocks skipped.
fn claude_lines(source: &str) -> Vec<(usize, &str)> {
    let lines: Vec<&str> = source.lines().collect();
    let tests = test_module_ranges(&lines);
    (1..)
        .zip(lines)
        .enumerate()
        .filter(|(at, _)| !tests.iter().any(|range| range.contains(at)))
        .map(|(_, numbered)| numbered)
        .filter(|(_, line)| !line.trim_start().starts_with("//") && line.contains("CLAUDE_"))
        .collect()
}

#[test]
fn the_scan_skips_comments_and_inline_test_modules_but_reads_code_after_them() {
    let source = "const A: &str = \"CLAUDE_A\";\n\
                  // const B: &str = \"CLAUDE_B\";\n\
                  #[cfg(test)]\n\
                  mod tests {\n    \
                      const C: &str = \"CLAUDE_C\";\n    \
                      fn f() {\n    \
                      }\n\
                  }\n\
                  const D: &str = \"CLAUDE_D\";\n";

    let found: Vec<usize> = claude_lines(source).into_iter().map(|(n, _)| n).collect();

    assert_eq!(found, [1, 9], "line 1 and the code after the tests module");
}

#[test]
fn the_scan_finds_every_name_in_the_harness_module_itself() {
    let found = claude_lines(include_str!("../claude_code.rs"));

    assert_eq!(
        found.len(),
        7,
        "the seven CLAUDE_ constants, so the scan is not vacuous: {found:?}"
    );
}

#[test]
fn a_declared_test_module_maps_to_its_file_and_directory() {
    let paths = test_module_paths(Path::new("src/foo.rs"), "#[cfg(test)]\nmod tests;\n");

    assert_eq!(
        paths,
        [
            PathBuf::from("src/foo/tests.rs"),
            PathBuf::from("src/foo/tests")
        ]
    );
}

#[test]
fn no_claude_literal_appears_outside_the_harness_module() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let files: Vec<(PathBuf, String)> = rust_files(&src)
        .into_iter()
        .map(|file| {
            let source = fs::read_to_string(&file).expect("a UTF-8 source file");
            (file, source)
        })
        .collect();
    let mut skipped = vec![src.join("harness.rs"), src.join("harness")];
    for (file, source) in &files {
        skipped.extend(test_module_paths(file, source));
    }

    let found: Vec<String> = files
        .iter()
        .filter(|(file, _)| !skipped.iter().any(|root| file.starts_with(root)))
        .flat_map(|(file, source)| {
            let shown = file
                .strip_prefix(&src)
                .expect("under src")
                .display()
                .to_string();
            claude_lines(source)
                .into_iter()
                .map(move |(number, line)| format!("{shown}:{number}: {}", line.trim()))
        })
        .collect();

    assert!(
        found.is_empty(),
        "harness variable names belong in `crate::harness`; found:\n{}",
        found.join("\n")
    );
}

/// The variable Claude Code passes `userConfig.<key>` in.
fn option_var(key: &str) -> String {
    let key: String = key
        .chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' => c.to_ascii_uppercase(),
            _ => '_',
        })
        .collect();
    format!("CLAUDE_PLUGIN_OPTION_{key}")
}

#[test]
fn the_plugin_option_variables_are_their_user_config_keys_upper_cased() {
    let derived = [
        option_var("cf_account_id"),
        option_var("cf_api_token"),
        option_var(super::MODE_KEY),
    ];

    assert_eq!(
        derived,
        [super::ACCOUNT_VAR, super::TOKEN_VAR, super::MODE_VAR]
    );
}

/// What `write` puts in a fresh buffer, as text.
fn written(write: impl FnOnce(&mut Vec<u8>) -> std::io::Result<()>) -> String {
    let mut out = Vec::new();
    write(&mut out).expect("a Vec accepts every write");
    String::from_utf8(out).expect("UTF-8")
}

#[test]
fn a_permission_decision_is_one_pre_tool_use_line_with_its_reason_folded() {
    let deny =
        written(|out| super::write_permission(out, super::PermissionDecision::Deny, "a\r\nb"));
    let ask = written(|out| super::write_permission(out, super::PermissionDecision::Ask, "c"));

    assert_eq!(
        [deny.as_str(), ask.as_str()],
        [
            "{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\
             \"deny\",\"permissionDecisionReason\":\"a  b\"}}\n",
            "{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\
             \"ask\",\"permissionDecisionReason\":\"c\"}}\n",
        ]
    );
}

#[test]
fn a_session_start_output_carries_the_context_for_the_model() {
    let output = written(|out| super::write_session_start(out, "ctx"));

    assert_eq!(
        output,
        "{\"hookSpecificOutput\":{\"hookEventName\":\"SessionStart\",\"additionalContext\":\
         \"ctx\"}}\n"
    );
}

#[test]
fn a_batch_output_leaves_out_the_parts_it_does_not_have() {
    let both = written(|out| super::write_batch(out, Some("why"), Some("ctx".to_owned())));
    let block = written(|out| super::write_batch(out, Some("why"), None));
    let context = written(|out| super::write_batch(out, None, Some("ctx".to_owned())));

    assert_eq!(
        [both.as_str(), block.as_str(), context.as_str()],
        [
            "{\"decision\":\"block\",\"reason\":\"why\",\"hookSpecificOutput\":{\"hookEventName\":\
             \"PostToolBatch\",\"additionalContext\":\"ctx\"}}\n",
            "{\"decision\":\"block\",\"reason\":\"why\"}\n",
            "{\"hookSpecificOutput\":{\"hookEventName\":\"PostToolBatch\",\"additionalContext\":\
             \"ctx\"}}\n",
        ]
    );
}

#[test]
fn a_prompt_output_shows_the_error_and_carries_the_context() {
    let both = written(|out| super::write_prompt(out, Some("ERROR e"), Some("ctx")));
    let error = written(|out| super::write_prompt(out, Some("ERROR e"), None));

    assert_eq!(
        [both.as_str(), error.as_str()],
        [
            "{\"systemMessage\":\"ERROR e\",\"hookSpecificOutput\":{\"hookEventName\":\
             \"UserPromptSubmit\",\"additionalContext\":\"ctx\"}}\n",
            "{\"systemMessage\":\"ERROR e\"}\n",
        ]
    );
}

#[test]
fn a_stop_payload_is_read_from_claude_codes_field_names() {
    let payload = "{\"session_id\":\"s\",\"transcript_path\":\"/t.jsonl\",\"cwd\":\"/w\",\
                   \"stop_hook_active\":true,\"last_assistant_message\":\"done\"}";

    let input: super::StopInput = serde_json::from_str(payload).expect("a Stop payload");

    assert_eq!(
        (
            input.session_id.as_deref(),
            input.transcript.as_deref(),
            input.cwd.as_deref(),
            input.after_block,
            input.last_message.as_deref(),
        ),
        (
            Some("s"),
            Some(Path::new("/t.jsonl")),
            Some(Path::new("/w")),
            true,
            Some("done"),
        )
    );
}

#[test]
fn the_test_keys_are_the_names_the_payload_types_read() {
    use super::keys::{
        HOOK_EVENT_NAME, LAST_ASSISTANT_MESSAGE, STOP_HOOK_ACTIVE, TOOL_INPUT, TOOL_RESPONSE,
        TRANSCRIPT_PATH,
    };
    let stop = serde_json::json!({
        TRANSCRIPT_PATH: "/t",
        STOP_HOOK_ACTIVE: true,
        LAST_ASSISTANT_MESSAGE: "m",
    });
    let call = serde_json::json!({
        HOOK_EVENT_NAME: "PostToolUse",
        "tool_name": "Read",
        TOOL_INPUT: {"a": 1},
        TOOL_RESPONSE: {"b": 2},
    });

    let stop: super::StopInput = serde_json::from_value(stop).expect("a Stop payload");
    let scan: super::ScanInput = serde_json::from_value(call).expect("a PostToolUse payload");

    assert_eq!(
        (
            stop.transcript.as_deref(),
            stop.after_block,
            stop.last_message.as_deref()
        ),
        (Some(Path::new("/t")), true, Some("m"))
    );
    assert_eq!(
        (
            scan.event.as_deref(),
            scan.call.name.as_str(),
            &scan.call.input,
            &scan.call.response
        ),
        (
            Some("PostToolUse"),
            "Read",
            &serde_json::json!({"a": 1}),
            &serde_json::json!({"b": 2})
        )
    );
}

#[test]
fn the_test_keys_are_the_names_the_writers_write() {
    use super::keys::{
        ADDITIONAL_CONTEXT, HOOK_EVENT_NAME_OUT, HOOK_SPECIFIC_OUTPUT, PERMISSION_DECISION,
        SYSTEM_MESSAGE,
    };
    let prompt = written(|out| super::write_prompt(out, Some("e"), Some("c")));
    let permission =
        written(|out| super::write_permission(out, super::PermissionDecision::Deny, "r"));
    let prompt: serde_json::Value = serde_json::from_str(&prompt).expect("JSON");
    let permission: serde_json::Value = serde_json::from_str(&permission).expect("JSON");

    let found = (
        prompt.get(SYSTEM_MESSAGE),
        prompt.pointer(&format!("/{HOOK_SPECIFIC_OUTPUT}/{HOOK_EVENT_NAME_OUT}")),
        prompt.pointer(&format!("/{HOOK_SPECIFIC_OUTPUT}/{ADDITIONAL_CONTEXT}")),
        permission.pointer(&format!("/{HOOK_SPECIFIC_OUTPUT}/{PERMISSION_DECISION}")),
    );

    assert_eq!(
        found,
        (
            Some(&serde_json::json!("e")),
            Some(&serde_json::json!("UserPromptSubmit")),
            Some(&serde_json::json!("c")),
            Some(&serde_json::json!("deny")),
        )
    );
}
