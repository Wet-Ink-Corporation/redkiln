//! Lower-precision capture from what agents write to disk: bullets under "Open questions"-style
//! headings in documents, and newly written TODO/FIXME/HACK lines in code.
//!
//! Only the text an edit *wrote* is scanned (a `Write`'s content, an `Edit`'s new string), so an
//! existing TODO is not re-raised every time the file is touched.

use serde::{Deserialize, Serialize};

/// A question found by scanning text, not yet raised in the ledger.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    /// The question, cleaned of list markers, comment markers and tags.
    pub question: String,
    /// 1-based line within the scanned text.
    pub line: usize,
    /// The question-style heading the bullet sat under; `None` for a code comment.
    pub heading: Option<String>,
    /// The trimmed source line, kept as the item's verbatim quote.
    pub quote: String,
    /// Found in a code comment (a `TODO`-style tag or "for now" prose) rather than a document.
    pub todo: bool,
}

const HEADINGS: &[&str] = &[
    "open question",
    "open questions",
    "questions",
    "unresolved",
    "open issues",
    "assumptions",
    "assumption",
    "decisions needed",
    "decisions pending",
    "pending decisions",
    "needs decision",
    "needs human",
    "needs your input",
    "for review",
    "questions for",
    "follow-ups",
    "follow ups",
    "to decide",
];

fn heading_level(line: &str) -> Option<(usize, String)> {
    let t = line.trim_start();
    let n = t.chars().take_while(|c| *c == '#').count();
    // The `#` marks are one byte each, so trimming them leaves what `t[n..]` would.
    let rest = t.trim_start_matches('#');
    if (1..=6).contains(&n) && rest.starts_with(' ') {
        let text = rest
            .trim()
            .trim_matches(|c: char| c == '*' || c == '_' || c == ':')
            .trim()
            .to_owned();
        return Some((n, text));
    }
    // Bold pseudo-headings: **Open questions**
    if t.starts_with("**") && t.trim_end().ends_with("**") && t.len() > 4 {
        let text = t
            .trim()
            .trim_matches('*')
            .trim()
            .trim_end_matches(':')
            .to_owned();
        return Some((7, text));
    }
    None
}

fn is_question_heading(text: &str) -> bool {
    let lower = text.to_lowercase();
    let lower = lower.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == ' ');
    HEADINGS.iter().any(|h| {
        lower == *h
            || lower.starts_with(&format!("{h} "))
            || lower.starts_with(&format!("{h}:"))
            || lower.starts_with(&format!("{h} ("))
    })
}

fn bullet_text(line: &str) -> Option<String> {
    let t = line.trim_start();
    let rest = if let Some(r) = t
        .strip_prefix("- ")
        .or_else(|| t.strip_prefix("* "))
        .or_else(|| t.strip_prefix("+ "))
    {
        r
    } else {
        numbered_rest(t)?
    };
    // Checked boxes are resolved; unchecked or plain bullets are candidates.
    if rest.starts_with("[x]") || rest.starts_with("[X]") {
        return None;
    }
    let rest = rest.trim_start_matches("[ ]").trim();
    // Items already routed through Weigh-In carry an arrow annotation.
    if rest.contains("→ Weigh-In") || rest.is_empty() {
        return None;
    }
    Some(rest.to_owned())
}

/// The text after a `1. ` or `1) ` list number, or `None` when `t` does not start with one.
fn numbered_rest(t: &str) -> Option<&str> {
    let after_digits = t.trim_start_matches(|c: char| c.is_ascii_digit());
    if after_digits.len() == t.len() {
        return None;
    }
    after_digits
        .strip_prefix(". ")
        .or_else(|| after_digits.strip_prefix(") "))
}

/// The lines after `from` that wrap the bullet `first`, joined to it with single spaces.
fn join_continuations(lines: &[&str], from: usize, first: String) -> String {
    let mut full = first;
    for next in lines.iter().skip(from) {
        let nt = next.trim_start();
        let nindent = next.len() - nt.len();
        if nt.is_empty()
            || nindent < 2
            || bullet_text(next).is_some()
            || nt.starts_with('#')
            || nt.starts_with("```")
        {
            break;
        }
        full.push(' ');
        full.push_str(nt.trim_end());
    }
    full
}

/// The question a joined bullet asks: its leading `**bold**` phrase when that is long enough to
/// stand alone, else the whole bullet.
fn bullet_question(full: &str) -> String {
    let lead = if full.starts_with("**") {
        full.trim_start_matches("**")
            .split("**")
            .next()
            .unwrap_or(full)
            .trim()
    } else {
        full
    };
    clean(if lead.len() < 8 { full } else { lead })
}

