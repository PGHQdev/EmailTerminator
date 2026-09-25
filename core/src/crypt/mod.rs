//! Secrets and keying for data at rest (PLAN.md 2.1, 2.2).

mod secrets;

pub use secrets::{EncryptedFile, Keychain, SecretStore, native_secret_store};

use zeroize::Zeroizing;

#[derive(Debug, thiserror::Error)]
pub enum CryptError {
    #[error("keychain: {0}")]
    Keychain(#[from] keyring::Error),
    #[error("secrets file: {0}")]
    Io(#[from] std::io::Error),
    #[error("secrets file does not decrypt")]
    Corrupt,
    #[error("stored key has {0} bytes, expected 32")]
    BadLength(usize),
    #[error("random source: {0}")]
    Random(getrandom::Error),
}

const DB_KEY: &str = "database-key";

/// The 256-bit key that SQLCipher opens the database with.
pub struct DbKey(Zeroizing<[u8; 32]>);

impl DbKey {
    pub fn generate() -> Result<Self, CryptError> {
        Ok(Self(random_32()?))
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptError> {
        let array: [u8; 32] = bytes
            .try_into()
            .map_err(|_| CryptError::BadLength(bytes.len()))?;
        Ok(Self(Zeroizing::new(array)))
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// The raw-key form SQLCipher accepts, which skips its password derivation.
    pub(crate) fn pragma_value(&self) -> Zeroizing<String> {
        let mut value = Zeroizing::new(String::with_capacity(67));
        value.push_str("x'");
        for byte in self.0.iter() {
            value.push_str(&format!("{byte:02x}"));
        }
        value.push('\'');
        value
    }
}

/// Returns the stored database key, or generates and stores one on first launch.
pub fn load_or_create_key(store: &dyn SecretStore) -> Result<DbKey, CryptError> {
    if let Some(bytes) = store.get(DB_KEY)? {
        return DbKey::from_bytes(&bytes);
    }
    let key = DbKey::generate()?;
    store.set(DB_KEY, key.as_bytes())?;
    Ok(key)
}

/// Deletes the database key. The database it opened can no longer be read.
pub fn forget_key(store: &dyn SecretStore) -> Result<(), CryptError> {
    store.delete(DB_KEY)
}

pub(crate) fn random_32() -> Result<Zeroizing<[u8; 32]>, CryptError> {
    let mut bytes = Zeroizing::new([0u8; 32]);
    getrandom::fill(bytes.as_mut()).map_err(CryptError::Random)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_call_creates_and_second_call_returns_the_same_key() {
        let dir = tempfile::tempdir().unwrap();
        let store = EncryptedFile::new(dir.path());

        let first = load_or_create_key(&store).unwrap();
        let second = load_or_create_key(&store).unwrap();

        assert_eq!(first.as_bytes(), second.as_bytes());
    }

    #[test]
    fn generated_keys_differ() {
        let a = DbKey::generate().unwrap();
        let b = DbKey::generate().unwrap();
        assert_ne!(a.as_bytes(), b.as_bytes());
    }

    #[test]
    fn a_key_of_the_wrong_length_is_rejected() {
        assert!(matches!(
            DbKey::from_bytes(&[0u8; 16]),
            Err(CryptError::BadLength(16))
        ));
    }

    #[test]
    fn pragma_value_is_a_quoted_hex_blob() {
        let key = DbKey::from_bytes(&[0xab; 32]).unwrap();
        let value = key.pragma_value();
        assert_eq!(value.len(), 67);
        assert!(value.starts_with("x'abab"));
        assert!(value.ends_with("ab'"));
    }
}
