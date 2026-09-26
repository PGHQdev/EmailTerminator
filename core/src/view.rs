//! What the M2 screens read (S03–S06): queries over the senders, services,
//! charges and rollups a scan rebuilt. Nothing here re-reads mail.
//!
//! Counts and amounts cross the IPC seam as `u32` and `i32`, which the
//! generated bindings carry as a TypeScript `number` without loss.

use std::collections::HashMap;

use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::Serialize;
use specta::Type;

use crate::extract::{parse_rfc3339_utc, rfc3339_utc};
use crate::scan::summary::{Cadence, Charge, price_changes};
use crate::store::{Store, StoreError};

/// Money as an integer count of minor units plus an ISO 4217 code (PLAN.md 2.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Amount {
    pub minor_units: i32,
    pub currency: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ServiceStatus {
    Active,
    Canceling,
    Canceled,
}

impl ServiceStatus {
    fn parse(value: &str) -> Self {
        match value {
            "canceling" => ServiceStatus::Canceling,
            "canceled" => ServiceStatus::Canceled,
            _ => ServiceStatus::Active,
        }
    }
}

fn cadence(value: Option<String>) -> Option<Cadence> {
    match value.as_deref() {
        Some("monthly") => Some(Cadence::Monthly),
        Some("annual") => Some(Cadence::Annual),
        Some("irregular") => Some(Cadence::Irregular),
        _ => None,
    }
}

fn amount(minor_units: Option<i32>, currency: Option<String>) -> Option<Amount> {
    Some(Amount {
        minor_units: minor_units?,
        currency: currency?,
    })
}

/// The `YYYY-MM` months of the twelve months that end with `now`'s month.
fn last_twelve_months(now: i64) -> Vec<String> {
    let stamp = rfc3339_utc(now);
    let (year, month): (i32, i32) = (
        stamp[..4].parse().unwrap_or(1970),
        stamp[5..7].parse().unwrap_or(1),
    );
    let index = year * 12 + month - 1;
    (index - 11..=index)
        .map(|i| format!("{:04}-{:02}", i.div_euclid(12), i.rem_euclid(12) + 1))
        .collect()
}

/// "Per year" everywhere means the last twelve months of the rollups: this
/// month and the eleven before it.
pub(crate) fn first_month(now: i64) -> String {
    last_twelve_months(now).swap_remove(0)
}

