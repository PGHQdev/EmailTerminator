use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use super::{CryptError, DbKey};

const KEYCHAIN_SERVICE: &str = "EmailTerminator";
const DB_KEY_ACCOUNT: &str = "database-key";

/// Where the database key lives. S17 names the backend in use.
pub trait KeyStore {
    fn load(&self) -> Result<Option<DbKey>, CryptError>;
    fn save(&self, key: &DbKey) -> Result<(), CryptError>;
    fn delete(&self) -> Result<(), CryptError>;
    fn describe(&self) -> &'static str;
}

/// macOS Keychain, Windows Credential Manager, or Linux Secret Service.
pub struct Keychain {
    entry: keyring::Entry,
}

impl Keychain {
    pub fn new() -> Result<Self, CryptError> {
        Ok(Self {
            entry: keyring::Entry::new(KEYCHAIN_SERVICE, DB_KEY_ACCOUNT)?,
        })
    }
}

impl KeyStore for Keychain {
    fn load(&self) -> Result<Option<DbKey>, CryptError> {
        match self.entry.get_secret() {
            Ok(bytes) => Ok(Some(DbKey::from_bytes(&bytes)?)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    fn save(&self, key: &DbKey) -> Result<(), CryptError> {
        Ok(self.entry.set_secret(key.as_bytes())?)
    }

    fn delete(&self) -> Result<(), CryptError> {
        match self.entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(err.into()),
        }
    }

    fn describe(&self) -> &'static str {
        "OS keychain"
    }
}

/// The Linux fallback when Secret Service is absent: a mode-0600 file beside
/// the database. It protects only against a copy that leaves this file behind.
pub struct KeyFile {
    path: PathBuf,
}

impl KeyFile {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl KeyStore for KeyFile {
    fn load(&self) -> Result<Option<DbKey>, CryptError> {
        match fs::read(&self.path) {
            Ok(bytes) => Ok(Some(DbKey::from_bytes(&bytes)?)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    fn save(&self, key: &DbKey) -> Result<(), CryptError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = owner_only(&self.path)?;
        file.write_all(key.as_bytes())?;
        file.sync_all()?;
        Ok(())
    }

    fn delete(&self) -> Result<(), CryptError> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err.into()),
        }
    }

    fn describe(&self) -> &'static str {
        "key file beside the database"
    }
}

#[cfg(unix)]
fn owner_only(path: &Path) -> std::io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn owner_only(path: &Path) -> std::io::Result<fs::File> {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
}

/// The keychain, except on Linux without a reachable Secret Service, where the
/// key file in `data_dir` takes its place (PLAN.md 2.2).
pub fn native_key_store(data_dir: &Path) -> Result<Box<dyn KeyStore>, CryptError> {
    match Keychain::new() {
        Ok(keychain) if cfg!(target_os = "linux") => match keychain.load() {
            Ok(_) => Ok(Box::new(keychain)),
            Err(_) => Ok(Box::new(KeyFile::new(data_dir.join("db.key")))),
        },
        Ok(keychain) => Ok(Box::new(keychain)),
        Err(_) if cfg!(target_os = "linux") => Ok(Box::new(KeyFile::new(data_dir.join("db.key")))),
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_file_round_trips_and_deletes() {
        let dir = tempfile::tempdir().unwrap();
        let store = KeyFile::new(dir.path().join("nested").join("db.key"));
        assert!(store.load().unwrap().is_none());

        let key = DbKey::generate().unwrap();
        store.save(&key).unwrap();
        assert_eq!(store.load().unwrap().unwrap().as_bytes(), key.as_bytes());

        store.delete().unwrap();
        assert!(store.load().unwrap().is_none());
        store.delete().unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn key_file_is_readable_by_the_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db.key");
        KeyFile::new(path.clone())
            .save(&DbKey::generate().unwrap())
            .unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
