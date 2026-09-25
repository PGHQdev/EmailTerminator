//! A scan: fetched mail becomes rows, then rows become senders, services,
//! charges and monthly totals (PLAN.md 2.1, M1).

pub mod group;
mod rebuild;
pub mod summary;

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use rusqlite::{OptionalExtension, Transaction, params};

use crate::extract::{Extraction, extract};
use crate::ingest::imap::{Fetched, SyncTarget};
use crate::store::{Store, StoreError};

pub use rebuild::rebuild;

/// S02's live figures. Senders, newsletters and subscriptions are counted as
/// they first appear; the rebuild after the scan settles them.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Counts {
    pub scanned: u64,
    pub senders: u64,
    pub subscriptions: u64,
    pub newsletters: u64,
    /// The name of the newest service or newsletter found.
    pub latest_find: Option<String>,
}

#[derive(Default)]
struct Seen {
    counts: Counts,
    senders: HashSet<String>,
    newsletters: HashSet<String>,
    subscriptions: HashSet<String>,
}

/// Stores what an IMAP sync fetches for one source.
pub struct ImapScan {
    store: Arc<Store>,
    source_id: i64,
    /// The account's own address: its sent mail in Gmail's All Mail is skipped.
    owner: String,
    seen: Mutex<Seen>,
}

impl ImapScan {
    pub fn new(store: Arc<Store>, source_id: i64, owner: &str) -> Self {
        Self {
            store,
            source_id,
            owner: owner.trim().to_lowercase(),
            seen: Mutex::default(),
        }
    }

    pub fn counts(&self) -> Counts {
        self.seen
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .counts
            .clone()
    }

    fn note(&self, extractions: &[Extraction]) {
        let mut seen = self.seen.lock().unwrap_or_else(|p| p.into_inner());
        for e in extractions {
            seen.counts.scanned += 1;
            let Some(address) = e.from_address.clone() else {
                continue;
            };
            if seen.senders.insert(address.clone()) {
                seen.counts.senders += 1;
            }
            let name = e.from_name.clone().unwrap_or_else(|| address.clone());
            if let Some(receipt) = &e.receipt {
                let key = receipt
                    .merchant
                    .clone()
                    .unwrap_or_else(|| group_key(&address));
                if seen.subscriptions.insert(key.clone()) {
                    seen.counts.subscriptions += 1;
                    seen.counts.latest_find = Some(receipt.merchant.clone().unwrap_or(name));
                }
            } else if e.is_list && seen.newsletters.insert(address) {
                seen.counts.newsletters += 1;
                seen.counts.latest_find = Some(name);
            }
        }
    }
}

impl SyncTarget for ImapScan {
    fn resume(&self, folder: &str, uid_validity: u32) -> Result<u32, String> {
        let (source_id, folder) = (self.source_id, folder.to_owned());
        self.store
            .write(move |conn| {
                let tx = conn.transaction()?;
                let known: Option<(i64, u32, u32)> = tx
                    .query_row(
                        "SELECT id, uid_validity, highest_uid FROM mailbox
                         WHERE source_id = ?1 AND name = ?2",
                        params![source_id, folder],
                        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                    )
                    .optional()?;
                let highest = match known {
                    Some((_, validity, highest)) if validity == uid_validity => highest,
                    Some((id, _, _)) => {
                        // The server renumbered the folder: its UIDs mean nothing now.
                        tx.execute("DELETE FROM message WHERE mailbox_id = ?1", [id])?;
                        tx.execute(
                            "UPDATE mailbox SET uid_validity = ?2, highest_uid = 0 WHERE id = ?1",
                            params![id, uid_validity],
                        )?;
                        0
                    }
                    None => {
                        tx.execute(
                            "INSERT INTO mailbox (source_id, name, uid_validity) VALUES (?1, ?2, ?3)",
                            params![source_id, folder, uid_validity],
                        )?;
                        0
                    }
                };
                tx.commit()?;
                Ok(highest)
            })
            .map_err(|e| e.to_string())
    }

