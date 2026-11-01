//! The `weigh-in` fenced block agents emit when they assume, defer, make an unexamined call, or
//! cannot decide:
//!
//! ````text
//! ```weigh-in
//! kind: assumption          # question | assumption | deferral | call
//! door: one-way             # one-way | two-way | unsure
//! authority: authority      # authority | judgment
//! q: Should retention overrides attach per record or per series?
//! did: per-series, matching RetentionPolicy; per-record not built
//! flip: a tenant needs per-record legal hold
//! at: src/Retention/RetentionPolicy.cs:88
//! blocks: none
//! ```
//! ````
//!
//! Parsing is forgiving about case, spacing and trailing comments, and strict about one thing:
//! a block without `q:` is not an item.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// What the agent is telling the owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// The agent could not decide and asks. The default for an unrecognised value.
    #[default]
    Question,
    /// The agent proceeded on an assumption the owner should confirm.
    Assumption,
    /// The agent left something undone on purpose, or a `TODO`-style deferral.
    Deferral,
    /// The agent made an unexamined call and logs it.
    Call,
}

impl Kind {
    /// Read a `kind:` value, ignoring case and surrounding space. Infallible: anything
    /// unrecognised is [`Kind::Question`].
    #[must_use]
    pub fn parse(v: &str) -> Self {
        match v.trim().to_ascii_lowercase().as_str() {
            "assumption" | "assumed" => Self::Assumption,
            "deferral" | "deferred" | "defer" | "todo" => Self::Deferral,
            "call" | "decision" => Self::Call,
            _ => Self::Question,
        }
    }

    /// The kebab-case name used in the marker block and in the ledger.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Question => "question",
            Self::Assumption => "assumption",
            Self::Deferral => "deferral",
            Self::Call => "call",
        }
    }
}

/// How reversible the choice is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Door {
    /// Hard or impossible to undo.
    OneWay,
    /// Cheap to undo.
    TwoWay,
    /// The agent could not say. The default for an unrecognised value.
    #[default]
    Unsure,
}

impl Door {
    /// Read a `door:` value, ignoring case and treating `_` and spaces as `-`. Infallible:
    /// anything unrecognised is [`Door::Unsure`].
    #[must_use]
    pub fn parse(v: &str) -> Self {
        let v = v.trim().to_ascii_lowercase().replace(['_', ' '], "-");
        match v.as_str() {
            "one-way" | "oneway" | "1-way" | "irreversible" => Self::OneWay,
            "two-way" | "twoway" | "2-way" | "reversible" => Self::TwoWay,
            _ => Self::Unsure,
        }
    }

    /// The kebab-case name used in the marker block and in the ledger.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OneWay => "one-way",
            Self::TwoWay => "two-way",
            Self::Unsure => "unsure",
        }
    }
}

/// Whose call the question is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Authority {
    /// Needs the human's authority: security, money, data retention, migrations, product calls,
    /// or intent only the user knows.
    #[default]
    Authority,
    /// A judgment the agent is entitled to make and log.
    Judgment,
}

impl Authority {
    /// Read an `authority:` value, ignoring case and surrounding space. Infallible: anything
    /// unrecognised is [`Authority::Authority`], erring towards asking the owner.
    #[must_use]
    pub fn parse(v: &str) -> Self {
        match v.trim().to_ascii_lowercase().as_str() {
            "judgment" | "judgement" | "agent" => Self::Judgment,
            _ => Self::Authority,
        }
    }

    /// The kebab-case name used in the marker block and in the ledger.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Authority => "authority",
            Self::Judgment => "judgment",
        }
    }
}

/// One parsed `weigh-in` block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Marker {
    /// The `kind:` field.
    pub kind: Kind,
    /// The `door:` field.
    pub door: Door,
    /// The `authority:` field.
    pub authority: Authority,
    /// The `q:` (or `question:`) field, never empty.
    pub question: String,
    /// What the agent did meanwhile (`did:`); absent when empty, `none` or `-`.
    pub did: Option<String>,
    /// The fact that would change the answer (`flip:`); absent when empty, `none` or `-`.
    pub flip: Option<String>,
    /// Where in the code the question arose (`at:`), as `path:line`.
    pub at: Option<String>,
    /// The symbol the question concerns (`sym:` or `symbol:`).
    pub sym: Option<String>,
    /// What the open question blocks (`blocks:`); absent when `none`.
    pub blocks: Option<String>,
    /// Keys this parser does not know, kept verbatim.
    pub extra: BTreeMap<String, String>,
    /// The block exactly as written, for the item's verbatim quote.
    pub raw: String,
}

