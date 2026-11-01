//! The commit trailer that records a decision.

use weighin_core::id::ItemId;

/// The git trailer that records a decision, `Weigh-In: <id> decided <option>`. The decide receipt
/// and `weighin trailer` both print it through this type, so the two cannot drift apart.
pub(crate) struct Trailer<'a> {
    /// The decided item.
    pub(crate) id: &'a ItemId,
    /// The option it was decided for.
    pub(crate) option: &'a str,
}

impl std::fmt::Display for Trailer<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {} decided {}", Self::KEY, self.id, self.option)
    }
}

impl Trailer<'_> {
    /// The trailer's key.
    const KEY: &'static str = "Weigh-In";

    /// The item and option a `Weigh-In: <id> decided <option>` line records, as [`Display`]
    /// prints it (`ci-check` reads them back). The key matches in any ASCII case, the line is
    /// trimmed, and the option is the rest of the line; it cannot be empty.
    ///
    /// [`Display`]: std::fmt::Display
    pub(crate) fn parse(line: &str) -> Option<(ItemId, &str)> {
        let (key, rest) = line.trim().split_once(':')?;
        if !key.eq_ignore_ascii_case(Self::KEY) {
            return None;
        }
        let (id, option) = rest.trim_start().split_once(" decided ")?;
        let option = option.trim();
        if option.is_empty() {
            return None;
        }
        Some((ItemId::parse(id)?, option))
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use weighin_core::id::ItemId;

    use super::Trailer;

    #[test]
    fn a_printed_trailer_parses_back_to_its_item_and_option() -> Result<()> {
        let id = ItemId::parse("wi-4dfe38").ok_or_else(|| anyhow::anyhow!("an id"))?;
        let printed = Trailer {
            id: &id,
            option: "per series, as the schema has",
        }
        .to_string();

        let parsed = Trailer::parse(&printed);

        assert_eq!(parsed, Some((id, "per series, as the schema has")));
        Ok(())
    }

    #[test]
    fn a_trailer_key_in_any_case_and_padding_parses() -> Result<()> {
        let id = ItemId::parse("wi-4dfe38").ok_or_else(|| anyhow::anyhow!("an id"))?;

        let parsed = Trailer::parse("  weigh-in: wi-4dfe38 decided B  ");

        assert_eq!(parsed, Some((id, "B")));
        Ok(())
    }

    #[test]
    fn a_trailer_with_a_bad_id_or_no_option_does_not_parse() {
        let lines = [
            "Weigh-In: wi-XYZ decided A",
            "Weigh-In: wi-4dfe38 decided ",
            "Weigh-In: wi-4dfe38 deferred A",
            "Signed-off-by: wi-4dfe38 decided A",
        ];

        let parsed = lines.map(Trailer::parse);

        assert_eq!(parsed, [None, None, None, None]);
    }
}
