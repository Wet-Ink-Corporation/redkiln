//! Reading an atom's frontmatter.
//!
//! The frontmatter is the lines between a first line `---` and the next line `---`, with one
//! leading byte order mark and each line's `\r` passed over. Each `key: value` line is kept; a
//! value written as a JSON string, as [`crate::yaml_str`] writes it, is decoded, and any other
//! value is trimmed. Lines without a `:` are passed over. This reads what the writer writes and
//! hand-edited atoms like it; it is not a YAML parser.

use std::borrow::Cow;

/// An atom's frontmatter as read: its `key: value` lines in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawFrontmatter<'a> {
    lines: Vec<(&'a str, &'a str)>,
}

impl RawFrontmatter<'_> {
    /// The value of the last line with `key`, decoded. `None` when no line has it.
    ///
    /// ```
    /// use knowledge_contract::read::frontmatter;
    /// let fm = frontmatter("---\nkind: decision\ntitle: \"A \\\"b\\\"\"\n---\n");
    /// assert_eq!(fm.as_ref().and_then(|f| f.get("kind")).as_deref(), Some("decision"));
    /// assert_eq!(fm.as_ref().and_then(|f| f.get("title")).as_deref(), Some("A \"b\""));
    /// ```
    #[must_use]
    pub fn get(&self, key: &str) -> Option<Cow<'_, str>> {
        self.lines
            .iter()
            .rev()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| scalar(v))
    }
}

/// The frontmatter of `text`, or `None` when `text` does not open with a `---` line closed by
/// another.
#[must_use]
pub fn frontmatter(text: &str) -> Option<RawFrontmatter<'_>> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.lines().map(|l| l.strip_suffix('\r').unwrap_or(l));
    if lines.next()? != "---" {
        return None;
    }
    let mut kept = Vec::new();
    for line in lines {
        if line == "---" {
            return Some(RawFrontmatter { lines: kept });
        }
        if let Some(pair) = line.split_once(':') {
            kept.push(pair);
        }
    }
    // No closing `---`: not frontmatter.
    None
}

/// A frontmatter scalar: a JSON string decoded; anything else trimmed.
fn scalar(value: &str) -> Cow<'_, str> {
    let v = value.trim();
    if v.starts_with('"')
        && let Ok(s) = serde_json::from_str::<String>(v)
    {
        return Cow::Owned(s);
    }
    Cow::Borrowed(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bom_and_crlf_lines_are_passed_over() {
        let fm = frontmatter("\u{feff}---\r\nkind: decision\r\n---\r\nbody").expect("frontmatter");
        assert_eq!(fm.get("kind").as_deref(), Some("decision"));
    }

    #[test]
    fn the_last_line_with_a_key_wins() {
        let fm = frontmatter("---\nstatus: proposed\nstatus: accepted\n---\n").expect("read");
        assert_eq!(fm.get("status").as_deref(), Some("accepted"));
    }

    #[test]
    fn text_without_closed_frontmatter_has_none() {
        assert_eq!(frontmatter("kind: decision\n"), None);
        assert_eq!(frontmatter("---\nkind: decision\n"), None);
        assert_eq!(frontmatter(""), None);
    }

    #[test]
    fn a_missing_key_and_a_bad_json_string_read_as_written() {
        let fm = frontmatter("---\ntitle: \"unclosed\nnote\n---\n").expect("read");
        assert_eq!(fm.get("title").as_deref(), Some("\"unclosed"));
        assert_eq!(fm.get("note"), None);
        assert_eq!(fm.get("kind"), None);
    }
}