impl Marker {
    /// `did: blocked` means the agent took no one-way path and continued with other work.
    #[must_use]
    pub fn is_blocked(&self) -> bool {
        self.did.as_deref().is_some_and(did_is_blocked)
    }
}

/// Whether a `did` value says the agent was blocked.
///
/// It starts with the word "blocked", alone or followed by an explanation (`blocked; left the
/// guard as it is`). Such an item stays a question and is never logged as a call the agent made
/// (wi-64c8ea).
#[must_use]
pub fn did_is_blocked(did: &str) -> bool {
    const WORD: &str = "blocked";
    let did = did.trim_start();
    did.get(..WORD.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(WORD))
        && !did
            .get(WORD.len()..)
            .and_then(|rest| rest.chars().next())
            .is_some_and(|c| c.is_alphanumeric() || c == '_')
}

/// Parse every `weigh-in` (or `weighin`) fenced block in `text`.
///
/// A block without a `q:` field is not an item and is skipped.
///
/// # Examples
///
/// ````
/// use weighin_core::marker::{Authority, Door, Kind, parse_markers};
///
/// let reply = r#"Migration done.
///
/// ```weigh-in
/// kind: assumption
/// door: one-way
/// authority: authority
/// q: Should retention overrides attach per record or per series?
/// did: per-series, matching RetentionPolicy
/// flip: a tenant needs per-record legal hold   # why it might be wrong
/// at: src/Retention/RetentionPolicy.cs:88
/// blocks: none
/// ```
/// "#;
///
/// let markers = parse_markers(reply);
/// assert_eq!(markers.len(), 1);
/// let marker = markers.first().ok_or("one block was written")?;
/// assert_eq!(marker.kind, Kind::Assumption);
/// assert_eq!(marker.door, Door::OneWay);
/// assert_eq!(marker.authority, Authority::Authority);
/// assert_eq!(marker.flip.as_deref(), Some("a tenant needs per-record legal hold"));
/// assert_eq!(marker.blocks, None, "`none` reads as absent");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ````
#[must_use]
pub fn parse_markers(text: &str) -> Vec<Marker> {
    let mut out = Vec::new();
    let mut unclosed = Unclosed::default();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();
        let Some(fence) = Fence::open(trimmed) else {
            continue;
        };
        if fence.is_weigh_in() {
            let body: Vec<&str> = lines
                .by_ref()
                .take_while(|inner| !fence.is_closed_by(inner))
                .collect();
            if let Some(m) = parse_body(&body) {
                out.push(m);
            }
            continue;
        }
        // Any other fence is consumed whole, so a `weigh-in` block shown inside it (a ````markdown
        // example) is its content, not an item. Two departures keep later blocks capturable.
        // CommonMark: a line indented four spaces or more is indented code, not a fence. And,
        // unlike CommonMark, which runs an unclosed fence to the end of the document, a fence with
        // no closer is read as an ordinary line: a stray or truncated example must not swallow a
        // real block after it.
        if indent_opens_fence(line, trimmed) && !unclosed.rules_out(&fence) {
            let mut rest = lines.clone();
            if rest.any(|inner| fence.is_closed_by(inner)) {
                lines = rest;
            } else {
                unclosed.record(&fence);
            }
        }
    }
    out
}

/// Whether the indentation before a fence's marks (`line` minus `trimmed`) lets it open one: at
/// most three spaces, as `CommonMark` allows.
fn indent_opens_fence(line: &str, trimmed: &str) -> bool {
    line.strip_suffix(trimmed)
        .is_some_and(|indent| indent.len() <= 3 && indent.bytes().all(|b| b == b' '))
}

/// The shortest backtick and tilde fences already found to have no closer in the rest of the
/// text. A later fence of the same mark, at least as long, has none either (its closer would have
/// closed the earlier one), so each run of unclosed fences is scanned to the end only once.
#[derive(Default)]
struct Unclosed {
    backtick: Option<usize>,
    tilde: Option<usize>,
}

impl Unclosed {
    fn slot(&mut self, mark: char) -> &mut Option<usize> {
        if mark == '`' {
            &mut self.backtick
        } else {
            &mut self.tilde
        }
    }

    fn rules_out(&mut self, fence: &Fence<'_>) -> bool {
        self.slot(fence.mark).is_some_and(|len| fence.len >= len)
    }

