//! What an operation is, as a batch spells it in JSON, and how a batch parses.

use anyhow::Result;
use serde::Deserialize;
use weighin_core::card::Card;
use weighin_core::event::{Channel, Timing};
use weighin_core::marker::Marker;

/// What an agent or a person raises.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub(crate) struct RaiseInput {
    pub(crate) q: String,
    pub(crate) kind: Option<String>,
    pub(crate) door: Option<String>,
    pub(crate) authority: Option<String>,
    pub(crate) did: Option<String>,
    pub(crate) flip: Option<String>,
    pub(crate) at: Option<String>,
    pub(crate) sym: Option<String>,
    pub(crate) blocks: Option<String>,
    pub(crate) channel: Option<String>,
    pub(crate) quote: Option<String>,
    pub(crate) source_path: Option<String>,
    pub(crate) source_line: Option<usize>,
}

impl RaiseInput {
    #[must_use]
    pub(crate) fn from_marker(m: &Marker, channel: Channel) -> Self {
        Self {
            q: m.question.clone(),
            kind: Some(m.kind.as_str().into()),
            door: Some(m.door.as_str().into()),
            authority: Some(m.authority.as_str().into()),
            did: m.did.clone(),
            flip: m.flip.clone(),
            at: m.at.clone(),
            sym: m.sym.clone(),
            blocks: m.blocks.clone(),
            channel: Some(channel.as_str().into()),
            quote: Some(m.raw.clone()),
            source_path: None,
            source_line: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case")]
pub(crate) enum Op {
    Raise(RaiseInput),
    Triage(TriageOp),
    Ground {
        id: String,
        card: Card,
        #[serde(default)]
        reason: Option<String>,
    },
    Present {
        id: String,
        #[serde(default)]
        surface: Option<String>,
    },
    Decide(DecideOp),
    Defer(DeferOp),
    Default {
        id: String,
        option: String,
        cause: String,
        reason: String,
    },
    Dismiss {
        id: String,
        why: String,
        #[serde(default)]
        note: Option<String>,
        reason: String,
    },
    Reopen {
        id: String,
        #[serde(default)]
        note: Option<String>,
        #[serde(default)]
        reason: String,
    },
    Supersede {
        id: String,
        by: String,
        reason: String,
    },
    Link {
        id: String,
        kind: String,
        target: String,
        #[serde(default)]
        reason: Option<String>,
    },
    Ack {
        id: String,
        #[serde(default)]
        reason: Option<String>,
    },
}

/// `Op::Triage`: route an item, naming its door and blast radius.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TriageOp {
    pub(crate) id: String,
    pub(crate) door: String,
    pub(crate) blast: String,
    #[serde(default)]
    pub(crate) authority: Option<String>,
    pub(crate) route: String,
    #[serde(default)]
    pub(crate) timing: Option<Timing>,
    #[serde(default)]
    pub(crate) resolve_by: Option<String>,
    pub(crate) reason: String,
}

/// `Op::Decide`: record the option chosen. `note` defaults to empty and `via` to `chat`.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct DecideOp {
    pub(crate) id: String,
    pub(crate) option: String,
    #[serde(default)]
    pub(crate) note: Option<String>,
    #[serde(default)]
    pub(crate) provenance: Option<String>,
    #[serde(default)]
    pub(crate) via: Option<String>,
    #[serde(default)]
    pub(crate) seconds: Option<u64>,
}

/// `Op::Defer`: wait for a named fact, with a trigger, a resolve-by and a default.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct DeferOp {
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) flip: String,
    pub(crate) trigger: String,
    pub(crate) resolve_by: String,
    pub(crate) default: String,
    #[serde(default)]
    pub(crate) owner: Option<String>,
    #[serde(default)]
    pub(crate) unqualified: bool,
    #[serde(default)]
    pub(crate) reason: String,
}

/// An operation that could not be parsed.
#[derive(Debug)]
pub(crate) struct BadOp {
    /// Whether it was tagged `"op": "raise"`: such an op still takes its `@N` slot
    /// ([`Session::apply_parsed`](super::Session::apply_parsed)).
    pub(crate) raise: bool,
    /// serde's error, with what it expected.
    pub(crate) error: serde_json::Error,
}

/// Parse a JSON array (or single object) of operations.
///
/// One bad operation does not drop the others: each is parsed on its own, and its error is
/// returned in its slot.
///
/// # Errors
/// Fails only when the text is not JSON at all, with serde's error and position.
pub(crate) fn parse_ops(text: &str) -> Result<Vec<Result<Op, BadOp>>> {
    let v: serde_json::Value = serde_json::from_str(text)?;
    let arr = match v {
        serde_json::Value::Array(a) => a,
        other => vec![other],
    };
    Ok(arr
        .into_iter()
        .map(|o| {
            let raise = o.get("op").and_then(serde_json::Value::as_str) == Some("raise");
            serde_json::from_value(o).map_err(|error| BadOp { raise, error })
        })
        .collect())
}
