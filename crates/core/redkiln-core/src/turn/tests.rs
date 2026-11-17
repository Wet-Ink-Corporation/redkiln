use super::{Report, Section, TurnStatus, read};

/// The report's status in `message`, an unknown word as its text; `None` without a report.
fn status_of(message: &str) -> Option<Result<TurnStatus, String>> {
    read(message).map(|report| report.status().map_err(|unknown| unknown.word().to_owned()))
}

/// `message`'s report, which the test's message has.
fn report(message: &str) -> Report {
    read(message).expect("the message has a report")
}

/// `word` in alternating letter case, `DoNe_wItH…`.
fn alternating(word: &str) -> String {
    word.chars()
        .enumerate()
        .map(|(i, c)| {
            if i % 2 == 0 {
                c.to_ascii_uppercase()
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect()
}

#[test]
fn reads_each_status_word_in_any_case() {
    assert_eq!(
        TurnStatus::ALL.map(TurnStatus::word),
        [
            "DONE",
            "DONE_WITH_CONCERNS",
            "NEEDS_CONTEXT",
            "BLOCKED",
            "WIP"
        ],
        "the five words of ET-04, in order"
    );
    for status in TurnStatus::ALL {
        let word = status.word();
        for written in [
            word.to_owned(),
            word.to_ascii_lowercase(),
            alternating(word),
        ] {
            assert_eq!(
                status_of(&format!("Status: {written}\n")),
                Some(Ok(status)),
                "{written}"
            );
        }
    }
}

#[test]
fn a_report_in_a_text_fence_is_read() {
    let message = "Report.\n\n```text\nStatus: DONE\nGate: fmt ok\n```\n";

    assert_eq!(
        status_of(message),
        Some(Ok(TurnStatus::Done)),
        "ET-06: fences do not hide it"
    );
    assert_eq!(
        report(message).section(Section::Gate),
        Some("fmt ok"),
        "the closing fence ends the section"
    );
}

#[test]
fn list_quote_heading_markers_emphasis_and_code_spans_are_read_through() {
    let lines = [
        "- **Status:** `DONE`",
        "+ Status: DONE",
        "> Status: DONE",
        "## Status: DONE",
        "  *Status*: DONE",
        "`Status: DONE`",
        "> - **Status:** DONE",
        "\t-Status: DONE",
    ];
    for line in lines {
        assert_eq!(status_of(line), Some(Ok(TurnStatus::Done)), "{line:?}");
    }
    assert_eq!(
        report("- **Status:** `DONE`\n- **Gate:** `fmt ok`").section(Section::Gate),
        Some("fmt ok"),
        "the section text is taken from the normalised line"
    );
}

#[test]
fn trailing_punctuation_is_trimmed_and_the_rest_is_the_reason() {
    let cases = [
        ("Status: WIP: paused for CI", TurnStatus::Wip),
        ("Status: DONE.", TurnStatus::Done),
        ("Status: BLOCKED! which scope?", TurnStatus::Blocked),
        (
            "Status: needs_context, see the question",
            TurnStatus::NeedsContext,
        ),
        ("Status: DONE_WITH_CONCERNS;", TurnStatus::DoneWithConcerns),
        ("Status:WIP", TurnStatus::Wip),
    ];
    for (line, status) in cases {
        assert_eq!(status_of(line), Some(Ok(status)), "{line:?}");
    }
}

#[test]
fn no_status_line_is_none() {
    let messages = [
        "",
        "I looked around; the parser lives in src/parse.rs.",
        "Gate: green\nChanged: a",
        "The status: fine",
        "Statuses: one",
        "Status DONE",
    ];
    for message in messages {
        assert_eq!(read(message), None, "{message:?}");
    }
}

#[test]
fn an_unknown_word_is_kept_whole_as_unknown_status() {
    let long = format!("{}{}", "é".repeat(63), "x".repeat(500));

    assert_eq!(
        status_of(&format!("Status: {long} and more")),
        Some(Err(long)),
        "the whole first word, however long"
    );
    assert_eq!(
        status_of("Status: IN_PROGRESS."),
        Some(Err("IN_PROGRESS".to_owned())),
        "trailing punctuation is trimmed from an unknown word too"
    );
    assert_eq!(
        status_of("Status:"),
        Some(Err(String::new())),
        "nothing after the colon is an empty unknown word"
    );
}

#[test]
fn the_last_status_line_is_the_report() {
    let message = "Status: BLOCKED\nGate: one\n\nlater:\nStatus: DONE\nGate: two\n";

    assert_eq!(status_of(message), Some(Ok(TurnStatus::Done)), "ET-05");
    assert_eq!(
        report(message).section(Section::Gate),
        Some("two"),
        "the report's own sections"
    );
}

#[test]
fn prose_on_the_last_status_line_is_a_malformed_report() {
    let message = "Status: DONE\nGate: green\n\nStatus: waiting on CI, see above\n";

    assert_eq!(
        status_of(message),
        Some(Err("waiting".to_owned())),
        "ET-D1 A: the earlier DONE is not the report"
    );
}

#[test]
fn sections_count_only_after_the_status_line() {
    let message = "Gate: before\nStatus: DONE\nReview: approved";

    assert_eq!(
        report(message).section(Section::Gate),
        None,
        "before the report"
    );
    assert_eq!(
        report(message).section(Section::Review),
        Some("approved"),
        "after it"
    );
}

#[test]
fn the_first_occurrence_of_a_section_is_read() {
    let message = "Status: DONE\nGate: one\n\nGate: two";

    assert_eq!(report(message).section(Section::Gate), Some("one"));
}

#[test]
fn section_text_joins_continuations_up_to_a_blank_fence_or_key() {
    let cases = [
        ("Status: DONE\nGate: a\n  b\n\n  c", "a b"),
        ("Status: DONE\nGate: a\n- b\n```\nc", "a b"),
        ("Status: DONE\nGate: a\nb\n   ~~~text\nc", "a b"),
        ("Status: DONE\nGate:   a   \n  b  c  \nReview: r", "a b c"),
        ("Status: DONE\nGate: a\n**Not verified:** none", "a"),
    ];
    for (message, text) in cases {
        assert_eq!(
            report(message).section(Section::Gate),
            Some(text),
            "{message:?}"
        );
    }
}

#[test]
fn declined_is_a_key_and_ends_decisions_needed() {
    let message = "Status: DONE\nDecisions needed: none\nDeclined: the unwrap; a Result instead\n\
                   Not verified: none";

    assert_eq!(
        report(message).section(Section::DecisionsNeeded),
        Some("none"),
        "ET-D3 (a)"
    );
    assert_eq!(
        report(message).section(Section::Declined),
        Some("the unwrap; a Result instead")
    );
    assert_eq!(report(message).section(Section::NotVerified), Some("none"));
}

#[test]
fn absent_is_none_and_present_empty_is_some_empty() {
    let message = "Status: DONE\nGate:\nReview: ok";

    assert_eq!(
        report(message).section(Section::Gate),
        Some(""),
        "present, empty"
    );
    assert_eq!(report(message).section(Section::Changed), None, "absent");
}

#[test]
fn sections_are_read_after_a_malformed_status() {
    let message = "Status: IN_PROGRESS\nGate: fmt ok";

    assert_eq!(
        report(message).section(Section::Gate),
        Some("fmt ok"),
        "the report is malformed, its sections are still there"
    );
}

#[test]
fn double_underscore_emphasis_is_not_read_through() {
    assert_eq!(read("__Status:__ DONE"), None, "ET-D3 (b)");
    assert_eq!(read("_Status_: DONE"), None, "single underscores too");
    assert_eq!(
        report("Status: DONE\n__Gate:__ green").section(Section::Gate),
        None,
        "nor around a section key"
    );
}

#[test]
fn section_keys_are_the_six_in_report_order() {
    assert_eq!(
        Section::ALL.map(Section::key),
        [
            "Gate",
            "Review",
            "Changed",
            "Decisions needed",
            "Declined",
            "Not verified"
        ],
        "ET-07"
    );
    assert!(Section::ALL.is_sorted(), "`Ord` follows the report order");
}

#[test]
fn is_honest_exit_is_wip_and_blocked_only() {
    let honest: Vec<TurnStatus> = TurnStatus::ALL
        .into_iter()
        .filter(|status| status.is_honest_exit())
        .collect();

    assert_eq!(honest, [TurnStatus::Blocked, TurnStatus::Wip], "HK-7");
}

/// Properties over generated messages, from a fixed-seed PRNG so a failure reproduces exactly
/// (as `weighin-core`'s properties do, with no property-testing dependency).
mod properties {
    use super::super::{Section, TurnStatus, read};

    /// Cases per property.
    const CASES: u64 = 3000;

    /// Pieces that have broken line readers before, and the grammar's own tokens.
    const FRAGMENTS: &[&str] = &[
        "",
        " ",
        "\t",
        "\n",
        "\n",
        "\n",
        "\r\n",
        "\r",
        "\u{0}",
        "\u{2028}",
        "é",
        "x",
        ":",
        "*",
        "**",
        "`",
        "_",
        "- ",
        "+ ",
        "> ",
        "#",
        "## ",
        "    ",
        "```",
        "```text",
        "~~~",
        "````",
        "Status:",
        "status: wip",
        "Status: DONE",
        "STATUS: blocked.",
        "Status: prose here",
        "Gate:",
        "Review: ok",
        "Changed:",
        "Decisions needed:",
        "Declined:",
        "Not verified:",
        "DONE",
        "WIP",
        "NEEDS_CONTEXT",
        "weigh-in",
    ];

    /// splitmix64 (Steele, Lea and Flood).
    struct Rng(u64);

    impl Rng {
        fn new(seed: u64, case: u64) -> Self {
            Self(seed ^ case.wrapping_mul(0xA076_1D64_78BD_642F))
        }

        fn next_u64(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }

        /// A value in `0..n`; `n` is never 0 here.
        fn below(&mut self, n: usize) -> usize {
            let n = u64::try_from(n).expect("a small bound");
            usize::try_from(self.next_u64() % n).expect("below a usize bound")
        }

        /// Up to `most` fragments, joined.
        fn text(&mut self, most: usize) -> String {
            let count = self.below(most + 1);
            (0..count)
                .map(|_| FRAGMENTS[self.below(FRAGMENTS.len())])
                .collect()
        }
    }

    #[test]
    fn read_never_panics_and_finds_a_report_only_where_a_line_holds_status() {
        for case in 0..CASES {
            let text = Rng::new(0x7E57_0001, case).text(40);

            let unmarked: String = text.chars().filter(|c| !matches!(c, '*' | '`')).collect();
            if !unmarked.to_ascii_lowercase().contains("status:") {
                assert_eq!(read(&text), None, "case {case}: {text:?}");
            }
            let closed = read(&format!("{text}\nStatus: DONE"))
                .unwrap_or_else(|| panic!("case {case}: a final status line is a report"));
            assert_eq!(
                closed.status(),
                Ok(TurnStatus::Done),
                "case {case}: {text:?}"
            );
            for section in Section::ALL {
                assert_eq!(
                    closed.section(section),
                    None,
                    "case {case}: nothing follows"
                );
            }
        }
    }

    #[test]
    fn the_report_is_unchanged_by_text_appended_before_it() {
        for case in 0..CASES {
            let mut rng = Rng::new(0x7E57_0002, case);
            let before = rng.text(20);
            let message = format!("{}\nStatus: {}{}", rng.text(10), rng.text(3), rng.text(30));

            assert_eq!(
                read(&format!("{before}\n{message}")),
                read(&message),
                "case {case}: {before:?} before {message:?}"
            );
        }
    }
}
