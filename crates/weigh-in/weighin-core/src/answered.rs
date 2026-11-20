//! Whether a person's answer to a shown question witnesses a pick of one of an item's options.
//!
//! A pick read from an answered dialog may be recorded at `witnessed` trust only when the code
//! that records it took the answer from the surface itself (SPEC TR-03), so a raiser whose
//! request needs `witnessed` can accept it (DR-06). This module decides, from plain values, when
//! such an answer names exactly one option the person chose; reading those values from a hook's
//! input is the binary's harness adapter's job, and anything given here as a CLI argument stays
//! `asserted` (TR-05).
//!
//! [`witness`] returns a [`Choice`] only when all of these hold, and otherwise the first
//! [`Unwitnessed`] reason in this order:
//!
//! 1. the person resolved the dialog: no idle timeout, follow-up or free-text response;
//! 2. the question is single-select;
//! 3. the item's card passes [`Card::validate`], so its option ids are unique, no option's
//!    title ends with [`RECOMMENDED_SUFFIX`], and no two options can be shown under the same
//!    label, counting the recommended option's suffixed form;
//! 4. the question is the item's own: `<id> <question>` byte for byte, followed by
//!    [`ONE_WAY_SUFFIX`] exactly when the item's door is not two-way (one-way or unsure), and
//!    the item's question is not blank;
//! 5. no label was shown twice, and no option under two labels;
//! 6. every eligible option (one that fails no MUST) was shown under its [`label`]; the
//!    recommended option may instead carry [`RECOMMENDED_SUFFIX`] once, but only when the
//!    item's door is two-way and the card earned its recommendation
//!    ([`Card::recommendation_earned`]), as a sitting presents it;
//! 7. exactly one label was selected, and it was one of the labels shown;
//! 8. that label names one of the card's options, and that option is eligible.
//!
//! A shown label that names no option, such as a `Defer to …` choice, is allowed, but picking it
//! is not a choice of an option. What the dialog showed besides the question and the labels
//! (option descriptions, the header) is not checked here.

use crate::marker::Door;
use std::collections::BTreeSet;
use weighin_contract::ItemId;
use weighin_contract::card::{Card, CardError, CardOption};

/// The suffix a shown question carries after the item's question exactly when the item's door
/// is not two-way: a sitting presents such an item options first.
pub const ONE_WAY_SUFFIX: &str = " (one-way: options first)";

/// The one suffix the recommended option's label may carry after [`label`].
///
/// It is allowed only when the item's door is two-way and the card earned its recommendation:
/// what a sitting appends to the recommendation it pre-selects.
pub const RECOMMENDED_SUFFIX: &str = " (Recommended)";

/// A question as the dialog showed it to the person.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ShownQuestion {
    /// The question text as shown: `<id> <question>` for an item's own question.
    pub text: String,
    /// Whether the dialog let the person select more than one option.
    pub multi_select: bool,
    /// The option labels shown, in the order shown.
    pub labels: Vec<String>,
}

/// How the dialog ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    /// The person resolved it by selecting among the options shown.
    Person,
    /// Nobody answered before the dialog's idle timeout.
    AfkTimeout,
    /// The person asked a follow-up question instead of answering.
    FollowUp,
    /// The person typed their own response instead of selecting a shown option.
    FreeText,
}

/// The person's answer to one shown question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// How the dialog ended.
    pub resolution: Resolution,
    /// The labels selected, as the dialog reported them.
    pub selected: Vec<String>,
}

/// The item a question is checked against: its id, its question as raised and its card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemCard<'a> {
    /// The item's id.
    pub id: &'a ItemId,
    /// The item's question as first raised (`Item::question`).
    pub question: &'a str,
    /// The item's card from its latest grounding.
    pub card: &'a Card,
    /// The item's door as triaged, else as hinted (`Item::door`). Only a two-way item is shown
    /// without [`ONE_WAY_SUFFIX`] and may have its recommendation marked.
    pub door: Door,
}

/// A witnessed pick: the option the person chose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    /// The chosen option's id on the item's card.
    pub option: String,
}

