//! The end-of-turn report: the last `Status:` line of a turn's final message and the section
//! lines after it (SPEC `spec/turns.md`, ET-02 to ET-07).
//!
//! [`read`] takes the message's text as given; the caller reads it from wherever its harness
//! keeps it and caps it (HK-6). Reading never fails (ET-09): no report is `None`, and a status
//! line whose word is not one of the five is a report with an [`UnknownStatus`]. Plugin blocks
//! (ET-08) are not read here: their framing is implemented by their one owner, Weigh-In's marker
//! parser, and pinned by the SPEC's turn fixtures.
//!
//! ```
//! use redkiln_core::turn::{Section, TurnStatus, read};
//!
//! let message = "Paused.\n\n```text\nStatus: WIP: waiting on CI\nGate: fmt ok\n```\n";
//! let report = read(message).ok_or("the message has a report")?;
//! assert_eq!(report.status(), Ok(TurnStatus::Wip));
//! assert!(TurnStatus::Wip.is_honest_exit());
//! assert_eq!(report.section(Section::Gate), Some("fmt ok"));
//! assert_eq!(report.section(Section::Review), None);
//! # Ok::<(), &str>(())
//! ```

use std::borrow::Cow;

/// The key of a status line (ET-03).
const STATUS_KEY: &str = "Status";

/// A `Status:` word (ET-04), matched in any ASCII letter case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TurnStatus {
    /// `DONE`: done and verified.
    Done,
    /// `DONE_WITH_CONCERNS`: done, with concerns the caller should read.
    DoneWithConcerns,
    /// `NEEDS_CONTEXT`: cannot go on without context the caller has.
    NeedsContext,
    /// `BLOCKED`: needs a decision it cannot take.
    Blocked,
    /// `WIP`: paused, unfinished, with the reason given.
    Wip,
}

impl TurnStatus {
    /// Every word, in the SPEC's order.
    pub const ALL: [Self; 5] = [
        Self::Done,
        Self::DoneWithConcerns,
        Self::NeedsContext,
        Self::Blocked,
        Self::Wip,
    ];

    /// The word as the SPEC writes it: `DONE`, `DONE_WITH_CONCERNS`, `NEEDS_CONTEXT`, `BLOCKED`
    /// or `WIP`.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Done => "DONE",
            Self::DoneWithConcerns => "DONE_WITH_CONCERNS",
            Self::NeedsContext => "NEEDS_CONTEXT",
            Self::Blocked => "BLOCKED",
            Self::Wip => "WIP",
        }
    }

    /// Whether this is an honest exit a Stop gate stands down on: `WIP` or `BLOCKED` (HK-7).
    #[must_use]
    pub const fn is_honest_exit(self) -> bool {
        matches!(self, Self::Wip | Self::Blocked)
    }

    /// The status `word` names, in any ASCII letter case.
    fn from_word(word: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|status| status.word().eq_ignore_ascii_case(word))
    }
}

/// A registered section key (ET-07). `Ord` is the report order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Section {
    /// `Gate:`, the gate's result.
    Gate,
    /// `Review:`, the review's verdict.
    Review,
    /// `Changed:`, what the turn changed.
    Changed,
    /// `Decisions needed:`, what waits on a person.
    DecisionsNeeded,
    /// `Declined:`, what the agent refused or did differently.
    Declined,
    /// `Not verified:`, what the agent could not check.
    NotVerified,
}

impl Section {
    /// Every key, in report order.
    pub const ALL: [Self; 6] = [
        Self::Gate,
        Self::Review,
        Self::Changed,
        Self::DecisionsNeeded,
        Self::Declined,
        Self::NotVerified,
    ];

    /// The key as the SPEC writes it, without its colon: `Gate`, …, `Not verified`.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Gate => "Gate",
            Self::Review => "Review",
            Self::Changed => "Changed",
            Self::DecisionsNeeded => "Decisions needed",
            Self::Declined => "Declined",
            Self::NotVerified => "Not verified",
        }
    }
}

/// A message's report: its last status line and the sections after it (ET-05, ET-07).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    status: Result<TurnStatus, UnknownStatus>,
    /// The sections found, in report order, each with its text.
    sections: Vec<(Section, String)>,
}

impl Report {
    /// The report's status.
    ///
    /// # Errors
    ///
    /// [`UnknownStatus`] when the status line's word is not one of the five: the message has a
    /// malformed report (ET-05), which a gate reads as neither `WIP` nor `BLOCKED`.
    pub fn status(&self) -> Result<TurnStatus, &UnknownStatus> {
        self.status.as_ref().copied()
    }

