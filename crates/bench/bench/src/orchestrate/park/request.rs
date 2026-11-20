//! A park as Weigh-In's decision request (DR-02, decision D2): the question, its 2 to 4
//! options, the recommendation, one-way with no default, due at the end of `--resolve-by`.
//!
//! A ruling as one too (call F, decision D4 B): two-way, a judgment, defaulted to what was
//! ruled and with no deadline, which Weigh-In folds as already defaulted (DR-07).

use redkiln_core::envelope::Namespace;
use sha2::{Digest as _, Sha256};
use weighin_contract::request::{
    ANCHOR, Attachment, Blocks, OptionId, Raiser, Request, RequestOption, TrustNeeded,
};
use weighin_contract::vocab::{Authority, Door};

use crate::brief::ticket::Ticket;
use crate::decide::id::DecisionId;

/// The attachment kind that carries the Clef D2 decision that said park, or that a ruling
/// followed (T9), keeping 0.x's link from the decision to the item for labelling it later.
pub(super) const CLEF_DECISION: &str = "bench.clef_decision";

/// The session prefix of a raiser in GitHub Actions, which needs `attested` (DR-04).
const CI_SESSION: &str = "gha:";

/// One option of a park, its texts checked.
#[derive(Debug, Clone, Copy)]
pub(super) struct ParkOption<'a> {
    /// What the option is.
    pub(super) label: &'a str,
    /// When it is the right call.
    pub(super) holds_when: &'a str,
    /// What it costs if it is wrong.
    pub(super) cost_if_wrong: &'a str,
}

/// A park's arguments, each checked.
#[derive(Debug)]
pub(super) struct Park<'a> {
    /// The ticket the call blocks.
    pub(super) ticket: &'a Ticket,
    /// The question.
    pub(super) question: &'a str,
    /// The options, `a` to `d` in order.
    pub(super) options: Vec<ParkOption<'a>>,
    /// The recommended option.
    pub(super) recommend: OptionId,
    /// Where the call arises, `path:line`.
    pub(super) at: &'a str,
    /// The last day to decide, `YYYY-MM-DD`.
    pub(super) resolve_by: &'a str,
    /// The Clef D2 decision that said park.
    pub(super) decision: Option<DecisionId>,
    /// The session raising it.
    pub(super) session: Option<&'a str>,
}

/// A ruling's arguments, each checked.
#[derive(Debug)]
pub(super) struct Ruling<'a> {
    /// The ticket the call was made in.
    pub(super) ticket: &'a Ticket,
    /// What was ruled.
    pub(super) what: &'a str,
    /// Why.
    pub(super) why: &'a str,
    /// What undoing it costs if the ruling is wrong.
    pub(super) cost: &'a str,
    /// The Clef D2 decision behind the ruling.
    pub(super) decision: Option<DecisionId>,
    /// The session ruling.
    pub(super) session: Option<&'a str>,
}

/// The label of a ruling's option `b`.
const UNDO_LABEL: &str = "Undo the ruling";

/// When a ruling's option `b` holds.
const UNDO_HOLDS: &str = "the reason given does not hold";

/// What a ruling's option `b` costs if it is wrong.
const UNDO_COST: &str = "the ruled work is undone for nothing";

impl Ruling<'_> {
    /// The decision request: see the module documentation. Its guards are checked when it is
    /// raised (`Request::check`).
    pub(super) fn request(&self) -> Request {
        let option = |id, label: &str, holds_when: &str, cost_if_wrong: &str| RequestOption {
            id,
            label: label.to_owned(),
            holds_when: holds_when.to_owned(),
            cost_if_wrong: cost_if_wrong.to_owned(),
        };
        let attachments = self.decision.as_ref().map(|decision| vec![clef(decision)]);
        Request {
            question: format!("Ticket {}: {}", self.ticket, self.what),
            options: vec![
                option(OptionId::A, self.what, self.why, self.cost),
                option(OptionId::B, UNDO_LABEL, UNDO_HOLDS, UNDO_COST),
            ],
            recommendation: OptionId::A,
            door: Door::TwoWay,
            authority: Authority::Judgment,
            blast: None,
            default: Some(OptionId::A),
            deadline: None,
            blocks: Blocks::None,
            trust_needed: trust_needed(self.session),
            raiser: Raiser {
                namespace: Namespace::Bench.into(),
                key: raiser_key("rule", self.ticket, self.what),
                run: None,
                session: self.session.map(str::to_owned),
            },
            subject: None,
            attachments,
        }
    }
}

impl Park<'_> {
    /// The decision request: see the module documentation. Its guards are checked when it is
    /// raised (`Request::check`).
    pub(super) fn request(&self) -> Request {
        let options = self
            .options
            .iter()
            .zip(OptionId::ALL)
            .map(|(option, id)| RequestOption {
                id,
                label: option.label.to_owned(),
                holds_when: option.holds_when.to_owned(),
                cost_if_wrong: option.cost_if_wrong.to_owned(),
            })
            .collect();
        let mut attachments = vec![Attachment {
            kind: ANCHOR.to_owned(),
            locator: self.at.to_owned(),
            hash: None,
        }];
        attachments.extend(self.decision.as_ref().map(clef));
        Request {
            question: self.question.to_owned(),
            options,
            recommendation: self.recommend,
            door: Door::OneWay,
            authority: Authority::Authority,
            blast: None,
            default: None,
            deadline: Some(format!("{}T23:59:59Z", self.resolve_by)),
            blocks: Blocks::Run,
            trust_needed: trust_needed(self.session),
            raiser: Raiser {
                namespace: Namespace::Bench.into(),
                key: raiser_key("park", self.ticket, self.question),
                run: None,
                session: self.session.map(str::to_owned),
            },
            subject: None,
            attachments: Some(attachments),
        }
    }
}

/// The trust an answer needs (DR-04): `attested` for a raiser in GitHub Actions, else
/// `witnessed`.
fn trust_needed(session: Option<&str>) -> TrustNeeded {
    if session.is_some_and(|s| s.starts_with(CI_SESSION)) {
        TrustNeeded::Attested
    } else {
        TrustNeeded::Witnessed
    }
}

/// The attachment that carries the Clef D2 `decision` (T9).
fn clef(decision: &DecisionId) -> Attachment {
    Attachment {
        kind: CLEF_DECISION.to_owned(),
        locator: decision.as_str().to_owned(),
        hash: None,
    }
}

/// The raiser key of a park or a ruling (T10): `orchestrate/<verb>/<ticket>/<16 hex of
/// sha256(text)>`, its question or what it ruled, so the same text on the same ticket is the
/// same item.
fn raiser_key(verb: &str, ticket: &Ticket, text: &str) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(text.as_bytes());
    let mut key = format!("orchestrate/{verb}/{ticket}/");
    for byte in digest.iter().take(8) {
        for nibble in [byte >> 4, byte & 0x0f] {
            if let Some(digit) = DIGITS.get(usize::from(nibble)) {
                key.push(char::from(*digit));
            }
        }
    }
    key
}
