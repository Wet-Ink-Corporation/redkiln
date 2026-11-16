//! The promotion bar: whether a decision point's scores let it leave shadow mode.
//!
//! A verdict is only reported, never applied: promotion stays a human edit of the producer's
//! configuration.

use serde::Serialize;

/// The promotion verdict. Never applied: calibration only reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    /// Keep the point in shadow.
    StayShadow,
    /// The point may be set to `enforce`.
    EligibleToEnforce,
}

impl Verdict {
    /// The name, as serialised.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StayShadow => "stay-shadow",
            Self::EligibleToEnforce => "eligible-to-enforce",
        }
    }
}

/// Why a question stays in shadow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    /// Fewer than [`Bar::min_labelled`] labelled decisions.
    TooFewLabelled,
    /// A Brier score over [`Bar::max_brier`].
    BrierAboveBound,
    /// An ask rate of [`Bar::max_ask_rate`] or more.
    AskRateAtOrAboveBound,
}

impl Reason {
    /// The name, as serialised.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TooFewLabelled => "too_few_labelled",
            Self::BrierAboveBound => "brier_above_bound",
            Self::AskRateAtOrAboveBound => "ask_rate_at_or_above_bound",
        }
    }
}

/// The bar a question clears to be eligible: enough labelled decisions, a Brier score no higher
/// than the bound, and an ask rate under its bound.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bar {
    /// Fewest labelled decisions an eligible question has.
    pub min_labelled: u32,
    /// Highest Brier score an eligible question may have.
    pub max_brier: f64,
    /// The ask rate at which a question stays in shadow.
    pub max_ask_rate: f64,
}

impl Bar {
    /// The bar anneal's `calibrate` has used since 0.x: 50 labelled, Brier 0.15, ask rate 0.20.
    pub const DEFAULT: Self = Self {
        min_labelled: 50,
        max_brier: 0.15,
        max_ask_rate: 0.20,
    };

    /// The verdict for a question with `labelled` labelled decisions, its Brier score and its
    /// ask rate (`None` without decisions), and every reason it stays in shadow, in the order
    /// [`Reason`] lists them.
    #[must_use]
    pub fn verdict(
        &self,
        labelled: u32,
        brier: Option<f64>,
        ask_rate: Option<f64>,
    ) -> (Verdict, Vec<Reason>) {
        let reasons: Vec<Reason> = [
            (labelled < self.min_labelled).then_some(Reason::TooFewLabelled),
            brier
                .is_some_and(|brier| brier > self.max_brier)
                .then_some(Reason::BrierAboveBound),
            ask_rate
                .is_some_and(|rate| rate >= self.max_ask_rate)
                .then_some(Reason::AskRateAtOrAboveBound),
        ]
        .into_iter()
        .flatten()
        .collect();
        let verdict = if reasons.is_empty() {
            Verdict::EligibleToEnforce
        } else {
            Verdict::StayShadow
        };
        (verdict, reasons)
    }
}

#[cfg(test)]
mod tests;
