//! A sweep (S11): what a set of senders and services would give up, then
//! one unsubscribe after another with an event per item.
//!
//! A service is unsubscribed through each of its senders that sends list
//! mail. Receipts carry no list headers, so billing mail keeps arriving.

use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use specta::Type;

use super::Outcome;
use super::unsubscribe::{Attempt, Route, Sent, finish, plan};
use crate::extract::rfc3339_utc;
use crate::source::unix_now;
use crate::store::{Store, StoreError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Target {
    Sender { id: u32 },
    Service { id: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum RouteKind {
    OneClick,
    Page,
    Mail,
    Nothing,
}

impl From<&Route> for RouteKind {
    fn from(route: &Route) -> Self {
        match route {
            Route::OneClick { .. } => RouteKind::OneClick,
            Route::Page { .. } => RouteKind::Page,
            Route::Mail { .. } => RouteKind::Mail,
            Route::Nothing => RouteKind::Nothing,
        }
    }
}

/// One S11 row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub target: Target,
    pub name: String,
    /// List mail from its senders in the last twelve months.
    pub emails_per_year: u32,
    pub critical: bool,
    /// The best route among its senders: one-click, then a page, then email.
    pub route: RouteKind,
    /// Senders the sweep would unsubscribe.
    pub senders: u32,
    /// Critical services start excluded unless the user turned that off.
    pub included: bool,
}

/// Senders with list mail that a target reaches. A sender asked for by
/// name is always reached; a service reaches those still subscribed.
fn senders(conn: &Connection, target: Target) -> Result<Vec<i64>, StoreError> {
    let (sql, id) = match target {
        Target::Sender { id } => ("SELECT id FROM sender WHERE id = ?1", id),
        Target::Service { id } => (
            "SELECT s.id FROM sender s
             WHERE s.service_id = ?1 AND s.unsubscribed_at IS NULL
               AND EXISTS (SELECT 1 FROM message m WHERE m.sender_id = s.id AND m.is_list = 1)
             ORDER BY s.message_count DESC, s.id",
            id,
        ),
    };
    let mut stmt = conn.prepare(sql)?;
    let ids = stmt
        .query_map([id], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(ids)
}

/// The name, and whether the target is or belongs to a critical service.
fn describe(conn: &Connection, target: Target) -> Result<Option<(String, bool)>, StoreError> {
    let row = match target {
        Target::Sender { id } => conn.query_row(
            "SELECT coalesce(s.display_name, s.address), coalesce(v.is_critical, 0)
             FROM sender s LEFT JOIN service v ON v.id = s.service_id WHERE s.id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ),
        Target::Service { id } => conn.query_row(
            "SELECT name, is_critical FROM service WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ),
    };
    Ok(row.optional()?)
}

fn rank(route: RouteKind) -> u8 {
    match route {
        RouteKind::OneClick => 0,
        RouteKind::Page => 1,
        RouteKind::Mail => 2,
        RouteKind::Nothing => 3,
    }
}

/// S11's list, in the order given. Unknown targets are left out.
pub fn review(
    store: &Store,
    targets: &[Target],
    exclude_critical: bool,
    now: i64,
) -> Result<Vec<Item>, StoreError> {
    let conn = store.read()?;
    let first = crate::view::first_month(now);
    let mut items = Vec::new();
    for &target in targets {
        let Some((name, critical)) = describe(&conn, target)? else {
            continue;
        };
        let ids = senders(&conn, target)?;
        let mut route = RouteKind::Nothing;
        let mut emails_per_year = 0;
        for &id in &ids {
            if let Some(attempt) = plan(&conn, id)? {
                let kind = RouteKind::from(&attempt.route);
                if rank(kind) < rank(route) {
                    route = kind;
                }
            }
            emails_per_year += conn.query_row(
                "SELECT coalesce(sum(message_count), 0) FROM aggregate
                 WHERE subject_kind = 'sender' AND subject_id = ?1 AND month >= ?2",
                rusqlite::params![id, first],
                |r| r.get::<_, u32>(0),
            )?;
        }
        items.push(Item {
            target,
            name,
            emails_per_year,
            critical,
            route,
            senders: ids.len() as u32,
            included: !(critical && exclude_critical),
        });
    }
    Ok(items)
}

/// What the sweep should run: a target, and whether the user confirmed it
/// in S10 when it is critical.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunItem {
    pub target: Target,
    pub critical_confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ItemResult {
    pub outcome: Outcome,
    pub detail: String,
    /// The S14 rows written, one per sender.
    pub action_ids: Vec<u32>,
    /// Where the user finishes a `needsYou`: a web page or a `mailto:` URI.
    pub finish_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Event {
    Started { index: u32 },
    Finished { index: u32, result: ItemResult },
}

async fn db<T: Send + 'static>(
    store: &Arc<Store>,
    f: impl FnOnce(&Store) -> Result<T, StoreError> + Send + 'static,
) -> Result<T, StoreError> {
    let store = store.clone();
    tokio::task::spawn_blocking(move || f(&store))
        .await
        .map_err(|_| StoreError::WriterGone)?
}

/// Runs `items` in order and reports each. `send` takes a route; the app
/// passes `unsubscribe::send`. Setting `stop` ends the sweep after the item
/// that is running; the rest are not started. Returns how many ran.
pub async fn run<F, Fut>(
    store: Arc<Store>,
    items: Vec<RunItem>,
    stop: Arc<AtomicBool>,
    send: F,
    events: impl Fn(Event),
) -> Result<u32, StoreError>
where
    F: Fn(Route) -> Fut,
    Fut: Future<Output = Sent>,
{
    let mut ran = 0;
    for (index, item) in (0u32..).zip(items) {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        events(Event::Started { index });
        let target = item.target;
        let (critical, attempts) = db(&store, move |store| {
            let conn = store.read()?;
            let critical = describe(&conn, target)?.is_some_and(|(_, c)| c);
            let mut attempts = Vec::new();
            for id in senders(&conn, target)? {
                attempts.extend(plan(&conn, id)?);
            }
            Ok((critical, attempts))
        })
        .await?;

        let result = if critical && !item.critical_confirmed {
            ItemResult {
                outcome: Outcome::Failed,
                detail: "not run: a critical service needs its own confirmation".into(),
                action_ids: Vec::new(),
                finish_at: None,
            }
        } else {
            let mut done: Vec<(Attempt, Sent)> = Vec::new();
            for attempt in attempts {
                let sent = send(attempt.route.clone()).await;
                done.push((attempt, sent));
            }
            let at = rfc3339_utc(unix_now());
            let logged = done.clone();
            let action_ids = db(&store, move |store| {
                store.write(move |conn| {
                    let tx = conn.transaction()?;
                    let mut ids = Vec::new();
                    for (attempt, sent) in &logged {
                        ids.push(finish(&tx, attempt, sent, &at)? as u32);
                    }
                    tx.commit()?;
                    Ok(ids)
                })
            })
            .await?;
            summarize(&done, action_ids)
        };
        events(Event::Finished { index, result });
        ran += 1;
    }
    Ok(ran)
}

/// One result for an item that may cover several senders.
fn summarize(done: &[(Attempt, Sent)], action_ids: Vec<u32>) -> ItemResult {
    let count = |o: Outcome| done.iter().filter(|(_, s)| s.outcome == o).count();
    let finish_at = done.iter().find_map(|(a, s)| match (&a.route, s.outcome) {
        (Route::Page { url }, Outcome::NeedsYou) => Some(url.clone()),
        (Route::Mail { uri }, Outcome::NeedsYou) => Some(uri.clone()),
        _ => None,
    });
    let (outcome, detail) = match done {
        [] => (
            Outcome::Failed,
            "no sender of this service sends list mail".to_owned(),
        ),
        [(_, sent)] => (sent.outcome, sent.detail.clone()),
        _ => {
            let (ok, failed) = (count(Outcome::Succeeded), count(Outcome::Failed));
            let outcome = if failed > 0 {
                Outcome::Failed
            } else if ok == done.len() {
                Outcome::Succeeded
            } else {
                Outcome::NeedsYou
            };
            (
                outcome,
                format!("{ok} of {} senders unsubscribed", done.len()),
            )
        }
    };
    ItemResult {
        outcome,
        detail,
        action_ids,
        finish_at,
    }
}
