//! The SQLCipher database (PLAN.md 2.1): one writer thread, read connections
//! opened on demand.

mod migrate;

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread::JoinHandle;

use rusqlite::{Connection, ErrorCode, OpenFlags};
use zeroize::Zeroizing;

use crate::crypt::DbKey;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// The key does not open this file: the keychain entry was replaced or the
    /// file is not ours. S16 offers a fresh scan.
    #[error("the database key does not open this file")]
    Locked,
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("sqlite build lacks SQLCipher")]
    NotEncrypted,
    #[error("the database is at schema {0}, newer than this app")]
    NewerSchema(usize),
    #[error("the writer thread has stopped")]
    WriterGone,
}

type Job = Box<dyn FnOnce(&mut Connection) + Send>;

pub struct Store {
    path: PathBuf,
    key: Zeroizing<String>,
    jobs: Option<mpsc::Sender<Job>>,
    writer: Option<JoinHandle<()>>,
}

impl Store {
    /// Opens or creates the database at `path`, encrypted under `key`, and
    /// brings its schema up to date.
    pub fn open(path: &Path, key: &DbKey) -> Result<Self, StoreError> {
        let key = key.pragma_value();
        let mut conn = connect(path, &key, OpenFlags::default())?;
        conn.pragma_update(None, "journal_mode", "wal")?;
        migrate::migrate(&mut conn)?;

        let (jobs, inbox) = mpsc::channel::<Job>();
        let writer = std::thread::Builder::new()
            .name("store-writer".into())
            .spawn(move || {
                for job in inbox {
                    job(&mut conn);
                }
            })
            .map_err(|_| StoreError::WriterGone)?;

        Ok(Self {
            path: path.to_owned(),
            key,
            jobs: Some(jobs),
            writer: Some(writer),
        })
    }

    /// Runs `f` on the single writer connection and waits for its result.
    /// Blocking: async callers wrap it in `spawn_blocking`.
    pub fn write<T, F>(&self, f: F) -> Result<T, StoreError>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> Result<T, StoreError> + Send + 'static,
    {
        let (reply, answer) = mpsc::sync_channel(1);
        let job: Job = Box::new(move |conn| {
            let _ = reply.send(f(conn));
        });
        self.jobs
            .as_ref()
            .ok_or(StoreError::WriterGone)?
            .send(job)
            .map_err(|_| StoreError::WriterGone)?;
        answer.recv().map_err(|_| StoreError::WriterGone)?
    }

    /// A new read-only connection. Cheap: the key is raw, so there is no
    /// password derivation.
    pub fn read(&self) -> Result<Connection, StoreError> {
        connect(&self.path, &self.key, OpenFlags::SQLITE_OPEN_READ_ONLY)
    }

    pub fn cipher_version(&self) -> Result<String, StoreError> {
        self.read()?
            .query_row("PRAGMA cipher_version", [], |row| row.get(0))
            .map_err(|err| match err {
                rusqlite::Error::QueryReturnedNoRows => StoreError::NotEncrypted,
                other => other.into(),
            })
    }

    pub fn schema_version(&self) -> Result<usize, StoreError> {
        let version: i64 = self
            .read()?
            .pragma_query_value(None, "user_version", |row| row.get(0))?;
        Ok(usize::try_from(version).unwrap_or(usize::MAX))
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        drop(self.jobs.take());
        if let Some(writer) = self.writer.take() {
            let _ = writer.join();
        }
    }
}

