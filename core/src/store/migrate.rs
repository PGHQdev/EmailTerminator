use rusqlite::Connection;

use super::StoreError;

/// Forward-only migrations, applied at open (PLAN.md 2.1). The file at index
/// `i` moves `user_version` from `i` to `i + 1`.
const MIGRATIONS: &[&str] = &[
    include_str!("../../migrations/001-initial.sql"),
    include_str!("../../migrations/002-price-increase.sql"),
    include_str!("../../migrations/003-actions.sql"),
];

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_003_refetches_from_the_oldest_one_click_candidate() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(MIGRATIONS[0]).unwrap();
        conn.execute_batch(MIGRATIONS[1]).unwrap();
        conn.execute_batch(
            "INSERT INTO source (id, kind, label, config, created_at)
                 VALUES (1, 'imap', 'me', '{}', '2026-01-01T00:00:00Z');
             INSERT INTO mailbox (id, source_id, name, uid_validity, highest_uid)
                 VALUES (1, 1, 'INBOX', 1, 50), (2, 1, 'News', 1, 9);
             INSERT INTO message (source_id, mailbox_id, locator, list_unsubscribe_post)
                 VALUES (1, 1, '3', 0), (1, 1, '30', 1), (1, 1, '7', 1), (1, 2, '4', 0);",
        )
        .unwrap();
        conn.execute_batch(MIGRATIONS[2]).unwrap();
        let highest = |id: i64| -> i64 {
            conn.query_row("SELECT highest_uid FROM mailbox WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .unwrap()
        };
        assert_eq!(highest(1), 6, "UID 7 is fetched again");
        assert_eq!(highest(2), 9, "nothing there asked for one-click");
    }
}
