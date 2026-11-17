//! The status block a subagent ends with: the end-of-turn report of SPEC `spec/turns.md`,
//! read by Core ([`turn::read`], ET-02 to ET-07), and the sections Bench requires of it.
//!
//! The report is temper's block (`references/process.md` §5): the last `Status:` line (ET-05),
//! read inside code fences too (ET-06) and through list, quote and heading markers, emphasis
//! (`*`) and code spans, in any letter case (ET-02), then its section lines (ET-07). Bench
//! requires the five sections of [`SECTIONS`]; `Declined` is read (it ends the section above
//! it) but not required. This module turns Core's reading into what `bench.subagent_status`
//! records and what Clef D4 is asked.

use bench_contract::event::AgentStatus;
use redkiln_core::turn::{self, Report, Section, TurnStatus};

/// The sections Bench requires after the `Status:` line, in report order.
pub(crate) const SECTIONS: [Section; 5] = [
    Section::Gate,
    Section::Review,
    Section::Changed,
    Section::DecisionsNeeded,
    Section::NotVerified,
];

/// How many characters of an unknown status the violation shows.
const SHOWN_CHARS: usize = 64;

/// What an unknown status's violation says is expected.
const EXPECTED: &str = "expected DONE, DONE_WITH_CONCERNS, NEEDS_CONTEXT, BLOCKED or WIP";

/// A status line with no word, put before a message that has none, so that Core reads every
/// line of the message as following it: a section anywhere in such a message counts as present.
const BARE_STATUS_LINE: &str = "Status:\n";

/// The status block as read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Parsed {
    /// The status, when the last `Status:` line carries a known one.
    pub(crate) status: Option<AgentStatus>,
    /// What is wrong, in one line, when anything is.
    pub(crate) violation: Option<String>,
    /// The sections not found after the `Status:` line (after nothing, without one).
    pub(crate) missing: Vec<String>,
}

/// Reads the status block of `message`. Never fails: whatever is wrong is the violation.
pub(crate) fn parse(message: &str) -> Parsed {
    let report = turn::read(message);
    let missing = match &report {
        Some(report) => missing_from(report),
        None => missing_anywhere(message),
    };
    let mut problems = Vec::new();
    let status = match report.as_ref().map(Report::status) {
        None => {
            problems.push("no Status: line".to_owned());
            None
        }
        Some(Ok(status)) => Some(agent_status(status)),
        Some(Err(unknown)) => {
            let shown = shortened(unknown.word());
            problems.push(format!("unknown status {shown:?}; {EXPECTED}"));
            None
        }
    };
    if !missing.is_empty() {
        problems.push(format!("missing sections: {}", missing.join(", ")));
    }
    Parsed {
        status,
        violation: (!problems.is_empty()).then(|| problems.join("; ")),
        missing,
    }
}

/// Each section of [`SECTIONS`] with its text in the report of `message`, in report order: the
/// text after its key, then its continuation lines up to a blank line, a code fence, a status
/// line or any registered key (`Declined` among them), joined by spaces (ET-07); empty for a
/// section the report lacks. `None` when `message` is not a report: it has no `Status:` line,
/// or its last one does not carry one of the five words (a prose "Status: …" line, ET-05), as
/// [`parse`] reads it.
pub(crate) fn section_texts(message: &str) -> Option<Vec<(&'static str, String)>> {
    let report = turn::read(message)?;
    if report.status().is_err() {
        return None;
    }
    let texts = SECTIONS
        .into_iter()
        .map(|section| {
            // An absent section is empty text: the documented reading of a missing one.
            let text = report
                .section(section)
                .map_or_else(String::new, str::to_owned);
            (section.key(), text)
        })
        .collect();
    Some(texts)
}

/// What `bench.subagent_status` records for a Core status word: the five map one to one.
fn agent_status(status: TurnStatus) -> AgentStatus {
    match status {
        TurnStatus::Done => AgentStatus::Done,
        TurnStatus::DoneWithConcerns => AgentStatus::DoneWithConcerns,
        TurnStatus::NeedsContext => AgentStatus::NeedsContext,
        TurnStatus::Blocked => AgentStatus::Blocked,
        TurnStatus::Wip => AgentStatus::Wip,
    }
}

/// The keys of the [`SECTIONS`] `report` lacks, in report order.
fn missing_from(report: &Report) -> Vec<String> {
    SECTIONS
        .into_iter()
        .filter(|section| report.section(*section).is_none())
        .map(|section| section.key().to_owned())
        .collect()
}