    fn record(&mut self, fence: &Fence<'_>) {
        let slot = self.slot(fence.mark);
        *slot = Some(slot.map_or(fence.len, |len| len.min(fence.len)));
    }
}

/// An opening code fence: three or more backticks or tildes, then an info string.
struct Fence<'a> {
    mark: char,
    len: usize,
    info: &'a str,
}

impl<'a> Fence<'a> {
    fn open(line: &'a str) -> Option<Self> {
        let mark = line.chars().next()?;
        if mark != '`' && mark != '~' {
            return None;
        }
        let len = line.chars().take_while(|c| *c == mark).count();
        if len < 3 {
            return None;
        }
        // The fence marks are one byte each, so trimming them leaves what `line[len..]` would.
        let rest = line.trim_start_matches(mark);
        // CommonMark: a backtick fence's info string has no backticks; such a line is inline code.
        if mark == '`' && rest.contains('`') {
            return None;
        }
        let info = rest.split_whitespace().next().unwrap_or("");
        Some(Self { mark, len, info })
    }

    fn is_weigh_in(&self) -> bool {
        ["weigh-in", "weighin", "weigh_in"]
            .iter()
            .any(|name| self.info.eq_ignore_ascii_case(name))
    }

    /// The `CommonMark` rule: a fence closes on a line of the same mark, at least as many of them,
    /// and nothing else but whitespace.
    fn is_closed_by(&self, line: &str) -> bool {
        let t = line.trim();
        t.chars().all(|c| c == self.mark) && t.chars().count() >= self.len
    }
}

fn strip_comment(v: &str) -> &str {
    // A comment is a ` #` followed by whitespace or the end of the value; a `#` inside a value
    // (C#, a#b) or starting a word (#123) is kept.
    v.match_indices(" #")
        .find(|(i, m)| {
            v.get(i + m.len()..)
                .and_then(|rest| rest.chars().next())
                .is_none_or(char::is_whitespace)
        })
        .and_then(|(i, _)| v.get(..i))
        .unwrap_or(v)
        .trim_end()
}

/// Collect `key: value` lines into a map; an indented or key-less line continues the value above.
fn collect_fields(body: &[&str]) -> BTreeMap<String, String> {
    let mut fields: BTreeMap<String, String> = BTreeMap::new();
    let mut last_key: Option<String> = None;
    for raw in body {
        if raw.trim().is_empty() || raw.trim_start().starts_with('#') {
            continue;
        }
        if let Some((k, v)) = raw.split_once(':') {
            let key = k.trim().to_ascii_lowercase();
            if !key.is_empty()
                && key
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                && !raw.starts_with(char::is_whitespace)
            {
                fields.insert(key.clone(), strip_comment(v.trim()).to_owned());
                last_key = Some(key);
                continue;
            }
        }
        // Continuation line: append to the previous value.
        if let Some(k) = &last_key
            && let Some(v) = fields.get_mut(k)
        {
            v.push(' ');
            v.push_str(raw.trim());
        }
    }
    fields
}

