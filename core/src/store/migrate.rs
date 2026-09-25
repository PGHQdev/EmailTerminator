use rusqlite::Connection;

use super::StoreError;

/// Forward-only migrations, applied at open (PLAN.md 2.1). The file at index
/// `i` moves `user_version` from `i` to `i + 1`.
const MIGRATIONS: &[&str] = &[include_str!("../../migrations/001-initial.sql")];

pub(super) fn migrate(conn: &mut Connection) -> Result<(), StoreError> {
    let current: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let current = usize::try_from(current).unwrap_or(usize::MAX);
    if current > MIGRATIONS.len() {
        return Err(StoreError::NewerSchema(current));
    }
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(current) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (index + 1) as i64)?;
        tx.commit()?;
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn latest() -> usize {
    MIGRATIONS.len()
}
