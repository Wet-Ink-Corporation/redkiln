//! The request record without Core: what weighin-core folds (DR-01, DR-02).

use super::*;

const GOLDEN: &str = include_str!("../../../../../spec/fixtures/decision-request/valid/dr-02.json");

#[test]
fn a_request_record_reads_without_core() {
    let request: Request = serde_json::from_str(GOLDEN.trim_end()).expect("DR-02's golden reads");
    assert_eq!(request.raiser.namespace.as_str(), "rust");
    assert_eq!(request.blocks.as_str(), "merge");
    assert_eq!(
        request.options.iter().map(|o| o.id).collect::<Vec<_>>(),
        [OptionId::A, OptionId::B]
    );
    assert_eq!(
        serde_json::to_string(&request).expect("writes"),
        GOLDEN.trim_end(),
        "the record writes back byte for byte"
    );
}

#[test]
fn a_raiser_namespace_parses_only_ev_03_words() {
    for word in NAMESPACES {
        assert_eq!(
            RaiserNamespace::parse(word).map(RaiserNamespace::as_str),
            Some(word)
        );
    }
    assert_eq!(RaiserNamespace::parse("rusty"), None);
    assert_eq!(RaiserNamespace::parse("Rust"), None);
}

#[test]
fn every_blocks_word_is_its_serde_spelling() {
    for blocks in [
        Blocks::Merge,
        Blocks::Release,
        Blocks::Run,
        Blocks::None,
        Blocks::Unknown,
    ] {
        assert_eq!(
            serde_json::to_value(blocks).expect("writes"),
            serde_json::Value::from(blocks.as_str())
        );
    }
}