/// Bullets under question-style headings in markdown. A bullet's wrapped continuation lines are
/// joined to it; nested bullets are detail and are skipped.
#[must_use]
pub fn scan_markdown(text: &str) -> Vec<Candidate> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    let mut active: Option<(usize, String)> = None;
    let mut in_code = false;
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            continue;
        }
        if let Some((level, heading)) = heading_level(line) {
            if is_question_heading(&heading) {
                active = Some((level, heading));
            } else if active.as_ref().is_some_and(|(l, _)| level <= *l || *l == 7) {
                active = None;
            }
            continue;
        }
        let Some((_, heading)) = &active else {
            continue;
        };
        let indent = line.len() - line.trim_start().len();
        if indent > 1 {
            continue;
        }
        let Some(first) = bullet_text(line) else {
            continue;
        };
        let full = join_continuations(&lines, i + 1, first);
        out.push(Candidate {
            question: bullet_question(&full),
            line: i + 1,
            heading: Some(heading.clone()),
            quote: line.trim().to_owned(),
            todo: false,
        });
    }
    out
}

const TODO_TAGS: &[&str] = &["TODO", "FIXME", "HACK", "XXX"];

/// Newly written TODO/FIXME/HACK/XXX comments in code. `revisit later` style prose in comments is
/// caught too.
#[must_use]
pub fn scan_todos(text: &str) -> Vec<Candidate> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.contains("→ Weigh-In") {
            continue;
        }
        let Some((comment, tag_tail)) = flagged_comment(line) else {
            continue;
        };
        let body = tag_tail.map_or(comment, after_tag_and_owner);
        let body = clean(body.trim_start_matches(['/', '#', '*', '-', ' ', '<', '!']));
        if body.len() < 4 {
            continue;
        }
        out.push(Candidate {
            question: body,
            line: i + 1,
            heading: None,
            quote: line.trim().to_owned(),
            todo: true,
        });
    }
    out
}

/// Byte offset of the first tag in `upper` (an ASCII-uppercased comment) that stands alone as a
/// word; a first hit inside a longer word hides later ones.
fn tag_offset(upper: &str) -> Option<usize> {
    TODO_TAGS.iter().find_map(|t| {
        upper.find(t).filter(|&p| {
            let before_ok = p
                .checked_sub(1)
                .and_then(|before| upper.as_bytes().get(before))
                .is_none_or(|b| !b.is_ascii_alphanumeric());
            let after = upper.as_bytes().get(p + t.len()).copied();
            before_ok && !after.is_some_and(|c| c.is_ascii_alphanumeric())
        })
    })
}

/// "Revisit later" style prose, which counts as a deferral without a tag.
fn has_prose_hint(comment: &str) -> bool {
    let lower = comment.to_lowercase();
    [
        "revisit later",
        "revisit this",
        "for now",
        "temporary",
        "temporarily",
        "hardcoded for now",
        "come back to",
    ]
    .iter()
    .any(|p| lower.contains(p))
}

/// What follows a tag, minus a `(owner)`: `TODO(owner): ...` carries an owner, which is not part
/// of the question. `tail` starts at the tag.
fn after_tag_and_owner(tail: &str) -> &str {
    let after_tag = tail.trim_start_matches(|c: char| c.is_ascii_alphabetic());
    let after_owner = match after_tag.strip_prefix('(') {
        Some(rest) => rest.split_once(')').map_or(rest, |(_, r)| r),
        None => after_tag,
    };
    after_owner.trim_start_matches([':', ' ', '-'])
}

/// The first comment in `line` that carries a tag or a prose hint, with the text from its tag on
/// when it has one. A block comment that closes without either (`/* verbose */`) does not hide a
/// later comment on the same line; that one counts where it follows whitespace.
fn flagged_comment(line: &str) -> Option<(&str, Option<&str>)> {
    let mut start = comment_start(line)?;
    loop {
        let (comment, after) = up_to_closer(line.get(start..)?);
        // Uppercasing ASCII keeps byte offsets, so an offset found in `upper` is valid in `comment`.
        let tag_tail = tag_offset(&comment.to_ascii_uppercase()).and_then(|p| comment.get(p..));
        if tag_tail.is_some() || has_prose_hint(comment) {
            return Some((comment, tag_tail));
        }
        // `after` is a suffix of `line`, and absent once the comment runs to the end of it.
        let after = after?;
        start = line.len().checked_sub(after.len())? + comment_start_in_code(after)?;
    }
}

