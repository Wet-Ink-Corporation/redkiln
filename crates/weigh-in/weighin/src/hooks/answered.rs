//! `PostToolUse` of the dialog that asks the person questions: each Weigh-In card the person
//! answered there is decided at `witnessed` trust, from the hook's own input (TR-03, DR-06).
//!
//! A question is a card when its text starts with an item id the ledger holds (`<id> `, as a
//! sitting shows it, SKILL.md step 4); any other question is not Weigh-In's and is passed over
//! in silence. A card of an item that is no longer open is passed over too, so a repeated payload
//! writes nothing. For a card of an open item with a card, `weighin_core::answered::witness`
//! decides whether the answer names one option the person picked: when it does, the decision is
//! written with the [`Witness`] of this payload (`Session::decide_witnessed`), and when it does
//! not, nothing is written. Either way the model is told in one line of context, so batch 2 of
//! the sitting records only what the hook did not. Failures are logged and noted like any hook's
//! and never fail the session.

use std::collections::BTreeMap;
use weighin_core::ItemId;
use weighin_core::answered::{self, ItemCard};

use super::{Answered, Asked, Failures, Hook, HookOutput};
use crate::ops::Session;
use crate::witness::Witness;

/// The actor the hook's session writes as; the decision's envelope names the owner instead.
const ACTOR: &str = "hook";

impl Hook {
    /// Decide each card `answered` witnesses a pick of, and say what was and was not recorded.
    pub(super) fn answered(
        &self,
        failures: &mut Failures,
        answered: Option<&Answered>,
    ) -> HookOutput {
        // A dialog that shows no card is not Weigh-In's: nothing is read, written or logged.
        let Some(answered) = answered.filter(|a| a.asked.iter().any(|q| card_id(q).is_some()))
        else {
            return HookOutput::default();
        };
        let witness = match Witness::ask_user_question(answered) {
            Ok(witness) => witness,
            Err(e) => {
                let e = anyhow::Error::new(e).context("the dialog's tool use id is no evidence");
                self.log(failures, &e);
                return HookOutput::default();
            }
        };
        let mut session = match Session::open(&self.env, ACTOR) {
            Ok(session) => session,
            Err(e) => {
                self.log(failures, &e.context("reading the ledger after the dialog"));
                return HookOutput::default();
            }
        };
        let shown = times_shown(&answered.asked);
        let mut lines = Vec::new();
        for asked in &answered.asked {
            let repeated = shown.get(asked.shown.text.as_str()).is_some_and(|n| *n > 1);
            match record(&mut session, asked, repeated, &witness) {
                Ok(Some(line)) => lines.push(line),
                Ok(None) => {}
                Err(e) => self.log(failures, &e),
            }
        }
        HookOutput {
            context: (!lines.is_empty()).then(|| lines.join("\n")),
            message: None,
        }
    }
}

/// How many times each question text was shown: the answers are keyed by that text, so a text
/// shown twice cannot say which of the two an answer was to.
fn times_shown(asked: &[Asked]) -> BTreeMap<&str, usize> {
    let mut shown = BTreeMap::new();
    for a in asked {
        let n = shown.entry(a.shown.text.as_str()).or_insert(0_usize);
        *n = n.saturating_add(1);
    }
    shown
}

/// The item id a card's question starts with (`<id> <question>`), when it has one.
fn card_id(asked: &Asked) -> Option<ItemId> {
    asked
        .shown
        .text
        .split_once(' ')
        .and_then(|(first, _)| ItemId::parse(first))
}

/// Decide the card `asked` shows when its answer witnesses a pick, and the line saying what
/// happened; `None` for a question that is no open item's card.
///
/// # Errors
/// The decision could not be written.
fn record(
    session: &mut Session<'_>,
    asked: &Asked,
    repeated: bool,
    witness: &Witness,
) -> anyhow::Result<Option<String>> {
    let Some(id) = card_id(asked) else {
        return Ok(None);
    };
    let Some(item) = session.ledger.get(&id) else {
        return Ok(None);
    };
    if !item.status.is_open() {
        return Ok(None);
    }
    let unrecorded = |why: &str| {
        Some(format!(
            "Weigh-In did not record {id} from the dialog: {why}; record the person's answer in \
             batch 2 as usual."
        ))
    };
    if repeated {
        return Ok(unrecorded("its question was shown more than once"));
    }
    let Some(card) = &item.card else {
        return Ok(unrecorded("it has no card"));
    };
    let of_item = ItemCard {
        id: &item.id,
        question: &item.question,
        card,
        door: item.door(),
    };
    let option = match answered::witness(&asked.shown, &asked.answer, &of_item) {
        Ok(choice) => choice.option,
        Err(why) => return Ok(unrecorded(&why.to_string())),
    };
    let receipt = session.decide_witnessed(&id, &option, witness)?;
    Ok(Some(format!(
        "Weigh-In recorded the person's answer from the dialog, witnessed: {receipt}. Leave {id} \
         out of batch 2."
    )))
}
