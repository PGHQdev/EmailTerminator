//! After a scan: classify senders, group billing mail into services, derive
//! charges and cadence, and rebuild the monthly rollups S03 and S06 read.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use rusqlite::{Transaction, params};

use super::group::{Candidate, keys};
use super::group_key;
use super::summary::{Charge, summarize};
use crate::extract::{is_platform, parse_rfc3339_utc};
use crate::store::{Store, StoreError};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Totals {
    pub senders: u64,
    pub subscriptions: u64,
    pub newsletters: u64,
}

pub fn rebuild(store: &Store) -> Result<Totals, StoreError> {
    store.write(|conn| {
        let tx = conn.transaction()?;
        senders(&tx)?;
        services(&tx)?;
        aggregates(&tx)?;
        let count = |sql: &str| -> Result<u64, StoreError> {
            Ok(tx.query_row(sql, [], |r| r.get::<_, i64>(0))? as u64)
        };
        let totals = Totals {
            senders: count("SELECT count(*) FROM sender")?,
            subscriptions: count("SELECT count(*) FROM service WHERE cadence IS NOT NULL")?,
            newsletters: count("SELECT count(*) FROM sender WHERE classification = 'newsletter'")?,
        };
        tx.commit()?;
        Ok(totals)
    })
}

/// A sender with billing mail is a service; one whose mail is mostly list
/// mail is a newsletter; the rest is other mail.
fn senders(tx: &Transaction<'_>) -> Result<(), StoreError> {
    tx.execute_batch(
        "UPDATE sender SET
             message_count = (SELECT count(*) FROM message m WHERE m.sender_id = sender.id),
             first_seen = (SELECT min(date) FROM message m WHERE m.sender_id = sender.id),
             last_seen = (SELECT max(date) FROM message m WHERE m.sender_id = sender.id);
         UPDATE sender SET
             classification = CASE
                 WHEN EXISTS (SELECT 1 FROM message m JOIN receipt r ON r.message_id = m.id
                              WHERE m.sender_id = sender.id) THEN 'service'
                 WHEN (SELECT avg(is_list) FROM message m WHERE m.sender_id = sender.id) >= 0.5
                     THEN 'newsletter'
                 ELSE 'other'
             END,
             confidence = coalesce(
                 (SELECT avg(is_list) FROM message m WHERE m.sender_id = sender.id), 0),
             classified_by = 'parser'
         WHERE classified_by = 'parser';",
    )?;
    Ok(())
}

struct Row {
    receipt_id: i64,
    header_id: Option<String>,
    kind: String,
    amount: Option<i64>,
    currency: Option<String>,
    at: Option<i64>,
    domain: String,
    display_name: Option<String>,
    merchant: Option<String>,
}

