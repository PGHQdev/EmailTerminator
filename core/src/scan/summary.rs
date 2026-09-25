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
    /// Overlapping plans share a cadence when every plan has it.
    pub cadence: Option<Cadence>,
    /// Charges after the same receipt in two folders is merged.
    pub charge_count: usize,
    /// Within one regular plan: two plans, or two purchases, at different
    /// prices are not a rise.
    pub price_increase: bool,
    pub latest: Option<(i64, String)>,
    /// What the service costs per month now: each plan's latest charge, a
    /// twelfth of it when annual, summed; unknown when any plan is irregular.
    pub monthly_minor_units: Option<i64>,
    /// Plans billed side by side, such as two subscriptions from one vendor.
    pub plans: usize,
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

    let plans = plans(&sorted);
    let cadences: Vec<Option<Cadence>> = plans.iter().map(|p| cadence(p)).collect();
    let cadence = match cadences.first() {
        Some(first) if cadences.iter().all(|c| c == first) => *first,
        _ => Some(Cadence::Irregular),
    };
    let monthly_minor_units = plans
        .iter()
        .zip(&cadences)
        .map(|(plan, cadence)| match (cadence, plan.last()) {
            (Some(Cadence::Monthly), Some(c)) => Some(c.minor_units),
            (Some(Cadence::Annual), Some(c)) => Some((c.minor_units + 6) / 12),
            _ => None,
        })
        .sum();
    Summary {
        cadence,
        charge_count: sorted.len(),
        // Only a regular plan has a price to raise; one-off purchases just differ.
        price_increase: plans.iter().zip(&cadences).any(|(plan, cadence)| {
            matches!(cadence, Some(Cadence::Monthly | Cadence::Annual))
                && plan.windows(2).any(|w| {
                    w[0].currency == w[1].currency
                        && w[1].minor_units > w[0].minor_units
                        && w[0].minor_units > 0
                })
        }),
        latest: sorted.last().map(|c| (c.minor_units, c.currency.clone())),
        monthly_minor_units,
        plans: plans.len(),
    }
}

/// Splits charges into plans billed side by side. Charges are grouped by
/// amount; a group that starts after another ends continues it (a price
/// change), one that overlaps it is a second plan. The split stands only when
/// every plan is regular on its own with at least three charges; otherwise
/// the charges are one irregular run, like a shop's purchases.
fn plans<'a>(sorted: &[&'a Charge]) -> Vec<Vec<&'a Charge>> {
    let mut groups: Vec<Vec<&Charge>> = Vec::new();
    for charge in sorted {
        match groups
            .iter_mut()
            .find(|g| g[0].minor_units == charge.minor_units && g[0].currency == charge.currency)
        {
            Some(group) => group.push(charge),
            None => groups.push(vec![charge]),
        }
    }
    let mut plans: Vec<Vec<&Charge>> = Vec::new();
    for group in groups {
        let start = group[0].at;
        match plans
            .iter_mut()
            .find(|p| p.last().is_some_and(|last| last.at <= start + DAY))
        {
            Some(plan) => plan.extend(group),
            None => plans.push(group),
        }
    }
    let regular = |plan: &Vec<&Charge>| {
        plan.len() >= 3 && matches!(cadence(plan), Some(Cadence::Monthly | Cadence::Annual))
    };
    if plans.len() > 1 && plans.iter().all(regular) {
        plans
    } else {
        vec![sorted.to_vec()]
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
    fn two_plans_side_by_side_are_two_plans() {
        let mut charges = monthly(8, |_| 1500);
        charges.extend(monthly(8, |_| 400).into_iter().map(|mut c| {
            c.at += 5 * DAY;
            c
        }));
        let s = summarize(&charges);
        assert_eq!(s.plans, 2);
        assert_eq!(s.cadence, Some(Cadence::Monthly));
        assert_eq!(s.monthly_minor_units, Some(1900));
        assert!(!s.price_increase);
        assert_eq!(s.charge_count, 16);
    }

    #[test]
    fn irregular_purchases_stay_one_run() {
        let charges: Vec<Charge> = [(0, 3000), (41, 4500), (97, 1200), (180, 4500)]
            .iter()
            .map(|(d, a)| Charge {
                at: 1_767_225_600 + d * DAY,
                minor_units: *a,
                currency: "USD".into(),
            })
            .collect();
        let s = summarize(&charges);
        assert_eq!((s.plans, s.cadence), (1, Some(Cadence::Irregular)));
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
