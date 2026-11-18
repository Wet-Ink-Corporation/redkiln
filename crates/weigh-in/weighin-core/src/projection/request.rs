//! A decision request folded into an item (DR-01): grounded on arrival, channel `request`, its
//! card the request's own options with ids `a` to `d`.
//!
//! A two-way judgment request that names a default and no deadline is the raiser's own two-way
//! call (Bench call F): it folds already defaulted, so the digest shows it as a triaged
//! `route: default` item, and nobody is asked (SPEC DR-07).

use super::{DefaultTaken, Item, Status};
use crate::card::{Card, CardOption, Evidence, Recommendation};
use crate::event::{Channel, DefaultCause, EventEnvelope, Requested, Route};
use crate::id::question_key;
use crate::marker::{Authority, Door, Kind};
use weighin_contract::open::RequestRecord;
use weighin_contract::request::{ANCHOR, Blocks, Request};

/// The reason a call F default carries in the digest.
const CALL_F_REASON: &str = "the raiser's own two-way call, logged";

impl Item {
    /// A new item from its first `request_raised` event, written at `position`. The fold borrows
    /// its events, so the item owns a copy of each field it keeps, as `from_raised` does (H-08).
    pub(super) fn from_request(position: u64, env: &EventEnvelope, r: &Requested) -> Self {
        let request = &r.request;
        let default = call_f_default(request).map(|option| DefaultTaken {
            option: option.to_owned(),
            cause: DefaultCause::TwoWay,
            reason: CALL_F_REASON.to_owned(),
            ts: env.ts.clone(),
            acknowledged: false,
        });
        let (status, route) = if default.is_some() {
            (Status::Defaulted, Route::Default)
        } else {
            (Status::Grounded, Route::Present)
        };
        Self {
            id: r.item_id.clone(),
            kind: Kind::Question,
            question: request.question.clone(),
            question_key: question_key(&request.question),
            did: None,
            flip_hint: None,
            at: anchor(request),
            sym: None,
            blocks: blocks(request.blocks),
            door_hint: request.door,
            authority_hint: request.authority,
            channel: Channel::Request,
            quote: request.question.clone(),
            source_path: None,
            source_line: None,
            session_id: env
                .session_id
                .clone()
                .or_else(|| request.raiser.session.clone()),
            worktree: env.worktree.clone(),
            branch: env.branch.clone(),
            raised_ts: env.ts.clone(),
            raised_position: position,
            door: Some(request.door),
            blast: request.blast,
            authority: Some(request.authority),
            route: Some(route),
            timing: None,
            resolve_by: request.deadline.clone(),
            card: Some(card(request)),
            card_rev: Some(position),
            presented_ts: None,
            presented_tree: None,
            times_presented: 0,
            status,
            decision: None,
            deferral: None,
            default,
            dismissed: None,
            superseded_by: None,
            records: Vec::new(),
            links: Vec::new(),
            also_raised_in: Vec::new(),
            reopened: 0,
            last_position: position,
            last_ts: env.ts.clone(),
            request: Some(RequestRecord {
                position,
                request: request.clone(),
            }),
            reviews: 0,
            reopened_after_review: 0,
        }
    }
}

/// The option a call F request has already taken: a two-way judgment request with a default and
/// no deadline. Any other request waits on a human, whatever it names as its default.
fn call_f_default(request: &Request) -> Option<&'static str> {
    let own_call = request.door == Door::TwoWay
        && request.authority == Authority::Judgment
        && request.deadline.is_none();
    request
        .default
        .filter(|_| own_call)
        .map(|option| option.as_str())
}

/// The locator of the request's first anchor attachment, the item's `path:line`.
fn anchor(request: &Request) -> Option<String> {
    request
        .attachments
        .iter()
        .flatten()
        .find(|a| a.kind == ANCHOR)
        .map(|a| a.locator.clone())
}

/// What the item blocks: nothing for `none`, else the request's word, so it ranks ahead.
fn blocks(blocks: Blocks) -> Option<String> {
    match blocks {
        Blocks::None => None,
        Blocks::Merge | Blocks::Release | Blocks::Run | Blocks::Unknown => {
            Some(blocks.as_str().to_owned())
        }
    }
}

/// The card the request carries: its options as written, its recommendation and its
/// attachments as evidence.
fn card(request: &Request) -> Card {
    let raiser = &request.raiser;
    Card {
        why: format!(
            "Asked by {} through a decision request ({})",
            raiser.namespace.as_str(),
            raiser.key
        ),
        evidence: request
            .attachments
            .iter()
            .flatten()
            .map(|a| Evidence {
                src: a.locator.clone(),
                text: a.kind.clone(),
            })
            .collect(),
        options: request
            .options
            .iter()
            .map(|o| CardOption {
                id: o.id.as_str().to_owned(),
                title: o.label.clone(),
                holds_if: o.holds_when.clone(),
                if_wrong: o.cost_if_wrong.clone(),
                ..CardOption::default()
            })
            .collect(),
        recommendation: Recommendation {
            option: request.recommendation.as_str().to_owned(),
            ..Recommendation::default()
        },
        ..Card::default()
    }
}

#[cfg(test)]
mod tests;
