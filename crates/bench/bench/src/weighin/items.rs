//! The items weigh-in has decided, from `weighin open --all --json`: the rulings D6 can offer
//! before (or without) their decision atoms in `.kb/decision/` or `.kb/_intake/decisions/`.
//!
//! The listing is read through weighin-contract's lenient reader
//! ([`weighin_contract::open::read_listing`]): unknown keys and words are ignored (CLI-04,
//! EV-09), and an item that does not read (an id that is not a `wi-` id among them) is skipped.
//! Only `decided` items with a decision are kept, each summarised by the decided option's card
//! title (a request item's option label), else the option as decided.

use weighin_contract::ItemId;
use weighin_contract::open::{ListingError, Status, read_listing};

/// A ruling weigh-in holds: the item, its question and the option decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Decided {
    /// The item.
    pub(crate) id: ItemId,
    /// Its question.
    pub(crate) question: String,
    /// The option decided: its card title when it has one, else the option as decided.
    pub(crate) option: String,
}

/// The decided items in `json`, a listing asked for at most `asked` items, in the order listed.
///
/// # Errors
/// [`ListingError::Parse`] when `json` is not an array, [`ListingError::Truncated`] when it
/// holds `asked` items or more.
pub(crate) fn decided(json: &[u8], asked: usize) -> Result<Vec<Decided>, ListingError> {
    Ok(read_listing(json, asked)?
        .into_iter()
        .filter(|item| item.status == Some(Status::Decided))
        .filter_map(|item| {
            // Owned: the label outlives the listed item it is borrowed from.
            let option = item.decided_label()?.to_owned();
            Some(Decided {
                id: item.id,
                question: item.question,
                option,
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::decided;

    // What a listing reads as, and which items it skips, is pinned by weighin-contract's
    // `open` tests; these pin what D6 takes from it.

    #[test]
    fn a_decided_request_item_is_summarised_by_its_option_label() {
        let listed = json!([
            {"id": "wi-0c9f04eb9a", "status": "decided", "channel": "request",
             "question": "Which store should Bench keep its events in?",
             "decision": {"option": "a", "applies": "once"},
             "card": {"options": [{"id": "a", "title": "The Core store"},
                                  {"id": "b", "title": "Its own SQLite file"}]}},
            {"id": "wi-e41e02", "status": "decided", "question": "Shared?",
             "decision": {"option": "shared"}}
        ]);

        let found = decided(listed.to_string().as_bytes(), 4096).expect("an array");

        let options: Vec<(&str, &str)> = found
            .iter()
            .map(|item| (item.id.as_str(), item.option.as_str()))
            .collect();
        assert_eq!(
            options,
            [("wi-0c9f04eb9a", "The Core store"), ("wi-e41e02", "shared")]
        );
    }
}
