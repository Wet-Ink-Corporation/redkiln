//! Properties of `answered::witness` over generated dialogs: a well-formed answer always
//! witnesses the option picked, breaking any one rule gives that rule's reason, and whatever
//! the input, a choice only ever comes with every rule holding.

use crate::support::{CASES, Rng, adversarial, one_line};
use weighin_core::ItemId;
use weighin_core::answered::{
    Answer, Choice, ItemCard, ONE_WAY_SUFFIX, RECOMMENDED_SUFFIX, Resolution, ShownQuestion,
    Unwitnessed, label, witness,
};
use weighin_core::card::{Card, CardOption, Recommendation, Stability};

const SEED: u64 = 0x1250_A115_3E4E_D001;
const IDS: [&str; 4] = ["A", "B", "C", "D"];
const UNRESOLVED: [Resolution; 3] = [
    Resolution::AfkTimeout,
    Resolution::FollowUp,
    Resolution::FreeText,
];
/// A shown label that names no option: no option label starts with `Defer`.
const DEFER: &str = "Defer to the DPA review (default A by 2026-12-01)";

/// A generated item: its id, question and a card whose first option is eligible and recommended.
struct Generated {
    id: ItemId,
    question: String,
    card: Card,
}

impl Generated {
    fn item(&self) -> ItemCard<'_> {
        ItemCard {
            id: &self.id,
            question: &self.question,
            card: &self.card,
        }
    }

    fn eligible(&self) -> Vec<&CardOption> {
        self.card
            .options
            .iter()
            .filter(|o| o.fails_must.is_none())
            .collect()
    }
}

/// Whether a dialog showing `text` may mark `card`'s recommendation: it was earned, and the
/// question is not one-way.
fn may_mark(card: &Card, text: &str) -> bool {
    card.recommendation_earned() && !text.ends_with(ONE_WAY_SUFFIX)
}

/// Whether `shown` names `option` of `card`, written apart from the module: the option's label,
/// or, when `marked`, the recommended option's label with the suffix once.
fn names(card: &Card, marked: bool, option: &CardOption, shown: &str) -> bool {
    let own = format!("{}: {}", option.id, option.title);
    shown == own
        || (marked && option.id == card.recommendation.option && shown == own + RECOMMENDED_SUFFIX)
}

/// Every label each option of `card` could be shown under.
fn forms(card: &Card) -> Vec<String> {
    let mut forms = Vec::new();
    for o in &card.options {
        forms.push(label(o));
        if o.id == card.recommendation.option {
            forms.push(label(o) + RECOMMENDED_SUFFIX);
        }
    }
    forms
}

/// An item with 2 to 4 options and a question that is never blank. With `plain_ids` the ids are
/// distinct letters, so no two labels can coincide; otherwise they are adversarial and may
/// repeat, which makes the card invalid.
fn item(rng: &mut Rng, plain_ids: bool) -> Generated {
    let question = format!("Q{}", one_line(rng, 6));
    let id = ItemId::mint(&one_line(rng, 3), &question);
    let n = 2 + rng.below(3);
    let options = IDS
        .iter()
        .take(n)
        .enumerate()
        .map(|(i, letter)| CardOption {
            id: if plain_ids {
                (*letter).to_owned()
            } else {
                adversarial(rng, 3)
            },
            title: adversarial(rng, 5),
            fails_must: (i > 0 && rng.one_in(3)).then(|| "a MUST".to_owned()),
            ..Default::default()
        })
        .collect::<Vec<_>>();
    let recommendation = Recommendation {
        option: options.first().map(|o| o.id.clone()).unwrap_or_default(),
        stability: rng.one_in(4).then(|| Stability {
            agree: 1,
            of: 4,
            ..Default::default()
        }),
        ..Default::default()
    };
    Generated {
        id,
        question,
        card: Card {
            options,
            recommendation,
            ..Default::default()
        },
    }
}

