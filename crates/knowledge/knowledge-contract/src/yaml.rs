//! YAML scalars for frontmatter values.
//!
//! Strings are written as JSON strings, which are valid YAML double-quoted scalars, so no value
//! can break the frontmatter, and a reader decodes them with a JSON parser (`read`).

/// A YAML-safe double-quoted scalar.
///
/// JSON escaping covers C0 controls, quotes and backslashes; YAML also refuses DEL, the C1
/// controls, U+FFFE/U+FFFF and treats U+2028/U+2029 as line breaks, so those are escaped too.
///
/// ```
/// assert_eq!(knowledge_contract::yaml_str("a \"b\""), r#""a \"b\"""#);
/// assert_eq!(knowledge_contract::yaml_str("a\u{7f}b"), r#""a\u007fb""#);
/// ```
#[must_use]
pub fn yaml_str(s: &str) -> String {
    // Serialising a `&str` cannot fail: serde_json errs only on non-string map keys and on a
    // failing writer, and a `String` writer never fails. The fallback keeps this total.
    let json = serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into());
    json.chars()
        .map(|c| {
            let u = u32::from(c);
            if (0x7f..=0x9f).contains(&u) || matches!(u, 0x2028 | 0x2029 | 0xfffe | 0xffff) {
                format!("\\u{u:04x}")
            } else {
                c.to_string()
            }
        })
        .collect()
}

/// A YAML flow sequence of [`yaml_str`] scalars: `[]` when empty, else `["a", "b"]`.
///
/// ```
/// assert_eq!(knowledge_contract::yaml_list(&[]), "[]");
/// assert_eq!(knowledge_contract::yaml_list(&["a".into(), "b".into()]), r#"["a", "b"]"#);
/// ```
#[must_use]
pub fn yaml_list(items: &[String]) -> String {
    if items.is_empty() {
        "[]".into()
    } else {
        format!(
            "[{}]",
            items
                .iter()
                .map(|s| yaml_str(s))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaml_hostile_characters_are_escaped() {
        let v = yaml_str("a\u{7f}b\u{85}c\u{2028}d\u{ffff}");
        assert_eq!(v, "\"a\\u007fb\\u0085c\\u2028d\\uffff\"");
    }
}
