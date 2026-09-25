//! The local data directory (PLAN.md 2.1, S17): where it is, moving it, and
//! erasing it.
//!
//! Both changes finish at the next launch, before the database opens, so no
//! file is removed while a connection holds it; Windows refuses that. The
//! app restarts right after it asks for either one.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::crypt::{DbKey, SecretStore, forget_key, load_or_create_key};
use crate::source;
use crate::store::{Store, StoreError};

pub const DB_FILE: &str = "emailterminator.db";

/// In the default directory: the path the data moved to.
const POINTER: &str = "location";
/// In the data directory: erase at the next launch.
const ERASE: &str = "erase-requested";
/// In a moved-to directory: the directory it was copied from, to clean up.
const MOVED_FROM: &str = "moved-from";

/// The files that are the local data. The Linux secrets file (PLAN.md 2.2)
/// moves with the database because its key sits beside it.
const DATA_FILES: &[&str] = &[
    DB_FILE,
    "emailterminator.db-wal",
    "emailterminator.db-shm",
    "secrets.bin",
    "secrets.key",
];

#[derive(Debug, thiserror::Error)]
pub enum MoveError {
    #[error("the data is already there")]
    Same,
    #[error("{0} already holds files; choose an empty folder")]
    NotEmpty(PathBuf),
    #[error("the copy does not match the original, so nothing was moved")]
    Mismatch,
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("{0}")]
    Store(#[from] StoreError),
}

/// The data directory in use: the default one, or where a move put it.
pub fn resolve(default: &Path) -> PathBuf {
    fs::read_to_string(default.join(POINTER))
        .ok()
        .map(|p| PathBuf::from(p.trim()))
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| default.to_owned())
}

/// Finishes what the last session asked for, before the database opens.
pub fn prepare(dir: &Path, secrets: &dyn SecretStore) -> io::Result<()> {
    if let Ok(from) = fs::read_to_string(dir.join(MOVED_FROM)) {
        let from = PathBuf::from(from.trim());
        if from != dir {
            remove_data_files(&from)?;
        }
        fs::remove_file(dir.join(MOVED_FROM))?;
    }
    if dir.join(ERASE).exists() {
        erase_now(dir, secrets)?;
    }
    Ok(())
}

/// Erases the local data at the next launch. The mailbox is untouched.
pub fn request_erase(dir: &Path) -> io::Result<()> {
    fs::write(dir.join(ERASE), b"")
}

/// Deletes each source's password and the database key, then the files. A
/// database the key no longer opens has no readable sources; its passwords
/// stay in the keychain until a new source reuses the name.
fn erase_now(dir: &Path, secrets: &dyn SecretStore) -> io::Result<()> {
    let other = |e: &dyn std::fmt::Display| io::Error::other(e.to_string());
    if let Ok(key) = load_or_create_key(secrets)
        && let Ok(store) = Store::open(&dir.join(DB_FILE), &key)
        && let Ok(sources) = source::list(&store)
    {
        for s in sources {
            secrets
                .delete(&source::password_secret(s.id))
                .map_err(|e| other(&e))?;
        }
    }
    forget_key(secrets).map_err(|e| other(&e))?;
    // The database files only: the Linux secrets file will also hold the
    // licence, which an erase keeps (DESIGN.md S17).
    for name in &DATA_FILES[..3] {
        remove_if_present(&dir.join(name))?;
    }
    fs::remove_file(dir.join(ERASE))
}

/// Copies the data into `parent/EmailTerminator`, checks the copy opens with
/// the same contents, and points the default directory at it. The original
/// goes at the next launch, once the copy is the one in use.
pub fn move_to(
    store: &Store,
    key: &DbKey,
    from: &Path,
    default: &Path,
    parent: &Path,
) -> Result<PathBuf, MoveError> {
    let to = parent.join("EmailTerminator");
    if to == from {
        return Err(MoveError::Same);
    }
    if to.exists() && fs::read_dir(&to)?.next().is_some() {
        return Err(MoveError::NotEmpty(to));
    }
    store.write(|conn| {
        conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
        Ok(())
    })?;
    fs::create_dir_all(&to)?;
    for name in DATA_FILES {
        let source = from.join(name);
        if source.exists() {
            fs::copy(&source, to.join(name))?;
        }
    }

    let fingerprint = |s: &Store| -> Result<(usize, i64), StoreError> {
        let messages = s
            .read()?
            .query_row("SELECT count(*) FROM message", [], |r| r.get(0))?;
        Ok((s.schema_version()?, messages))
    };
    let copy = Store::open(&to.join(DB_FILE), key);
    let matches = match &copy {
        Ok(copy) => fingerprint(copy)? == fingerprint(store)?,
        Err(_) => false,
    };
    drop(copy);
    if !matches {
        remove_data_files(&to)?;
        return Err(MoveError::Mismatch);
    }

    fs::write(to.join(MOVED_FROM), from.to_string_lossy().as_bytes())?;
    if to == default {
        remove_if_present(&default.join(POINTER))?;
    } else {
        fs::create_dir_all(default)?;
        fs::write(default.join(POINTER), to.to_string_lossy().as_bytes())?;
    }
    Ok(to)
}

