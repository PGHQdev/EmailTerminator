//! M3's acceptance: a sweep over a scanned mailbox reports each item,
//! leaves critical services out by default, and logs evidence for each.

mod support {
    pub mod mail;
}

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use et_core::action::bulk::{self, Event, RouteKind, RunItem, Target};
use et_core::action::unsubscribe::{self, Route, Sent};
use et_core::action::{self, Outcome};
use et_core::crypt::DbKey;
use et_core::extract::parse_rfc3339_utc;
use et_core::store::Store;
use et_core::view;
use support::mail::{newsletter, receipt, scan};

/// Dispatch offers one-click; Digest only a page; Acme bills and also sends
/// list mail from its own domain.
fn scanned() -> (tempfile::TempDir, Arc<Store>) {
    let dir = tempfile::tempdir().unwrap();
    let store =
        Arc::new(Store::open(&dir.path().join("et.db"), &DbKey::generate().unwrap()).unwrap());
    store
        .write(|conn| {
            conn.execute_batch(
                "INSERT INTO source (id, kind, label, config, created_at)
                 VALUES (1, 'imap', 'me@example.test', '{}', '2026-01-01T00:00:00Z');",
            )?;
            Ok(())
        })
        .unwrap();
    let mut mail = Vec::new();
    for day in 1..=21 {
        mail.push(newsletter("dispatch", day, true));
    }
    for day in [3, 17] {
        mail.push(newsletter("digest", day, false));
    }
    for month in 12..18 {
        mail.push(receipt("acme", month, "$10.00"));
    }
    for day in [2, 9] {
        mail.push(newsletter("acme", day, true));
    }
    scan(&store, mail);
    (dir, store)
}

fn sender(store: &Store, address: &str) -> u32 {
    store
        .read()
        .unwrap()
        .query_row("SELECT id FROM sender WHERE address = ?1", [address], |r| {
            r.get(0)
        })
        .unwrap()
}

fn now() -> i64 {
    parse_rfc3339_utc("2026-07-01T00:00:00Z").unwrap()
}

/// Answers every one-click POST with 200 and takes the other routes as the
/// app does, so no request leaves the test.
async fn fake_send(route: Route) -> Sent {
    match route {
        Route::OneClick { url } => Sent {
            outcome: Outcome::Succeeded,
            detail: "the sender answered 200".into(),
            request: Some(format!("POST {url}")),
        },
        other => unsubscribe::send(&other).await,
    }
}

#[tokio::test]
async fn a_sweep_reports_each_item_and_leaves_critical_services_out() {
    let (_dir, store) = scanned();
    store
        .write(|conn| {
            Ok(conn.execute(
                "UPDATE service SET is_critical = 1 WHERE group_key = 'acme.test'",
                [],
            )?)
        })
        .unwrap();
    let acme: u32 = store
        .read()
        .unwrap()
        .query_row(
            "SELECT id FROM service WHERE group_key = 'acme.test'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let targets = [
        Target::Sender {
            id: sender(&store, "news@dispatch.test"),
        },
        Target::Sender {
            id: sender(&store, "news@digest.test"),
        },
        Target::Service { id: acme },
    ];

    let review = bulk::review(&store, &targets, true, now()).unwrap();
    let shape: Vec<(RouteKind, bool, bool)> = review
        .iter()
        .map(|i| (i.route, i.critical, i.included))
        .collect();
    assert_eq!(
        shape,
        vec![
            (RouteKind::OneClick, false, true),
            (RouteKind::Page, false, true),
            (RouteKind::OneClick, true, false),
        ]
    );
    assert_eq!(review[0].emails_per_year, 21);
    assert_eq!(review[2].senders, 1, "the list sender, not billing@");
    assert!(
        bulk::review(&store, &targets, false, now()).unwrap()[2].included,
        "the setting can include critical services"
    );

    // The critical one is sent unconfirmed, as a bug in the UI would.
    let items = targets
        .iter()
        .map(|&target| RunItem {
            target,
            critical_confirmed: false,
        })
        .collect();
    let events = Arc::new(Mutex::new(Vec::new()));
    let seen = events.clone();
    let ran = bulk::run(
        store.clone(),
        items,
        Arc::new(AtomicBool::new(false)),
        fake_send,
        move |e| seen.lock().unwrap().push(e),
    )
    .await
    .unwrap();
    assert_eq!(ran, 3);

    let results: Vec<_> = events
        .lock()
        .unwrap()
        .iter()
        .filter_map(|e| match e {
            Event::Finished { result, .. } => Some(result.clone()),
            Event::Started { .. } => None,
        })
        .collect();
    assert_eq!(results[0].outcome, Outcome::Succeeded);
    assert_eq!(results[1].outcome, Outcome::NeedsYou);
    assert_eq!(
        results[1].finish_at.as_deref(),
        Some("https://digest.test/u")
    );
    assert_eq!(results[2].outcome, Outcome::Failed);
    assert!(
        results[2].action_ids.is_empty(),
        "nothing ran, nothing logged"
    );

    // Each attempt has a row, and each row names the email it came from.
    let log = action::log(&store).unwrap();
    assert_eq!(log.len(), 2);
    let dispatch = log.iter().find(|e| e.target == "dispatch").unwrap();
    assert_eq!(
        dispatch.request.as_deref(),
        Some("POST https://dispatch.test/u")
    );
    let evidence = dispatch.evidence.as_ref().unwrap();
    assert_eq!(evidence.subject.as_deref(), Some("Issue 21"));
    assert!(evidence.message_id.is_some());
    assert!(log.iter().all(|e| e.evidence.is_some()));

    let newsletters = view::newsletters(&store, now()).unwrap();
    let state = |name: &str| {
        newsletters
            .iter()
            .find(|n| n.name == name)
            .unwrap()
            .unsubscribed_at
            .is_some()
    };
    assert!(state("dispatch"));
    assert!(!state("digest"), "a page the user has not used yet");
    assert_eq!(
        view::dashboard(&store, now()).unwrap().unsubscribe_all,
        2 + 2,
        "Digest and Acme's list mail remain"
    );
}

#[tokio::test]
async fn a_confirmed_critical_service_runs_and_a_stopped_sweep_does_not() {
    let (_dir, store) = scanned();
    store
        .write(|conn| Ok(conn.execute("UPDATE service SET is_critical = 1", [])?))
        .unwrap();
    let acme: u32 = store
        .read()
        .unwrap()
        .query_row("SELECT id FROM service", [], |r| r.get(0))
        .unwrap();
    let item = RunItem {
        target: Target::Service { id: acme },
        critical_confirmed: true,
    };

    let stopped = bulk::run(
        store.clone(),
        vec![item],
        Arc::new(AtomicBool::new(true)),
        fake_send,
        |_| {},
    )
    .await
    .unwrap();
    assert_eq!(stopped, 0);
    assert!(action::log(&store).unwrap().is_empty());

    bulk::run(
        store.clone(),
        vec![item],
        Arc::new(AtomicBool::new(false)),
        fake_send,
        |_| {},
    )
    .await
    .unwrap();
    let log = action::log(&store).unwrap();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].outcome, Outcome::Succeeded);
}
