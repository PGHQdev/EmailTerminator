//! What a run of charges says about a subscription: how often it bills,
//! what it costs per month, and whether the price went up.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cadence {
    Monthly,
    Annual,
    Irregular,
}

impl Cadence {
    pub fn as_str(self) -> &'static str {
        match self {
            Cadence::Monthly => "monthly",
            Cadence::Annual => "annual",
            Cadence::Irregular => "irregular",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Charge {
    /// Seconds since the epoch.
    pub at: i64,
    pub minor_units: i64,
    pub currency: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    /// `None` for a single charge: one purchase is not a subscription yet.
    pub cadence: Option<Cadence>,
    pub price_increase: bool,
    pub latest: Option<(i64, String)>,
    /// The latest charge spread over a month: itself when monthly, a twelfth
    /// when annual, unknown otherwise.
    pub monthly_minor_units: Option<i64>,
}

const DAY: i64 = 86_400;

/// `charges` in any order; charges on the same day with the same amount count
/// once, which is how one receipt delivered to two folders reads.
pub fn summarize(charges: &[Charge]) -> Summary {
    let mut sorted: Vec<&Charge> = charges.iter().collect();
    sorted.sort_by_key(|c| (c.at, c.minor_units));
    sorted.dedup_by(|b, a| {
        b.at.div_euclid(DAY) == a.at.div_euclid(DAY)
            && b.minor_units == a.minor_units
            && b.currency == a.currency
    });

    let cadence = cadence(&sorted);
    let latest = sorted.last().map(|c| (c.minor_units, c.currency.clone()));
    let price_increase = sorted.windows(2).any(|w| {
        w[0].currency == w[1].currency
            && w[1].minor_units > w[0].minor_units
            && w[0].minor_units > 0
    });
    let monthly_minor_units = match (cadence, &latest) {
        (Some(Cadence::Monthly), Some((amount, _))) => Some(*amount),
        (Some(Cadence::Annual), Some((amount, _))) => Some((*amount + 6) / 12),
        _ => None,
    };
    Summary {
        cadence,
        price_increase,
        latest,
        monthly_minor_units,
    }
}

fn cadence(sorted: &[&Charge]) -> Option<Cadence> {
    if sorted.len() < 2 {
        return None;
    }
    let mut gaps: Vec<i64> = sorted
        .windows(2)
        .map(|w| (w[1].at - w[0].at) / DAY)
        .collect();
    gaps.sort_unstable();
    let median = gaps[gaps.len() / 2];
    Some(match median {
        25..=35 => Cadence::Monthly,
        350..=380 => Cadence::Annual,
        _ => Cadence::Irregular,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monthly(months: i64, amount: impl Fn(i64) -> i64) -> Vec<Charge> {
        (0..months)
            .map(|m| Charge {
                at: 1_767_225_600 + m * 30 * DAY,
                minor_units: amount(m),
                currency: "USD".into(),
            })
            .collect()
    }

    #[test]
    fn a_monthly_run_with_a_raise() {
        let s = summarize(&monthly(24, |m| if m < 12 { 1000 } else { 1200 }));
        assert_eq!(s.cadence, Some(Cadence::Monthly));
        assert!(s.price_increase);
        assert_eq!(s.monthly_minor_units, Some(1200));
    }

    #[test]
    fn an_annual_plan_is_spread_over_twelve_months() {
        let charges: Vec<Charge> = (0..3)
            .map(|y| Charge {
                at: 1_704_067_200 + y * 365 * DAY,
                minor_units: 12_000,
                currency: "USD".into(),
            })
            .collect();
        let s = summarize(&charges);
        assert_eq!(s.cadence, Some(Cadence::Annual));
        assert_eq!(s.monthly_minor_units, Some(1000));
        assert!(!s.price_increase);
    }

    #[test]
    fn one_purchase_is_not_recurring() {
        let s = summarize(&monthly(1, |_| 4900));
        assert_eq!(s.cadence, None);
        assert_eq!(s.monthly_minor_units, None);
    }

    #[test]
    fn the_same_receipt_twice_counts_once() {
        let mut charges = monthly(3, |_| 500);
        charges.push(charges[1].clone());
        let s = summarize(&charges);
        assert_eq!(s.cadence, Some(Cadence::Monthly));
    }
}
