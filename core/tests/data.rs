//! M4: the bundled `data/` names services, flags critical ones, and carries
//! the playbook S08 shows (PLAN.md 2.6).

mod support {
    // This file builds its own mail and uses only `scan`.
    #[allow(dead_code)]
    pub mod mail;
}

use std::sync::Arc;

use et_core::action::unsubscribe::{self, Route};
use et_core::action::{self, Kind, Outcome, playbook};
use et_core::crypt::DbKey;
use et_core::store::Store;
use et_core::view::{self, ServiceStatus};
use support::mail::scan;

fn billed(from: &str, month: u32, total: &str) -> Vec<u8> {
    format!(
        "From: {from}\r\n\
         Subject: Your receipt\r\n\
         Date: Mon, 5 {} 2026 10:00:00 +0000\r\n\
         Message-ID: <{month}.{}>\r\n\r\n\
         Monthly plan\r\nTotal {total}\r\n",
        ["Jan", "Feb", "Mar", "Apr", "May", "Jun"][month as usize],
        from.replace(['<', '>', ' '], ""),
    )
    .into_bytes()
}

fn plain(from: &str, day: u32) -> Vec<u8> {
    format!(
        "From: {from}\r\n\
         Subject: News {day}\r\n\
         Date: Sun, {day} Jun 2026 08:00:00 +0000\r\n\
         Message-ID: <plain{day}.{}>\r\n\r\n\
         Hello.\r\n",
        from.replace(['<', '>', ' '], ""),
    )
    .into_bytes()
}

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
    for month in 0..4 {
        mail.push(billed(
            "Netflix <info@account.netflix.com>",
            month,
            "$15.49",
        ));
        mail.push(billed(
            "Amazon.com <auto-confirm@amazon.com>",
            month,
            "$14.99",
        ));
        mail.push(billed(
            "Amazon Web Services <no-reply-aws@amazon.com>",
            month,
            "$3.20",
        ));
    }
    for day in 1..=3 {
        mail.push(plain("Netflix <info@members.netflix.com>", day));
    }
    scan(&store, mail);
    (dir, store)
}

fn service(store: &Store, name: &str) -> (i64, Option<String>, bool) {
    store
        .read()
        .unwrap()
        .query_row(
            "SELECT id, data_key, is_critical FROM service WHERE name = ?1",
            [name],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn entries_name_services_and_flag_critical_ones() {
    let (_dir, store) = scanned();

    assert_eq!(service(&store, "Netflix").1.as_deref(), Some("netflix"));
    assert!(!service(&store, "Netflix").2);
    assert_eq!(
        service(&store, "Amazon Prime").1.as_deref(),
        Some("amazonprime")
    );
    assert!(
        !service(&store, "Amazon Prime").2,
        "the shop is not critical"
    );

    // AWS mails from amazon.com, yet its sender pattern keeps it apart.
    let (aws, data_key, critical) = service(&store, "Amazon Web Services");
    assert_eq!(data_key, None);
    assert!(critical);

    let senders: Vec<(String, Option<String>)> = store
        .read()
        .unwrap()
        .prepare(
            "SELECT s.address, v.name FROM sender s LEFT JOIN service v ON v.id = s.service_id
             ORDER BY s.address",
        )
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        senders,
        [
            ("auto-confirm@amazon.com", "Amazon Prime"),
            ("info@account.netflix.com", "Netflix"),
            ("info@members.netflix.com", "Netflix"),
            ("no-reply-aws@amazon.com", "Amazon Web Services"),
        ]
        .map(|(a, n)| (a.to_owned(), Some(n.to_owned())))
    );

    let detail = view::service_detail(&store, aws as u32, 1_782_864_000)
        .unwrap()
        .unwrap();
    assert!(detail.is_critical);
    assert_eq!(detail.playbook, None);
}

#[test]
fn a_playbook_is_shown_and_its_finish_is_logged() {
    let (_dir, store) = scanned();
    let (netflix, _, _) = service(&store, "Netflix");

    let view = view::playbook(&store, netflix as u32, 1_782_864_000)
        .unwrap()
        .unwrap();
    assert_eq!(view.source, "https://help.netflix.com/en/node/407");
    assert!(!view.steps.is_empty());
    assert_eq!(
        view.improve,
        "https://github.com/PGHQdev/EmailTerminator/edit/main/data/services/netflix.toml"
    );
    assert_eq!(
        view.service.playbook.as_ref().map(|p| p.steps as usize),
        Some(view.steps.len())
    );

    let row = store
        .write(move |conn| playbook::finish(conn, netflix, "2026-09-26T10:00:00Z"))
        .unwrap()
        .unwrap();
    let entry = store
        .read()
        .map(|conn| action::get(&conn, row as u32).unwrap().unwrap())
        .unwrap();
    assert_eq!(entry.kind, Kind::Playbook);
    assert_eq!(entry.outcome, Outcome::Succeeded);
    assert_eq!(
        entry.request.as_deref(),
        Some("https://help.netflix.com/en/node/407")
    );
    let detail = view::service_detail(&store, netflix as u32, 1_782_864_000)
        .unwrap()
        .unwrap();
    assert_eq!(detail.status, ServiceStatus::Canceled);
}

#[test]
fn a_sender_without_a_header_goes_to_the_vendors_preferences_page() {
    let (_dir, store) = scanned();
    let sender: i64 = store
        .read()
        .unwrap()
        .query_row(
            "SELECT id FROM sender WHERE address = 'info@members.netflix.com'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let attempt = store
        .read()
        .map(|conn| unsubscribe::plan(&conn, sender).unwrap().unwrap())
        .unwrap();
    assert_eq!(
        attempt.route,
        Route::Page {
            url: "https://www.netflix.com/notificationsettings/email".into()
        }
    );
}
