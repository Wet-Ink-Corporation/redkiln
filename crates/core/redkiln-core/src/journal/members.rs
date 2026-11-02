//! The top-level members of a JSON object, as spans of its text, so a line can gain or lose a
//! key while every other byte, the payload's own key order and number forms included, is kept.
//!
//! The text has already been read by `serde_json` (strictly, no key named twice), so this only
//! has to find the boundaries of valid JSON; anything it cannot follow is `None`.

use std::ops::Range;

/// One member: its key, decoded, and the span of `"key":value` in the text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Member {
    pub(super) key: String,
    pub(super) span: Range<usize>,
}

/// The members of the object `text` holds, in order.
pub(super) fn members(text: &str) -> Option<Vec<Member>> {
    let bytes = text.as_bytes();
    let mut at = skip_space(bytes, 0);
    if bytes.get(at) != Some(&b'{') {
        return None;
    }
    at = skip_space(bytes, at.checked_add(1)?);
    let mut found = Vec::new();
    if bytes.get(at) == Some(&b'}') {
        return Some(found);
    }
    loop {
        let start = at;
        let key_end = skip_string(bytes, at)?;
        let key: String = serde_json::from_str(text.get(start..key_end)?).ok()?;
        at = skip_space(bytes, key_end);
        if bytes.get(at) != Some(&b':') {
            return None;
        }
        at = skip_space(bytes, at.checked_add(1)?);
        let end = skip_value(bytes, at)?;
        found.push(Member {
            key,
            span: start..end,
        });
        at = skip_space(bytes, end);
        match bytes.get(at) {
            Some(b',') => at = skip_space(bytes, at.checked_add(1)?),
            Some(b'}') => return Some(found),
            _ => return None,
        }
    }
}

fn skip_space(bytes: &[u8], mut at: usize) -> usize {
    while matches!(bytes.get(at), Some(b' ' | b'\t' | b'\n' | b'\r')) {
        at = at.saturating_add(1);
    }
    at
}

/// The end of the string starting at `at` (just past its closing quote).
fn skip_string(bytes: &[u8], at: usize) -> Option<usize> {
    if bytes.get(at) != Some(&b'"') {
        return None;
    }
    let mut i = at.checked_add(1)?;
    loop {
        match bytes.get(i)? {
            b'\\' => i = i.checked_add(2)?,
            b'"' => return i.checked_add(1),
            _ => i = i.checked_add(1)?,
        }
    }
}

/// The end of the value starting at `at`.
fn skip_value(bytes: &[u8], at: usize) -> Option<usize> {
    match bytes.get(at)? {
        b'"' => skip_string(bytes, at),
        b'{' | b'[' => {
            let mut depth = 0_usize;
            let mut i = at;
            loop {
                match bytes.get(i)? {
                    b'"' => {
                        i = skip_string(bytes, i)?;
                        continue;
                    }
                    b'{' | b'[' => depth = depth.checked_add(1)?,
                    b'}' | b']' => {
                        depth = depth.checked_sub(1)?;
                        if depth == 0 {
                            return i.checked_add(1);
                        }
                    }
                    _ => {}
                }
                i = i.checked_add(1)?;
            }
        }
        _ => {
            let mut i = at;
            while !matches!(
                bytes.get(i),
                None | Some(b',' | b'}' | b']' | b' ' | b'\t' | b'\n' | b'\r')
            ) {
                i = i.checked_add(1)?;
            }
            (i > at).then_some(i)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spans(text: &str) -> Vec<(String, &str)> {
        members(text)
            .expect("an object")
            .into_iter()
            .map(|m| (m.key, text.get(m.span).expect("in the text")))
            .collect()
    }

    #[test]
    fn members_are_found_with_their_exact_text() {
        let text = r#" { "a" : 1.50 , "b\u0022":{"x":[1,"]}",{"y":"\\"}]},"d\u0061ta":{"k":"v"} , "n":null,"t":true } "#;
        assert_eq!(
            spans(text),
            [
                ("a".to_owned(), r#""a" : 1.50"#),
                ("b\"".to_owned(), r#""b\u0022":{"x":[1,"]}",{"y":"\\"}]}"#),
                ("data".to_owned(), r#""d\u0061ta":{"k":"v"}"#),
                ("n".to_owned(), r#""n":null"#),
                ("t".to_owned(), r#""t":true"#),
            ]
        );
        assert_eq!(spans("{}"), []);
    }

    #[test]
    fn what_is_not_an_object_has_no_members() {
        for text in [
            "[]",
            "1",
            r#"{"a"}"#,
            r#"{"a":1"#,
            r#"{"a":"#,
            "",
            r#"{"a":1,}"#,
        ] {
            assert_eq!(members(text), None, "{text}");
        }
    }
}
