//! A park as Weigh-In's decision request (DR-02, decision D2): the question, its 2 to 4
//! options, the recommendation, one-way with no default, due at the end of `--resolve-by`.

use redkiln_core::envelope::Namespace;
use sha2::{Digest as _, Sha256};
use weighin_contract::request::{
    ANCHOR, Attachment, Blocks, OptionId, Raiser, Request, RequestOption, TrustNeeded,
};
use weighin_contract::vocab::{Authority, Door};

use crate::brief::ticket::Ticket;
use crate::decide::id::DecisionId;

/// The attachment kind that carries the Clef D2 decision that said park (T9), keeping 0.x's
/// link from the decision to the item for labelling it later.
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
        let trust_needed = if self.session.is_some_and(|s| s.starts_with(CI_SESSION)) {
            TrustNeeded::Attested
        } else {
            TrustNeeded::Witnessed
        };
        let mut attachments = vec![Attachment {
            kind: ANCHOR.to_owned(),
            locator: self.at.to_owned(),
            hash: None,
        }];
        if let Some(decision) = &self.decision {
            attachments.push(Attachment {
                kind: CLEF_DECISION.to_owned(),
                locator: decision.as_str().to_owned(),
                hash: None,
            });
        }
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
            trust_needed,
            raiser: Raiser {
                namespace: Namespace::Bench.into(),
                key: park_key(self.ticket, self.question),
                run: None,
                session: self.session.map(str::to_owned),
            },
            subject: None,
            attachments: Some(attachments),
        }
    }
}

/// The raiser key of a park (T10): `orchestrate/park/<ticket>/<16 hex of sha256(question)>`,
/// so the same question on the same ticket is the same item.
pub(super) fn park_key(ticket: &Ticket, question: &str) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(question.as_bytes());
    let mut key = format!("orchestrate/park/{ticket}/");
    for byte in digest.iter().take(8) {
        for nibble in [byte >> 4, byte & 0x0f] {
            if let Some(digit) = DIGITS.get(usize::from(nibble)) {
                key.push(char::from(*digit));
            }
        }
    }
    key
}
