//! The one place weighin builds a trust above `asserted` (SPEC TR-03, TR-05).
//!
//! A [`Witness`] is made only from a hook's [`Answered`], which the harness adapter reads from
//! the harness's own result of the dialog that asks the person questions: it is `witnessed`
//! through `ask-user-question`, its `ref` the tool use's id. No argument, variable, batch field,
//! config key or file builds one, so every command, `batch` (whatever its `via`), the board and
//! the pane write `asserted`. A test of this module pins that no other source file names a level
//! above `asserted`.

use redkiln_core::trust::{Level, Trust, TrustError, Via};

use crate::hooks::Answered;

/// The trust of a decision the person made on a surface the agent cannot drive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Witness(Trust);

impl Witness {
    /// The witness of what the person answered in `answered`: `witnessed` through
    /// `ask-user-question`, with the tool use's id as its `ref`.
    ///
    /// # Errors
    /// [`TrustError::Text`] when the tool use's id is empty or holds a control character.
    pub(crate) fn ask_user_question(answered: &Answered) -> Result<Self, TrustError> {
        // The trust keeps its own copy of the id; the dialog's stays with the hook's input.
        Trust::new(Level::Witnessed, Some(Via::AskUserQuestion))?
            .with_reference(answered.tool_use_id.as_str())
            .map(Self)
    }

    /// The trust the decision's envelope carries.
    pub(crate) fn trust(&self) -> &Trust {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::Witness;
    use crate::hooks::Answered;
    use redkiln_core::trust::{Level, TrustError, Via};
    use std::path::Path;

    fn answered(tool_use_id: &str) -> Answered {
        Answered {
            tool_use_id: tool_use_id.to_owned(),
            asked: Vec::new(),
        }
    }

    #[test]
    fn a_witness_is_witnessed_via_ask_user_question_with_the_tool_use_id() {
        let witness =
            Witness::ask_user_question(&answered("toolu_01QKbVNaJMqaP8VSwGhuFxkh")).expect("ok");

        let trust = witness.trust();
        assert_eq!(
            (trust.level(), trust.via(), trust.reference(), trust.by()),
            (
                Level::Witnessed,
                Some(Via::AskUserQuestion),
                Some("toolu_01QKbVNaJMqaP8VSwGhuFxkh"),
                None
            )
        );
    }

    #[test]
    fn a_tool_use_id_that_cannot_be_evidence_builds_no_witness() {
        for id in ["", "toolu\n01"] {
            assert_eq!(
                Witness::ask_user_question(&answered(id)),
                Err(TrustError::Text),
                "{id:?}"
            );
        }
    }

    /// Every `.rs` file under `dir`, recursively.
    fn sources(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_dir() {
                sources(&path, out)?;
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
        Ok(())
    }

    /// TR-05: only this module raises trust. The tokens are split so this file is the only
    /// one the scan could ever find them in by accident.
    #[test]
    fn only_witness_rs_raises_trust() -> std::io::Result<()> {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        sources(&src, &mut files)?;
        let tokens = [
            ["Level::", "Witnessed"].concat(),
            ["Level::", "Attested"].concat(),
            ["Trust::", "new("].concat(),
            ["Via::", "AskUserQuestion"].concat(),
        ];
        let mut found = Vec::new();
        for file in &files {
            if file.file_name().is_some_and(|n| n == "witness.rs") {
                continue;
            }
            let text = std::fs::read_to_string(file)?;
            for token in &tokens {
                if text.contains(token.as_str()) {
                    found.push(format!("{}: {token}", file.display()));
                }
            }
        }
        assert!(files.len() > 10, "the scan read the crate's sources");
        assert_eq!(found, Vec::<String>::new());
        Ok(())
    }
}