/// A dialog that follows every rule: the item's question, every eligible option shown (in some
/// order, the recommendation perhaps marked, perhaps with the ineligible options and a defer
/// choice), and one eligible option picked under the label it was shown with.
fn well_formed(rng: &mut Rng, g: &Generated) -> (ShownQuestion, Answer, String) {
    let mut text = format!("{} {}", g.id, g.question);
    if rng.one_in(2) {
        text.push_str(ONE_WAY_SUFFIX);
    }
    let marked = may_mark(&g.card, &text) && rng.one_in(2);
    let shown_label = |o: &CardOption| {
        if marked && o.id == g.card.recommendation.option {
            label(o) + RECOMMENDED_SUFFIX
        } else {
            label(o)
        }
    };
    let mut labels: Vec<String> = g
        .card
        .options
        .iter()
        .filter(|o| o.fails_must.is_none() || rng.one_in(2))
        .map(shown_label)
        .collect();
    if rng.one_in(2) {
        labels.push(DEFER.to_owned());
    }
    for i in (1..labels.len()).rev() {
        labels.swap(i, rng.below(i + 1));
    }
    let chosen = *rng.pick(&g.eligible());
    let shown = ShownQuestion {
        text,
        multi_select: false,
        labels,
    };
    let answer = Answer {
        resolution: Resolution::Person,
        selected: vec![shown_label(chosen)],
    };
    (shown, answer, chosen.id.clone())
}

#[test]
fn a_well_formed_answer_witnesses_the_option_picked() {
    for case in 0..CASES {
        let mut rng = Rng::new(SEED, case);
        let g = item(&mut rng, true);
        let (shown, answer, chosen) = well_formed(&mut rng, &g);
        assert_eq!(
            witness(&shown, &answer, &g.item()),
            Ok(Choice { option: chosen }),
            "case {case}"
        );
    }
}

/// Leave out one eligible option, under whichever label it was shown.
fn drop_eligible(rng: &mut Rng, g: &Generated, shown: &mut ShownQuestion) -> Unwitnessed {
    let dropped = *rng.pick(&g.eligible());
    shown.labels.retain(|l| !names(&g.card, true, dropped, l));
    Unwitnessed::OptionNotShown(dropped.id.clone())
}

/// Select a label that was not shown: an ineligible option's, removed from the dialog, or one
/// no option has.
fn pick_unshown(g: &Generated, shown: &mut ShownQuestion, answer: &mut Answer) -> Unwitnessed {
    let picked = g
        .card
        .options
        .iter()
        .find(|o| o.fails_must.is_some())
        .map_or_else(|| "Z: never shown".to_owned(), label);
    shown.labels.retain(|l| *l != picked);
    answer.selected = vec![picked.clone()];
    Unwitnessed::LabelNotShown(picked)
}

/// Select a shown label that names no option.
fn pick_extra(shown: &mut ShownQuestion, answer: &mut Answer) -> Unwitnessed {
    if !shown.labels.iter().any(|l| l == DEFER) {
        shown.labels.push(DEFER.to_owned());
    }
    answer.selected = vec![DEFER.to_owned()];
    Unwitnessed::UnknownLabel(DEFER.to_owned())
}

/// Select an ineligible option, shown under its label; when the card has none, the last option
/// (never the recommended first) is made ineligible.
fn pick_ineligible(
    g: &mut Generated,
    shown: &mut ShownQuestion,
    answer: &mut Answer,
) -> Unwitnessed {
    if !g.card.options.iter().any(|o| o.fails_must.is_some())
        && let Some(last) = g.card.options.last_mut()
    {
        last.fails_must = Some("a MUST".to_owned());
    }
    let ineligible = g
        .card
        .options
        .iter()
        .find(|o| o.fails_must.is_some())
        .expect("the card has an ineligible option");
    let picked = label(ineligible);
    if !shown.labels.contains(&picked) {
        shown.labels.push(picked.clone());
    }
    answer.selected = vec![picked];
    Unwitnessed::IneligibleOption(ineligible.id.clone())
}

/// Show one of the labels a second time.
fn repeat_label(rng: &mut Rng, shown: &mut ShownQuestion) -> Unwitnessed {
    let repeated = rng.pick(&shown.labels).clone();
    shown.labels.push(repeated.clone());
    Unwitnessed::DuplicateLabel(repeated)
}

