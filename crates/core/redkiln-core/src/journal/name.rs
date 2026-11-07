//! A journal's name (SPEC JR-06): its session id, lowercased, with every character outside
//! `[a-z0-9_-]` replaced by `_`, so it is always a valid git ref component; `store-<store id>`
//! for events with no session.

use std::fmt;

use happenstance_core::StoreId;

use crate::ids::SessionId;

/// The suffix of a journal's file, `<name>.jsonl` (JR-07).
pub const JOURNAL_SUFFIX: &str = ".jsonl";

/// The longest name [`JournalName::parse`] accepts, in bytes.
pub const MAX_NAME_BYTES: usize = 200;

/// A name read back from git (a ref or a branch) that JR-06 cannot produce.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` is not a journal name")]
pub struct NameError(pub String);

/// A journal's name. Two session ids can share one (`A` and `a`), which is harmless: every
/// write merges by `origin`, and identity removes duplicates (JR-06, JR-11).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct JournalName(String);

impl JournalName {
    /// The journal of `session`. Lowercasing is ASCII's: any other character is replaced.
    #[must_use]
    pub fn of_session(session: &SessionId) -> Self {
        Self(
            session
                .as_str()
                .chars()
                .map(|c| match c.to_ascii_lowercase() {
                    lower @ ('a'..='z' | '0'..='9' | '_' | '-') => lower,
                    _ => '_',
                })
                .collect(),
        )
    }

    /// The journal of events with no session in the store `store`, `store-<store id>`.
    #[must_use]
    pub fn of_store(store: StoreId) -> Self {
        Self(format!("store-{store}"))
    }

    /// The journal an event belongs in: its session's, or, with none, the one of the store
    /// that first accepted it (its identity's), so one fact always maps to one journal.
    #[must_use]
    pub fn for_event(session: Option<&SessionId>, origin: StoreId) -> Self {
        session.map_or_else(|| Self::of_store(origin), Self::of_session)
    }

    /// A name read back from a ref or a branch: one or more of `[a-z0-9_-]`, at most
    /// [`MAX_NAME_BYTES`], so it is always one valid ref component.
    ///
    /// # Errors
    /// [`NameError`] for anything else, holding what was given.
    pub fn parse(name: &str) -> Result<Self, NameError> {
        let valid = !name.is_empty()
            && name.len() <= MAX_NAME_BYTES
            && name
                .bytes()
                .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-'));
        if valid {
            Ok(Self(name.to_owned()))
        } else {
            Err(NameError(name.to_owned()))
        }
    }

    /// The name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Its file's name, `<name>.jsonl`.
    #[must_use]
    pub fn file_name(&self) -> String {
        format!("{}{JOURNAL_SUFFIX}", self.0)
    }
}

impl fmt::Display for JournalName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::parse_store_id;

    #[test]
    fn a_session_id_is_lowercased_and_anything_else_becomes_an_underscore() {
        let session = SessionId::parse("8B6FB94B-0975-5275-B434-6c9e60544265").expect("valid");
        let name = JournalName::of_session(&session);
        assert_eq!(name.as_str(), "8b6fb94b-0975-5275-b434-6c9e60544265");
        assert_eq!(
            name.file_name(),
            "8b6fb94b-0975-5275-b434-6c9e60544265.jsonl"
        );
        let ci = SessionId::ci(812_345, 9912, 1);
        assert_eq!(JournalName::of_session(&ci).as_str(), "gha_812345_9912_1");
        let odd = SessionId::parse("caf\u{e9} Run.x/y").expect("valid");
        assert_eq!(JournalName::of_session(&odd).as_str(), "caf__run_x_y");
    }

    #[test]
    fn a_name_read_back_must_be_one_jr_06_can_produce() {
        for good in ["a", "gha_1_2_3", "store-0f3c", "-x_"] {
            assert_eq!(JournalName::parse(good).expect(good).as_str(), good);
        }
        let long = "a".repeat(MAX_NAME_BYTES + 1);
        for bad in [
            "",
            "A",
            "a.b",
            "a/b",
            "..",
            "a b",
            "caf\u{e9}",
            long.as_str(),
        ] {
            assert_eq!(
                JournalName::parse(bad),
                Err(NameError(bad.to_owned())),
                "{bad}"
            );
        }
    }

    #[test]
    fn events_with_no_session_go_to_the_store_journal() {
        let store = parse_store_id("0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f").expect("valid");
        assert_eq!(
            JournalName::of_store(store).as_str(),
            "store-0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f"
        );
    }
}
