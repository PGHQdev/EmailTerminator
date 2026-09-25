//! What the app does, and the activity log that records it (PLAN.md Part 4
//! `action`, S14). Every attempt writes one row, whatever its outcome.

pub mod bulk;
pub mod http;
pub mod unsubscribe;

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::store::{Store, StoreError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Unsubscribe,
    Playbook,
    Agent,
    /// A scan of one source.
    Sync,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Kind::Unsubscribe => "unsubscribe",
            Kind::Playbook => "playbook",
            Kind::Agent => "agent",
            Kind::Sync => "sync",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "playbook" => Kind::Playbook,
            "agent" => Kind::Agent,
            "sync" => Kind::Sync,
            _ => Kind::Unsubscribe,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    Succeeded,
    Failed,
    /// Nothing the app can send does it; the user finishes on the sender's
    /// page or by email.
    NeedsYou,
}

impl Outcome {
    fn as_str(self) -> &'static str {
        match self {
            Outcome::Succeeded => "succeeded",
            Outcome::Failed => "failed",
            Outcome::NeedsYou => "needs_you",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "succeeded" => Outcome::Succeeded,
            "needs_you" => Outcome::NeedsYou,
            _ => Outcome::Failed,
        }
    }
}

/// A row to write. `message` is the email the action came from, whose
/// subject, sender and date are copied into the row.
#[derive(Debug, Clone)]
pub struct NewEntry {
    pub kind: Kind,
    pub target: String,
    pub sender_id: Option<i64>,
    pub service_id: Option<i64>,
    pub source_id: Option<i64>,
    pub at: String,
    pub outcome: Outcome,
    pub detail: String,
    pub request: Option<String>,
    pub message: Option<i64>,
}

pub fn record(conn: &Connection, entry: &NewEntry) -> Result<i64, StoreError> {
    Ok(conn.query_row(
        "INSERT INTO action (kind, target, sender_id, service_id, source_id, at, outcome,
                             detail, request, message_id,
                             evidence_subject, evidence_from, evidence_date)
         SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                m.subject, coalesce(s.display_name || ' <' || s.address || '>', s.address), m.date
         FROM (SELECT 1) LEFT JOIN message m ON m.id = ?10
                         LEFT JOIN sender s ON s.id = m.sender_id
         RETURNING id",
        params![
            entry.kind.as_str(),
            entry.target,
            entry.sender_id,
            entry.service_id,
            entry.source_id,
            entry.at,
            entry.outcome.as_str(),
            entry.detail,
            entry.request,
            entry.message,
        ],
        |r| r.get(0),
    )?)
}

/// One S14 row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: u32,
    pub kind: Kind,
    pub target: String,
    pub at: String,
    pub outcome: Outcome,
    pub detail: String,
    pub request: Option<String>,
    pub evidence: Option<Evidence>,
}

/// The email an action came from, as it was when the action ran.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Evidence {
    /// The `message` row, or null when a rescan removed it: an IMAP folder
    /// renumbered, or the source was disconnected.
    pub message_id: Option<u32>,
    pub subject: Option<String>,
    pub from: Option<String>,
    pub date: Option<String>,
}

const COLUMNS: &str = "id, kind, target, at, outcome, detail, request, message_id,
                       evidence_subject, evidence_from, evidence_date";

fn entry(r: &rusqlite::Row<'_>) -> rusqlite::Result<Entry> {
    let message_id: Option<u32> = r.get(7)?;
    let subject: Option<String> = r.get(8)?;
    let from: Option<String> = r.get(9)?;
    let date: Option<String> = r.get(10)?;
    let evidence =
        (message_id.is_some() || subject.is_some() || from.is_some()).then_some(Evidence {
            message_id,
            subject,
            from,
            date,
        });
    Ok(Entry {
        id: r.get(0)?,
        kind: Kind::parse(&r.get::<_, String>(1)?),
        target: r.get(2)?,
        at: r.get(3)?,
        outcome: Outcome::parse(&r.get::<_, String>(4)?),
        detail: r.get(5)?,
        request: r.get(6)?,
        evidence,
    })
}

/// Newest first.
pub fn log(store: &Store) -> Result<Vec<Entry>, StoreError> {
    let conn = store.read()?;
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM action ORDER BY at DESC, id DESC"
    ))?;
    let rows = stmt.query_map([], entry)?.collect::<Result<_, _>>()?;
    Ok(rows)
}

pub fn get(conn: &Connection, id: u32) -> Result<Option<Entry>, StoreError> {
    Ok(conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM action WHERE id = ?1"),
            [id],
            entry,
        )
        .optional()?)
}

/// Where the original of a message is, for an IMAP source: the folder, its
/// UIDVALIDITY when the message was stored, and the UID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Locator {
    pub source_id: i64,
    pub folder: String,
    pub uid_validity: u32,
    pub uid: u32,
}

pub fn locate(conn: &Connection, message_id: u32) -> Result<Option<Locator>, StoreError> {
    Ok(conn
        .query_row(
            "SELECT m.source_id, b.name, b.uid_validity, m.locator
             FROM message m JOIN mailbox b ON b.id = m.mailbox_id
             WHERE m.id = ?1",
            [message_id],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, u32>(2)?,
                    r.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?
        .and_then(|(source_id, folder, uid_validity, locator)| {
            Some(Locator {
                source_id,
                folder,
                uid_validity,
                uid: locator.parse().ok()?,
            })
        }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypt::DbKey;

    #[test]
    fn a_row_keeps_its_evidence_after_the_message_goes() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("et.db"), &DbKey::generate().unwrap()).unwrap();
        let id = store
            .write(|conn| {
                conn.execute_batch(
                    "INSERT INTO source (id, kind, label, config, created_at)
                         VALUES (1, 'imap', 'me', '{}', '2026-01-01T00:00:00Z');
                     INSERT INTO mailbox (id, source_id, name, uid_validity) VALUES (1, 1, 'INBOX', 9);
                     INSERT INTO sender (id, address, display_name, domain)
                         VALUES (1, 'news@dispatch.test', 'The Dispatch', 'dispatch.test');
                     INSERT INTO message (id, source_id, mailbox_id, locator, sender_id, subject, date)
                         VALUES (5, 1, 1, '42', 1, 'Issue 12', '2026-03-01T09:30:00Z');",
                )?;
                record(
                    conn,
                    &NewEntry {
                        kind: Kind::Unsubscribe,
                        target: "The Dispatch".into(),
                        sender_id: Some(1),
                        service_id: None,
                        source_id: None,
                        at: "2026-03-02T10:00:00Z".into(),
                        outcome: Outcome::Succeeded,
                        detail: "the sender answered 200".into(),
                        request: Some("POST https://dispatch.test/u".into()),
                        message: Some(5),
                    },
                )
            })
            .unwrap();

        let conn = store.read().unwrap();
        assert_eq!(
            locate(&conn, 5).unwrap(),
            Some(Locator {
                source_id: 1,
                folder: "INBOX".into(),
                uid_validity: 9,
                uid: 42
            })
        );
        drop(conn);
        store
            .write(|conn| Ok(conn.execute("DELETE FROM message", [])?))
            .unwrap();
        let row = get(&store.read().unwrap(), id as u32).unwrap().unwrap();
        assert_eq!(row.outcome, Outcome::Succeeded);
        assert_eq!(
            row.evidence,
            Some(Evidence {
                message_id: None,
                subject: Some("Issue 12".into()),
                from: Some("The Dispatch <news@dispatch.test>".into()),
                date: Some("2026-03-01T09:30:00Z".into()),
            })
        );
        assert_eq!(log(&store).unwrap().len(), 1);
    }
}
