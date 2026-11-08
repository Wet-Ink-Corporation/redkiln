//! Each SPEC example that has a golden equals it byte for byte: the text inside the fenced block
//! after the requirement's paragraph is the golden without its final `\n`.

use super::{one_line, spec_dir};

/// The first fenced block after the line starting with `anchor` in `spec/<file>`, without its
/// final newline.
fn example(file: &str, anchor: &str) -> String {
    let text = std::fs::read_to_string(spec_dir().join(file)).expect("the SPEC section");
    let mut lines = text
        .lines()
        .skip_while(|line| !line.starts_with(anchor))
        .skip_while(|line| !line.starts_with("```"));
    let fence = lines
        .next()
        .unwrap_or_else(|| panic!("{file}: {anchor} has an example"));
    assert!(
        fence == "```json" || fence == "```text",
        "{file}: {anchor}: {fence}"
    );
    let block: Vec<&str> = lines.take_while(|line| !line.starts_with("```")).collect();
    block.join("\n")
}

/// Asserts the example after `anchor` in `file` is the golden `rel`.
fn is_golden(file: &str, anchor: &str, rel: &str) {
    assert_eq!(
        example(file, anchor),
        String::from_utf8(one_line(rel)).expect("UTF-8"),
        "{file} {anchor} and {rel}"
    );
}

#[test]
fn ev_01_example_is_its_golden() {
    is_golden(
        "events.md",
        "**EV-01",
        "envelope/valid/ev-01-item-decided.json",
    );
}

#[test]
fn ap_01_example_is_its_golden() {
    is_golden(
        "approvals.md",
        "**AP-01",
        "envelope/valid/ap-01-approval-recorded.json",
    );
}

#[test]
fn jr_02_example_is_its_golden() {
    is_golden(
        "journals.md",
        "**JR-02",
        "journal/valid/jr-02-item-raised.jsonl",
    );
}

#[test]
fn dr_02_example_is_its_golden() {
    // DR-02's example follows DR-03's paragraph.
    is_golden(
        "decision-requests.md",
        "**DR-03",
        "decision-request/valid/dr-02.json",
    );
}

#[test]
fn cli_02_example_is_its_golden() {
    is_golden("cli.md", "**CLI-02", "cli/valid/gate-failed.json");
}
