//! Where mail comes from (S12): one row per connected mailbox or file.

use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::extract::rfc3339_utc;
use crate::store::{Store, StoreError};

/// An IMAP source's settings. A mailbox source's secret lives in the secret store, never
/// in the database: an app password under [`password_secret`], an Outlook
/// refresh token under [`refresh_secret`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImapConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub id: i64,
    pub kind: String,
    pub label: String,
    pub last_sync_at: Option<String>,
    pub message_count: i64,
}

/// An Outlook.com source's settings: Graph needs only the address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutlookConfig {
    pub username: String,
}

/// A mailbox source, by how it is read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mailbox {
    Imap(ImapConfig),
    Outlook(OutlookConfig),
}

impl Mailbox {
    pub fn username(&self) -> &str {
        match self {
            Mailbox::Imap(c) => &c.username,
            Mailbox::Outlook(c) => &c.username,
        }
    }
}

pub fn password_secret(source_id: i64) -> String {
    format!("imap:{source_id}")
}

pub fn refresh_secret(source_id: i64) -> String {
    format!("outlook:{source_id}")
}

/// Seconds since the epoch.
pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn now() -> String {
    rfc3339_utc(unix_now())
}

pub fn add_imap(store: &Store, label: &str, config: &ImapConfig) -> Result<i64, StoreError> {
    add(store, "imap", label, config)
}

pub fn add_outlook(store: &Store, label: &str, config: &OutlookConfig) -> Result<i64, StoreError> {
    add(store, "outlook", label, config)
}

fn add(store: &Store, kind: &str, label: &str, config: &impl Serialize) -> Result<i64, StoreError> {
    let (kind, label, config) = (
        kind.to_owned(),
        label.to_owned(),
        serde_json::to_string(config).unwrap_or_default(),
    );
    store.write(move |conn| {
        conn.execute(
            "INSERT INTO source (kind, label, config, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![kind, label, config, now()],
        )?;
        Ok(conn.last_insert_rowid())
    })
}

pub fn list(store: &Store) -> Result<Vec<Source>, StoreError> {
    let conn = store.read()?;
    let mut stmt = conn
        .prepare("SELECT id, kind, label, last_sync_at, message_count FROM source ORDER BY id")?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Source {
                id: r.get(0)?,
                kind: r.get(1)?,
                label: r.get(2)?,
                last_sync_at: r.get(3)?,
                message_count: r.get(4)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

pub fn mailbox(store: &Store, id: i64) -> Result<Option<Mailbox>, StoreError> {
    let row: Option<(String, String)> = store
        .read()?
        .query_row(
            "SELECT kind, config FROM source WHERE id = ?1 AND kind IN ('imap', 'outlook')",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    Ok(row.and_then(|(kind, config)| match kind.as_str() {
        "outlook" => serde_json::from_str(&config).ok().map(Mailbox::Outlook),
        _ => serde_json::from_str(&config).ok().map(Mailbox::Imap),
    }))
}

pub fn mark_synced(store: &Store, id: i64) -> Result<(), StoreError> {
    store.write(move |conn| {
        conn.execute(
            "UPDATE source SET last_sync_at = ?2 WHERE id = ?1",
            params![id, now()],
        )?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypt::DbKey;

    #[test]
    fn mailbox_sources_round_trip_without_their_secrets() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("et.db"), &DbKey::generate().unwrap()).unwrap();
        let config = ImapConfig {
            host: "imap.fastmail.com".into(),
            port: 993,
            username: "me@fastmail.test".into(),
        };
        let id = add_imap(&store, "Fastmail", &config).unwrap();
        assert_eq!(mailbox(&store, id).unwrap(), Some(Mailbox::Imap(config)));
        let outlook = OutlookConfig {
            username: "me@outlook.test".into(),
        };
        let outlook_id = add_outlook(&store, "Outlook", &outlook).unwrap();
        assert_eq!(
            mailbox(&store, outlook_id).unwrap(),
            Some(Mailbox::Outlook(outlook))
        );

        mark_synced(&store, id).unwrap();
        let sources = list(&store).unwrap();
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].label, "Fastmail");
        assert!(sources[0].last_sync_at.is_some());
    }
}