/// The recommended option, the first of every generated card.
fn recommended(g: &Generated) -> CardOption {
    g.card.options.first().cloned().expect("a card has options")
}

/// Mark the recommendation on a one-way question, where a sitting never marks it.
fn mark_one_way(g: &Generated, shown: &mut ShownQuestion) -> Unwitnessed {
    if !shown.text.ends_with(ONE_WAY_SUFFIX) {
        shown.text.push_str(ONE_WAY_SUFFIX);
    }
    let rec = recommended(g);
    shown.labels.retain(|l| !names(&g.card, true, &rec, l));
    shown.labels.push(label(&rec) + RECOMMENDED_SUFFIX);
    Unwitnessed::OptionNotShown(rec.id)
}

/// Show the recommended option under both of its labels, where marking is allowed.
fn show_twice(g: &mut Generated, shown: &mut ShownQuestion) -> Unwitnessed {
    g.card.recommendation.stability = None;
    if let Some(plain) = shown.text.strip_suffix(ONE_WAY_SUFFIX) {
        shown.text = plain.to_owned();
    }
    let rec = recommended(g);
    shown.labels.retain(|l| !names(&g.card, true, &rec, l));
    shown.labels.push(label(&rec));
    shown.labels.push(label(&rec) + RECOMMENDED_SUFFIX);
    Unwitnessed::OptionShownTwice(rec.id)
}

/// Give one option a title that claims the recommendation.
fn claim_recommendation(rng: &mut Rng, g: &mut Generated) -> Unwitnessed {
    let i = rng.below(g.card.options.len());
    let option = g.card.options.get_mut(i).expect("an option at i");
    option.title.push_str(RECOMMENDED_SUFFIX);
    Unwitnessed::TitleMarksRecommended(option.id.clone())
}

/// Make the first two options render to the same label: `A` titled `y: z` and `A: y` titled `z`.
fn collide(g: &mut Generated) -> Unwitnessed {
    if let [first, second, ..] = g.card.options.as_mut_slice() {
        "y: z".clone_into(&mut first.title);
        second.id = format!("{}: y", first.id);
        "z".clone_into(&mut second.title);
    }
    let first = recommended(g);
    Unwitnessed::CollidingLabels(label(&first))
}

/// Break one rule of a well-formed dialog, and say which reason that must give.
fn break_one_rule(
    rng: &mut Rng,
    g: &mut Generated,
    shown: &mut ShownQuestion,
    answer: &mut Answer,
) -> Unwitnessed {
    match rng.below(13) {
        0 => {
            answer.resolution = *rng.pick(&UNRESOLVED);
            match answer.resolution {
                Resolution::AfkTimeout => Unwitnessed::AfkTimeout,
                Resolution::FollowUp => Unwitnessed::FollowUp,
                _ => Unwitnessed::FreeText,
            }
        }
        1 => {
            shown.multi_select = true;
            Unwitnessed::MultiSelect
        }
        2 => {
            shown.text.push_str(&one_line(rng, 3));
            shown.text.push('x');
            Unwitnessed::NotTheItemsQuestion
        }
        3 => drop_eligible(rng, g, shown),
        4 => {
            if rng.one_in(2) {
                answer.selected.clear();
            } else {
                answer.selected.push(rng.pick(&shown.labels).clone());
            }
            Unwitnessed::SelectionCount(answer.selected.len())
        }
        5 => pick_unshown(g, shown, answer),
        6 => pick_extra(shown, answer),
        7 => pick_ineligible(g, shown, answer),
        8 => repeat_label(rng, shown),
        9 => mark_one_way(g, shown),
        10 => show_twice(g, shown),
        11 => claim_recommendation(rng, g),
        _ => collide(g),
    }
}

#[test]
fn breaking_any_one_rule_gives_that_rules_reason() {
    for case in 0..CASES {
        let mut rng = Rng::new(SEED ^ 1, case);
        let mut g = item(&mut rng, true);
        let (mut shown, mut answer, _) = well_formed(&mut rng, &g);
        let expected = break_one_rule(&mut rng, &mut g, &mut shown, &mut answer);
        assert_eq!(
            witness(&shown, &answer, &g.item()),
            Err(expected),
            "case {case}"
        );
    }
}

