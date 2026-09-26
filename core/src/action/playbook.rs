//! S08: the user follows a playbook by hand and says when it is done. The
//! app sends nothing; it records the cancellation and marks the service.

use rusqlite::{Connection, OptionalExtension};

use super::{Kind, NewEntry, Outcome, record};
use crate::store::StoreError;

/// Records that the user cancelled a service with its playbook. Returns the
/// activity row, or `None` when the service is gone.
pub fn finish(conn: &Connection, service_id: i64, at: &str) -> Result<Option<i64>, StoreError> {
    let Some((name, data_key)) = conn
        .query_row(
            "SELECT name, data_key FROM service WHERE id = ?1",
            [service_id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)),
        )
        .optional()?
    else {
        return Ok(None);
    };
    let source = data_key
        .as_deref()
        .and_then(|key| crate::data::catalog().service(key))
        .and_then(|s| s.playbook.as_ref())
        .map(|p| p.source.clone());
    let id = record(
        conn,
        &NewEntry {
            kind: Kind::Playbook,
            target: name,
            sender_id: None,
            service_id: Some(service_id),
            source_id: None,
            at: at.to_owned(),
            outcome: Outcome::Succeeded,
            detail: "You followed the playbook and confirmed the cancellation.".into(),
            // The app sent nothing; the row keeps where the steps came from.
            request: source,
            message: None,
        },
    )?;
    conn.execute(
        "UPDATE service SET status = 'canceled' WHERE id = ?1",
        [service_id],
    )?;
    Ok(Some(id))
}
