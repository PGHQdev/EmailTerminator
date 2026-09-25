//! Where mail comes from (S12): one row per connected mailbox or file.

use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::extract::rfc3339_utc;
use crate::store::{Store, StoreError};

/// An IMAP source's settings. The password lives in the secret store under
/// [`password_secret`], never in the database.
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

pub fn password_secret(source_id: i64) -> String {
    format!("imap:{source_id}")
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
    let (label, config) = (
        label.to_owned(),
        serde_json::to_string(config).unwrap_or_default(),
    );
    store.write(move |conn| {
        conn.execute(
            "INSERT INTO source (kind, label, config, created_at) VALUES ('imap', ?1, ?2, ?3)",
            params![label, config, now()],
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

pub fn imap_config(store: &Store, id: i64) -> Result<Option<ImapConfig>, StoreError> {
    let config: Option<String> = store
        .read()?
        .query_row(
            "SELECT config FROM source WHERE id = ?1 AND kind = 'imap'",
            [id],
            |r| r.get(0),
        )
        .optional()?;
    Ok(config.and_then(|c| serde_json::from_str(&c).ok()))
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
    fn an_imap_source_round_trips_without_its_password() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("et.db"), &DbKey::generate().unwrap()).unwrap();
        let config = ImapConfig {
            host: "imap.fastmail.com".into(),
            port: 993,
            username: "me@fastmail.test".into(),
        };
        let id = add_imap(&store, "Fastmail", &config).unwrap();
        assert_eq!(imap_config(&store, id).unwrap(), Some(config));

        mark_synced(&store, id).unwrap();
        let sources = list(&store).unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].label, "Fastmail");
        assert!(sources[0].last_sync_at.is_some());
    }
}