fn connect(path: &Path, key: &str, flags: OpenFlags) -> Result<Connection, StoreError> {
    let conn = Connection::open_with_flags(path, flags)?;
    conn.pragma_update(None, "key", key)?;
    // SQLCipher reports a wrong key only on the first read.
    match conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(())) {
        Ok(()) => {}
        Err(rusqlite::Error::SqliteFailure(err, _)) if err.code == ErrorCode::NotADatabase => {
            return Err(StoreError::Locked);
        }
        Err(err) => return Err(err.into()),
    }
    conn.pragma_update(None, "foreign_keys", true)?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(byte: u8) -> DbKey {
        DbKey::from_bytes(&[byte; 32]).unwrap()
    }

    fn open(dir: &tempfile::TempDir, byte: u8) -> Store {
        Store::open(&dir.path().join("et.db"), &key(byte)).unwrap()
    }

    #[test]
    fn open_applies_every_migration_once() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(open(&dir, 1).schema_version().unwrap(), migrate::latest());
        assert_eq!(open(&dir, 1).schema_version().unwrap(), migrate::latest());
    }

    #[test]
    fn a_newer_schema_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let store = open(&dir, 1);
        store
            .write(|conn| Ok(conn.pragma_update(None, "user_version", 999)?))
            .unwrap();
        drop(store);
        let err = Store::open(&dir.path().join("et.db"), &key(1))
            .err()
            .unwrap();
        assert!(matches!(err, StoreError::NewerSchema(999)));
    }

    #[test]
    fn data_survives_a_reopen_with_the_same_key() {
        let dir = tempfile::tempdir().unwrap();
        open(&dir, 1)
            .write(|conn| {
                conn.execute("INSERT INTO setting VALUES ('k', 'kept')", [])?;
                Ok(())
            })
            .unwrap();
        let v: String = open(&dir, 1)
            .read()
            .unwrap()
            .query_row("SELECT value FROM setting WHERE key = 'k'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(v, "kept");
    }

    #[test]
    fn the_file_on_disk_is_not_plain_sqlite() {
        let dir = tempfile::tempdir().unwrap();
        let store = open(&dir, 2);
        store
            .write(|conn| {
                conn.execute(
                    "INSERT INTO setting VALUES ('k', 'secret subject line')",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        drop(store);

        let needle = b"secret subject line";
        for name in ["et.db", "et.db-wal"] {
            let Ok(bytes) = std::fs::read(dir.path().join(name)) else {
                continue;
            };
            assert!(!bytes.starts_with(b"SQLite format 3\0"), "{name}");
            assert!(!bytes.windows(needle.len()).any(|w| w == needle), "{name}");
        }
    }

    #[test]
    fn a_wrong_key_reports_locked() {
        let dir = tempfile::tempdir().unwrap();
        drop(open(&dir, 3));
        assert!(matches!(
            Store::open(&dir.path().join("et.db"), &key(4)),
            Err(StoreError::Locked)
        ));
    }

    #[test]
    fn writes_from_many_threads_are_serialised() {
        let dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(open(&dir, 5));
        let handles: Vec<_> = (0..8)
            .map(|i| {
                let store = store.clone();
                std::thread::spawn(move || {
                    store
                        .write(move |conn| {
                            conn.execute(
                                "INSERT INTO setting VALUES (?1, 'v')",
                                [format!("k{i}")],
                            )?;
                            Ok(())
                        })
                        .unwrap()
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }
        let n: i64 = store
            .read()
            .unwrap()
            .query_row("SELECT count(*) FROM setting", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 8);
    }

    #[test]
    fn the_build_is_sqlcipher_and_message_search_works() {
        let dir = tempfile::tempdir().unwrap();
        let store = open(&dir, 6);
        assert!(!store.cipher_version().unwrap().is_empty());
        store
            .write(|conn| {
                conn.execute_batch(
                    "INSERT INTO source (id, kind, label, config, created_at)
                         VALUES (1, 'imap', 'test', '{}', '2026-01-01T00:00:00Z');
                     INSERT INTO sender (id, address, display_name, domain)
                         VALUES (1, 'billing@acme.test', 'Acme Billing', 'acme.test');
                     INSERT INTO message (source_id, locator, sender_id, subject)
                         VALUES (1, '1', 1, 'Your invoice for March');",
                )?;
                Ok(())
            })
            .unwrap();
        let read = store.read().unwrap();
        let hits = |q: &str| -> i64 {
            read.query_row(
                "SELECT count(*) FROM message_fts WHERE message_fts MATCH ?1",
                [q],
                |r| r.get(0),
            )
            .unwrap()
        };
        assert_eq!(hits("invoice"), 1);
        assert_eq!(hits("acme"), 1);
        assert_eq!(hits("refund"), 0);
    }

    #[test]
    fn a_message_is_stored_once_per_locator() {
        let dir = tempfile::tempdir().unwrap();
        let store = open(&dir, 7);
        let result = store.write(|conn| {
            conn.execute_batch(
                "INSERT INTO source (id, kind, label, config, created_at)
                     VALUES (1, 'imap', 'test', '{}', '2026-01-01T00:00:00Z');
                 INSERT INTO mailbox (id, source_id, name, uid_validity) VALUES (1, 1, 'INBOX', 7);
                 INSERT INTO message (source_id, mailbox_id, locator) VALUES (1, 1, '42');
                 INSERT INTO message (source_id, mailbox_id, locator) VALUES (1, 1, '42');",
            )?;
            Ok(())
        });
        assert!(result.is_err());
    }
}