fn parse_body(body: &[&str]) -> Option<Marker> {
    let mut fields = collect_fields(body);
    let question = fields
        .remove("q")
        .or_else(|| fields.remove("question"))?
        .trim()
        .to_owned();
    if question.is_empty() {
        return None;
    }
    let take = |f: &mut BTreeMap<String, String>, k: &str| {
        f.remove(k)
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty() && !v.eq_ignore_ascii_case("none") && v != "-")
    };
    let kind = fields
        .remove("kind")
        .map(|v| Kind::parse(&v))
        .unwrap_or_default();
    let door = fields
        .remove("door")
        .map(|v| Door::parse(&v))
        .unwrap_or_default();
    let authority = fields
        .remove("authority")
        .map(|v| Authority::parse(&v))
        .unwrap_or_default();
    let did = take(&mut fields, "did");
    let flip = take(&mut fields, "flip");
    let at = take(&mut fields, "at");
    let sym = take(&mut fields, "sym").or_else(|| take(&mut fields, "symbol"));
    let blocks = take(&mut fields, "blocks");
    Some(Marker {
        kind,
        door,
        authority,
        question,
        did,
        flip,
        at,
        sym,
        blocks,
        extra: fields,
        raw: body.join("\n"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "Done with the migration.\n\n```weigh-in\nkind: assumption          # question | assumption\ndoor: one-way\nauthority: authority\nq: Should retention overrides attach per record or per series?\ndid: per-series, matching RetentionPolicy; per-record not built\nflip: a tenant needs per-record legal hold\nat: src/Retention/RetentionPolicy.cs:88\nblocks: none\n```\n\nAnd a normal code block:\n```rust\nfn main() {}\n```\n";

    #[test]
    fn parses_the_documented_block() {
        let m = parse_markers(SAMPLE);
        assert_eq!(m.len(), 1);
        let m = &m[0];
        assert_eq!(m.kind, Kind::Assumption);
        assert_eq!(m.door, Door::OneWay);
        assert_eq!(m.authority, Authority::Authority);
        assert_eq!(
            m.question,
            "Should retention overrides attach per record or per series?"
        );
        assert_eq!(m.at.as_deref(), Some("src/Retention/RetentionPolicy.cs:88"));
        assert_eq!(m.blocks, None, "`none` reads as absent");
        assert!(m.did.as_deref().unwrap().starts_with("per-series"));
    }

    #[test]
    fn a_block_without_q_is_not_an_item() {
        assert!(parse_markers("```weigh-in\nkind: call\ndid: x\n```").is_empty());
    }

    #[test]
    fn several_blocks_and_tildes() {
        let t = "~~~weighin\nq: first?\n~~~\ntext\n````weigh-in\nq: second?\ndoor: two-way\nauthority: judgment\n````";
        let m = parse_markers(t);
        assert_eq!(m.len(), 2);
        assert_eq!(m[1].door, Door::TwoWay);
        assert_eq!(m[1].authority, Authority::Judgment);
    }

    #[test]
    fn keeps_hash_inside_values_and_unknown_keys() {
        let m =
            &parse_markers("```weigh-in\nq: Port the C# client or wrap it?\nowner: ryan\n```")[0];
        assert_eq!(m.question, "Port the C# client or wrap it?");
        assert_eq!(m.extra.get("owner").map(String::as_str), Some("ryan"));
    }

    #[test]
    fn continuation_lines_join() {
        let m = &parse_markers("```weigh-in\nq: A long question\n  that wraps?\n```")[0];
        assert_eq!(m.question, "A long question that wraps?");
    }

    #[test]
    fn fences_need_three_marks_and_a_matching_close() {
        assert!(parse_markers("``weigh-in\nq: two ticks?\n``").is_empty());
        let m = parse_markers("  ```weigh-in extra words\nq: indented?\n```");
        assert_eq!(m.len(), 1);
        let m = parse_markers("~~~~ weigh_in\nq: tilde?\n```\nstill inside\n~~~~~\nafter");
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].question, "tilde? ``` still inside");
        assert!(parse_markers("```\nq: no info string?\n```").is_empty());
        assert!(parse_markers("```rust\nq: wrong language?\n```").is_empty());
    }

    #[test]
    fn comments_are_stripped_only_after_whitespace() {
        let m = &parse_markers(
            "```weigh-in\nq: Port C# or Rust?   # why not\ndid: done # note\nflip:   #\nat: a#b\n```",
        )[0];
        assert_eq!(m.question, "Port C# or Rust?");
        assert_eq!(m.did.as_deref(), Some("done"));
        assert_eq!(m.flip.as_deref(), Some("#"));
        assert_eq!(m.at.as_deref(), Some("a#b"));
    }

    #[test]
    fn issue_refs_are_not_comments() {
        let m = &parse_markers(
            "```weigh-in\nq: Should we fix #123 before #124?\nflip: if #76 recurs\n```",
        )[0];
        assert_eq!(m.question, "Should we fix #123 before #124?");
        assert_eq!(m.flip.as_deref(), Some("if #76 recurs"));
        let m = &parse_markers("```weigh-in\nq: Pick A? # a comment\ndid: A #\n```")[0];
        assert_eq!(m.question, "Pick A?");
        assert_eq!(m.did.as_deref(), Some("A"));
    }

    #[test]
    fn blocks_inside_an_outer_fence_are_examples() {
        assert!(
            parse_markers("````markdown\n```weigh-in\nq: example?\n```\n````\nafter").is_empty()
        );
        assert!(parse_markers("~~~~text\n```weigh-in\nq: example?\n```\n~~~~").is_empty());
        assert!(parse_markers("```text\n~~~weigh-in\nq: example?\n~~~\n```").is_empty());
        let m = parse_markers(
            "````markdown\n```weigh-in\nq: example?\n```\n````\n```weigh-in\nq: real?\n```",
        );
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].question, "real?");
        let m = parse_markers("```rust\nlet x = 1;\n```\n```weigh-in\nq: real?\n```");
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].question, "real?");
        // As CommonMark renders it: a ```weigh-in line cannot close an open ```rust fence
        // (a closer carries no info), so the block is that fence's content.
        assert!(parse_markers("```rust\nlet x;\n```weigh-in\nq: lost?\n```").is_empty());
    }

    #[test]
    fn an_outer_fence_indented_four_spaces_is_indented_code() {
        let m = parse_markers("    ```rust\n    let x = 1;\n\n```weigh-in\nq: real?\n```");
        assert_eq!(m.len(), 1, "{m:?}");
        assert_eq!(m[0].question, "real?");
    }

    #[test]
    fn an_outer_fence_that_never_closes_does_not_hide_later_blocks() {
        let m = parse_markers("````markdown\nan example\n```weigh-in\nq: real?\n```\nafter");
        assert_eq!(m.len(), 1, "{m:?}");
        assert_eq!(m[0].question, "real?");
        let m = parse_markers("~~~~rust\n~~~~rust\n```weigh-in\nq: real?\n```");
        assert_eq!(m.len(), 1, "{m:?}");
        assert_eq!(m[0].question, "real?");
    }

    #[test]
    fn a_shorter_fence_after_an_unclosed_longer_one_is_still_a_fence() {
        let m = parse_markers("`````md\n```rust\nx\n```\n```weigh-in\nq: real?\n```");
        assert_eq!(m.len(), 1, "{m:?}");
        assert_eq!(m[0].question, "real?");
        let m = parse_markers("`````md\n```md\n```weigh-in\nq: example?\n```\n```\nafter");
        assert!(
            m.is_empty(),
            "the shorter fence closes and hides its example: {m:?}"
        );
    }

    #[test]
    fn a_closed_outer_fence_still_hides_its_example() {
        let m = parse_markers("  ````markdown\n```weigh-in\nq: example?\n```\n   ````\nafter");
        assert!(m.is_empty(), "{m:?}");
    }

    #[test]
    fn a_backtick_line_with_backticks_in_its_info_is_not_a_fence() {
        let m = parse_markers("```x``` is inline code\n```weigh-in\nq: real?\n```");
        assert_eq!(m.len(), 1, "{m:?}");
        assert_eq!(m[0].question, "real?");
        let m = parse_markers("~~~x`y\nq: inside a tilde fence\n~~~\n```weigh-in\nq: real?\n```");
        assert_eq!(m.len(), 1, "a tilde fence may carry backticks in its info");
    }

    #[test]
    fn unterminated_blocks_run_to_the_end() {
        let m = parse_markers("```weigh-in\nq: open ended?\nkind: call");
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].kind, Kind::Call);
        assert_eq!(m[0].raw, "q: open ended?\nkind: call");
    }

    #[test]
    fn blocked_is_recognised() {
        let m = &parse_markers("```weigh-in\nq: Drop the column?\ndid: blocked\n```")[0];
        assert!(m.is_blocked());
    }

    #[test]
    fn blocked_followed_by_an_explanation_is_still_blocked() {
        for did in [
            "blocked",
            " Blocked ",
            "blocked; left the guard as it is",
            "BLOCKED - waiting on the owner",
            "blocked: no access",
            "blocked, nothing built",
        ] {
            assert!(did_is_blocked(did), "{did:?}");
        }
        for did in [
            "",
            "blockedness checked",
            "unblocked the queue",
            "left blocked",
        ] {
            assert!(!did_is_blocked(did), "{did:?}");
        }
        let m = &parse_markers("```weigh-in\nq: Drop the column?\ndid: blocked; kept it\n```")[0];
        assert!(m.is_blocked());
    }
}

