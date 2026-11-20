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
//! 3. the question is the item's own: `<id> <question>` exactly, or with [`ONE_WAY_SUFFIX`];
//! 4. every eligible option (one that fails no MUST) was shown, under its [`label`];
//! 5. exactly one label was selected, and it was one of the labels shown;
//! 6. that label is the label of exactly one of the card's options, and that option is eligible.
//!
//! A shown label that names no option, such as a `Defer to …` choice, is allowed, but picking it
//! is not a choice of an option.

use weighin_contract::ItemId;
use weighin_contract::card::{Card, CardOption};

/// The one suffix a shown question may carry after the item's question: what a sitting appends
/// to a one-way item's question, which it presents options first.
pub const ONE_WAY_SUFFIX: &str = " (one-way: options first)";

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
}

/// A witnessed pick: the option the person chose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    /// The chosen option's id on the item's card.
    pub option: String,
}

/// Why an answer does not witness a pick of one option.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
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
    /// The question let the person select several options.
    #[error("the question was multi-select")]
    MultiSelect,
    /// The question shown is not the item's own question.
    #[error("the question shown is not the item's own")]
    NotTheItemsQuestion,
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
    /// The selected label is the label of more than one option; the label.
    #[error("the selected label `{0}` names more than one option")]
    AmbiguousLabel(String),
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
/// let item = ItemCard { id: &id, question: "Keep holds per series?", card: &card };
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
    if !is_items_question(&shown.text, item) {
        return Err(Unwitnessed::NotTheItemsQuestion);
    }
    let was_shown = |l: &str| shown.labels.iter().any(|s| s == l);
    if let Some(missing) = item
        .card
        .options
        .iter()
        .filter(|o| o.fails_must.is_none())
        .find(|o| !was_shown(&label(o)))
    {
        return Err(Unwitnessed::OptionNotShown(missing.id.clone()));
    }
    let picked = match answer.selected.as_slice() {
        [one] => one,
        many => return Err(Unwitnessed::SelectionCount(many.len())),
    };
    if !was_shown(picked) {
        return Err(Unwitnessed::LabelNotShown(picked.clone()));
    }
    let mut matching = item.card.options.iter().filter(|o| label(o) == *picked);
    let option = match (matching.next(), matching.next()) {
        (Some(option), None) => option,
        (None, _) => return Err(Unwitnessed::UnknownLabel(picked.clone())),
        (Some(_), Some(_)) => return Err(Unwitnessed::AmbiguousLabel(picked.clone())),
    };
    if option.fails_must.is_some() {
        return Err(Unwitnessed::IneligibleOption(option.id.clone()));
    }
    Ok(Choice {
        option: option.id.clone(),
    })
}

/// Whether `text` is `<id> <question>` for `item`, exactly or followed by [`ONE_WAY_SUFFIX`].
fn is_items_question(text: &str, item: &ItemCard<'_>) -> bool {
    let rest = text
        .strip_prefix(item.id.as_str())
        .and_then(|r| r.strip_prefix(' '))
        .and_then(|r| r.strip_prefix(item.question));
    matches!(rest, Some("" | ONE_WAY_SUFFIX))
}

#[cfg(test)]
mod tests {
    use super::*;
    use weighin_contract::card::Recommendation;

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

    fn check(shown: &ShownQuestion, answer: &Answer, card: &Card) -> Result<Choice, Unwitnessed> {
        let id = id();
        witness(
            shown,
            answer,
            &ItemCard {
                id: &id,
                question: QUESTION,
                card,
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
    fn a_one_way_question_with_its_suffix_is_still_the_items_own() {
        let mut s = shown(&id());
        s.text.push_str(ONE_WAY_SUFFIX);
        assert_eq!(
            check(&s, &picked("A: Option A"), &card()),
            Ok(Choice { option: "A".into() })
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

    #[test]
    fn an_eligible_option_left_out_or_relabelled_is_unwitnessed() {
        let mut s = shown(&id());
        s.labels.retain(|l| l != "B: Option B");
        assert_eq!(
            check(&s, &picked("A: Option A"), &card()),
            Err(Unwitnessed::OptionNotShown("B".into()))
        );
        let mut s = shown(&id());
        for l in &mut s.labels {
            if l == "B: Option B" {
                "B: Option B (Recommended)".clone_into(l);
            }
        }
        assert_eq!(
            check(&s, &picked("A: Option A"), &card()),
            Err(Unwitnessed::OptionNotShown("B".into()))
        );
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
    fn a_label_two_options_render_to_is_unwitnessed() {
        let mut c = card();
        let mut first = opt("X: y");
        "z".clone_into(&mut first.title);
        let mut second = opt("X");
        "y: z".clone_into(&mut second.title);
        c.options = vec![first, second];
        let mut s = shown(&id());
        s.labels = vec!["X: y: z".into()];
        assert_eq!(
            check(&s, &picked("X: y: z"), &c),
            Err(Unwitnessed::AmbiguousLabel("X: y: z".into()))
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