/// Messages per service over the last twelve months, from the rollups.
fn service_volume(conn: &Connection, now: i64) -> Result<HashMap<i64, u32>, StoreError> {
    let first = first_month(now);
    let mut stmt = conn.prepare(
        "SELECT subject_id, sum(message_count) FROM aggregate
         WHERE subject_kind = 'service' AND month >= ?1 GROUP BY subject_id",
    )?;
    let rows = stmt
        .query_map([first], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

// ---------------------------------------------------------------- S03

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Dashboard {
    pub sources: u32,
    pub last_sync_at: Option<String>,
    pub scanned: u32,
    /// What active subscriptions cost per month, one entry per currency, the
    /// dominant currency first. There is no conversion (PLAN.md Part 4).
    pub monthly_spend: Vec<Amount>,
    pub subscriptions: u32,
    pub newsletters: u32,
    /// Mail from subscriptions and newsletters in the last twelve months
    /// ([`first_month`]).
    pub emails_per_year: u32,
    /// Every subscription, dominant currency first, then by monthly cost.
    pub services: Vec<Tile>,
    /// The four senders that mailed most in the last twelve months.
    pub loudest: Vec<Loud>,
    /// What "cancel all" saves per month: active subscriptions that are not
    /// critical, per currency.
    pub cancel_all: Vec<Amount>,
    /// Mail in the last twelve months from newsletters still subscribed:
    /// what "unsubscribe all" removes.
    pub unsubscribe_all: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Tile {
    pub id: u32,
    pub name: String,
    pub monthly: Option<Amount>,
    pub price_increase: bool,
    pub is_critical: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Loud {
    pub sender_id: u32,
    pub name: String,
    pub last_year: u32,
}

/// Sums per currency, the one most subscriptions bill in first, then by total.
fn by_currency<'a>(amounts: impl Iterator<Item = &'a Amount>) -> Vec<Amount> {
    let mut totals: Vec<(Amount, usize)> = Vec::new();
    for a in amounts {
        match totals.iter_mut().find(|(t, _)| t.currency == a.currency) {
            Some((t, n)) => {
                t.minor_units = t.minor_units.saturating_add(a.minor_units);
                *n += 1;
            }
            None => totals.push((a.clone(), 1)),
        }
    }
    totals.sort_by(|(a, n), (b, m)| m.cmp(n).then(b.minor_units.cmp(&a.minor_units)));
    totals.into_iter().map(|(a, _)| a).collect()
}

pub fn dashboard(store: &Store, now: i64) -> Result<Dashboard, StoreError> {
    let conn = store.read()?;
    let since = first_month(now);
    let (sources, last_sync_at): (u32, Option<String>) =
        conn.query_row("SELECT count(*), max(last_sync_at) FROM source", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?;
    let scanned: u32 = conn.query_row("SELECT count(*) FROM message", [], |r| r.get(0))?;
    let newsletters: u32 = conn.query_row(
        "SELECT count(*) FROM sender WHERE classification = 'newsletter'",
        [],
        |r| r.get(0),
    )?;
    // A newsletter sender can belong to a service; count each sender once.
    let emails_per_year: u32 = conn.query_row(
        "SELECT coalesce(sum(a.message_count), 0) FROM aggregate a JOIN sender s ON s.id = a.subject_id
         WHERE a.subject_kind = 'sender' AND a.month >= ?1
           AND (s.classification = 'newsletter' OR s.service_id IN
                (SELECT id FROM service WHERE cadence IS NOT NULL))",
        [&since],
        |r| r.get(0),
    )?;
    let unsubscribe_all: u32 = conn.query_row(
        "SELECT coalesce(sum(a.message_count), 0) FROM aggregate a JOIN sender s ON s.id = a.subject_id
         WHERE a.subject_kind = 'sender' AND a.month >= ?1 AND s.classification = 'newsletter'
           AND s.unsubscribed_at IS NULL",
        [&since],
        |r| r.get(0),
    )?;

    let subscriptions = subscriptions(store, now)?;
    let active = || {
        subscriptions
            .iter()
            .filter(|s| s.status != ServiceStatus::Canceled)
    };
    let monthly_spend = by_currency(active().filter_map(|s| s.monthly.as_ref()));
    let cancel_all = by_currency(
        active()
            .filter(|s| !s.is_critical)
            .filter_map(|s| s.monthly.as_ref()),
    );
    let dominant = monthly_spend.first().map(|a| a.currency.clone());
    let mut services: Vec<Tile> = subscriptions
        .iter()
        .map(|s| Tile {
            id: s.id,
            name: s.name.clone(),
            monthly: s.monthly.clone(),
            price_increase: s.price_increase,
            is_critical: s.is_critical,
        })
        .collect();
    // Priced ones first, the dominant currency before the others, dearest first.
    services.sort_by_key(|t| match &t.monthly {
        Some(a) => (
            false,
            Some(&a.currency) != dominant.as_ref(),
            -a.minor_units,
        ),
        None => (true, true, 0),
    });

    let loudest = conn
        .prepare(
            "SELECT s.id, coalesce(s.display_name, s.address), sum(a.message_count) AS n
             FROM aggregate a JOIN sender s ON s.id = a.subject_id
             WHERE a.subject_kind = 'sender' AND a.month >= ?1
               AND s.classification IN ('newsletter', 'service')
             GROUP BY s.id ORDER BY n DESC, s.id LIMIT 4",
        )?
        .query_map([&since], |r| {
            Ok(Loud {
                sender_id: r.get(0)?,
                name: r.get(1)?,
                last_year: r.get(2)?,
            })
        })?
        .collect::<Result<_, _>>()?;

    Ok(Dashboard {
        sources,
        last_sync_at,
        scanned,
        monthly_spend,
        subscriptions: subscriptions.len() as u32,
        newsletters,
        emails_per_year,
        services,
        loudest,
        cancel_all,
        unsubscribe_all,
    })
}

// ---------------------------------------------------------------- S04

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Subscription {
    pub id: u32,
    pub name: String,
    /// `None` when the service bills irregularly.
    pub monthly: Option<Amount>,
    pub cadence: Option<Cadence>,
    pub last_charge_at: Option<String>,
    pub emails_per_year: u32,
    pub price_increase: bool,
    pub is_critical: bool,
    pub status: ServiceStatus,
}

/// Services that bill more than once. A single purchase is not a
/// subscription yet (`summary::Summary::cadence`).
pub fn subscriptions(store: &Store, now: i64) -> Result<Vec<Subscription>, StoreError> {
    let conn = store.read()?;
    let volume = service_volume(&conn, now)?;
    let mut stmt = conn.prepare(
        "SELECT v.id, v.name, v.monthly_minor_units, v.currency, v.cadence,
                (SELECT max(charged_at) FROM charge c WHERE c.service_id = v.id),
                v.price_increase, v.is_critical, v.status
         FROM service v WHERE v.cadence IS NOT NULL ORDER BY v.name COLLATE NOCASE",
    )?;
    let rows = stmt
        .query_map([], |r| {
            let id: i64 = r.get(0)?;
            Ok(Subscription {
                id: id as u32,
                name: r.get(1)?,
                monthly: amount(r.get(2)?, r.get(3)?),
                cadence: cadence(r.get(4)?),
                last_charge_at: r.get(5)?,
                emails_per_year: volume.get(&id).copied().unwrap_or(0),
                price_increase: r.get(6)?,
                is_critical: r.get(7)?,
                status: ServiceStatus::parse(&r.get::<_, String>(8)?),
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

// ---------------------------------------------------------------- S05

#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Newsletter {
    pub id: u32,
    pub name: String,
    pub address: String,
    pub received: u32,
    /// Messages in the last twelve months.
    pub last_year: u32,
    /// The service the sender mails for, whose detail S05 opens.
    pub service_id: Option<u32>,
    /// Messages per week between the first and the last one, at least a week apart.
    pub per_week: f64,
    /// The newest list mail meets RFC 8058, so unsubscribing is one POST
    /// (`action::unsubscribe`).
    pub one_click: bool,
    /// When the app unsubscribed the sender; null while subscribed.
    pub unsubscribed_at: Option<String>,
}

pub fn newsletters(store: &Store, now: i64) -> Result<Vec<Newsletter>, StoreError> {
    let conn = store.read()?;
    let mut stmt = conn.prepare(
        "SELECT s.id, coalesce(s.display_name, s.address), s.address, s.message_count,
                s.first_seen, s.last_seen,
                (SELECT m.one_click FROM message m
                 WHERE m.sender_id = s.id AND m.is_list = 1
                 ORDER BY m.list_unsubscribe IS NULL, m.date DESC, m.id DESC LIMIT 1),
                (SELECT coalesce(sum(a.message_count), 0) FROM aggregate a
                 WHERE a.subject_kind = 'sender' AND a.subject_id = s.id AND a.month >= ?1),
                s.service_id, s.unsubscribed_at
         FROM sender s WHERE s.classification = 'newsletter'
         ORDER BY s.message_count DESC, s.id",
    )?;
    let rows = stmt
        .query_map([first_month(now)], |r| {
            let received: u32 = r.get(3)?;
            let span = |i| -> rusqlite::Result<Option<i64>> {
                Ok(r.get::<_, Option<String>>(i)?
                    .and_then(|d| parse_rfc3339_utc(&d)))
            };
            let weeks = match (span(4)?, span(5)?) {
                (Some(first), Some(last)) => ((last - first) as f64 / (7.0 * 86_400.0)).max(1.0),
                _ => 1.0,
            };
            Ok(Newsletter {
                id: r.get(0)?,
                name: r.get(1)?,
                address: r.get(2)?,
                received,
                last_year: r.get(7)?,
                service_id: r.get(8)?,
                per_week: f64::from(received) / weeks,
                one_click: r.get::<_, Option<bool>>(6)?.unwrap_or(false),
                unsubscribed_at: r.get(9)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

// ---------------------------------------------------------------- S06

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ServiceDetail {
    pub id: u32,
    pub name: String,
    pub status: ServiceStatus,
    pub is_critical: bool,
    pub price_increase: bool,
    pub cadence: Option<Cadence>,
    pub monthly: Option<Amount>,
    /// Addresses that mail as this service, the busiest first.
    pub senders: Vec<String>,
    pub first_seen: Option<String>,
    pub last_charge: Option<Charged>,
    /// Spend per month with a charge, oldest first, per currency.
    pub spend: Vec<MonthSpend>,
    /// Messages per month for the last twelve months, zeros included.
    pub volume: Vec<MonthCount>,
    pub emails_per_year: u32,
    pub receipts_per_year: u32,
    /// Oldest first.
    pub price_changes: Vec<PriceMove>,
    /// Newest first.
    pub receipts: Vec<ReceiptLine>,
    /// The `data/` playbook for this service, if it has one (M4).
    pub playbook: Option<PlaybookSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlaybookSummary {
    pub steps: u32,
    pub minutes: Option<u32>,
    /// When someone last checked the steps against the vendor's page.
    pub checked: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Charged {
    pub at: String,
    pub amount: Amount,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MonthSpend {
    pub month: String,
    pub amount: Amount,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MonthCount {
    pub month: String,
    pub messages: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PriceMove {
    pub at: String,
    pub from: Amount,
    pub to: Amount,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptLine {
    /// The `message` row: the evidence link re-reads the original from it (M3).
    pub message_id: u32,
    pub date: Option<String>,
    pub subject: Option<String>,
    /// `ReceiptKind` as stored: `charge`, `refund`, `trial_ending` and so on.
    pub kind: String,
    pub amount: Option<Amount>,
}

pub fn service_detail(
    store: &Store,
    id: u32,
    now: i64,
) -> Result<Option<ServiceDetail>, StoreError> {
    let conn = store.read()?;
    let id = i64::from(id);
    let head = conn
        .query_row(
            "SELECT name, status, is_critical, price_increase, cadence,
                    monthly_minor_units, currency, data_key
             FROM service WHERE id = ?1",
            [id],
            |r: &Row<'_>| {
                Ok((
                    r.get::<_, String>(0)?,
                    ServiceStatus::parse(&r.get::<_, String>(1)?),
                    r.get::<_, bool>(2)?,
                    r.get::<_, bool>(3)?,
                    cadence(r.get(4)?),
                    amount(r.get(5)?, r.get(6)?),
                    r.get::<_, Option<String>>(7)?,
                ))
            },
        )
        .optional()?;
    let Some((name, status, is_critical, price_increase, cadence, monthly, data_key)) = head else {
        return Ok(None);
    };

    let senders: Vec<String> = conn
        .prepare(
            "SELECT address FROM sender WHERE service_id = ?1
             ORDER BY message_count DESC, address",
        )?
        .query_map([id], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    let first_seen: Option<String> = conn.query_row(
        "SELECT min(first_seen) FROM sender WHERE service_id = ?1",
        [id],
        |r| r.get(0),
    )?;

    let charges: Vec<(String, Amount)> = conn
        .prepare(
            "SELECT charged_at, amount_minor_units, currency FROM charge
             WHERE service_id = ?1 ORDER BY charged_at, id",
        )?
        .query_map([id], |r| {
            Ok((
                r.get(0)?,
                Amount {
                    minor_units: r.get(1)?,
                    currency: r.get(2)?,
                },
            ))
        })?
        .collect::<Result<_, _>>()?;
    let last_charge = charges.last().map(|(at, amount)| Charged {
        at: at.clone(),
        amount: amount.clone(),
    });
    let price_changes = price_changes(
        &charges
            .iter()
            .filter_map(|(at, a)| {
                Some(Charge {
                    at: parse_rfc3339_utc(at)?,
                    minor_units: i64::from(a.minor_units),
                    currency: a.currency.clone(),
                })
            })
            .collect::<Vec<_>>(),
    )
    .into_iter()
    .map(|c| {
        let money = |minor: i64| Amount {
            minor_units: i32::try_from(minor).unwrap_or(i32::MAX),
            currency: c.currency.clone(),
        };
        PriceMove {
            at: rfc3339_utc(c.at),
            from: money(c.from),
            to: money(c.to),
        }
    })
    .collect();

    let spend = conn
        .prepare(
            "SELECT month, spend_minor_units, currency FROM aggregate
             WHERE subject_kind = 'service' AND subject_id = ?1 AND spend_minor_units > 0
             ORDER BY month, currency",
        )?
        .query_map([id], |r| {
            Ok(MonthSpend {
                month: r.get(0)?,
                amount: Amount {
                    minor_units: r.get(1)?,
                    currency: r.get(2)?,
                },
            })
        })?
        .collect::<Result<_, _>>()?;

    let months = last_twelve_months(now);
    let months_from = months[0].clone();
    let counted: HashMap<String, u32> = conn
        .prepare(
            "SELECT month, sum(message_count) FROM aggregate
             WHERE subject_kind = 'service' AND subject_id = ?1 AND month >= ?2
             GROUP BY month",
        )?
        .query_map(params![id, months[0]], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    let volume: Vec<MonthCount> = months
        .into_iter()
        .map(|month| MonthCount {
            messages: counted.get(&month).copied().unwrap_or(0),
            month,
        })
        .collect();
    let emails_per_year = volume.iter().map(|m| m.messages).sum();

    let receipts: Vec<ReceiptLine> = conn
        .prepare(
            "SELECT m.id, m.date, m.subject, r.kind, r.amount_minor_units, r.currency
             FROM receipt r
             JOIN message m ON m.id = r.message_id
             JOIN sender s ON s.id = m.sender_id
             WHERE s.service_id = ?1
                OR r.id IN (SELECT receipt_id FROM charge WHERE service_id = ?1)
             ORDER BY m.date DESC, m.id DESC",
        )?
        .query_map([id], |r| {
            Ok(ReceiptLine {
                message_id: r.get(0)?,
                date: r.get(1)?,
                subject: r.get(2)?,
                kind: r.get(3)?,
                amount: amount(r.get(4)?, r.get(5)?),
            })
        })?
        .collect::<Result<_, _>>()?;
    let receipts_per_year = receipts
        .iter()
        .filter(|r| r.date.as_deref().is_some_and(|d| d >= months_from.as_str()))
        .count() as u32;

    Ok(Some(ServiceDetail {
        id: id as u32,
        name,
        status,
        is_critical,
        price_increase,
        cadence,
        monthly,
        senders,
        first_seen,
        last_charge,
        spend,
        volume,
        emails_per_year,
        receipts_per_year,
        price_changes,
        receipts,
        playbook: playbook_of(data_key.as_deref()).map(|p| PlaybookSummary {
            steps: p.steps.len() as u32,
            minutes: p.minutes,
            checked: p.checked.clone(),
        }),
    }))
}

fn playbook_of(data_key: Option<&str>) -> Option<&'static crate::data::Playbook> {
    crate::data::catalog().service(data_key?)?.playbook.as_ref()
}

// ---------------------------------------------------------------- S08

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlaybookView {
    pub service: ServiceDetail,
    /// The vendor's help page the steps come from.
    pub source: String,
    pub steps: Vec<PlaybookStep>,
    /// Where a correction goes: the entry's file in the repository.
    pub improve: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlaybookStep {
    pub text: String,
    pub link: Option<String>,
}

const REPOSITORY: &str = "https://github.com/PGHQdev/EmailTerminator";

pub fn playbook(store: &Store, id: u32, now: i64) -> Result<Option<PlaybookView>, StoreError> {
    let Some(service) = service_detail(store, id, now)? else {
        return Ok(None);
    };
    let data_key: Option<String> = store.read()?.query_row(
        "SELECT data_key FROM service WHERE id = ?1",
        [i64::from(id)],
        |r| r.get(0),
    )?;
    let Some((key, playbook)) = data_key
        .as_deref()
        .and_then(|key| Some((key, playbook_of(Some(key))?)))
    else {
        return Ok(None);
    };
    Ok(Some(PlaybookView {
        service,
        source: playbook.source.clone(),
        steps: playbook
            .steps
            .iter()
            .map(|s| PlaybookStep {
                text: s.text.clone(),
                link: s.link.clone(),
            })
            .collect(),
        improve: format!("{REPOSITORY}/edit/main/data/services/{key}.toml"),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twelve_months_cross_a_year_boundary() {
        // 2026-02-10
        let months = last_twelve_months(1_770_681_600);
        assert_eq!(months.len(), 12);
        assert_eq!(months[0], "2025-03");
        assert_eq!(months[10], "2026-01");
        assert_eq!(months[11], "2026-02");
    }

    #[test]
    fn the_dominant_currency_bills_the_most_subscriptions() {
        let a = |minor_units, currency: &str| Amount {
            minor_units,
            currency: currency.into(),
        };
        let amounts = [a(5000, "EUR"), a(999, "USD"), a(1299, "USD"), a(100, "GBP")];
        let totals = by_currency(amounts.iter());
        assert_eq!(totals, vec![a(2298, "USD"), a(5000, "EUR"), a(100, "GBP")]);
    }
}