/// Why an answer does not witness a pick of one option.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum Unwitnessed {
    /// The dialog ended at its idle timeout, so nobody answered.
    #[error("the dialog timed out with nobody answering")]
    AfkTimeout,
    /// The person asked a follow-up question instead of answering.
    #[error("the person asked a follow-up instead of answering")]
    FollowUp,
    /// The person typed their own response, which names no shown option for certain.
    #[error("the person typed a response instead of selecting an option")]
    FreeText,
    /// The item's card breaks a rule every card must pass, such as repeated option ids; cards
    /// folded from the store or imported ledgers are not otherwise re-validated.
    #[error("the item's card is not valid: {0}")]
    InvalidCard(#[source] CardError),
    /// An option's title ends with [`RECOMMENDED_SUFFIX`], so its label would claim a
    /// recommendation the card did not make; the option's id.
    #[error("option `{0}` has a title that claims the recommendation")]
    TitleMarksRecommended(String),
    /// Two options can be shown under the same label, counting the recommended option's
    /// suffixed form; the label.
    #[error("two options can be shown as `{0}`")]
    CollidingLabels(String),
    /// The question let the person select several options.
    #[error("the question was multi-select")]
    MultiSelect,
    /// The question shown is not the item's own question.
    #[error("the question shown is not the item's own")]
    NotTheItemsQuestion,
    /// The same label was shown more than once; the label.
    #[error("the label `{0}` was shown more than once")]
    DuplicateLabel(String),
    /// More than one shown label names the same option; the option's id.
    #[error("option `{0}` was shown under more than one label")]
    OptionShownTwice(String),
    /// An eligible option was not shown under its label; the option's id.
    #[error("eligible option `{0}` was not shown")]
    OptionNotShown(String),
    /// Not exactly one label was selected; how many were.
    #[error("{0} labels were selected, not one")]
    SelectionCount(usize),
    /// The selected label is not one of the labels shown; the label.
    #[error("the selected label `{0}` was not shown")]
    LabelNotShown(String),
    /// The selected label is no option's label; the label.
    #[error("the selected label `{0}` names no option")]
    UnknownLabel(String),
    /// The selected option fails a MUST, so it cannot be decided; the option's id.
    #[error("the selected option `{0}` fails a MUST")]
    IneligibleOption(String),
}

/// The label an option is shown under: `<id>: <title>`.
///
/// # Examples
///
/// ```
/// use weighin_core::answered::label;
/// use weighin_core::card::CardOption;
///
/// let option = CardOption { id: "A".into(), title: "Per series".into(), ..Default::default() };
/// assert_eq!(label(&option), "A: Per series");
/// ```
#[must_use]
pub fn label(option: &CardOption) -> String {
    format!("{}: {}", option.id, option.title)
}

