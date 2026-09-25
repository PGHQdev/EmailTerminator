//! Preferences as key/value rows (PLAN.md Part 4, `setting`).

use rusqlite::{OptionalExtension, params};

use crate::store::{Store, StoreError};

pub fn get(store: &Store, key: &str) -> Result<Option<String>, StoreError> {
    Ok(store
        .read()?
        .query_row("SELECT value FROM setting WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .optional()?)
}

pub fn set(store: &Store, key: &str, value: &str) -> Result<(), StoreError> {
    let (key, value) = (key.to_owned(), value.to_owned());
    store.write(move |conn| {
        conn.execute(
            "INSERT INTO setting (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypt::DbKey;

    #[test]
    fn a_setting_is_replaced_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("et.db"), &DbKey::generate().unwrap()).unwrap();
        assert_eq!(get(&store, "appearance").unwrap(), None);
        set(&store, "appearance", "dark").unwrap();
        set(&store, "appearance", "light").unwrap();
        assert_eq!(get(&store, "appearance").unwrap().as_deref(), Some("light"));
    }
}
