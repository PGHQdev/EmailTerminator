use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use zeroize::{Zeroize, Zeroizing};

use super::{CryptError, random_32};

const KEYCHAIN_SERVICE: &str = "EmailTerminator";

/// Where secrets live: the database key, app passwords, OAuth tokens, API keys.
/// S17 names the backend in use.
pub trait SecretStore: Send + Sync {
    fn get(&self, name: &str) -> Result<Option<Zeroizing<Vec<u8>>>, CryptError>;
    fn set(&self, name: &str, secret: &[u8]) -> Result<(), CryptError>;
    fn delete(&self, name: &str) -> Result<(), CryptError>;
    fn describe(&self) -> &'static str;
}

/// macOS Keychain, Windows Credential Manager, or Linux Secret Service.
pub struct Keychain;

impl Keychain {
    fn entry(name: &str) -> Result<keyring::Entry, CryptError> {
        Ok(keyring::Entry::new(KEYCHAIN_SERVICE, name)?)
    }
}

impl SecretStore for Keychain {
    fn get(&self, name: &str) -> Result<Option<Zeroizing<Vec<u8>>>, CryptError> {
        match Self::entry(name)?.get_secret() {
            Ok(bytes) => Ok(Some(Zeroizing::new(bytes))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    fn set(&self, name: &str, secret: &[u8]) -> Result<(), CryptError> {
        Ok(Self::entry(name)?.set_secret(secret)?)
    }

    fn delete(&self, name: &str) -> Result<(), CryptError> {
        match Self::entry(name)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(err.into()),
        }
    }

    fn describe(&self) -> &'static str {
        "OS keychain"
    }
}

/// The Linux fallback when Secret Service is absent (PLAN.md 2.2): every secret
/// in one XChaCha20-Poly1305 file, keyed by a random mode-0600 key file beside
/// it. It protects only against a copy that leaves the key file behind.
pub struct EncryptedFile {
    data: PathBuf,
    key: PathBuf,
    lock: std::sync::Mutex<()>,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
struct Secrets(BTreeMap<String, Vec<u8>>);

impl Drop for Secrets {
    fn drop(&mut self) {
        self.0.values_mut().for_each(Zeroize::zeroize);
    }
}

impl EncryptedFile {
    pub fn new(dir: &Path) -> Self {
        Self {
            data: dir.join("secrets.bin"),
            key: dir.join("secrets.key"),
            lock: std::sync::Mutex::new(()),
        }
    }

    fn cipher(&self) -> Result<XChaCha20Poly1305, CryptError> {
        let key = match fs::read(&self.key) {
            Ok(bytes) => Zeroizing::new(bytes),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                let key = random_32()?;
                write_owner_only(&self.key, key.as_ref())?;
                Zeroizing::new(key.to_vec())
            }
            Err(err) => return Err(err.into()),
        };
        XChaCha20Poly1305::new_from_slice(&key).map_err(|_| CryptError::BadLength(key.len()))
    }

    fn load(&self) -> Result<Secrets, CryptError> {
        let sealed = match fs::read(&self.data) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Secrets::default());
            }
            Err(err) => return Err(err.into()),
        };
        if sealed.len() < 24 {
            return Err(CryptError::Corrupt);
        }
        let (nonce, body) = sealed.split_at(24);
        let nonce = XNonce::try_from(nonce).map_err(|_| CryptError::Corrupt)?;
        let plain = Zeroizing::new(
            self.cipher()?
                .decrypt(&nonce, body)
                .map_err(|_| CryptError::Corrupt)?,
        );
        serde_json::from_slice(&plain).map_err(|_| CryptError::Corrupt)
    }

    fn save(&self, secrets: &Secrets) -> Result<(), CryptError> {
        let plain = Zeroizing::new(serde_json::to_vec(secrets).map_err(|_| CryptError::Corrupt)?);
        let mut nonce_bytes = [0u8; 24];
        getrandom::fill(&mut nonce_bytes).map_err(CryptError::Random)?;
        let nonce = XNonce::from(nonce_bytes);
        let body = self
            .cipher()?
            .encrypt(&nonce, plain.as_slice())
            .map_err(|_| CryptError::Corrupt)?;
        let mut sealed = nonce_bytes.to_vec();
        sealed.extend_from_slice(&body);

        let tmp = self.data.with_extension("bin.tmp");
        let _ = fs::remove_file(&tmp);
        write_owner_only(&tmp, &sealed)?;
        fs::rename(&tmp, &self.data)?;
        Ok(())
    }
}

impl SecretStore for EncryptedFile {
    fn get(&self, name: &str) -> Result<Option<Zeroizing<Vec<u8>>>, CryptError> {
        let _guard = self.lock.lock().unwrap_or_else(|p| p.into_inner());
        Ok(self.load()?.0.get(name).cloned().map(Zeroizing::new))
    }

    fn set(&self, name: &str, secret: &[u8]) -> Result<(), CryptError> {
        let _guard = self.lock.lock().unwrap_or_else(|p| p.into_inner());
        let mut secrets = self.load()?;
        secrets.0.insert(name.to_owned(), secret.to_vec());
        self.save(&secrets)
    }

    fn delete(&self, name: &str) -> Result<(), CryptError> {
        let _guard = self.lock.lock().unwrap_or_else(|p| p.into_inner());
        let mut secrets = self.load()?;
        if let Some(mut old) = secrets.0.remove(name) {
            old.zeroize();
            self.save(&secrets)?;
        }
        Ok(())
    }

    fn describe(&self) -> &'static str {
        "encrypted file with its key beside it"
    }
}

fn write_owner_only(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// The keychain, except on Linux without a reachable Secret Service, where the
/// encrypted file in `data_dir` takes its place (PLAN.md 2.2).
pub fn native_secret_store(data_dir: &Path) -> Box<dyn SecretStore> {
    if cfg!(target_os = "linux") && Keychain.get("probe").is_err() {
        return Box::new(EncryptedFile::new(data_dir));
    }
    Box::new(Keychain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_round_trip_and_delete() {
        let dir = tempfile::tempdir().unwrap();
        let store = EncryptedFile::new(dir.path());
        assert!(store.get("imap:1").unwrap().is_none());

        store.set("imap:1", b"app-password").unwrap();
        store.set("database-key", &[7u8; 32]).unwrap();
        assert_eq!(
            store.get("imap:1").unwrap().unwrap().as_slice(),
            b"app-password"
        );

        store.delete("imap:1").unwrap();
        assert!(store.get("imap:1").unwrap().is_none());
        assert_eq!(store.get("database-key").unwrap().unwrap().len(), 32);
        store.delete("imap:1").unwrap();
    }

    #[test]
    fn the_file_holds_no_plaintext_and_rejects_another_key() {
        let dir = tempfile::tempdir().unwrap();
        EncryptedFile::new(dir.path())
            .set("imap:1", b"app-password")
            .unwrap();
        let sealed = fs::read(dir.path().join("secrets.bin")).unwrap();
        assert!(!sealed.windows(12).any(|w| w == b"app-password"));

        fs::remove_file(dir.path().join("secrets.key")).unwrap();
        assert!(matches!(
            EncryptedFile::new(dir.path()).get("imap:1"),
            Err(CryptError::Corrupt)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn both_files_are_readable_by_the_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        EncryptedFile::new(dir.path()).set("k", b"v").unwrap();
        for name in ["secrets.bin", "secrets.key"] {
            let mode = fs::metadata(dir.path().join(name))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600, "{name}");
        }
    }
}