/// The keys of the [`SECTIONS`] that start no line of `message`, a message with no `Status:`
/// line: there is no report to look after, so the whole message is looked through, under the
/// same key rule (ET-02, ET-03).
fn missing_anywhere(message: &str) -> Vec<String> {
    let whole = format!("{BARE_STATUS_LINE}{message}");
    // `whole` starts with a Status line, so `read` always gives a report; the fallback, every
    // section missing, is only the conservative answer should that ever change.
    turn::read(&whole).map_or_else(
        || SECTIONS.map(|section| section.key().to_owned()).to_vec(),
        |report| missing_from(&report),
    )
}

/// `word`'s first [`SHOWN_CHARS`] characters, with `…` when it is longer, so a violation stays
/// short.
fn shortened(word: &str) -> String {
    let mut chars = word.chars();
    let mut shown: String = chars.by_ref().take(SHOWN_CHARS).collect();
    if chars.next().is_some() {
        shown.push('…');
    }
    shown
}

#[cfg(test)]
mod tests {
    use bench_contract::event::AgentStatus;
    use redkiln_core::turn::TurnStatus;

    use super::{Parsed, agent_status, parse, section_texts};

    const FULL: &str = "\
Gate: fmt ok | clippy ok | test ok (3 passed) | doc ok
Review: approved
Changed: crates/x (+1/-0)
Decisions needed: none
Not verified: none";

    #[test]
    fn each_known_status_with_every_section_is_clean() {
        let statuses = [
            ("DONE", AgentStatus::Done),
            ("DONE_WITH_CONCERNS", AgentStatus::DoneWithConcerns),
            ("NEEDS_CONTEXT", AgentStatus::NeedsContext),
            ("BLOCKED", AgentStatus::Blocked),
            ("WIP", AgentStatus::Wip),
        ];
        for (word, status) in statuses {
            let message = format!("Report.\n\n```text\nStatus: {word}\n{FULL}\n```\n");

            assert_eq!(
                parse(&message),
                Parsed {
                    status: Some(status),
                    violation: None,
                    missing: Vec::new(),
                },
                "{word}: a fenced block counts"
            );
        }
    }

    #[test]
    fn markdown_emphasis_and_list_markers_around_the_keys_are_read_through() {
        let message = "- **Status:** `DONE`\n- **Gate:** green\n- **Review:** approved\n\
                       - **Changed:** a\n- **Decisions needed:** none\n- **Not verified:** none";

        assert_eq!(
            parse(message),
            Parsed {
                status: Some(AgentStatus::Done),
                violation: None,
                missing: Vec::new(),
            },
            "emphasis, code spans and list markers are not part of the key"
        );
    }

    #[test]
    fn missing_sections_are_listed_in_order_and_make_a_violation() {
        let message = "Status: DONE\nGate: ok\nChanged: a\n";

        assert_eq!(
            parse(message),
            Parsed {
                status: Some(AgentStatus::Done),
                violation: Some(
                    "missing sections: Review, Decisions needed, Not verified".to_owned()
                ),
                missing: vec![
                    "Review".to_owned(),
                    "Decisions needed".to_owned(),
                    "Not verified".to_owned(),
                ],
            },
            "the status is kept, the gap recorded"
        );
    }

    #[test]
    fn a_long_unknown_status_is_cut_to_64_characters_on_a_char_boundary() {
        let word = format!("{}{}", "é".repeat(63), "x".repeat(500));

        let parsed = parse(&format!("Status: {word}\n{FULL}"));

        let violation = parsed.violation.unwrap_or_default();
        let shown = format!("unknown status \"{}…\";", "é".repeat(63) + "x");
        assert!(violation.starts_with(&shown), "{violation}");
    }

    #[test]
    fn an_unknown_status_is_a_violation_without_a_status() {
        let parsed = parse(&format!("Status: IN_PROGRESS\n{FULL}"));

        assert_eq!(
            parsed,
            Parsed {
                status: None,
                violation: Some(
                    "unknown status \"IN_PROGRESS\"; expected DONE, DONE_WITH_CONCERNS, \
                     NEEDS_CONTEXT, BLOCKED or WIP"
                        .to_owned()
                ),
                missing: Vec::new(),
            },
            "IN_PROGRESS is not one of the five"
        );
    }

    #[test]
    fn a_message_without_a_status_line_is_a_violation_listing_every_section() {
        let parsed = parse("I looked around; the parser lives in src/parse.rs.");

        assert_eq!(parsed.status, None, "no status");
        assert_eq!(
            parsed.violation.as_deref(),
            Some(
                "no Status: line; missing sections: Gate, Review, Changed, Decisions needed, Not verified"
            ),
            "both gaps in one line"
        );
        assert_eq!(parsed.missing.len(), 5, "every section is missing");
    }

