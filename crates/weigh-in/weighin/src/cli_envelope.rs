//! The CLI envelope (SPEC CLI-01 to CLI-05): one JSON object on one line, whose `ok` and the
//! process's exit code come from one [`Exit`] value, so they cannot disagree.
//!
//! Only the `journal` subcommands write it so far; step 6 brings the other commands onto it.

use std::io::{self, Write};

use serde::Serialize;

/// How a command ended: its exit code and its envelope's `ok`, from one value (CLI-02, CLI-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Exit {
    /// 0: the command did its work.
    Success,
    /// 1: the relay left the inbox branch for a person (`journal relay`'s own code).
    LeftInPlace,
    /// 2: bad arguments; nothing is written on stdout.
    Usage,
    /// 70: the command could not run (a refused store, a git failure).
    Internal,
    /// 74: stdout or stderr could not be written.
    Output,
}

impl Exit {
    /// The process exit code.
    pub(crate) const fn code(self) -> u8 {
        match self {
            Self::Success => 0,
            Self::LeftInPlace => 1,
            Self::Usage => 2,
            Self::Internal => 70,
            Self::Output => 74,
        }
    }

    /// The envelope's `ok`: `true` exactly when the exit code is 0.
    pub(crate) const fn ok(self) -> bool {
        matches!(self, Self::Success)
    }
}

/// How much a finding matters (CLI-03).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Severity {
    Error,
    Warning,
    Note,
}

/// A problem a command found (CLI-03): a key stable across runs, and a one-line message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Finding {
    pub(crate) key: String,
    pub(crate) message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) severity: Option<Severity>,
}

impl Finding {
    /// A finding of `severity`, its message cut to one line of at most `MAX_MESSAGE_CHARS`.
    pub(crate) fn new(key: String, message: &str, severity: Severity) -> Self {
        Self {
            key,
            message: one_line(message),
            severity: Some(severity),
        }
    }
}

/// The most characters a finding's message keeps.
const MAX_MESSAGE_CHARS: usize = 400;

/// `text` on one line (each line break a space), cut to [`MAX_MESSAGE_CHARS`].
pub(crate) fn one_line(text: &str) -> String {
    let flat: String = text
        .chars()
        .map(|c| if matches!(c, '\n' | '\r') { ' ' } else { c })
        .collect();
    crate::text::clip(&flat, MAX_MESSAGE_CHARS)
}

/// One command result as CLI-01 writes it. `data` is borrowed, never cloned (OWN-01).
pub(crate) struct Envelope<'a, D> {
    pub(crate) schema: &'static str,
    pub(crate) command: &'static str,
    pub(crate) data: &'a D,
    pub(crate) findings: &'a [Finding],
    pub(crate) exit: Exit,
}

/// The envelope's keys in CLI-02's order.
#[derive(Serialize)]
struct Wire<'a, D> {
    schema: &'static str,
    ok: bool,
    command: &'static str,
    data: &'a D,
    findings: &'a [Finding],
}

impl<D: Serialize> Envelope<'_, D> {
    /// The envelope on one line, ended by `\n`.
    ///
    /// # Errors
    /// Encoding or writing failed.
    pub(crate) fn write(&self, mut out: impl Write) -> io::Result<()> {
        let wire = Wire {
            schema: self.schema,
            ok: self.exit.ok(),
            command: self.command,
            data: self.data,
            findings: self.findings,
        };
        serde_json::to_writer(&mut out, &wire)?;
        out.write_all(b"\n")
    }
}

#[cfg(test)]
mod tests {
    use super::{Envelope, Exit, Finding, Severity};
    use serde_json::json;

    #[test]
    fn an_envelope_is_one_line_with_keys_in_spec_order() {
        let findings = [Finding {
            key: "journal/deferred:s1".to_owned(),
            message: "late".to_owned(),
            severity: Some(Severity::Warning),
        }];
        let data = json!({"appended": 2});
        let mut out = Vec::new();
        Envelope {
            schema: "weighin.journal-checkpoint/1",
            command: "journal checkpoint",
            data: &data,
            findings: &findings,
            exit: Exit::Success,
        }
        .write(&mut out)
        .expect("written");
        assert_eq!(
            String::from_utf8(out).expect("utf-8"),
            "{\"schema\":\"weighin.journal-checkpoint/1\",\"ok\":true,\"command\":\"journal checkpoint\",\
             \"data\":{\"appended\":2},\"findings\":[{\"key\":\"journal/deferred:s1\",\
             \"message\":\"late\",\"severity\":\"warning\"}]}\n"
        );
    }

    #[test]
    fn ok_is_true_exactly_when_the_exit_is_success() {
        let all = [
            Exit::Success,
            Exit::LeftInPlace,
            Exit::Usage,
            Exit::Internal,
            Exit::Output,
        ];
        let got: Vec<(u8, bool)> = all.iter().map(|e| (e.code(), e.ok())).collect();
        assert_eq!(
            got,
            [(0, true), (1, false), (2, false), (70, false), (74, false)]
        );
    }

    #[test]
    fn a_finding_message_is_one_line_cut_to_its_cap() {
        let long = format!("git said:\nfatal: {}", "x".repeat(500));
        let finding = Finding::new("relay/failed:b".to_owned(), &long, Severity::Error);
        assert_eq!(finding.message.chars().count(), 400);
        assert!(finding.message.starts_with("git said: fatal: xxx"));
        assert!(finding.message.ends_with('…'));
        assert_eq!(finding.severity, Some(Severity::Error));
    }

    #[test]
    fn a_finding_without_severity_has_no_severity_key() {
        let finding = Finding {
            key: "internal".to_owned(),
            message: "boom".to_owned(),
            severity: None,
        };
        assert_eq!(
            serde_json::to_string(&finding).expect("encoded"),
            "{\"key\":\"internal\",\"message\":\"boom\"}"
        );
    }
}
