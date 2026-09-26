//! Which billing senders are one service.

use std::collections::{BTreeMap, BTreeSet};

use super::group_key;
use super::summary::{Charge, Summary, summarize};
use crate::data::{Catalog, Critical, Service};
use crate::extract::{Extraction, is_platform, parse_rfc3339_utc};

/// The `data/` entry that names a sender (PLAN.md 2.6).
#[derive(Debug, Clone, Copy)]
pub enum Entry<'c> {
    Service(&'c Service),
    Critical(&'c Critical),
}

impl Entry<'_> {
    pub fn name(&self) -> &str {
        match self {
            Entry::Service(s) => &s.name,
            Entry::Critical(c) => &c.name,
        }
    }
}

/// The most specific entry across both collections. On a tie the service
/// entry wins, because it carries the playbook.
pub fn entry<'c>(catalog: &'c Catalog, address: &str) -> Option<Entry<'c>> {
    let service = catalog
        .service_for(address)
        .and_then(|s| Some((s.matcher.strength(address)?, Entry::Service(s))));
    let critical = catalog
        .critical_for(address)
        .and_then(|c| Some((c.matcher.strength(address)?, Entry::Critical(c))));
    match (service, critical) {
        (Some((s, service)), Some((c, critical))) => Some(if c > s { critical } else { service }),
        (service, critical) => service.or(critical).map(|(_, e)| e),
    }
}

/// One billing message's sender, as grouping sees it.
pub struct Candidate<'a> {
    pub address: &'a str,
    pub domain: &'a str,
    pub display_name: Option<&'a str>,
    /// The merchant a payment platform named.
    pub merchant: Option<&'a str>,
}

/// Display names that say what a mailbox does, not whose it is.
const GENERIC: &[&str] = &[
    "billing",
    "support",
    "receipts",
    "receipt",
    "noreply",
    "no-reply",
    "team",
    "notifications",
    "info",
    "accounts",
    "account",
    "invoice",
    "invoices",
    "payments",
    "orders",
    "hello",
    "admin",
];

fn alnum(text: &str) -> String {
    text.chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_ascii_lowercase()
}

/// Whether a candidate is a payment platform relaying a merchant's receipt.
fn relayed(c: &Candidate<'_>) -> bool {
    c.merchant.is_some() && is_platform(c.domain)
}

/// A key per candidate. Platform receipts group by merchant. A sender a
/// `data/` entry names groups under that entry's name, so AWS billing from
/// amazon.com stays apart from Amazon's shop. Everything else groups by
/// registrable domain, except that domains which share a specific display
/// name that each domain starts with are one brand that moved its mail:
/// `Brightline` at `brightline.test` and `brightlinehq.test`.
pub fn keys(catalog: &Catalog, candidates: &[Candidate<'_>]) -> Vec<String> {
    let entries: Vec<Option<Entry<'_>>> = candidates
        .iter()
        .map(|c| {
            if relayed(c) {
                None
            } else {
                entry(catalog, c.address)
            }
        })
        .collect();
    let base: Vec<String> = candidates
        .iter()
        .zip(&entries)
        .map(|(c, entry)| match (c.merchant, entry) {
            (Some(merchant), _) if relayed(c) => merchant.trim().to_owned(),
            (_, Some(entry)) => entry.name().to_owned(),
            _ => group_key(c.domain),
        })
        .collect();

    let mut brands: BTreeMap<String, (String, BTreeSet<String>)> = BTreeMap::new();
    for ((c, key), entry) in candidates.iter().zip(&base).zip(&entries) {
        if relayed(c) || entry.is_some() {
            continue;
        }
        let Some(name) = c.display_name.map(str::trim) else {
            continue;
        };
        let brand = alnum(name);
        let label = alnum(key.split('.').next().unwrap_or(""));
        if brand.len() >= 3 && !GENERIC.contains(&brand.as_str()) && label.starts_with(&brand) {
            brands
                .entry(brand)
                .or_insert_with(|| (name.to_owned(), BTreeSet::new()))
                .1
                .insert(key.clone());
        }
    }
    let moved: BTreeMap<&String, &String> = brands
        .values()
        .filter(|(_, domains)| domains.len() > 1)
        .flat_map(|(name, domains)| domains.iter().map(move |d| (d, name)))
        .collect();

    base.iter()
        .map(|key| {
            moved
                .get(key)
                .map_or_else(|| key.clone(), |name| (*name).clone())
        })
        .collect()
}

/// Groups extracted messages into services and summarises each one's charges.
/// The same pure path the rebuild takes, for tests over the corpus.
pub fn series(extractions: &[Extraction]) -> BTreeMap<String, Summary> {
    let billing: Vec<&Extraction> = extractions.iter().filter(|e| e.receipt.is_some()).collect();
    let candidates: Vec<Candidate<'_>> = billing
        .iter()
        .map(|e| Candidate {
            address: e.from_address.as_deref().unwrap_or(""),
            domain: e
                .from_address
                .as_deref()
                .and_then(|a| a.rsplit_once('@'))
                .map_or("", |(_, d)| d),
            display_name: e.from_name.as_deref(),
            merchant: e.receipt.as_ref().and_then(|r| r.merchant.as_deref()),
        })
        .collect();

    let mut charges: BTreeMap<String, Vec<Charge>> = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for (e, key) in billing
        .iter()
        .zip(keys(crate::data::catalog(), &candidates))
    {
        let entry = charges.entry(key).or_default();
        let Some(r) = &e.receipt else { continue };
        let (Some(amount), Some(currency), Some(at)) = (
            r.amount_minor_units,
            &r.currency,
            e.date.as_deref().and_then(parse_rfc3339_utc),
        ) else {
            continue;
        };
        if !r.kind.is_spend()
            || e.message_id
                .as_ref()
                .is_some_and(|id| !seen.insert(id.clone()))
        {
            continue;
        }
        entry.push(Charge {
            at,
            minor_units: amount,
            currency: currency.clone(),
        });
    }
    charges
        .into_iter()
        .map(|(key, charges)| (key, summarize(&charges)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate<'a>(domain: &'a str, name: &'a str) -> Candidate<'a> {
        Candidate {
            address: "billing@example.invalid",
            domain,
            display_name: Some(name),
            merchant: None,
        }
    }

    #[test]
    fn a_brand_that_moved_domains_is_one_service() {
        let keys = keys(
            &Catalog::default(),
            &[
                candidate("brightline.test", "Brightline"),
                candidate("mail.brightlinehq.test", "Brightline"),
            ],
        );
        assert_eq!(keys, vec!["Brightline", "Brightline"]);
    }

    #[test]
    fn a_generic_name_never_merges_vendors() {
        let keys = keys(
            &Catalog::default(),
            &[
                candidate("acme.test", "Billing"),
                candidate("zenith.test", "Billing"),
            ],
        );
        assert_eq!(keys, vec!["acme.test", "zenith.test"]);
    }

    #[test]
    fn a_single_domain_keeps_its_domain_key() {
        let keys = keys(
            &Catalog::default(),
            &[candidate("billing.atlasbook.test", "Atlasbook")],
        );
        assert_eq!(keys, vec!["atlasbook.test"]);
    }

    #[test]
    fn platform_receipts_group_by_merchant() {
        let keys = keys(
            &Catalog::default(),
            &[Candidate {
                address: "receipts@stripe.com",
                domain: "stripe.com",
                display_name: Some("Lumen Analytics"),
                merchant: Some("Lumen Analytics"),
            }],
        );
        assert_eq!(keys, vec!["Lumen Analytics"]);
    }
}