/// The option the person picked, when `answer` to `shown` witnesses a pick of one of `item`'s
/// options (see the [module docs](self) for the rules, in the order they are checked).
///
/// # Errors
/// Returns the first rule the answer breaks, as an [`Unwitnessed`] reason.
///
/// # Examples
///
/// ```
/// use weighin_core::answered::{
///     Answer, Choice, ItemCard, Resolution, ShownQuestion, Unwitnessed, witness,
/// };
/// use weighin_core::card::{Card, CardOption, Recommendation};
/// use weighin_core::marker::Door;
/// use weighin_core::ItemId;
///
/// let option = |id: &str, title: &str| CardOption {
///     id: id.into(),
///     title: title.into(),
///     ..Default::default()
/// };
/// let card = Card {
///     options: vec![option("A", "Per series"), option("B", "Per record")],
///     recommendation: Recommendation { option: "A".into(), ..Default::default() },
///     ..Default::default()
/// };
/// let id = ItemId::mint("s1", "Keep holds per series?");
/// let item = ItemCard {
///     id: &id,
///     question: "Keep holds per series?",
///     card: &card,
///     door: Door::TwoWay,
/// };
/// let shown = ShownQuestion {
///     text: format!("{id} Keep holds per series?"),
///     multi_select: false,
///     labels: vec!["A: Per series".into(), "B: Per record".into()],
/// };
///
/// let picked = Answer { resolution: Resolution::Person, selected: vec!["B: Per record".into()] };
/// assert_eq!(witness(&shown, &picked, &item), Ok(Choice { option: "B".into() }));
///
/// let idle = Answer { resolution: Resolution::AfkTimeout, selected: vec![] };
/// assert_eq!(witness(&shown, &idle, &item), Err(Unwitnessed::AfkTimeout));
/// ```
pub fn witness(
    shown: &ShownQuestion,
    answer: &Answer,
    item: &ItemCard<'_>,
) -> Result<Choice, Unwitnessed> {
    match answer.resolution {
        Resolution::Person => {}
        Resolution::AfkTimeout => return Err(Unwitnessed::AfkTimeout),
        Resolution::FollowUp => return Err(Unwitnessed::FollowUp),
        Resolution::FreeText => return Err(Unwitnessed::FreeText),
    }
    if shown.multi_select {
        return Err(Unwitnessed::MultiSelect);
    }
    let card = item.card;
    check_card(card)?;
    // The door, not the text the model wrote, decides how the question must be shown: only a
    // two-way item goes without the one-way suffix, and only its recommendation may be marked.
    let two_way = item.door == Door::TwoWay;
    if items_question(&shown.text, item) != Some(!two_way) {
        return Err(Unwitnessed::NotTheItemsQuestion);
    }
    let marked = two_way && card.recommendation_earned();
    let names = |o: &CardOption, l: &str| names(card, marked, o, l);
    let mut seen = BTreeSet::new();
    if let Some(repeated) = shown.labels.iter().find(|l| !seen.insert(l.as_str())) {
        return Err(Unwitnessed::DuplicateLabel(repeated.clone()));
    }
    if let Some(twice) = card
        .options
        .iter()
        .find(|o| shown.labels.iter().filter(|l| names(o, l)).nth(1).is_some())
    {
        return Err(Unwitnessed::OptionShownTwice(twice.id.clone()));
    }
    if let Some(missing) = card
        .options
        .iter()
        .filter(|o| o.fails_must.is_none())
        .find(|o| !shown.labels.iter().any(|l| names(o, l)))
    {
        return Err(Unwitnessed::OptionNotShown(missing.id.clone()));
    }
    let picked = match answer.selected.as_slice() {
        [one] => one,
        many => return Err(Unwitnessed::SelectionCount(many.len())),
    };
    if !shown.labels.contains(picked) {
        return Err(Unwitnessed::LabelNotShown(picked.clone()));
    }
    // No two options share a label (`shared_label`), so the first that matches is the only one.
    let Some(option) = card.options.iter().find(|o| names(o, picked)) else {
        return Err(Unwitnessed::UnknownLabel(picked.clone()));
    };
    if option.fails_must.is_some() {
        return Err(Unwitnessed::IneligibleOption(option.id.clone()));
    }
    Ok(Choice {
        option: option.id.clone(),
    })
}

/// Rule 3: the card is valid, no title claims the recommendation and no two options share a
/// label.
fn check_card(card: &Card) -> Result<(), Unwitnessed> {
    card.validate().map_err(Unwitnessed::InvalidCard)?;
    if let Some(o) = card
        .options
        .iter()
        .find(|o| o.title.ends_with(RECOMMENDED_SUFFIX))
    {
        return Err(Unwitnessed::TitleMarksRecommended(o.id.clone()));
    }
    match shared_label(card) {
        Some(shared) => Err(Unwitnessed::CollidingLabels(shared)),
        None => Ok(()),
    }
}

/// Whether the shown label `shown` names `option` of `card`: its [`label`], or, when `marked`
/// allows it and `option` is the card's recommended one, its label with [`RECOMMENDED_SUFFIX`]
/// once.
fn names(card: &Card, marked: bool, option: &CardOption, shown: &str) -> bool {
    let own = label(option);
    shown == own
        || (marked
            && option.id == card.recommendation.option
            && shown.strip_suffix(RECOMMENDED_SUFFIX) == Some(own.as_str()))
}

/// A label two of `card`'s options could both be shown under, counting the recommended
/// option's suffixed form whether or not the recommendation is marked.
fn shared_label(card: &Card) -> Option<String> {
    let mut forms = Vec::with_capacity(card.options.len().saturating_add(1));
    for o in &card.options {
        let own = label(o);
        if o.id == card.recommendation.option {
            forms.push(format!("{own}{RECOMMENDED_SUFFIX}"));
        }
        forms.push(own);
    }
    // An option's two forms differ, so a repeat is always two options.
    let mut seen = BTreeSet::new();
    forms.iter().find(|f| !seen.insert(f.as_str())).cloned()
}

