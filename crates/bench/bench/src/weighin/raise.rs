//! Raising a Weigh-In decision request in the suite store (DR-01, DR-03, ID-04).
//!
//! The request is appended through weighin-contract (EV-04) under its own condition: no
//! `weighin.item_raised` and no `weighin.request_raised` holds its item. When one does, the
//! holders are read: one of the same raiser key is this request raised before (nothing is
//! written), any other is a collision, and the id is extended from 10 to 12 to 16 hex.

use std::num::NonZeroUsize;

use redkiln_core::envelope::Envelope;
use redkiln_core::ids::HexLen;
use redkiln_core::store::{CoreKey, Filter, Tag};
use weighin_contract::ItemId;
use weighin_contract::event::Raised as Captured;
use weighin_contract::names::{Named, REQUEST_RAISED};
use weighin_contract::request::{
    Request, RequestError, raise_condition, raise_event_at, same_raiser,
};

use crate::store::{Appended, Store, StoreError};

/// Most holders of one item id read to tell a retry from a collision: the conditioned append
/// lets at most one raise hold an id, so more than a few means the store was written by hand.
const MAX_HOLDERS: NonZeroUsize = match NonZeroUsize::new(8) {
    Some(max) => max,
    None => NonZeroUsize::MIN,
};

/// The id lengths tried in turn (ID-04).
const LENGTHS: [HexLen; 3] = [HexLen::Ten, HexLen::Twelve, HexLen::Sixteen];

/// What a raise found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Raised {
    /// Appended at `position`.
    New {
        /// The request's item.
        item: ItemId,
        /// Where it was written.
        position: u64,
    },
    /// The same raiser key was already raised, at `position` (DR-03): nothing written.
    Already {
        /// The request's item.
        item: ItemId,
        /// Where the earlier raise sits.
        position: u64,
    },
}

/// Why a request was not raised. Nothing was written.
#[derive(Debug, thiserror::Error)]
pub(crate) enum RaiseError {
    /// A DR-04 guard or a field shape refused it.
    #[error("weigh-in's contract refused the request")]
    Refused(#[source] Box<RequestError>),
    /// Every id length (10, 12, 16 hex) names another item.
    #[error("{item} and its longer forms all name another item")]
    IdTaken {
        /// The id at its default length.
        item: ItemId,
    },
    /// The store could not be read or written.
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl RaiseError {
    /// [`RaiseError::Refused`], with the contract's refusal boxed to keep the error small.
    fn refused(error: RequestError) -> Self {
        Self::Refused(Box::new(error))
    }
}

/// Raises `envelope`'s request in `store` (DR-01, DR-03, ID-04).
///
/// # Errors
/// [`RaiseError::Refused`] for a request the contract refuses, [`RaiseError::IdTaken`] when
/// every id length names another item, [`RaiseError::Store`] when the store fails.
pub(crate) fn raise(store: &Store, envelope: &Envelope<Request>) -> Result<Raised, RaiseError> {
    for len in LENGTHS {
        let item = envelope.data.item_id_at(len).map_err(RaiseError::refused)?;
        let event = raise_event_at(envelope, len).map_err(RaiseError::refused)?;
        let condition = raise_condition(&item).map_err(RaiseError::refused)?;
        match store.append_foreign_if(event, &condition)? {
            Appended::At(position) => return Ok(Raised::New { item, position }),
            Appended::Conflict => {
                if let Some(position) = earlier_raise(store, &item, &envelope.data)? {
                    return Ok(Raised::Already { item, position });
                }
            }
        }
    }
    let item = envelope
        .data
        .item_id_at(HexLen::Ten)
        .map_err(RaiseError::refused)?;
    Err(RaiseError::IdTaken { item })
}

/// The position of a raise of `request`'s own raiser key among the holders of `item`, or
/// `None` when another item holds it. Building the lookup is the store's business, so its
/// failures are store errors, never the contract refusing the request.
fn earlier_raise(
    store: &Store,
    item: &ItemId,
    request: &Request,
) -> Result<Option<u64>, StoreError> {
    let tag = Tag::core(CoreKey::Item, item.as_str()).map_err(StoreError::Tag)?;
    let filter = Filter::all()
        .of_type(<Captured as Named>::NAME.name)
        .and_then(|filter| filter.of_type(REQUEST_RAISED.name))?
        .tagged(tag);
    let holders = store.read_foreign(&filter, MAX_HOLDERS)?;
    Ok(holders
        .iter()
        .find(|stored| same_raiser(stored, request))
        .map(|stored| stored.position))
}

#[cfg(test)]
mod tests;
