use super::{Bar, Reason, Verdict};

#[test]
fn the_default_bar_is_50_labelled_a_brier_of_0_15_and_an_ask_rate_under_0_20() {
    assert_eq!(
        (
            Bar::DEFAULT.min_labelled,
            Bar::DEFAULT.max_brier,
            Bar::DEFAULT.max_ask_rate
        ),
        (50, 0.15, 0.20)
    );
}

#[test]
fn the_default_bar_holds_at_each_boundary() {
    let bar = Bar::DEFAULT;

    assert_eq!(
        bar.verdict(49, Some(0.0), Some(0.0)),
        (Verdict::StayShadow, vec![Reason::TooFewLabelled])
    );
    assert_eq!(
        bar.verdict(50, Some(0.15), Some(0.1999)),
        (Verdict::EligibleToEnforce, vec![])
    );
    assert_eq!(
        bar.verdict(50, Some(0.151), Some(0.20)),
        (
            Verdict::StayShadow,
            vec![Reason::BrierAboveBound, Reason::AskRateAtOrAboveBound]
        )
    );
}

#[test]
fn no_score_and_no_ask_rate_fail_only_the_labelled_count() {
    assert_eq!(
        Bar::DEFAULT.verdict(0, None, None),
        (Verdict::StayShadow, vec![Reason::TooFewLabelled])
    );
}

#[test]
fn a_bar_of_its_own_moves_each_boundary() {
    let bar = Bar {
        min_labelled: 10,
        max_brier: 0.05,
        max_ask_rate: 0.5,
    };

    assert_eq!(
        bar.verdict(10, Some(0.06), Some(0.4)),
        (Verdict::StayShadow, vec![Reason::BrierAboveBound])
    );
}

#[test]
fn verdicts_and_reasons_serialise_as_calibrate_writes_them() -> Result<(), serde_json::Error> {
    let names = (
        serde_json::to_value([Verdict::StayShadow, Verdict::EligibleToEnforce])?,
        serde_json::to_value([
            Reason::TooFewLabelled,
            Reason::BrierAboveBound,
            Reason::AskRateAtOrAboveBound,
        ])?,
    );

    assert_eq!(
        names,
        (
            serde_json::json!(["stay-shadow", "eligible-to-enforce"]),
            serde_json::json!([
                "too_few_labelled",
                "brier_above_bound",
                "ask_rate_at_or_above_bound"
            ])
        )
    );
    for verdict in [Verdict::StayShadow, Verdict::EligibleToEnforce] {
        assert_eq!(serde_json::to_value(verdict)?, verdict.as_str());
    }
    for reason in [
        Reason::TooFewLabelled,
        Reason::BrierAboveBound,
        Reason::AskRateAtOrAboveBound,
    ] {
        assert_eq!(serde_json::to_value(reason)?, reason.as_str());
    }
    Ok(())
}
