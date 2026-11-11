// The first three moved from `bench-ledger`'s `tag.rs` (Move 2 step 3, PR 3.4) unchanged.

use super::{LedgerTag, SubjectKey, TagError, TagKey};

#[test]
fn every_subject_key_is_the_ledger_key_of_the_same_name() {
    for key in SubjectKey::ALL {
        assert_eq!(TagKey::from(key).as_str(), key.as_str(), "{key}");
    }
}

#[test]
fn key_value_text_parses_into_a_vocabulary_tag() {
    let tag: LedgerTag = "task:T1:a".parse().expect("a task tag");

    assert_eq!(tag.key(), TagKey::Task, "split at the first colon");
    assert_eq!(tag.as_str(), "task:T1:a", "the value keeps later colons");
}

#[test]
fn unknown_keys_missing_colons_and_empty_values_are_refused() {
    assert!(
        matches!("item:x".parse::<LedgerTag>(), Err(TagError::UnknownKey { key }) if key == "item"),
        "item is not in the vocabulary"
    );
    assert!(
        matches!(
            "task".parse::<LedgerTag>(),
            Err(TagError::NotKeyValue { .. })
        ),
        "no colon"
    );
    assert!(
        matches!(
            "plan:".parse::<LedgerTag>(),
            Err(TagError::Value {
                key: TagKey::Plan,
                ..
            })
        ),
        "empty value"
    );
}

#[test]
fn bench_keys_are_stored_under_bench_and_read_in_either_spelling() {
    let short: LedgerTag = "plan:p.md".parse().expect("the short key");
    let stored: LedgerTag = "bench.plan:p.md".parse().expect("the stored key");
    let task: LedgerTag = "task:T1".parse().expect("Core's key");

    assert_eq!(
        (short.as_str(), stored.as_str(), task.as_str()),
        ("bench.plan:p.md", "bench.plan:p.md", "task:T1"),
        "plan, point, agent and migration are bench.<key>; task and pr are Core's (EV-12)"
    );
    assert!(
        matches!(
            "bench.task:T1".parse::<LedgerTag>(),
            Err(TagError::UnknownKey { .. })
        ),
        "task is Core's key, never bench.task"
    );
}

#[test]
fn values_core_refuses_are_tag_errors() {
    for (key, value) in [
        (TagKey::Pr, "0"),
        (TagKey::Pr, "07"),
        (TagKey::Worktree, "/repo"),
        (TagKey::Agent, &"a".repeat(300)),
    ] {
        assert!(
            matches!(LedgerTag::new(key, value), Err(TagError::Value { key: k, .. }) if k == key),
            "{key}:{value}"
        );
    }
}
