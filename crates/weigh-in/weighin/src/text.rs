//! Text helpers shared by the command output, the context packs and the hooks.

/// `s` cut to at most `n` characters, the last of them an ellipsis when anything was cut.
#[must_use]
pub(crate) fn clip(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_owned();
    }
    // With no room at all, not even the ellipsis fits.
    let Some(keep) = n.checked_sub(1) else {
        return String::new();
    };
    s.chars().take(keep).chain(['…']).collect()
}

/// An error and each of its causes on one line, `outer: cause: root`, for an error that is only
/// borrowed and so cannot become an `anyhow::Error` with its chain.
pub(crate) struct Report<'a>(pub(crate) &'a (dyn std::error::Error + 'static));

impl std::fmt::Display for Report<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let chain = std::iter::successors(Some(self.0), |&e| e.source());
        for (depth, cause) in chain.enumerate() {
            if depth > 0 {
                f.write_str(": ")?;
            }
            write!(f, "{cause}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Report, clip};

    #[test]
    fn short_text_is_kept_and_long_text_ends_in_an_ellipsis() {
        assert_eq!(clip("abc", 3), "abc");
        assert_eq!(clip("abcdef", 4), "abc…");
        assert_eq!(clip("abc", 0), "");
        assert_eq!(clip("abc", 1), "…");
        assert_eq!(clip("", 0), "");
        assert_eq!(clip("a\nb", 3), "a\nb");
    }

    #[test]
    fn a_report_prints_the_whole_chain_on_one_line() {
        let parse = serde_json::from_str::<u8>("x").expect_err("not a number");
        let e = anyhow::Error::new(parse).context("cannot parse /c.json");

        assert_eq!(
            Report(e.as_ref()).to_string(),
            "cannot parse /c.json: expected value at line 1 column 1"
        );
    }
}