/// Markers that start a comment anywhere on a line. A `*` (a wrapped block comment) or a `'` (a
/// VB-style comment) counts only at the start of one: mid-line they are multiplication, string
/// quotes or contractions.
const MID_LINE_COMMENTS: &[&str] = &["//", "#", "--", "/*", "<!--", "REM "];

/// Byte offset where the first comment in `line` starts. A marker inside a double-quoted string is
/// not one (wi-52e471).
fn comment_start(line: &str) -> Option<usize> {
    let body = line.trim_start();
    let indent = line.len().checked_sub(body.len())?;
    if body.starts_with(['*', '\'']) {
        return Some(indent);
    }
    outside_strings(line).find(|&i| starts_a_comment(line, i))
}

/// Byte offset of the next comment in `code`, the rest of a line after a block comment closed on
/// it. That text is code, so a marker counts only where a comment can start on its own: at the
/// start of `code` or after whitespace, and outside a double-quoted string. A marker glued to
/// other text (a URL's `//` or `/#` in a single-quoted string) is not a comment.
fn comment_start_in_code(code: &str) -> Option<usize> {
    outside_strings(code).find(|&i| {
        let after_space = code
            .get(..i)
            .and_then(|before| before.chars().next_back())
            .is_none_or(char::is_whitespace);
        after_space && starts_a_comment(code, i)
    })
}

fn starts_a_comment(text: &str, at: usize) -> bool {
    text.get(at..)
        .is_some_and(|rest| MID_LINE_COMMENTS.iter().any(|m| rest.starts_with(m)))
}

/// Byte offsets of the characters in `text` that are not inside a double-quoted string. A `\`
/// escapes the next character inside one, and a `"` between two `'`, escaped or not (`'"'`,
/// `'\"'`), is a char literal, which opens nothing. Single quotes are not tracked: `'` is a char literal, a lifetime, a string
/// delimiter or an apostrophe depending on the language, and guessing wrong hides a real comment.
/// A stray `"` in prose hides the rest of its line, the cost wi-52e471 accepted.
fn outside_strings(text: &str) -> impl Iterator<Item = usize> + '_ {
    let bytes = text.as_bytes();
    let mut in_string = false;
    let mut escaped = false;
    text.char_indices().filter_map(move |(i, c)| {
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            return None;
        }
        let before = |back: usize| i.checked_sub(back).and_then(|p| bytes.get(p)).copied();
        let opens_char =
            before(1) == Some(b'\'') || (before(1) == Some(b'\\') && before(2) == Some(b'\''));
        let char_literal =
            opens_char && i.checked_add(1).and_then(|n| bytes.get(n)) == Some(&b'\'');
        in_string = c == '"' && !char_literal;
        Some(i)
    })
}

/// `comment` up to the closer of its block comment (`*/`, `-->`), so code after a comment that
/// closes on the same line is not read as part of it, and the text after that closer. Line
/// comments, and block comments left open, are returned whole with nothing after them.
fn up_to_closer(comment: &str) -> (&str, Option<&str>) {
    let (opener, closer) = if comment.starts_with("<!--") {
        ("<!--", "-->")
    } else if comment.starts_with("/*") {
        ("/*", "*/")
    } else if comment.starts_with('*') {
        ("*", "*/")
    } else {
        return (comment, None);
    };
    comment
        .get(opener.len()..)
        .and_then(|inside| inside.find(closer))
        .map(|p| opener.len() + p)
        .and_then(|end| Some((comment.get(..end)?, comment.get(end + closer.len()..)?)))
        .map_or((comment, None), |(cut, after)| (cut, Some(after)))
}

