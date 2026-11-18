//! Which retired types the import carries over.

use bench_contract::event::{RETIRED_TYPES, RetiredType};

use super::Retired;

#[test]
fn every_retired_type_has_its_own_arm_and_an_unknown_one_is_refused() {
    let arms: Vec<String> = RETIRED_TYPES
        .iter()
        .map(|retired| format!("{:?}", Retired::of(*retired)))
        .collect();
    let unknown = Retired::of(RetiredType {
        short: "gate.skipped",
        stored: "bench.gate.skipped",
    });

    assert_eq!(arms, ["Ok(WeighinParked)", "Ok(BridgeUnavailable)"]);
    assert!(
        matches!(&unknown, Err(reason)
            if reason == "the import does not carry the retired type bench.gate.skipped"),
        "{unknown:?}"
    );
}
