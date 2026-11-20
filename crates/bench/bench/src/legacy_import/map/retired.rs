//! An old record of a retired type (D6 B: `weighin_parked`, `bridge.unavailable`,
//! `orchestrate.ruled`), imported as anneal wrote it under its `bench.*` name and old identity (ST-15).
//!
//! Bench no longer declares these types, so the event reads as unusable and `bench status`
//! counts it (EV-10), as every one imported before step 7 does. Its payload is the record's
//! keys but the context, unchanged; its subject tag is `task:<ticket>`, as before.

use bench_contract::event::{
    BRIDGE_UNAVAILABLE, ORCHESTRATE_RULED, RetiredType, WEIGHIN_PARKED,
};
use redkiln_core::envelope::{Class, Payload};
use redkiln_core::journal::Mapped;
use redkiln_core::store::{CoreKey, NewEvent, Stored, Tag};
use redkiln_core::suite::Owner;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::{CONTEXT_KEYS, OldContext, Records};
use crate::error_chain::reason;
use crate::store::{EventContext, StoreError};

/// An old payload, as anneal wrote it.
type OldPayload = Map<String, Value>;

/// `bench.weighin_parked`'s old payload, kept whole.
#[derive(Debug, Serialize, Deserialize)]
#[serde(transparent)]
struct OldWeighinParked(OldPayload);

impl Payload for OldWeighinParked {
    const TYPE: &'static str = WEIGHIN_PARKED.stored;
    const VERSION: u32 = 1;
    const CLASS: Class = Class::Durable;
}

/// `bench.bridge.unavailable`'s old payload, kept whole.
#[derive(Debug, Serialize, Deserialize)]
#[serde(transparent)]
struct OldBridgeUnavailable(OldPayload);

impl Payload for OldBridgeUnavailable {
    const TYPE: &'static str = BRIDGE_UNAVAILABLE.stored;
    const VERSION: u32 = 1;
    const CLASS: Class = Class::Durable;
}

/// `bench.orchestrate.ruled`'s old payload, kept whole.
#[derive(Debug, Serialize, Deserialize)]
#[serde(transparent)]
struct OldOrchestrateRuled(OldPayload);

impl Payload for OldOrchestrateRuled {
    const TYPE: &'static str = ORCHESTRATE_RULED.stored;
    const VERSION: u32 = 1;
    const CLASS: Class = Class::Durable;
}

impl<F: FnOnce() -> Result<Option<Owner>, String>> Records<'_, F> {
    /// `stored`, an old record of `retired`, as Core imports it.
    pub(super) fn retired(
        &mut self,
        stored: &Stored,
        retired: RetiredType,
    ) -> Result<Mapped, String> {
        let not_anneal = |e: serde_json::Error| format!("not an anneal event: {e}");
        let old: OldContext = serde_json::from_slice(&stored.data).map_err(not_anneal)?;
        let mut payload: OldPayload = serde_json::from_slice(&stored.data).map_err(not_anneal)?;
        for key in CONTEXT_KEYS {
            payload.remove(key);
        }
        let tags: Vec<Tag> = payload
            .get("ticket")
            .and_then(Value::as_str)
            .map(|ticket| Tag::core(CoreKey::Task, ticket))
            .transpose()
            .map_err(|e| reason(&StoreError::Tag(e)))?
            .into_iter()
            .collect();
        let kind = Retired::of(retired)?;
        let context = self.context(old)?;
        let (event, _) = self.built(context, |context| build(context, kind, &payload, &tags))?;
        Ok(Mapped::Event {
            id: stored.id,
            event,
        })
    }
}

/// The retired types this import carries over, one arm each.
#[derive(Debug, Clone, Copy)]
enum Retired {
    /// `bench.weighin_parked`.
    WeighinParked,
    /// `bench.bridge.unavailable`.
    BridgeUnavailable,
    /// `bench.orchestrate.ruled`.
    OrchestrateRuled,
}

impl Retired {
    /// The arm of `retired`, matched by its stored name; a retired type this import does not
    /// know is refused rather than imported as another.
    fn of(retired: RetiredType) -> Result<Self, String> {
        if retired.stored == WEIGHIN_PARKED.stored {
            Ok(Self::WeighinParked)
        } else if retired.stored == BRIDGE_UNAVAILABLE.stored {
            Ok(Self::BridgeUnavailable)
        } else if retired.stored == ORCHESTRATE_RULED.stored {
            Ok(Self::OrchestrateRuled)
        } else {
            Err(format!(
                "the import does not carry the retired type {}",
                retired.stored
            ))
        }
    }
}

/// The event of `payload` as `kind` in `context`, tagged `tags`.
fn build(
    context: EventContext,
    kind: Retired,
    payload: &OldPayload,
    tags: &[Tag],
) -> Result<NewEvent, StoreError> {
    // Core's envelope owns its payload and tags while it encodes; a retry after a refused
    // branch or head builds the event again from the record's.
    let (payload, tags) = (payload.clone(), tags.to_vec());
    match kind {
        Retired::WeighinParked => context.event(OldWeighinParked(payload), tags),
        Retired::BridgeUnavailable => context.event(OldBridgeUnavailable(payload), tags),
        Retired::OrchestrateRuled => context.event(OldOrchestrateRuled(payload), tags),
    }
}

#[cfg(test)]
mod tests;
