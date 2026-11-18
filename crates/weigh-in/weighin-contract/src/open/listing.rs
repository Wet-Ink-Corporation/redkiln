//! `open --json` as a sibling reads it ([`read_listing`]), so a newer weighin that adds keys,
//! status words or `applies` words never breaks the reader (CLI-04, EV-09).
//!
//! [`OpenItem`](super::OpenItem) is what weighin writes and refuses any key it does not know;
//! [`Listed`] keeps only what a sibling reads (Bench's D6 hook): the id, status, question,
//! decision and the card's option titles. An item that does not read as one (an id that is
//! not a `wi-` id, a field of another type) is skipped, never failing the listing.

use serde::{Deserialize, Deserializer};

use super::Status;
use crate::event::Applies;
use crate::id::ItemId;

/// Every status word, to read one leniently.
const STATUSES: [Status; 9] = [
    Status::Raised,
    Status::Triaged,
    Status::Grounded,
    Status::Presented,
    Status::Deferred,
    Status::Decided,
    Status::Defaulted,
    Status::Dismissed,
    Status::Superseded,
];

/// One item of `open --json` as a sibling reads it: unknown keys ignored (EV-09).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Listed {
    /// The item's id; an item whose id is not a `wi-` id is skipped.
    #[serde(deserialize_with = "item_id")]
    pub id: ItemId,
    /// Where the item is in its lifecycle; `None` for a word this build does not know, or none.
    #[serde(default, deserialize_with = "status")]
    pub status: Option<Status>,
    /// The question as first raised; empty when absent.
    #[serde(default)]
    pub question: String,
    /// The latest decision; `None` when undecided.
    #[serde(default)]
    pub decision: Option<ListedDecision>,
    /// The card; `None` until grounded.
    #[serde(default)]
    pub card: Option<ListedCard>,
}

/// A listed item's decision, as a sibling reads it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ListedDecision {
    /// The chosen card option id, or the answer itself for an item with no card.
    pub option: String,
    /// Once or always; `None` when absent or a word this build does not know.
    #[serde(default, deserialize_with = "applies")]
    pub applies: Option<Applies>,
}

/// A listed item's card, as a sibling reads it: only its options.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ListedCard {
    /// The card's options, in card order.
    #[serde(default)]
    pub options: Vec<ListedOption>,
}

/// One option of a listed item's card.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ListedOption {
    /// The option's id, as a decision names it (`A`, or `a` for a request item).
    pub id: String,
    /// The option's short name (a request option's label); empty when absent.
    #[serde(default)]
    pub title: String,
}

impl Listed {
    /// The decided option's title from the card, else the option as decided; `None` when the
    /// item has no decision.
    #[must_use]
    pub fn decided_label(&self) -> Option<&str> {
        let option = self.decision.as_ref()?.option.as_str();
        let title = self
            .card
            .iter()
            .flat_map(|card| &card.options)
            .find(|listed| listed.id == option)
            .map(|listed| listed.title.as_str())
            .filter(|title| !title.trim().is_empty());
        Some(title.unwrap_or(option))
    }
}

/// Why a listing was refused.
#[derive(Debug, thiserror::Error)]
pub enum ListingError {
    /// The listing is not a JSON array.
    #[error("the listing did not parse")]
    Parse(#[source] serde_json::Error),
    /// The listing holds as many items as were asked for, so weighin may have left some out.
    #[error("the listing holds {cap} items or more and may be cut")]
    Truncated {
        /// The most items asked for.
        cap: usize,
    },
}

/// Every readable item of an `open --json` listing asked for at most `asked` items, in the
/// order listed; an item that does not read as [`Listed`] is skipped.
///
/// # Errors
/// [`ListingError::Parse`] when `json` is not a JSON array, [`ListingError::Truncated`] when it
/// holds `asked` elements or more, readable or not.
pub fn read_listing(json: &[u8], asked: usize) -> Result<Vec<Listed>, ListingError> {
    let elements: Vec<serde_json::Value> =
        serde_json::from_slice(json).map_err(ListingError::Parse)?;
    if elements.len() >= asked {
        return Err(ListingError::Truncated { cap: asked });
    }
    // Skipping an element that does not read is the reader's rule (T4), not a lost failure.
    Ok(elements
        .into_iter()
        .filter_map(|element| Listed::deserialize(element).ok())
        .collect())
}

/// An item id, refused unless it is a `wi-` id.
fn item_id<'de, D: Deserializer<'de>>(deserializer: D) -> Result<ItemId, D::Error> {
    let text = String::deserialize(deserializer)?;
    ItemId::parse(&text).ok_or_else(|| serde::de::Error::custom("not a weigh-in item id"))
}

/// A status word, `None` when absent, `null` or unknown.
fn status<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Status>, D::Error> {
    let word = Option::<String>::deserialize(deserializer)?;
    Ok(word.and_then(|word| STATUSES.into_iter().find(|status| status.as_str() == word)))
}

/// An `applies` word, `None` when absent, `null` or unknown.
fn applies<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Applies>, D::Error> {
    let word = Option::<String>::deserialize(deserializer)?;
    Ok(word.as_deref().and_then(Applies::parse))
}

#[cfg(test)]
mod tests;
