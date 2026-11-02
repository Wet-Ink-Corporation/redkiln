//! The envelope's JSON shapes: what is written, and the lenient shape it is read through.
//!
//! Written keys come in EV-01's order, and an absent optional key is left out, never `null`.
//! Read keys a build does not know are ignored, and a `null` reads as absent (EV-01, EV-09).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Envelope, TypeName};

/// An envelope as written.
#[derive(Debug, Serialize)]
pub(super) struct EnvelopeOut<'a> {
    #[serde(rename = "type")]
    event_type: &'a str,
    version: u32,
    ts: &'a str,
    actor: String,
    trust: TrustOut<'a>,
    writer: WriterOut<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    worktree: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    branch: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    head: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    origin: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    cites: Vec<String>,
}

/// `envelope`'s JSON with `,"data":<data>` as its last key: `data` is the payload's own JSON
/// object, written once and in its own key order.
pub(super) fn with_data(
    envelope: &EnvelopeOut<'_>,
    data: &[u8],
) -> Result<Vec<u8>, serde_json::Error> {
    let mut bytes = serde_json::to_vec(envelope)?;
    // A struct with fields serialises to `{...}`; its closing brace makes way for `data`.
    if bytes.pop() != Some(b'}') {
        return Err(serde::ser::Error::custom(
            "the envelope is not a JSON object",
        ));
    }
    bytes.reserve(data.len().saturating_add(10));
    bytes.extend_from_slice(b",\"data\":");
    bytes.extend_from_slice(data);
    bytes.push(b'}');
    Ok(bytes)
}

#[derive(Debug, Serialize)]
struct TrustOut<'a> {
    level: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    via: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    by: Option<&'a str>,
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    reference: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    claimed: Option<&'static str>,
}

#[derive(Debug, Serialize)]
struct WriterOut<'a> {
    binary: &'a str,
    version: &'a str,
    spec: &'a str,
}

impl<'a> EnvelopeOut<'a> {
    /// The written shape of `envelope`'s keys before `data`, of type `name` at `version`.
    pub(super) fn new<P>(envelope: &'a Envelope<P>, name: &'a TypeName, version: u32) -> Self {
        let trust = &envelope.trust;
        Self {
            event_type: name.as_str(),
            version,
            ts: envelope.ts.as_str(),
            actor: envelope.actor.to_string(),
            trust: TrustOut {
                level: trust.level().as_str(),
                via: trust.via().map(crate::trust::Via::as_str),
                by: trust.by(),
                reference: trust.reference(),
                claimed: trust.claimed().map(crate::trust::Level::as_str),
            },
            writer: WriterOut {
                binary: envelope.writer.binary(),
                version: envelope.writer.version(),
                spec: envelope.writer.spec(),
            },
            session: envelope.session.as_ref().map(crate::ids::SessionId::as_str),
            worktree: envelope
                .worktree
                .as_ref()
                .map(crate::ids::WorktreeId::as_str),
            branch: envelope.branch.as_deref(),
            head: envelope.head.as_deref(),
            origin: envelope.origin.map(|id| id.to_string()),
            cites: envelope.cites.iter().map(ToString::to_string).collect(),
        }
    }
}

/// An envelope as read: every key EV-01 defines, the optional ones as `Option` (absent and
/// `null` alike), and nothing else.
#[derive(Debug, Deserialize)]
pub(super) struct EnvelopeIn {
    #[serde(rename = "type")]
    pub(super) event_type: String,
    pub(super) version: u64,
    pub(super) ts: String,
    pub(super) actor: String,
    pub(super) trust: TrustIn,
    pub(super) writer: WriterIn,
    pub(super) session: Option<String>,
    pub(super) worktree: Option<String>,
    pub(super) branch: Option<String>,
    pub(super) head: Option<String>,
    pub(super) origin: Option<String>,
    pub(super) cites: Option<Vec<String>>,
    pub(super) data: Value,
}

#[derive(Debug, Deserialize)]
pub(super) struct TrustIn {
    pub(super) level: String,
    pub(super) via: Option<String>,
    pub(super) by: Option<String>,
    #[serde(rename = "ref")]
    pub(super) reference: Option<String>,
    pub(super) claimed: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct WriterIn {
    pub(super) binary: String,
    pub(super) version: String,
    pub(super) spec: String,
}
