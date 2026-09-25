//! The SQLCipher database (PLAN.md 2.1).

use std::path::Path;

use rusqlite::{Connection, ErrorCode};

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
}

pub struct Store {
    conn: Connection,
}

impl Store {
    /// Opens or creates the database at `path`, encrypted under `key`.
    pub fn open(path: &Path, key: &DbKey) -> Result<Self, StoreError> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "key", key.pragma_value().as_str())?;
        // SQLCipher reports a wrong key only on the first read.
        match conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(())) {
            Ok(()) => {}
            Err(rusqlite::Error::SqliteFailure(err, _)) if err.code == ErrorCode::NotADatabase => {
                return Err(StoreError::Locked);
            }
            Err(err) => return Err(err.into()),
        }
        let store = Self { conn };
        store.cipher_version()?;
        Ok(store)
    }

    pub fn cipher_version(&self) -> Result<String, StoreError> {
        self.conn
            .query_row("PRAGMA cipher_version", [], |row| row.get(0))
            .map_err(|err| match err {
                rusqlite::Error::QueryReturnedNoRows => StoreError::NotEncrypted,
                other => other.into(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(byte: u8) -> DbKey {
        DbKey::from_bytes(&[byte; 32]).unwrap()
    }

    #[test]
    fn data_survives_a_reopen_with_the_same_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("et.db");
        {
            let store = Store::open(&path, &key(1)).unwrap();
            store.conn.execute("CREATE TABLE t (v TEXT)", []).unwrap();
            store
                .conn
                .execute("INSERT INTO t VALUES ('kept')", [])
                .unwrap();
        }
        let store = Store::open(&path, &key(1)).unwrap();
        let v: String = store
            .conn
            .query_row("SELECT v FROM t", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, "kept");
    }

    #[test]
    fn the_file_on_disk_is_not_plain_sqlite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("et.db");
        let store = Store::open(&path, &key(2)).unwrap();
        store.conn.execute("CREATE TABLE t (v TEXT)", []).unwrap();
        store
            .conn
            .execute("INSERT INTO t VALUES ('secret subject line')", [])
            .unwrap();
        drop(store);

        let bytes = std::fs::read(&path).unwrap();
        assert!(!bytes.starts_with(b"SQLite format 3\0"));
        let needle = b"secret subject line";
        assert!(!bytes.windows(needle.len()).any(|w| w == needle));
    }

    #[test]
    fn a_wrong_key_reports_locked() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("et.db");
        {
            let store = Store::open(&path, &key(3)).unwrap();
            store.conn.execute("CREATE TABLE t (v TEXT)", []).unwrap();
        }
        assert!(matches!(
            Store::open(&path, &key(4)),
            Err(StoreError::Locked)
        ));
    }

    #[test]
    fn the_build_is_sqlcipher_with_fts5() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("et.db"), &key(5)).unwrap();
        assert!(!store.cipher_version().unwrap().is_empty());
        store
            .conn
            .execute("CREATE VIRTUAL TABLE f USING fts5(subject)", [])
            .unwrap();
        store
            .conn
            .execute("INSERT INTO f VALUES ('Your invoice from Acme')", [])
            .unwrap();
        let hits: i64 = store
            .conn
            .query_row("SELECT count(*) FROM f WHERE f MATCH 'invoice'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(hits, 1);
    }
}
