//! The source write-back of a decision: the line a question came from gets
//! `→ Weigh-In <id>: decided <option> (<date>)`, so it is not raised again. Pure: the line and
//! the decision in, the rewritten line or the reason it was left alone out.
//!
//! Only prose is written into: markdown and plain text, where a note at the end of a line is
//! still text. A code file is never touched. No rule that reads one line at a time keeps every
//! language parsing (multi-line strings, byte strings, regex literals, dollar quotes, PHP's
//! `?>`, JSX text), so the decision of a question raised from code lives only in the ledger,
//! its atom and the commit trailer.

/// How the source file reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Syntax {
    /// Markdown or plain text: the note goes at the end of the line.
    Prose,
    /// Anything else: the file is left as it was.
    Code,
}

/// The decision a note records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Decision<'a> {
    /// The item's `wi-` id.
    pub(crate) id: &'a str,
    /// The option decided, as the owner wrote it.
    pub(crate) option: &'a str,
    /// The day it was decided, `YYYY-MM-DD`.
    pub(crate) date: &'a str,
}

/// Why a source line was left as it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Skip {
    /// The file is code, which is never annotated.
    CodeFile,
    /// The option holds a line break: a control character (`\r`, `\n` and U+0085 included),
    /// U+2028 or U+2029.
    UnsafeText,
}

impl Skip {
    /// The reason, as a receipt shows it.
    pub(crate) fn reason(self) -> &'static str {
        match self {
            Self::CodeFile => "code file: the decision is in the ledger",
            Self::UnsafeText => "the option does not fit on one line",
        }
    }
}

/// `line` of a file that reads as `syntax`, with the note of `decision` at its end, or why it
/// was left alone.
///
/// # Errors
/// [`Skip::CodeFile`] for a code file; [`Skip::UnsafeText`] when the option could end the line.
pub(crate) fn annotate_source_line(
    line: &str,
    decision: &Decision<'_>,
    syntax: Syntax,
) -> Result<String, Skip> {
    match syntax {
        Syntax::Code => Err(Skip::CodeFile),
        Syntax::Prose if decision.option.chars().any(ends_a_line) => Err(Skip::UnsafeText),
        Syntax::Prose => Ok(format!(
            "{line} → Weigh-In {}: decided {} ({})",
            decision.id, decision.option, decision.date
        )),
    }
}

/// Whether `c` could end a line: a control character, or the Unicode line and paragraph
/// separators.
fn ends_a_line(c: char) -> bool {
    c.is_control() || matches!(c, '\u{2028}' | '\u{2029}')
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The decision of `option` on wi-abc123, on 2026-10-07.
    fn decision(option: &str) -> Decision<'_> {
        Decision {
            id: "wi-abc123",
            option,
            date: "2026-10-07",
        }
    }

    #[test]
    fn a_prose_line_gets_the_note_at_the_end() {
        let out = annotate_source_line(
            "- Should we shard by tenant?",
            &decision("B"),
            Syntax::Prose,
        );

        assert_eq!(
            out,
            Ok("- Should we shard by tenant? → Weigh-In wi-abc123: decided B (2026-10-07)".into())
        );
    }

    #[test]
    fn a_code_line_is_left_alone_even_when_it_is_a_whole_comment() {
        for line in [
            "    // TODO: pick a cap",
            "/* TODO: pick a cap */",
            "# TODO: x",
        ] {
            let out = annotate_source_line(line, &decision("4096"), Syntax::Code);

            assert_eq!(out, Err(Skip::CodeFile), "{line}");
        }
    }

    #[test]
    fn a_code_file_is_reported_before_an_unsafe_option() {
        let out = annotate_source_line("// TODO: pick", &decision("a\nb"), Syntax::Code);

        assert_eq!(out, Err(Skip::CodeFile));
    }

    #[test]
    fn an_option_with_a_line_break_is_refused_in_prose() {
        for option in [
            "10\n- Ship it?",
            "a\rb",
            "a\u{85}b",
            "a\u{2028}b",
            "a\u{2029}b",
        ] {
            let out = annotate_source_line("- Q?", &decision(option), Syntax::Prose);

            assert_eq!(out, Err(Skip::UnsafeText), "{option:?}");
        }
    }

    #[test]
    fn an_option_with_punctuation_is_written_into_prose_as_it_is() {
        let out = annotate_source_line("- Q?", &decision("a */ \"b\" --> `c`"), Syntax::Prose);

        assert_eq!(
            out,
            Ok("- Q? → Weigh-In wi-abc123: decided a */ \"b\" --> `c` (2026-10-07)".into())
        );
    }
}
