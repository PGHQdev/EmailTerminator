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

/// An imported file's settings: an mbox file or a Maildir directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileConfig {
    pub path: String,
}

/// A source, by how it is read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reader {
    Imap(ImapConfig),
    Outlook(OutlookConfig),
    Mbox(FileConfig),
    Maildir(FileConfig),
}

impl Reader {
    /// What S14 names as a scan's target: the address, or the file.
    pub fn target(&self) -> &str {
        match self {
            Reader::Imap(c) => &c.username,
            Reader::Outlook(c) => &c.username,
            Reader::Mbox(c) | Reader::Maildir(c) => &c.path,
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

pub fn add_mbox(store: &Store, label: &str, config: &FileConfig) -> Result<i64, StoreError> {
    add(store, "mbox", label, config)
}

pub fn add_maildir(store: &Store, label: &str, config: &FileConfig) -> Result<i64, StoreError> {
    add(store, "maildir", label, config)
}

/// Saves a source, or returns the one already saved with the same kind and
/// settings: a second sign-in to a mailbox, or a second import of a file,
/// reuses it (S12's reconnect).
fn add(store: &Store, kind: &str, label: &str, config: &impl Serialize) -> Result<i64, StoreError> {
    let (kind, label, config) = (
        kind.to_owned(),
        label.to_owned(),
        serde_json::to_string(config).unwrap_or_default(),
    );
    store.write(move |conn| {
        let saved = conn
            .query_row(
                "SELECT id FROM source WHERE kind = ?1 AND config = ?2",
                params![kind, config],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = saved {
            return Ok(id);
        }
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

pub fn reader(store: &Store, id: i64) -> Result<Option<Reader>, StoreError> {
    let row: Option<(String, String)> = store
        .read()?
        .query_row("SELECT kind, config FROM source WHERE id = ?1", [id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .optional()?;
    Ok(row.and_then(|(kind, config)| match kind.as_str() {
        "imap" => serde_json::from_str(&config).ok().map(Reader::Imap),
        "outlook" => serde_json::from_str(&config).ok().map(Reader::Outlook),
        "mbox" => serde_json::from_str(&config).ok().map(Reader::Mbox),
        "maildir" => serde_json::from_str(&config).ok().map(Reader::Maildir),
        _ => None,
    }))
}

/// Deletes a source with its folders and messages, and the senders that only
/// it held. The activity log keeps its rows. A rebuild settles the rest.
pub fn remove(store: &Store, id: i64) -> Result<(), StoreError> {
    store.write(move |conn| {
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM source WHERE id = ?1", [id])?;
        tx.execute(
            "DELETE FROM sender
             WHERE NOT EXISTS (SELECT 1 FROM message m WHERE m.sender_id = sender.id)",
            [],
        )?;
        tx.commit()?;
        Ok(())
    })
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
    fn sources_round_trip_without_their_secrets() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("et.db"), &DbKey::generate().unwrap()).unwrap();
        let config = ImapConfig {
            host: "imap.fastmail.com".into(),
            port: 993,
            username: "me@fastmail.test".into(),
        };
        let id = add_imap(&store, "Fastmail", &config).unwrap();
        assert_eq!(reader(&store, id).unwrap(), Some(Reader::Imap(config)));
        let outlook = OutlookConfig {
            username: "me@outlook.test".into(),
        };
        let outlook_id = add_outlook(&store, "Outlook", &outlook).unwrap();
        assert_eq!(
            reader(&store, outlook_id).unwrap(),
            Some(Reader::Outlook(outlook))
        );

        let mbox = FileConfig {
            path: "/tmp/All mail.mbox".into(),
        };
        let mbox_id = add_mbox(&store, "All mail.mbox", &mbox).unwrap();
        assert_eq!(reader(&store, mbox_id).unwrap(), Some(Reader::Mbox(mbox)));

        mark_synced(&store, id).unwrap();
        let sources = list(&store).unwrap();
        assert_eq!(sources.len(), 3);
        assert_eq!(sources[0].label, "Fastmail");
        assert!(sources[0].last_sync_at.is_some());
    }

    #[test]
    fn a_second_sign_in_reuses_the_source() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("et.db"), &DbKey::generate().unwrap()).unwrap();
        let config = OutlookConfig {
            username: "me@outlook.test".into(),
        };
        let first = add_outlook(&store, "Outlook", &config).unwrap();
        assert_eq!(add_outlook(&store, "Outlook", &config).unwrap(), first);
        let other = OutlookConfig {
            username: "you@outlook.test".into(),
        };
        assert_ne!(add_outlook(&store, "Outlook", &other).unwrap(), first);
        assert_eq!(list(&store).unwrap().len(), 2);
    }

    #[test]
    fn removing_a_source_drops_its_messages_and_lone_senders() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("et.db"), &DbKey::generate().unwrap()).unwrap();
        let file = |path: &str| FileConfig { path: path.into() };
        let gone = add_mbox(&store, "a.mbox", &file("/a.mbox")).unwrap();
        let kept = add_mbox(&store, "b.mbox", &file("/b.mbox")).unwrap();
        store
            .write(move |conn| {
                conn.execute_batch(&format!(
                    "INSERT INTO sender (id, address, domain) VALUES
                         (1, 'only@a.test', 'a.test'), (2, 'both@b.test', 'b.test');
                     INSERT INTO message (source_id, locator, sender_id) VALUES
                         ({gone}, '1', 1), ({gone}, '2', 2), ({kept}, '1', 2);"
                ))?;
                Ok(())
            })
            .unwrap();

        remove(&store, gone).unwrap();
        let conn = store.read().unwrap();
        let count = |sql: &str| conn.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap();
        assert_eq!(count("SELECT count(*) FROM message"), 1);
        assert_eq!(count("SELECT count(*) FROM sender"), 1);
        assert_eq!(list(&store).unwrap().len(), 1);
    }
}