    /// `section`'s text: the rest of its line and its continuation lines, joined by single
    /// spaces (ET-07). `None` when the report lacks it, `Some("")` when it is present and empty.
    #[must_use]
    pub fn section(&self, section: Section) -> Option<&str> {
        self.sections
            .iter()
            .find(|(found, _)| *found == section)
            .map(|(_, text)| text.as_str())
    }
}

/// The word of a status line that is not one of the five (ET-05).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownStatus {
    word: String,
}

impl UnknownStatus {
    /// The word as written, whole, with its trailing punctuation trimmed (ET-04); empty when
    /// nothing follows the colon.
    #[must_use]
    pub fn word(&self) -> &str {
        &self.word
    }
}

/// Reads `message`'s report: the last status line (ET-05) and the sections after it. `None` when
/// no line is a status line (ET-03). Code fences do not hide either (ET-06).
#[must_use]
pub fn read(message: &str) -> Option<Report> {
    let lines: Vec<Line<'_>> = message.lines().map(Line::new).collect();
    let (at, value) = lines
        .iter()
        .enumerate()
        .rev()
        .find_map(|(at, line)| line.value(STATUS_KEY).map(|value| (at, value)))?;
    let word = first_word(value);
    let status = TurnStatus::from_word(word).ok_or_else(|| UnknownStatus {
        word: word.to_owned(),
    });
    let rest = lines.get(at.saturating_add(1)..).unwrap_or(&[]);
    let sections = Section::ALL
        .into_iter()
        .filter_map(|section| section_text(rest, section).map(|text| (section, text)))
        .collect();
    Some(Report { status, sections })
}

/// `section`'s text in `rest`, from its first occurrence (ET-07).
fn section_text(rest: &[Line<'_>], section: Section) -> Option<String> {
    let (at, first) = rest
        .iter()
        .enumerate()
        .find_map(|(at, line)| line.value(section.key()).map(|value| (at, value)))?;
    let following = rest
        .iter()
        .skip(at.saturating_add(1))
        .take_while(|line| !line.ends_section())
        .map(|line| line.plain.as_ref());
    let words: Vec<&str> = std::iter::once(first)
        .chain(following)
        .flat_map(str::split_whitespace)
        .collect();
    Some(words.join(" "))
}

/// One line of the message, as written and normalised (ET-02).
struct Line<'m> {
    raw: &'m str,
    plain: Cow<'m, str>,
}

impl<'m> Line<'m> {
    fn new(raw: &'m str) -> Self {
        Self {
            raw,
            plain: normalised(raw),
        }
    }

    /// The text after `<key>:` when the normalised line starts with it, in any ASCII letter
    /// case (ET-03).
    fn value(&self, key: &str) -> Option<&str> {
        let head = self.plain.get(..key.len())?;
        if !head.eq_ignore_ascii_case(key) {
            return None;
        }
        self.plain.get(key.len()..)?.strip_prefix(':')
    }

    /// Whether this line ends the section above it: a blank line, a fence line, a status line
    /// or a registered key (ET-07).
    fn ends_section(&self) -> bool {
        self.plain.trim().is_empty()
            || is_fence(self.raw)
            || self.value(STATUS_KEY).is_some()
            || Section::ALL
                .into_iter()
                .any(|section| self.value(section.key()).is_some())
    }
}

/// `line` with every `*` and `` ` `` removed, then its leading whitespace, `-`, `+`, `>` and
/// `#` (ET-02). Borrowed when the line has no mark to remove.
fn normalised(line: &str) -> Cow<'_, str> {
    if line.contains(['*', '`']) {
        let mut unmarked: String = line.chars().filter(|c| !matches!(c, '*' | '`')).collect();
        let lead = unmarked.len().saturating_sub(without_lead(&unmarked).len());
        unmarked.drain(..lead);
        Cow::Owned(unmarked)
    } else {
        Cow::Borrowed(without_lead(line))
    }
}

/// `text` without its leading whitespace, `-`, `+`, `>` and `#`.
fn without_lead(text: &str) -> &str {
    text.trim_start_matches(|c: char| c.is_whitespace() || matches!(c, '-' | '+' | '>' | '#'))
}

/// Whether `line` is a fence line: its first non-blank characters are three backticks or three
/// tildes (ET-07).
fn is_fence(line: &str) -> bool {
    let line = line.trim_start();
    line.starts_with("```") || line.starts_with("~~~")
}

/// The first whitespace-separated word of `text`, with trailing `.`, `,`, `;`, `:` and `!`
/// trimmed (ET-04); empty when `text` is blank.
fn first_word(text: &str) -> &str {
    text.split_whitespace()
        .next()
        .map_or("", |word| word.trim_end_matches(['.', ',', ';', ':', '!']))
}

#[cfg(test)]
mod tests;