/// Whether `text` is `<id> <question>` for `item`: `Some(false)` when exactly, `Some(true)`
/// when followed by [`ONE_WAY_SUFFIX`], `None` otherwise. An item with a blank question has no
/// question of its own to match.
fn items_question(text: &str, item: &ItemCard<'_>) -> Option<bool> {
    if item.question.trim().is_empty() {
        return None;
    }
    let rest = text
        .strip_prefix(item.id.as_str())
        .and_then(|r| r.strip_prefix(' '))
        .and_then(|r| r.strip_prefix(item.question))?;
    match rest {
        "" => Some(false),
        ONE_WAY_SUFFIX => Some(true),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use weighin_contract::card::{EvasionCheck, Recommendation, Stability};

    const QUESTION: &str = "Keep holds per series?";

    fn opt(id: &str) -> CardOption {
        CardOption {
            id: id.into(),
            title: format!("Option {id}"),
            ..Default::default()
        }
    }

    /// Options A and B are eligible; C fails a MUST.
    fn card() -> Card {
        let mut ineligible = opt("C");
        ineligible.fails_must = Some("breaks the public contract".into());
        Card {
            options: vec![opt("A"), opt("B"), ineligible],
            recommendation: Recommendation {
                option: "A".into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn id() -> ItemId {
        ItemId::mint("s1", QUESTION)
    }

    fn shown(id: &ItemId) -> ShownQuestion {
        ShownQuestion {
            text: format!("{id} {QUESTION}"),
            multi_select: false,
            labels: vec![
                "A: Option A".into(),
                "B: Option B".into(),
                "C: Option C".into(),
                "Defer to the DPA review (default A by 2026-12-01)".into(),
            ],
        }
    }

    fn picked(label: &str) -> Answer {
        Answer {
            resolution: Resolution::Person,
            selected: vec![label.into()],
        }
    }

    /// Check against the two-way item asking [`QUESTION`].
    fn check(shown: &ShownQuestion, answer: &Answer, card: &Card) -> Result<Choice, Unwitnessed> {
        check_item(shown, answer, card, (QUESTION, Door::TwoWay))
    }

    fn check_item(
        shown: &ShownQuestion,
        answer: &Answer,
        card: &Card,
        (question, door): (&str, Door),
    ) -> Result<Choice, Unwitnessed> {
        let id = id();
        witness(
            shown,
            answer,
            &ItemCard {
                id: &id,
                question,
                card,
                door,
            },
        )
    }

    #[test]
    fn a_person_picking_one_eligible_option_of_the_items_question_is_a_choice() {
        assert_eq!(
            check(&shown(&id()), &picked("B: Option B"), &card()),
            Ok(Choice { option: "B".into() })
        );
    }

    #[test]
    fn a_one_way_or_unsure_question_with_its_suffix_is_still_the_items_own() {
        let mut s = shown(&id());
        s.text.push_str(ONE_WAY_SUFFIX);
        for door in [Door::OneWay, Door::Unsure] {
            assert_eq!(
                check_item(&s, &picked("A: Option A"), &card(), (QUESTION, door)),
                Ok(Choice { option: "A".into() }),
                "{door:?}"
            );
        }
    }

    #[test]
    fn the_one_way_suffix_must_match_the_items_door() {
        let mut one_way = shown(&id());
        one_way.text.push_str(ONE_WAY_SUFFIX);
        assert_eq!(
            check(&one_way, &picked("A: Option A"), &card()),
            Err(Unwitnessed::NotTheItemsQuestion)
        );
        for door in [Door::OneWay, Door::Unsure] {
            assert_eq!(
                check_item(
                    &shown(&id()),
                    &picked("A: Option A"),
                    &card(),
                    (QUESTION, door)
                ),
                Err(Unwitnessed::NotTheItemsQuestion),
                "{door:?}"
            );
        }
    }

    #[test]
    fn a_one_way_item_shown_as_two_way_with_its_recommendation_marked_is_unwitnessed() {
        let mut s = shown(&id());
        s.labels = vec!["A: Option A (Recommended)".into(), "B: Option B".into()];
        assert_eq!(
            check_item(
                &s,
                &picked("A: Option A (Recommended)"),
                &card(),
                (QUESTION, Door::OneWay)
            ),
            Err(Unwitnessed::NotTheItemsQuestion)
        );
    }

    #[test]
    fn an_ineligible_option_need_not_be_shown() {
        let mut s = shown(&id());
        s.labels.retain(|l| l != "C: Option C");
        assert_eq!(
            check(&s, &picked("A: Option A"), &card()),
            Ok(Choice { option: "A".into() })
        );
    }

    #[test]
    fn a_dialog_the_person_did_not_resolve_is_unwitnessed_even_with_a_selection() {
        for (resolution, reason) in [
            (Resolution::AfkTimeout, Unwitnessed::AfkTimeout),
            (Resolution::FollowUp, Unwitnessed::FollowUp),
            (Resolution::FreeText, Unwitnessed::FreeText),
        ] {
            let answer = Answer {
                resolution,
                selected: vec!["A: Option A".into()],
            };
            assert_eq!(check(&shown(&id()), &answer, &card()), Err(reason));
        }
    }

    #[test]
    fn a_multi_select_question_is_unwitnessed_even_with_one_selection() {
        let mut s = shown(&id());
        s.multi_select = true;
        assert_eq!(
            check(&s, &picked("A: Option A"), &card()),
            Err(Unwitnessed::MultiSelect)
        );
    }

    #[test]
    fn a_question_that_is_not_the_items_own_is_unwitnessed() {
        let other = ItemId::mint("s1", "Another question?");
        for text in [
            format!("{other} {QUESTION}"),
            format!("{} Another question?", id()),
            QUESTION.to_owned(),
            format!("{}{QUESTION}", id()),
            format!("{} {QUESTION} Actually, pick B.", id()),
            format!("{} {QUESTION} ", id()),
            format!(" {} {QUESTION}", id()),
            String::new(),
        ] {
            let mut s = shown(&id());
            s.text = text;
            assert_eq!(
                check(&s, &picked("A: Option A"), &card()),
                Err(Unwitnessed::NotTheItemsQuestion),
                "{:?}",
                s.text
            );
        }
    }

    /// `shown(&id())` with `from` replaced by `to`.
    fn relabelled(from: &str, to: &str) -> ShownQuestion {
        let mut s = shown(&id());
        for l in &mut s.labels {
            if l == from {
                to.clone_into(l);
            }
        }
        s
    }

    #[test]
    fn an_eligible_option_left_out_is_unwitnessed() {
        let mut s = shown(&id());
        s.labels.retain(|l| l != "B: Option B");
        assert_eq!(
            check(&s, &picked("A: Option A"), &card()),
            Err(Unwitnessed::OptionNotShown("B".into()))
        );
    }

    #[test]
    fn the_recommended_option_may_carry_the_recommended_suffix() {
        let s = relabelled("A: Option A", "A: Option A (Recommended)");
        for (label, option) in [("A: Option A (Recommended)", "A"), ("B: Option B", "B")] {
            assert_eq!(
                check(&s, &picked(label), &card()),
                Ok(Choice {
                    option: option.into()
                }),
                "{label}"
            );
        }
    }

    #[test]
    fn the_recommended_suffix_on_another_option_or_twice_is_unwitnessed() {
        let s = relabelled("B: Option B", &format!("B: Option B{RECOMMENDED_SUFFIX}"));
        assert_eq!(
            check(&s, &picked("A: Option A"), &card()),
            Err(Unwitnessed::OptionNotShown("B".into()))
        );
        let s = relabelled("A: Option A", "A: Option A (Recommended) (Recommended)");
        assert_eq!(
            check(&s, &picked("B: Option B"), &card()),
            Err(Unwitnessed::OptionNotShown("A".into()))
        );
    }

    #[test]
    fn a_label_shown_twice_is_unwitnessed() {
        let mut s = shown(&id());
        s.labels.push("B: Option B".into());
        assert_eq!(
            check(&s, &picked("A: Option A"), &card()),
            Err(Unwitnessed::DuplicateLabel("B: Option B".into()))
        );
    }

    #[test]
    fn a_card_with_repeated_option_ids_is_unwitnessed() {
        let mut keep = opt("A");
        "Keep".clone_into(&mut keep.title);
        let mut drop = opt("A");
        "Drop".clone_into(&mut drop.title);
        let mut c = card();
        c.options = vec![keep, drop];
        let mut s = shown(&id());
        s.labels = vec!["A: Keep".into(), "A: Drop".into()];
        assert_eq!(
            check(&s, &picked("A: Drop"), &c),
            Err(Unwitnessed::InvalidCard(CardError::DuplicateOption(
                "A".into()
            )))
        );
    }

    #[test]
    fn an_item_with_an_empty_question_is_unwitnessed() {
        for question in ["", " "] {
            let mut s = shown(&id());
            s.text = format!("{} {question}", id());
            assert_eq!(
                check_item(
                    &s,
                    &picked("A: Option A"),
                    &card(),
                    (question, Door::TwoWay)
                ),
                Err(Unwitnessed::NotTheItemsQuestion),
                "{question:?}"
            );
        }
    }

    #[test]
    fn not_exactly_one_selection_is_unwitnessed() {
        for selected in [vec![], vec!["A: Option A".into(), "B: Option B".into()]] {
            let n = selected.len();
            let answer = Answer {
                resolution: Resolution::Person,
                selected,
            };
            assert_eq!(
                check(&shown(&id()), &answer, &card()),
                Err(Unwitnessed::SelectionCount(n))
            );
        }
    }

    #[test]
    fn a_selected_label_that_was_not_shown_is_unwitnessed() {
        let mut s = shown(&id());
        s.labels.retain(|l| l != "C: Option C");
        assert_eq!(
            check(&s, &picked("C: Option C"), &card()),
            Err(Unwitnessed::LabelNotShown("C: Option C".into()))
        );
        assert_eq!(
            check(&shown(&id()), &picked("a: option a"), &card()),
            Err(Unwitnessed::LabelNotShown("a: option a".into()))
        );
    }

    #[test]
    fn a_shown_label_that_names_no_option_is_unwitnessed() {
        let defer = "Defer to the DPA review (default A by 2026-12-01)";
        assert_eq!(
            check(&shown(&id()), &picked(defer), &card()),
            Err(Unwitnessed::UnknownLabel(defer.into()))
        );
    }

    #[test]
    fn a_card_on_which_two_options_share_a_label_is_unwitnessed() {
        let titled = |id: &str, title: &str| CardOption {
            id: id.into(),
            title: title.into(),
            ..Default::default()
        };
        let mut c = card();
        c.options = vec![titled("X: y", "z"), titled("X", "y: z"), titled("C", "c")];
        "C".clone_into(&mut c.recommendation.option);
        let mut s = shown(&id());
        s.labels = vec!["X: y: z".into(), "C: c".into()];
        assert_eq!(
            check(&s, &picked("C: c"), &c),
            Err(Unwitnessed::CollidingLabels("X: y: z".into()))
        );
    }

    #[test]
    fn an_option_title_that_claims_the_recommendation_is_unwitnessed() {
        let mut c = card();
        if let Some(b) = c.options.get_mut(1) {
            "Option B (Recommended)".clone_into(&mut b.title);
        }
        let s = relabelled("B: Option B", "B: Option B (Recommended)");
        assert_eq!(
            check(&s, &picked("A: Option A"), &c),
            Err(Unwitnessed::TitleMarksRecommended("B".into()))
        );
    }

    #[test]
    fn the_recommended_suffix_on_a_one_way_question_names_no_option() {
        let mut s = relabelled("A: Option A", "A: Option A (Recommended)");
        s.text.push_str(ONE_WAY_SUFFIX);
        let marked = picked("A: Option A (Recommended)");
        for door in [Door::OneWay, Door::Unsure] {
            assert_eq!(
                check_item(&s, &marked, &card(), (QUESTION, door)),
                Err(Unwitnessed::OptionNotShown("A".into())),
                "{door:?}"
            );
        }
        s.labels.push("A: Option A".into());
        assert_eq!(
            check_item(&s, &marked, &card(), (QUESTION, Door::OneWay)),
            Err(Unwitnessed::UnknownLabel(
                "A: Option A (Recommended)".into()
            ))
        );
    }

    #[test]
    fn the_recommended_suffix_on_an_unearned_recommendation_names_no_option() {
        let mut evasive = card();
        evasive.evasion = Some(EvasionCheck {
            passed: false,
            flags: vec!["revisit later".into()],
        });
        let mut unstable = card();
        unstable.recommendation.stability = Some(Stability {
            agree: 1,
            of: 4,
            ..Default::default()
        });
        let s = relabelled("A: Option A", "A: Option A (Recommended)");
        for c in [evasive, unstable] {
            assert_eq!(
                check(&s, &picked("A: Option A (Recommended)"), &c),
                Err(Unwitnessed::OptionNotShown("A".into()))
            );
        }
    }

    #[test]
    fn an_option_shown_under_two_labels_is_unwitnessed() {
        let mut s = shown(&id());
        s.labels.push("A: Option A (Recommended)".into());
        assert_eq!(
            check(&s, &picked("B: Option B"), &card()),
            Err(Unwitnessed::OptionShownTwice("A".into()))
        );
    }

    #[test]
    fn an_option_that_fails_a_must_is_unwitnessed_even_when_shown_and_picked() {
        assert_eq!(
            check(&shown(&id()), &picked("C: Option C"), &card()),
            Err(Unwitnessed::IneligibleOption("C".into()))
        );
    }
}