/// Break any number of rules at random, each with some probability.
fn scramble(rng: &mut Rng, shown: &mut ShownQuestion, answer: &mut Answer) {
    if rng.one_in(4) {
        answer.resolution = *rng.pick(&UNRESOLVED);
    }
    if rng.one_in(4) {
        shown.multi_select = true;
    }
    if rng.one_in(4) {
        shown.text = one_line(rng, 6);
    }
    if rng.one_in(3) && !shown.labels.is_empty() {
        let i = rng.below(shown.labels.len());
        shown.labels.remove(i);
    }
    if rng.one_in(4) && !shown.labels.is_empty() {
        let repeated = rng.pick(&shown.labels).clone();
        shown.labels.push(repeated);
    }
    if rng.one_in(3) {
        answer.selected = (0..rng.below(3)).map(|_| one_line(rng, 3)).collect();
    }
}

/// The card rules hold: valid, no title claiming the recommendation, no label two options share.
fn assert_card_holds(case: u64, card: &Card) {
    assert_eq!(card.validate(), Ok(()), "case {case}");
    for o in &card.options {
        assert!(!o.title.ends_with(RECOMMENDED_SUFFIX), "case {case}");
    }
    let forms = forms(card);
    for (i, f) in forms.iter().enumerate() {
        assert!(
            !forms.iter().take(i).any(|e| e == f),
            "case {case}: two options share {f:?}"
        );
    }
}

/// Every rule holds for a dialog `witness` turned into `choice`, checked without its code.
fn assert_every_rule_holds(
    case: u64,
    g: &Generated,
    (shown, answer): (&ShownQuestion, &Answer),
    choice: &Choice,
) {
    assert_eq!(answer.resolution, Resolution::Person, "case {case}");
    assert!(!shown.multi_select, "case {case}");
    assert_card_holds(case, &g.card);
    let marked = may_mark(&g.card, &shown.text);
    let rest = shown
        .text
        .strip_prefix(&format!("{} {}", g.id, g.question))
        .unwrap_or("not the item's question");
    assert!(rest.is_empty() || rest == ONE_WAY_SUFFIX, "case {case}");
    for (i, l) in shown.labels.iter().enumerate() {
        assert!(
            !shown.labels.iter().take(i).any(|e| e == l),
            "case {case}: {l:?} twice"
        );
    }
    for o in &g.card.options {
        let shown_as = shown
            .labels
            .iter()
            .filter(|l| names(&g.card, marked, o, l))
            .count();
        let expected = if o.fails_must.is_none() { 1..=1 } else { 0..=1 };
        assert!(expected.contains(&shown_as), "case {case}: {shown_as}");
    }
    let [picked] = answer.selected.as_slice() else {
        panic!(
            "case {case}: a choice from {} selections",
            answer.selected.len()
        );
    };
    assert!(shown.labels.contains(picked), "case {case}");
    let matching: Vec<&CardOption> = g
        .card
        .options
        .iter()
        .filter(|o| names(&g.card, marked, o, picked))
        .collect();
    let [option] = matching.as_slice() else {
        panic!("case {case}: {} options match the label", matching.len());
    };
    assert_eq!(option.id, choice.option, "case {case}");
    assert!(option.fails_must.is_none(), "case {case}");
}

#[test]
fn a_choice_only_ever_comes_with_every_rule_holding() {
    let mut choices = 0_u64;
    for case in 0..CASES {
        let mut rng = Rng::new(SEED ^ 2, case);
        let plain_ids = rng.one_in(2);
        let g = item(&mut rng, plain_ids);
        let (mut shown, mut answer, _) = well_formed(&mut rng, &g);
        scramble(&mut rng, &mut shown, &mut answer);
        if let Ok(choice) = witness(&shown, &answer, &g.item()) {
            assert_every_rule_holds(case, &g, (&shown, &answer), &choice);
            choices += 1;
        }
    }
    // The scrambling leaves some dialogs intact, so the property is not vacuous.
    assert!(
        choices > CASES / 20,
        "only {choices} choices in {CASES} cases"
    );
}