/// Bytes on disk of the local data.
pub fn size(dir: &Path) -> u64 {
    DATA_FILES
        .iter()
        .filter_map(|name| fs::metadata(dir.join(name)).ok())
        .map(|m| m.len())
        .sum()
}

fn remove_data_files(dir: &Path) -> io::Result<()> {
    for name in DATA_FILES {
        remove_if_present(&dir.join(name))?;
    }
    Ok(())
}

fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(err) if err.kind() != io::ErrorKind::NotFound => Err(err),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypt::EncryptedFile;
    use crate::source::{ImapConfig, add_imap};

    fn open(dir: &Path, secrets: &dyn SecretStore) -> Store {
        Store::open(&dir.join(DB_FILE), &load_or_create_key(secrets).unwrap()).unwrap()
    }

    #[test]
    fn an_erase_finishes_at_the_next_launch() {
        let root = tempfile::tempdir().unwrap();
        let secrets = EncryptedFile::new(&root.path().join("keys"));
        let dir = root.path();
        let store = open(dir, &secrets);
        let config = ImapConfig {
            host: "imap.test".into(),
            port: 993,
            username: "me@example.test".into(),
        };
        let id = add_imap(&store, "Test", &config).unwrap();
        secrets.set(&source::password_secret(id), b"pw").unwrap();
        drop(store);

        request_erase(dir).unwrap();
        prepare(dir, &secrets).unwrap();

        assert!(!dir.join(DB_FILE).exists());
        assert!(!dir.join(ERASE).exists());
        assert!(secrets.get(&source::password_secret(id)).unwrap().is_none());
        assert!(secrets.get("database-key").unwrap().is_none());
        // A fresh launch starts empty under a new key.
        assert!(source::list(&open(dir, &secrets)).unwrap().is_empty());
    }

    #[test]
    fn a_move_copies_verifies_and_cleans_up_at_the_next_launch() {
        let root = tempfile::tempdir().unwrap();
        let secrets = EncryptedFile::new(&root.path().join("keys"));
        let default = root.path().join("default");
        let elsewhere = root.path().join("elsewhere");
        fs::create_dir_all(&default).unwrap();
        fs::create_dir_all(&elsewhere).unwrap();
        let store = open(&default, &secrets);
        let config = ImapConfig {
            host: "imap.test".into(),
            port: 993,
            username: "me@example.test".into(),
        };
        add_imap(&store, "Test", &config).unwrap();
        let key = load_or_create_key(&secrets).unwrap();

        let to = move_to(&store, &key, &default, &default, &elsewhere).unwrap();
        drop(store);
        assert_eq!(to, elsewhere.join("EmailTerminator"));
        assert_eq!(resolve(&default), to);
        assert!(
            default.join(DB_FILE).exists(),
            "the original stays until the next launch"
        );

        prepare(&to, &secrets).unwrap();
        assert!(!default.join(DB_FILE).exists());
        assert!(!to.join(MOVED_FROM).exists());
        assert_eq!(source::list(&open(&to, &secrets)).unwrap().len(), 1);
    }

    #[test]
    fn a_move_refuses_a_folder_that_holds_files() {
        let root = tempfile::tempdir().unwrap();
        let secrets = EncryptedFile::new(&root.path().join("keys"));
        let default = root.path().join("default");
        fs::create_dir_all(&default).unwrap();
        let store = open(&default, &secrets);
        let key = load_or_create_key(&secrets).unwrap();
        let busy = root.path().join("busy");
        fs::create_dir_all(busy.join("EmailTerminator")).unwrap();
        fs::write(busy.join("EmailTerminator/notes.txt"), b"mine").unwrap();

        assert!(matches!(
            move_to(&store, &key, &default, &default, &busy),
            Err(MoveError::NotEmpty(_))
        ));
    }
}