fn services(tx: &Transaction<'_>) -> Result<(), StoreError> {
    let rows: Vec<Row> = tx
        .prepare(
            "SELECT r.id, m.message_id, r.kind, r.amount_minor_units, r.currency, m.date,
                    s.domain, s.display_name, r.merchant
             FROM receipt r
             JOIN message m ON m.id = r.message_id
             JOIN sender s ON s.id = m.sender_id",
        )?
        .query_map([], |r| {
            Ok(Row {
                receipt_id: r.get(0)?,
                header_id: r.get(1)?,
                kind: r.get(2)?,
                amount: r.get(3)?,
                currency: r.get(4)?,
                at: r
                    .get::<_, Option<String>>(5)?
                    .and_then(|d| parse_rfc3339_utc(&d)),
                domain: r.get(6)?,
                display_name: r.get(7)?,
                merchant: r.get(8)?,
            })
        })?
        .collect::<Result<_, _>>()?;

    let candidates: Vec<Candidate<'_>> = rows
        .iter()
        .map(|r| Candidate {
            domain: &r.domain,
            display_name: r.display_name.as_deref(),
            merchant: r.merchant.as_deref(),
        })
        .collect();
    let mut groups: BTreeMap<String, Vec<&Row>> = BTreeMap::new();
    for (row, key) in rows.iter().zip(keys(&candidates)) {
        groups.entry(key).or_default().push(row);
    }

    tx.execute("DELETE FROM charge", [])?;
    tx.execute("UPDATE sender SET service_id = NULL", [])?;
    let mut kept = HashSet::new();
    for (key, rows) in &groups {
        let name = service_name(key, rows);
        let service_id: i64 = tx.query_row(
            "INSERT INTO service (name, group_key) VALUES (?1, ?2)
             ON CONFLICT (group_key) DO UPDATE SET name = excluded.name
             RETURNING id",
            params![name, key],
            |r| r.get(0),
        )?;
        kept.insert(service_id);

        // One receipt delivered twice (two folders, one Message-ID) is one charge.
        let mut seen_ids = HashSet::new();
        let mut charges = Vec::new();
        for row in rows
            .iter()
            .filter(|r| matches!(r.kind.as_str(), "charge" | "upgrade"))
        {
            let (Some(amount), Some(currency), Some(at)) = (row.amount, &row.currency, row.at)
            else {
                continue;
            };
            if let Some(id) = &row.header_id
                && !seen_ids.insert(id.clone())
            {
                continue;
            }
            tx.execute(
                "INSERT INTO charge (service_id, receipt_id, amount_minor_units, currency, charged_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![service_id, row.receipt_id, amount, currency, crate::extract::rfc3339_utc(at)],
            )?;
            charges.push(Charge {
                at,
                minor_units: amount,
                currency: currency.clone(),
            });
        }

        let summary = summarize(&charges);
        tx.execute(
            "UPDATE service SET cadence = ?2, monthly_minor_units = ?3, currency = ?4,
                                price_increase = ?5
             WHERE id = ?1",
            params![
                service_id,
                summary.cadence.map(|c| c.as_str()),
                summary.monthly_minor_units,
                summary.latest.map(|(_, c)| c),
                summary.price_increase,
            ],
        )?;

        // Every sender on the service's own domains belongs to it; platform
        // senders serve many merchants and belong to none.
        let domains: BTreeSet<String> = rows
            .iter()
            .filter(|r| !is_platform(&r.domain))
            .map(|r| group_key(&r.domain))
            .collect();
        for domain in domains {
            tx.execute(
                "UPDATE sender SET service_id = ?1
                 WHERE domain = ?2 OR domain LIKE '%.' || ?2",
                params![service_id, domain],
            )?;
        }
    }

    // Services whose receipts are gone (a renumbered folder) go too.
    let stale: Vec<i64> = tx
        .prepare("SELECT id FROM service")?
        .query_map([], |r| r.get(0))?
        .collect::<Result<Vec<i64>, _>>()?
        .into_iter()
        .filter(|id| !kept.contains(id))
        .collect();
    for id in stale {
        tx.execute("DELETE FROM service WHERE id = ?1", [id])?;
    }
    Ok(())
}

fn groups_by_merchant(rows: &[&Row]) -> bool {
    rows.iter()
        .all(|r| r.merchant.is_some() && is_platform(&r.domain))
}

/// The merchant's name, or the display name its senders use most, or the domain.
fn service_name(key: &str, rows: &[&Row]) -> String {
    if let Some(merchant) = rows.iter().find_map(|r| r.merchant.clone())
        && groups_by_merchant(rows)
    {
        return merchant;
    }
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for name in rows.iter().filter_map(|r| r.display_name.as_deref()) {
        *counts.entry(name).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(a.0)))
        .map(|(name, _)| name.to_owned())
        .unwrap_or_else(|| key.to_owned())
}

fn aggregates(tx: &Transaction<'_>) -> Result<(), StoreError> {
    tx.execute_batch(
        "DELETE FROM aggregate;
         INSERT INTO aggregate (subject_kind, subject_id, month, currency, message_count)
             SELECT 'sender', sender_id, substr(date, 1, 7), '', count(*)
             FROM message WHERE sender_id IS NOT NULL AND date IS NOT NULL
             GROUP BY sender_id, substr(date, 1, 7);
         INSERT INTO aggregate (subject_kind, subject_id, month, currency, message_count)
             SELECT 'service', s.service_id, substr(m.date, 1, 7), '', count(*)
             FROM message m JOIN sender s ON s.id = m.sender_id
             WHERE s.service_id IS NOT NULL AND m.date IS NOT NULL
             GROUP BY s.service_id, substr(m.date, 1, 7);
         INSERT INTO aggregate (subject_kind, subject_id, month, currency, spend_minor_units)
             SELECT 'service', service_id, substr(charged_at, 1, 7), currency,
                    sum(amount_minor_units)
             FROM charge
             GROUP BY service_id, substr(charged_at, 1, 7), currency;",
    )?;
    Ok(())
}
