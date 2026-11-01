//! Properties of `wi-` item ids, the normalised question and its key.

use crate::support::{CASES, Rng, TestResult, adversarial};
use std::collections::BTreeMap;
use weighin_core::ItemId;
use weighin_core::id::{DEFAULT_HEX_LEN, normalise_question, question_key};

fn is_lower_hex(s: &str) -> bool {
    s.chars()
        .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

#[test]
fn minted_ids_have_the_documented_shape_and_parse_back() -> TestResult {
    for case in 0..CASES {
        let mut rng = Rng::new(0x1d_0001, case);
        let session = adversarial(&mut rng, 6);
        let question = adversarial(&mut rng, 12);
        let len = rng.below(40);

        let id = ItemId::mint_with_len(&session, &question, len);

        let hex = id
            .as_str()
            .strip_prefix("wi-")
            .ok_or_else(|| format!("case {case}: {id} does not start with wi-"))?;
        assert_eq!(hex.len(), len.clamp(4, 16), "case {case}: hex length");
        assert!(is_lower_hex(hex), "case {case}: {id} is lowercase hex");
        assert_eq!(id.to_string(), id.as_str(), "case {case}: Display");
        assert_eq!(
            ItemId::parse(id.as_str()).as_ref(),
            Some(&id),
            "case {case}: parse round trip"
        );
        assert_eq!(
            ItemId::mint_with_len(&session, &question, len),
            id,
            "case {case}: minting is deterministic"
        );
        assert_eq!(
            ItemId::mint(&session, &question),
            ItemId::mint_with_len(&session, &question, DEFAULT_HEX_LEN),
            "case {case}: mint uses the default length"
        );
    }
    Ok(())
}

#[test]
fn longer_ids_extend_shorter_ones() {
    for case in 0..CASES {
        let mut rng = Rng::new(0x1d_0002, case);
        let session = adversarial(&mut rng, 4);
        let question = adversarial(&mut rng, 8);
        let short = rng.below(20);
        let long = short + rng.below(20);

        let a = ItemId::mint_with_len(&session, &question, short);
        let b = ItemId::mint_with_len(&session, &question, long);

        assert!(
            b.as_str().starts_with(a.as_str()),
            "case {case}: {b} extends {a}"
        );
    }
}

#[test]
fn parse_accepts_exactly_the_documented_shape() {
    for case in 0..CASES {
        let mut rng = Rng::new(0x1d_0003, case);
        let input = if rng.one_in(2) {
            adversarial(&mut rng, 8)
        } else {
            // Near misses: the prefix with arbitrary text after it.
            format!("wi-{}", adversarial(&mut rng, 4))
        };

        let parsed = ItemId::parse(&input);

        let trimmed = input.trim();
        let shape_ok = trimmed
            .strip_prefix("wi-")
            .is_some_and(|hex| (4..=16).contains(&hex.chars().count()) && is_lower_hex(hex));
        assert_eq!(parsed.is_some(), shape_ok, "case {case}: {input:?}");
        if let Some(id) = parsed {
            assert_eq!(id.as_str(), trimmed, "case {case}: keeps the trimmed text");
            assert_eq!(
                ItemId::parse(id.as_str()).as_ref(),
                Some(&id),
                "case {case}: parsing is idempotent"
            );
        }
    }
}

#[test]
fn normalising_twice_changes_nothing() {
    for case in 0..CASES {
        let mut rng = Rng::new(0x1d_0004, case);
        let question = adversarial(&mut rng, 12);

        let once = normalise_question(&question);

        assert_eq!(normalise_question(&once), once, "case {case}: {question:?}");
    }
}

#[test]
fn ids_and_keys_depend_only_on_the_normalised_question() {
    for case in 0..CASES {
        let mut rng = Rng::new(0x1d_0005, case);
        let session = adversarial(&mut rng, 4);
        let question = adversarial(&mut rng, 12);
        let normalised = normalise_question(&question);

        assert_eq!(
            ItemId::mint(&session, &question),
            ItemId::mint(&session, &normalised),
            "case {case}: {question:?}"
        );
        let key = question_key(&question);
        assert_eq!(key, question_key(&normalised), "case {case}: key");
        assert_eq!(key.len(), 16, "case {case}: key length");
        assert!(is_lower_hex(&key), "case {case}: key is lowercase hex");
    }
}

#[test]
fn distinct_normalised_questions_get_distinct_keys() {
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    for case in 0..CASES {
        let mut rng = Rng::new(0x1d_0006, case);
        let normalised = normalise_question(&adversarial(&mut rng, 10));

        let key = question_key(&normalised);

        if let Some(earlier) = seen.get(&key) {
            assert_eq!(earlier, &normalised, "case {case}: key {key} collides");
        }
        seen.insert(key, normalised);
    }
}
