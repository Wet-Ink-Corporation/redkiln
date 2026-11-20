//! Properties of `answered::witness` over generated dialogs: a well-formed answer always
//! witnesses the option picked, breaking any one rule never yields a choice, and whatever the
//! input, a choice only ever comes with every rule holding.

use crate::support::{CASES, Rng, adversarial, one_line};
use weighin_core::ItemId;
use weighin_core::answered::{
    Answer, Choice, ItemCard, ONE_WAY_SUFFIX, Resolution, ShownQuestion, Unwitnessed, label,
    witness,
};
use weighin_core::card::{Card, CardOption, Recommendation};

const SEED: u64 = 0x1250_A115_3E4E_D001;
const IDS: [&str; 4] = ["A", "B", "C", "D"];
const UNRESOLVED: [Resolution; 3] = [
    Resolution::AfkTimeout,
    Resolution::FollowUp,
    Resolution::FreeText,
];

/// A generated item: its id, question and a card whose first option is eligible.
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
}

/// An item with 2 to 4 options. With `plain_ids` the ids are distinct letters, so no two labels
/// can coincide; otherwise they are adversarial and may.
fn item(rng: &mut Rng, plain_ids: bool) -> Generated {
    let question = one_line(rng, 6);
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
/// order, perhaps with the ineligible ones and a defer choice), and one eligible option picked.
fn well_formed(rng: &mut Rng, g: &Generated) -> (ShownQuestion, Answer, String) {
    let mut text = format!("{} {}", g.id, g.question);
    if rng.one_in(2) {
        text.push_str(ONE_WAY_SUFFIX);
    }
    let mut labels: Vec<String> = g
        .card
        .options
        .iter()
        .filter(|o| o.fails_must.is_none() || rng.one_in(2))
        .map(label)
        .collect();
    if rng.one_in(2) {
        labels.push("Defer to the DPA review (default A by 2026-12-01)".to_owned());
    }
    for i in (1..labels.len()).rev() {
        labels.swap(i, rng.below(i + 1));
    }
    let eligible: Vec<&CardOption> = g
        .card
        .options
        .iter()
        .filter(|o| o.fails_must.is_none())
        .collect();
    let chosen = *rng.pick(&eligible);
    let shown = ShownQuestion {
        text,
        multi_select: false,
        labels,
    };
    let answer = Answer {
        resolution: Resolution::Person,
        selected: vec![label(chosen)],
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

#[test]
fn breaking_any_one_rule_is_never_a_choice() {
    for case in 0..CASES {
        let mut rng = Rng::new(SEED ^ 1, case);
        let g = item(&mut rng, true);
        let (mut shown, mut answer, _) = well_formed(&mut rng, &g);
        let expected = match rng.below(6) {
            0 => {
                answer.resolution = *rng.pick(&UNRESOLVED);
                None
            }
            1 => {
                shown.multi_select = true;
                Some(Unwitnessed::MultiSelect)
            }
            2 => {
                shown.text.push_str(&one_line(&mut rng, 3));
                shown.text.push('x');
                Some(Unwitnessed::NotTheItemsQuestion)
            }
            3 => {
                let eligible: Vec<String> = g
                    .card
                    .options
                    .iter()
                    .filter(|o| o.fails_must.is_none())
                    .map(label)
                    .collect();
                let dropped = rng.pick(&eligible).clone();
                shown.labels.retain(|l| *l != dropped);
                None
            }
            4 => {
                if rng.one_in(2) {
                    answer.selected.clear();
                } else {
                    answer.selected.push(rng.pick(&shown.labels).clone());
                }
                Some(Unwitnessed::SelectionCount(answer.selected.len()))
            }
            _ => {
                let picked = answer.selected.first().cloned().unwrap_or_default();
                shown.labels.retain(|l| *l != picked);
                answer.selected = vec![picked];
                None
            }
        };
        let got = witness(&shown, &answer, &g.item());
        assert!(got.is_err(), "case {case}: {got:?}");
        if let Some(reason) = expected {
            assert_eq!(got, Err(reason), "case {case}");
        }
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
    if rng.one_in(3) {
        answer.selected = (0..rng.below(3)).map(|_| one_line(rng, 3)).collect();
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
    let rest = shown
        .text
        .strip_prefix(&format!("{} {}", g.id, g.question))
        .unwrap_or("not the item's question");
    assert!(rest.is_empty() || rest == ONE_WAY_SUFFIX, "case {case}");
    for o in g.card.options.iter().filter(|o| o.fails_must.is_none()) {
        assert!(shown.labels.contains(&label(o)), "case {case}");
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
        .filter(|o| label(o) == *picked)
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