    #[test]
    fn the_last_status_line_wins_and_sections_count_only_after_it() {
        let message = format!("{FULL}\nStatus: BLOCKED\n\nlater:\nStatus: DONE\n{FULL}");

        assert_eq!(
            parse(&message).status,
            Some(AgentStatus::Done),
            "the final block is the report"
        );
        assert_eq!(
            parse(&format!("{FULL}\nStatus: DONE\n")).missing.len(),
            5,
            "sections before the status line belong to something else"
        );
    }

    #[test]
    fn section_texts_reads_each_section_of_the_last_block_in_order() {
        let message = "Earlier: Gate: ignored\n\n```text\nStatus: DONE\nGate: fmt ok | test ok\n\
                       - **Changed:** crates/x (+1/-0)\n  and crates/y\nNot verified: none\n```\n";

        let texts = section_texts(message).expect("a status line");

        assert_eq!(
            texts,
            vec![
                ("Gate", "fmt ok | test ok".to_owned()),
                ("Review", String::new()),
                ("Changed", "crates/x (+1/-0) and crates/y".to_owned()),
                ("Decisions needed", String::new()),
                ("Not verified", "none".to_owned()),
            ],
            "every section in report order, its continuation lines joined, absent ones empty"
        );
    }

    #[test]
    fn section_texts_reads_a_wip_block() {
        let texts = section_texts("Status: WIP: paused for CI\nGate: fmt ok\n").expect("a report");

        assert_eq!(
            texts.first(),
            Some(&("Gate", "fmt ok".to_owned())),
            "temper's WIP is a report, so Clef D4 reads its sections"
        );
    }

    #[test]
    fn section_texts_is_none_without_a_status_line() {
        assert_eq!(
            section_texts("Gate: green\nChanged: a"),
            None,
            "sections without a Status: line are not a report"
        );
    }

    #[test]
    fn section_texts_is_none_when_the_last_status_line_is_prose() {
        let prose = "Status: the build is still running\nGate: green\nChanged: a";
        let prose_last = "Status: DONE\nGate: green\n\nStatus: waiting on CI, see above\n";

        assert_eq!(section_texts(prose), None, "no status word: not a report");
        assert_eq!(
            section_texts(prose_last),
            None,
            "the last Status: line decides, as in `parse`"
        );
    }

    #[test]
    fn a_message_without_a_status_line_lists_only_the_sections_it_lacks() {
        let parsed = parse("- **Gate:** green\nChanged: a\n\nNot verified: none\n");

        assert_eq!(
            parsed,
            Parsed {
                status: None,
                violation: Some(
                    "no Status: line; missing sections: Review, Decisions needed".to_owned()
                ),
                missing: vec!["Review".to_owned(), "Decisions needed".to_owned()],
            },
            "without a Status: line, sections anywhere in the message count as present"
        );
    }

    #[test]
    fn section_texts_stops_decisions_needed_at_a_declined_line() {
        let message = "Status: DONE\nGate: ok\nDecisions needed: none\nDeclined: the unwrap; \
                       a Result instead (H-02)\nNot verified: none\n";

        let texts = section_texts(message).expect("a report");

        assert_eq!(
            texts.get(3),
            Some(&("Decisions needed", "none".to_owned())),
            "Declined: is a registered key (ET-07), so it ends the section above it: {texts:?}"
        );
    }

    #[test]
    fn every_core_word_maps_to_the_agent_status_of_its_name() {
        let expected = [
            AgentStatus::Done,
            AgentStatus::DoneWithConcerns,
            AgentStatus::NeedsContext,
            AgentStatus::Blocked,
            AgentStatus::Wip,
        ];

        let mapped: Vec<AgentStatus> = TurnStatus::ALL.into_iter().map(agent_status).collect();

        assert_eq!(
            mapped, expected,
            "DONE, DONE_WITH_CONCERNS, NEEDS_CONTEXT, BLOCKED, WIP map one to one, in order"
        );
    }

    #[test]
    fn section_texts_reads_a_block_whose_status_word_is_known_in_any_case() {
        let texts = section_texts("Status: blocked: waiting on CI\nGate: fmt ok\n")
            .expect("BLOCKED is a known status");

        assert_eq!(
            texts.first(),
            Some(&("Gate", "fmt ok".to_owned())),
            "{texts:?}"
        );
    }
}
