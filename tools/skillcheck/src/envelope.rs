//! The JSON envelope every command writes on stdout (SPEC CLI-01, CLI-02), and the findings it
//! carries (CLI-03). The shape is Bench's; the `schema` id is `skillcheck.<command>/1`.

use std::io::{self, Write};

use serde::{Deserialize, Serialize};

use crate::command::CommandName;
use crate::exit_codes::Code;

/// The binary's name: the namespace of its output ids and the prefix of its stderr lines.
pub const BINARY: &str = "skillcheck";

/// The version every output id has today (CLI-04).
pub const OUTPUT_VERSION: u32 = 1;

/// The output id (SPEC CLI-02) of every subcommand path, sorted.
pub const OUTPUTS: [&str; 3] = [
    "skillcheck.eval-routing/1",
    "skillcheck.eval-uplift/1",
    "skillcheck.lint-skills/1",
];

/// The output id of the subcommand path `command` (`"lint skills"` gives
/// `"skillcheck.lint-skills/1"`): the binary's namespace, the path's words joined by `-`, then
/// [`OUTPUT_VERSION`].
#[must_use]
pub fn output_id(command: &str) -> String {
    format!("{BINARY}.{}/{OUTPUT_VERSION}", command.replace(' ', "-"))
}

/// One problem a command found. The `key` is stable across runs.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Finding {
    /// Stable identity: the same problem yields the same key on every run.
    pub key: String,
    /// One-line human description.
    pub message: String,
}

impl Finding {
    /// A finding with a stable `key` and a one-line `message`.
    #[must_use]
    pub fn new(key: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            message: message.into(),
        }
    }
}

/// The result of a command, serialised as one JSON object on stdout:
/// `{"schema", "ok", "command", "data", "findings", "proves", "does_not_prove"}`, `schema`
/// first (CLI-02).
///
/// `ok` is derived from the exit code, and [`Envelope::emit`] returns that same code, so the
/// JSON and the exit status cannot disagree; `schema` and `command` come from the same command
/// name and subcommand.
#[derive(Debug, Serialize)]
pub struct Envelope<'a, D> {
    #[serde(skip)]
    code: Code,
    schema: String,
    ok: bool,
    command: String,
    data: D,
    findings: &'a [Finding],
    proves: &'static str,
    does_not_prove: &'static str,
}

impl<'a, D: Serialize> Envelope<'a, D> {
    /// An envelope with no findings for `name` and its `subcommand`, that will exit with
    /// `code`. `data` should serialise to a JSON object.
    #[must_use]
    pub fn new(name: CommandName, subcommand: Option<&str>, code: Code, data: D) -> Self {
        let claim = name.claim();
        let command = match subcommand {
            Some(sub) => format!("{name} {sub}"),
            None => name.as_str().to_owned(),
        };
        Self {
            code,
            schema: output_id(&command),
            ok: code.is_success(),
            command,
            data,
            findings: &[],
            proves: claim.proves,
            does_not_prove: claim.does_not_prove,
        }
    }

    /// The same envelope carrying `findings`.
    #[must_use]
    pub fn with_findings(self, findings: &'a [Finding]) -> Self {
        Self { findings, ..self }
    }

    /// Writes the envelope as one line of JSON and returns the exit code it was built with.
    ///
    /// # Errors
    /// Returns the I/O error when `out` cannot be written or `data` fails to serialise.
    pub fn emit<W: Write>(&self, mut out: W) -> io::Result<Code> {
        serde_json::to_writer(&mut out, self)?;
        out.write_all(b"\n")?;
        Ok(self.code)
    }
}

#[cfg(test)]
mod tests {
    use super::{Envelope, Finding, OUTPUTS, output_id};
    use crate::command::CommandName;
    use crate::exit_codes::{LINT_FINDINGS, OK};
    use serde_json::json;

    #[test]
    fn schema_is_the_first_key_and_names_the_command_path() {
        let findings = [Finding::new("lint/name:a:1", "m")];
        let mut out = Vec::new();
        let code = Envelope::new(CommandName::Lint, Some("skills"), LINT_FINDINGS, json!({}))
            .with_findings(&findings)
            .emit(&mut out)
            .expect("a Vec accepts every write");

        assert_eq!(code.value(), 1, "emit returns the envelope's code");
        assert!(
            out.starts_with(
                br#"{"schema":"skillcheck.lint-skills/1","ok":false,"command":"lint skills","#
            ),
            "CLI-02's order: {}",
            String::from_utf8_lossy(&out)
        );
    }

    #[test]
    fn ok_is_true_only_for_exit_code_zero() {
        let mut out = Vec::new();
        Envelope::new(CommandName::Eval, Some("routing"), OK, json!({}))
            .emit(&mut out)
            .expect("a Vec accepts every write");
        let value: serde_json::Value = serde_json::from_slice(&out).expect("valid JSON");

        assert_eq!(value["ok"], json!(true), "0 is ok");
    }

    #[test]
    fn outputs_are_sorted_and_each_the_id_of_a_command() {
        assert!(OUTPUTS.is_sorted(), "sorted");
        let ids: Vec<String> = ["eval routing", "eval uplift", "lint skills"]
            .into_iter()
            .map(output_id)
            .collect();
        assert_eq!(ids, OUTPUTS, "one id per subcommand path");
    }
}
