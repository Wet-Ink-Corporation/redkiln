//! A stored Bench event as `bench ledger` prints it: its position and the EV-01 envelope Core
//! keeps (`type` `bench.<name>`, `version`, `trust`, `writer`, the payload under `data`), read
//! from the stored bytes rather than rebuilt, so what is printed is what is stored.

use bench_contract::event as contract;
use redkiln_core::envelope::{Skip, Unusable};
use redkiln_core::store::Stored;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// A stored Bench event: where it sits and its envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StoredEvent {
    /// Where the event sits in the store, from 1, among every plugin's events (EV-02: the
    /// store's value, beside the envelope rather than in it).
    pub position: u64,
    /// The envelope, as stored.
    pub event: CoreEnvelope,
}

impl StoredEvent {
    /// `stored` as printed, once Bench's contract has read it: another namespace's event is
    /// [`Skip::Foreign`] and one this build cannot use is [`Skip::Unusable`] (EV-08, EV-10),
    /// as for every other read, so `bench ledger` lists exactly the events the gates see.
    pub(super) fn from_stored(stored: &Stored) -> Result<Self, Skip> {
        contract::decode(stored)?;
        let event = serde_json::from_slice(&stored.data)
            .map_err(|error| Skip::Unusable(Unusable::Json(error)))?;
        Ok(Self {
            position: stored.position,
            event,
        })
    }
}

/// An EV-01 envelope as stored: its keys in EV-01's order, the optional ones only when present,
/// then any key a later SPEC adds (EV-09), kept as read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreEnvelope {
    #[serde(rename = "type")]
    event_type: String,
    version: u32,
    ts: String,
    actor: String,
    trust: Value,
    writer: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    session: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    worktree: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    head: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    origin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cites: Option<Vec<String>>,
    data: Value,
    #[serde(flatten)]
    later: Map<String, Value>,
}