fn clean(s: &str) -> String {
    s.trim()
        .trim_end_matches("*/")
        .trim_end_matches("-->")
        .trim()
        .chars()
        .take(400)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bullets_under_open_questions() {
        let doc = "# Plan\n\n## Steps\n- do a\n\n## Open questions\n- Should we shard by tenant?\n  - detail line\n- [x] Settled one\n- [ ] Which queue, Service Bus or Storage Queues?\n\n## Next\n- not a question\n";
        let c = scan_markdown(doc);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].question, "Should we shard by tenant?");
        assert_eq!(c[0].line, 7);
        assert_eq!(c[1].question, "Which queue, Service Bus or Storage Queues?");
    }

    #[test]
    fn wrapped_bullets_are_joined() {
        let doc = "## Open questions\n\n- **Does compliance require stronger isolation than an in-process\n  key** (e.g. separate caches)? This isolates by key only.\n- Rates change infrequently enough that a short staleness\n  window is acceptable.\n  - nested detail\n";
        let c = scan_markdown(doc);
        assert_eq!(c.len(), 2, "{c:?}");
        assert_eq!(
            c[0].question,
            "Does compliance require stronger isolation than an in-process key"
        );
        assert_eq!(
            c[1].question,
            "Rates change infrequently enough that a short staleness window is acceptable."
        );
        assert_eq!(
            c[0].quote,
            "- **Does compliance require stronger isolation than an in-process"
        );
    }

    #[test]
    fn bold_heading_and_numbered_list() {
        let doc = "**Assumptions:**\n1. Tenants never share a key vault\n2) Retention is per series\n\nOther text";
        let c = scan_markdown(doc);
        assert_eq!(c.len(), 2);
        assert_eq!(c[1].question, "Retention is per series");
    }

    #[test]
    fn annotated_items_are_skipped() {
        let doc =
            "## Open questions\n- Use Postgres? → Weigh-In wi-3fa9c1: decided A (2026-09-26)\n";
        assert!(scan_markdown(doc).is_empty());
    }

    #[test]
    fn code_blocks_are_ignored() {
        let doc = "## Open questions\n```\n- not a bullet\n```\n- real one here\n";
        assert_eq!(scan_markdown(doc).len(), 1);
    }

    #[test]
    fn non_ascii_before_a_todo_does_not_panic_and_annotated_lines_are_skipped() {
        let c = scan_todos(
            "// ııé TODO: decide cache\n// TODO: pick one → Weigh-In wi-22d64d: decided yes (2026-09-27)\n",
        );
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].question, "decide cache");
    }

    #[test]
    fn todos_in_code() {
        let code = "fn a() {\n    // TODO(ryan): decide whether to cache tokens per tenant\n    let x = 1; // hardcoded for now\n    let todos = 3; // counts todos\n}\n# FIXME: handle DST\n";
        let c = scan_todos(code);
        assert_eq!(c.len(), 3, "{c:?}");
        assert_eq!(c[0].question, "decide whether to cache tokens per tenant");
        assert!(c.iter().all(|x| x.todo));
    }
    #[test]
    fn heading_levels_run_from_one_to_six() {
        for (hashes, expected) in [("#", 1), ("###", 3), ("######", 6)] {
            assert_eq!(
                heading_level(&format!("{hashes} Open questions")),
                Some((expected, "Open questions".to_owned()))
            );
        }
        assert_eq!(heading_level("####### Seven"), None);
        assert_eq!(heading_level("#NoSpace"), None);
        assert_eq!(
            heading_level("  ## **Open questions:**"),
            Some((2, "Open questions".to_owned()))
        );
        assert_eq!(
            heading_level("**Assumptions:**"),
            Some((7, "Assumptions".to_owned()))
        );
        assert_eq!(heading_level("****"), None);
    }

    #[test]
    fn bullet_markers_and_numbers() {
        assert_eq!(bullet_text("- a thing"), Some("a thing".to_owned()));
        assert_eq!(bullet_text("* a thing"), Some("a thing".to_owned()));
        assert_eq!(bullet_text("+ a thing"), Some("a thing".to_owned()));
        assert_eq!(bullet_text("10. ten"), Some("ten".to_owned()));
        assert_eq!(bullet_text("7) seven"), Some("seven".to_owned()));
        assert_eq!(bullet_text("12 apples"), None);
        assert_eq!(bullet_text("3.x"), None);
        assert_eq!(bullet_text("3."), None);
        assert_eq!(bullet_text("- [x] done"), None);
        assert_eq!(bullet_text("- [ ] open"), Some("open".to_owned()));
        assert_eq!(bullet_text("1. éé"), Some("éé".to_owned()));
        assert_eq!(bullet_text("é. not a number"), None);
    }

    #[test]
    fn a_short_bold_lead_falls_back_to_the_whole_bullet() {
        let doc = "## Open questions\n- **Why?** Because the whole bullet says more than that.\n";
        let c = scan_markdown(doc);
        assert_eq!(c.len(), 1);
        assert_eq!(
            c[0].question,
            "**Why?** Because the whole bullet says more than that."
        );
    }

    #[test]
    fn todo_tags_need_word_boundaries() {
        assert!(scan_todos("// XTODO: not a tag here\n").is_empty());
        assert!(scan_todos("// TODOS: not a tag here\n").is_empty());
        let c = scan_todos("//TODO: first\n// xxx - second thing\n/* FIXME(ann): third thing */\n");
        let q: Vec<&str> = c.iter().map(|x| x.question.as_str()).collect();
        assert_eq!(q, ["first", "second thing", "third thing"]);
    }

    #[test]
    fn star_comments_only_count_at_the_start_of_a_line() {
        assert!(scan_todos("let a = b * c; TODO later\n").is_empty());
        let c = scan_todos("  * TODO: wrapped javadoc style\n");
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].question, "wrapped javadoc style");
    }

    #[test]
    fn multibyte_text_around_comments_keeps_its_offsets() {
        let c = scan_todos("let s = \"héllo\"; // ünï TODO(é): décide ça\n");
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].question, "décide ça");
        assert_eq!(c[0].line, 1);
    }

    #[test]
    fn prose_comments_are_captured_whole() {
        let c = scan_todos("x = 1  # temporary until the vendor ships\n");
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].question, "temporary until the vendor ships");
    }

    #[test]
    fn code_after_a_closed_comment_is_not_part_of_the_question() {
        let c = scan_todos("    /* TODO: pick a buffer size */ return 0;\n");
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].question, "pick a buffer size");
        let c = scan_todos("<!-- TODO: choose a layout --> <div>\n");
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].question, "choose a layout");
        let c = scan_todos("// TODO: map a --> b */ c\n");
        assert_eq!(
            c[0].question, "map a --> b */ c",
            "line comments have no closer"
        );
    }

    #[test]
    fn a_tag_after_a_closed_block_comment_is_still_found() {
        let c = scan_todos("    call(/* verbose */ false); // TODO: make configurable\n");
        assert_eq!(c.len(), 1, "{c:?}");
        assert_eq!(c[0].question, "make configurable");
        let c = scan_todos("<!-- nav --> <a/> <!-- TODO: pick -->\n");
        assert_eq!(c.len(), 1, "{c:?}");
        assert_eq!(c[0].question, "pick");
        assert!(
            scan_todos("/* a */ x = b * TODO;\n").is_empty(),
            "a mid-line star after a closed comment is not a comment"
        );
    }

    #[test]
    fn a_comment_marker_inside_a_string_after_a_closed_comment_is_not_a_comment() {
        let c = scan_todos("/* TODOx */ let url = \"http://x/# TODO: fix\";\n");
        assert!(c.is_empty(), "{c:?}");
        let c = scan_todos("/* a */ s = \"q\\\"// TODO: no\"; // TODO: keep this\n");
        assert_eq!(c.len(), 1, "{c:?}");
        assert_eq!(c[0].question, "keep this");
    }

    #[test]
    fn a_marker_glued_to_code_after_a_closed_comment_is_not_a_comment() {
        let c = scan_todos("/* TODOx */ const u = 'http://x/# TODO: fix';\n");
        assert!(c.is_empty(), "a single-quoted URL is not a comment: {c:?}");
        let c = scan_todos("/* a */ let c = '\"'; // TODO: real\n");
        assert_eq!(c.len(), 1, "a quote in a char literal hides nothing: {c:?}");
        assert_eq!(c[0].question, "real");
    }

    /// wi-52e471: test fixtures hold comment markers in string literals, and raised 212 items.
    #[test]
    fn a_comment_marker_inside_a_string_is_not_a_comment() {
        for line in [
            "    let src = \"// TODO: decide the cache\";\n",
            "    write(\"x = 1  # FIXME: pick a value\\n\");\n",
            "s = f(\"a\", \"<!-- TODO: layout -->\")\n",
            "    let s = \"say \\\"hi\\\" // TODO: no\";\n",
        ] {
            assert!(
                scan_todos(line).is_empty(),
                "{line:?}: {:?}",
                scan_todos(line)
            );
        }
    }

    #[test]
    fn a_comment_after_a_closed_string_is_still_a_comment() {
        let c = scan_todos("    let s = \"a//b\"; // TODO: pick a name\n");
        assert_eq!(c.len(), 1, "{c:?}");
        assert_eq!(c[0].question, "pick a name");
        let c = scan_todos("    let s = \"\\\"\"; // TODO: escaped quote closes nothing\n");
        assert_eq!(c.len(), 1, "{c:?}");
        let c = scan_todos("    let q = '\"'; // TODO: a quote in a char literal\n");
        assert_eq!(c.len(), 1, "a char literal opens no string: {c:?}");
        let c = scan_todos("    case '\\\"': break; // TODO: an escaped quote in a char literal\n");
        assert_eq!(c.len(), 1, "an escaped char literal opens no string: {c:?}");
        let c = scan_todos("fn f<'a>(x: &'a str) {} // TODO: lifetimes are not quotes\n");
        assert_eq!(c.len(), 1, "{c:?}");
    }

    #[test]
    fn an_apostrophe_is_a_comment_only_at_the_start_of_a_line() {
        assert!(scan_todos("x = \"can't connect for now\"\n").is_empty());
        let c = scan_todos("  ' TODO: vb comment\n");
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].question, "vb comment");
    }
}
