//! Session ids (SPEC ID-05).

use std::fmt;

use happenstance_core::MAX_TAG_LEN;

use super::IdError;

/// The longest session id: what fits in a `session:<id>` tag (EV-11, EV-12).
pub(crate) const MAX_SESSION_BYTES: usize = MAX_TAG_LEN - "session:".len();

/// The agent session a write belongs to (ID-05): the Claude Code session id as the hook input,
/// `CLAUDE_SESSION_ID` or `CLAUDE_CODE_SESSION_ID` gives it, or a CI writer's run uid.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionId(String);

impl SessionId {
    /// The session a surface reports, as it gave it. An empty or blank value, or a `${…}` the
    /// surface did not substitute, is no session (`Ok(None)`).
    ///
    /// # Errors
    /// [`IdError::Session`] for a value [`SessionId::parse`] refuses.
    pub fn from_surface(value: &str) -> Result<Option<Self>, IdError> {
        if value.trim().is_empty() || value.contains("${") {
            return Ok(None);
        }
        Self::parse(value).map(Some)
    }

    /// A CI writer's session, `gha:<repo_id>/<run_id>/<run_attempt>` (assay's run uid).
    #[must_use]
    pub fn ci(repo_id: u64, run_id: u64, run_attempt: u64) -> Self {
        Self(format!("gha:{repo_id}/{run_id}/{run_attempt}"))
    }

    /// Reads a session id as written in an envelope or tag: non-empty, no control characters,
    /// and a valid value of its `session` tag, so writing the tag never fails.
    ///
    /// # Errors
    /// [`IdError::Session`] otherwise.
    pub fn parse(text: &str) -> Result<Self, IdError> {
        let tag_safe = !text.is_empty()
            && text.len() <= MAX_SESSION_BYTES
            && !text.chars().any(char::is_control)
            && happenstance_core::Tag::key_value("session", text).is_ok();
        if tag_safe {
            Ok(Self(text.to_owned()))
        } else {
            Err(IdError::Session)
        }
    }

    /// The session id as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_surface_value_is_the_session_id_as_given() {
        let id = SessionId::from_surface("8b6fb94b-0975-5275-b434-6c9e60544265")
            .expect("valid")
            .expect("a session");
        assert_eq!(id.as_str(), "8b6fb94b-0975-5275-b434-6c9e60544265");
    }

    #[test]
    fn an_empty_or_unsubstituted_value_is_no_session() {
        for value in ["", "   ", "${CLAUDE_SESSION_ID}", "x-${SESSION}"] {
            assert_eq!(SessionId::from_surface(value), Ok(None), "{value:?}");
        }
    }

    #[test]
    fn a_ci_writer_uses_the_run_uid() {
        assert_eq!(SessionId::ci(12, 345, 1).as_str(), "gha:12/345/1");
    }

    #[test]
    fn a_session_id_that_cannot_be_a_tag_is_refused() {
        assert_eq!(SessionId::parse("a\nb"), Err(IdError::Session));
        assert_eq!(SessionId::parse(""), Err(IdError::Session));
        let fits = "s".repeat(MAX_SESSION_BYTES);
        let id = SessionId::parse(&fits).expect("exactly the cap fits");
        assert_eq!(id.as_str(), fits);
        assert_eq!(SessionId::parse(&format!("{fits}s")), Err(IdError::Session));
        assert_eq!(
            SessionId::parse("a\u{202e}b"),
            Err(IdError::Session),
            "a bidirectional control happenstance refuses in a tag"
        );
    }
}
