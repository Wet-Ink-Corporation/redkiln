//! The slug that ends an atom's file name.

/// Lowercase ASCII slug of at most 48 characters on a word boundary.
///
/// ```
/// assert_eq!(
///     knowledge_contract::slug("Where should the ledger live?"),
///     "where-should-the-ledger-live"
/// );
/// ```
#[must_use]
pub fn slug(s: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    let out = out.trim_end_matches('-').to_owned();
    if out.len() <= 48 {
        return out;
    }
    // `out` holds only ASCII alphanumerics and `-`, so every offset is a char boundary and
    // `get` never misses; the fallbacks keep the function total without a panic path.
    let cut = out.get(..48).unwrap_or(&out);
    cut.rfind('-')
        .and_then(|i| cut.get(..i))
        .unwrap_or(cut)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_is_bounded() {
        assert_eq!(
            slug("Where should the ledger live?"),
            "where-should-the-ledger-live"
        );
        assert!(slug(&"word ".repeat(40)).len() <= 48);
    }
}