    fn commit(&self, folder: &str, uid_validity: u32, batch: Vec<Fetched>) -> Result<(), String> {
        let highest = batch.iter().map(|f| f.uid).max().unwrap_or(0);
        let rows: Vec<(u32, Extraction)> = batch
            .into_iter()
            .map(|f| (f.uid, extract(&f.raw)))
            .filter(|(_, e)| e.from_address.as_deref() != Some(self.owner.as_str()))
            .collect();
        self.note(&rows.iter().map(|(_, e)| e.clone()).collect::<Vec<_>>());

        let (source_id, folder) = (self.source_id, folder.to_owned());
        self.store
            .write(move |conn| {
                let tx = conn.transaction()?;
                let mailbox_id: i64 = tx.query_row(
                    "SELECT id FROM mailbox WHERE source_id = ?1 AND name = ?2 AND uid_validity = ?3",
                    params![source_id, folder, uid_validity],
                    |r| r.get(0),
                )?;
                for (uid, e) in &rows {
                    insert(&tx, source_id, mailbox_id, &uid.to_string(), e)?;
                }
                tx.execute(
                    "UPDATE mailbox SET highest_uid = max(highest_uid, ?2) WHERE id = ?1",
                    params![mailbox_id, highest],
                )?;
                tx.execute(
                    "UPDATE source SET message_count =
                         (SELECT count(*) FROM message WHERE source_id = ?1)
                     WHERE id = ?1",
                    [source_id],
                )?;
                tx.commit()?;
                Ok(())
            })
            .map_err(|e| e.to_string())
    }
}

fn insert(
    tx: &Transaction<'_>,
    source_id: i64,
    mailbox_id: i64,
    locator: &str,
    e: &Extraction,
) -> Result<(), StoreError> {
    let sender_id: Option<i64> = match &e.from_address {
        Some(address) => {
            let domain = address.rsplit_once('@').map(|(_, d)| d).unwrap_or("");
            Some(tx.query_row(
                "INSERT INTO sender (address, display_name, domain) VALUES (?1, ?2, ?3)
                 ON CONFLICT (address) DO UPDATE
                     SET display_name = coalesce(excluded.display_name, display_name)
                 RETURNING id",
                params![address, e.from_name, domain],
                |r| r.get(0),
            )?)
        }
        None => None,
    };
    let inserted = tx.execute(
        "INSERT OR IGNORE INTO message
             (source_id, mailbox_id, locator, message_id, sender_id, subject, date,
              list_unsubscribe, list_unsubscribe_post, list_id, is_list, dkim_domains)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            source_id,
            mailbox_id,
            locator,
            e.message_id,
            sender_id,
            e.subject,
            e.date,
            e.list_unsubscribe,
            e.list_unsubscribe_post,
            e.list_id,
            e.is_list,
            e.dkim_domains.join(","),
        ],
    )?;
    if inserted == 1
        && let Some(r) = &e.receipt
    {
        tx.execute(
            "INSERT INTO receipt
                 (message_id, kind, amount_minor_units, currency, charged_at, invoice_ref,
                  merchant, extracted_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'parser')",
            params![
                tx.last_insert_rowid(),
                serde_json::to_value(r.kind)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned)),
                r.amount_minor_units,
                r.currency,
                e.date,
                r.invoice_ref,
                r.merchant,
            ],
        )?;
    }
    Ok(())
}

/// The registrable domain of an address or host: `billing.acme.co.uk` and
/// `acme.co.uk` group together.
pub fn group_key(address_or_host: &str) -> String {
    let host = address_or_host
        .rsplit_once('@')
        .map_or(address_or_host, |(_, h)| h)
        .trim_end_matches('.')
        .to_lowercase();
    psl::domain_str(&host).unwrap_or(&host).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_keys_are_registrable_domains() {
        assert_eq!(group_key("billing@mail.acme.co.uk"), "acme.co.uk");
        assert_eq!(group_key("receipts@acme.com"), "acme.com");
        assert_eq!(group_key("x@localhost"), "localhost");
    }
}