/// Words that mark a question as authority-bound whatever the agent classified it as.
///
/// A hit keeps a "two-way, judgment" marker open for the sitting instead of logging it as a
/// default: the cheap deterministic half of a two-key rule, erring towards asking. Most entries
/// are stems matched at the start of a word; the acronyms must be whole words
/// ([`authority_tripwire`]).
pub const AUTHORITY_WORDS: &[&str] = &[
    "compliance",
    "compliant",
    "regulat",
    "gdpr",
    "hipaa",
    "fedramp",
    "cjis",
    "pii",
    "personal data",
    "privacy",
    "security",
    "secret",
    "credential",
    "encrypt",
    "permission",
    "access control",
    "retention",
    "residency",
    "legal",
    "contract",
    "licen",
    "sla",
    "billing",
    "pricing",
    "payment",
    "invoice",
    "migration",
    "migrate",
    "breaking change",
    "public api",
    "schema change",
    "delete data",
    "data loss",
    "production",
    "audit",
    "tenant isolation",
    "isolation",
    "customer data",
];

/// The [`AUTHORITY_WORDS`] that are acronyms, matched as whole words (a plural `s` allowed). The
/// rest are stems matched at a word start, so `regulat` catches `regulated` and `licen` both
/// spellings of licence; an acronym as a prefix would fire on ordinary words (`sla` in `slack`).
const WHOLE_WORDS: &[&str] = &["sla", "pii", "gdpr", "hipaa", "fedramp", "cjis"];

/// The first authority word found in `text`, if any, matched at the start of a word; the
/// acronyms among them must also end one.
#[must_use]
pub fn authority_tripwire(text: &str) -> Option<&'static str> {
    let lower = text.to_lowercase();
    let is_boundary = |b: Option<&u8>| b.is_none_or(|b| !b.is_ascii_alphanumeric());
    AUTHORITY_WORDS.iter().copied().find(|w| {
        lower.match_indices(w).any(|(i, _)| {
            let starts = is_boundary(i.checked_sub(1).and_then(|p| lower.as_bytes().get(p)));
            starts && (!WHOLE_WORDS.contains(w) || ends_word(lower.get(i + w.len()..)))
        })
    })
}

/// Whether `rest`, the text after an acronym, ends the word there or after a plural `s`.
fn ends_word(rest: Option<&str>) -> bool {
    let rest = rest.unwrap_or("");
    let rest = rest.strip_prefix('s').unwrap_or(rest);
    rest.bytes()
        .next()
        .is_none_or(|b| !b.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tripwire_tests {
    use super::{AUTHORITY_WORDS, WHOLE_WORDS, authority_tripwire};

    #[test]
    fn catches_authority_words_at_word_starts() {
        assert_eq!(
            authority_tripwire("What TTL and tenant-isolation strategy?"),
            Some("isolation")
        );
        assert_eq!(
            authority_tripwire("Is compliance a concern?"),
            Some("compliance")
        );
        assert_eq!(authority_tripwire("Name the helper apply or run?"), None);
        assert_eq!(
            authority_tripwire("reproduction steps"),
            None,
            "`production` must start a word"
        );
    }

    #[test]
    fn the_character_before_a_match_decides_whether_it_starts_a_word() {
        assert_eq!(authority_tripwire("privacy first"), Some("privacy"));
        assert_eq!(authority_tripwire("x-privacy"), Some("privacy"));
        assert_eq!(authority_tripwire("éprivacy"), Some("privacy"));
        assert_eq!(authority_tripwire("xprivacy"), None);
        assert_eq!(authority_tripwire("9privacy"), None);
        assert_eq!(authority_tripwire("a legal and an xlegal"), Some("legal"));
        assert_eq!(authority_tripwire(""), None);
    }

    #[test]
    fn acronyms_match_only_as_whole_words() {
        for text in [
            "Name the slash command /sync or /pull?",
            "Post to Slack?",
            "slate gray",
            "a slab of text",
            "piiece",
        ] {
            assert_eq!(authority_tripwire(text), None, "{text}");
        }
        for text in ["Check the SLA", "an SLA.", "two SLAs for the API", "sla"] {
            assert_eq!(authority_tripwire(text), Some("sla"), "{text}");
        }
        assert_eq!(authority_tripwire("Log the PII?"), Some("pii"));
        assert_eq!(authority_tripwire("GDPR-safe?"), Some("gdpr"));
    }

    #[test]
    fn every_whole_word_is_an_authority_word() {
        for w in WHOLE_WORDS {
            assert!(AUTHORITY_WORDS.contains(w), "{w} is not in AUTHORITY_WORDS");
        }
    }

    #[test]
    fn stems_still_match_their_longer_forms() {
        assert_eq!(authority_tripwire("Is it regulated?"), Some("regulat"));
        assert_eq!(authority_tripwire("Which licence?"), Some("licen"));
        assert_eq!(authority_tripwire("Encrypted at rest?"), Some("encrypt"));
        assert_eq!(
            authority_tripwire("Rotate the credentials?"),
            Some("credential")
        );
        assert_eq!(authority_tripwire("Run the migrations?"), Some("migration"));
        assert_eq!(authority_tripwire("Who migrated it?"), Some("migrate"));
    }
}
