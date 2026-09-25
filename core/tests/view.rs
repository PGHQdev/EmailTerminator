//! What S03–S06 read, over a store filled the way a scan fills it (PLAN.md M2).

use std::sync::Arc;

use et_core::crypt::DbKey;
use et_core::extract::parse_rfc3339_utc;
use et_core::ingest::imap::{Fetched, SyncTarget};
use et_core::scan::summary::Cadence;
use et_core::scan::{ImapScan, rebuild};
use et_core::store::Store;
use et_core::view::{self, Amount, ServiceStatus};

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

fn receipt(vendor: &str, month: usize, total: &str) -> Vec<u8> {
    let (year, month) = (2025 + month / 12, month % 12);
    format!(
        "From: {vendor} Billing <billing@{vendor}.test>\r\n\
         Subject: Your {vendor} receipt\r\n\
         Date: Mon, 5 {} {year} 10:00:00 +0000\r\n\
         Message-ID: <{vendor}{month}.{year}@{vendor}.test>\r\n\r\n\
         Pro plan\r\nTotal {total}\r\n",
        MONTHS[month],
    )
    .into_bytes()
}

fn newsletter(name: &str, day: u32, one_click: bool) -> Vec<u8> {
    let post = if one_click {
        "List-Unsubscribe-Post: List-Unsubscribe=One-Click\r\n"
    } else {
        ""
    };
    format!(
        "From: {name} <news@{name}.test>\r\n\
         Subject: Issue {day}\r\n\
         Date: Sun, {day} Jun 2026 08:00:00 +0000\r\n\
         Message-ID: <{name}{day}@{name}.test>\r\n\
         List-Unsubscribe: <https://{name}.test/u>, <mailto:u@{name}.test>\r\n\
         {post}\r\n\
         This week's issue.\r\n"
    )
    .into_bytes()
}

fn now() -> i64 {
    parse_rfc3339_utc("2026-07-01T00:00:00Z").unwrap()
}

/// Acme bills $10 then $12 from Jan 2025; Beta €9,99 and Gamma $5 from Jan
/// 2026; Delta once. Two newsletters, one with one-click unsubscribe.
fn scanned() -> (tempfile::TempDir, Arc<Store>) {
    let dir = tempfile::tempdir().unwrap();
    let store =
        Arc::new(Store::open(&dir.path().join("et.db"), &DbKey::generate().unwrap()).unwrap());
    store
        .write(|conn| {
            conn.execute_batch(
                "INSERT INTO source (id, kind, label, config, created_at, last_sync_at)
                 VALUES (1, 'imap', 'test', '{}', '2026-01-01T00:00:00Z', '2026-06-30T12:00:00Z');",
            )?;
            Ok(())
        })
        .unwrap();

    let mut mail = Vec::new();
    for month in 0..18 {
        mail.push(receipt(
            "acme",
            month,
            if month < 12 { "$10.00" } else { "$12.00" },
        ));
    }
    for month in 12..18 {
        mail.push(receipt("beta", month, "9,99 €"));
        mail.push(receipt("gamma", month, "$5.00"));
    }
    mail.push(receipt("delta", 14, "$40.00"));
    for day in 1..=21 {
        mail.push(newsletter("dispatch", day, true));
    }
    for day in [3, 17] {
        mail.push(newsletter("digest", day, false));
    }

    let scan = ImapScan::new(store.clone(), 1, "me@example.test");
    scan.resume("INBOX", 1).unwrap();
    let batch = mail
        .into_iter()
        .zip(1..)
        .map(|(raw, uid)| Fetched { uid, raw })
        .collect();
    scan.commit("INBOX", 1, batch).unwrap();
    rebuild(&store).unwrap();
    (dir, store)
}

fn usd(minor_units: i32) -> Amount {
    Amount {
        minor_units,
        currency: "USD".into(),
    }
}

#[test]
fn the_dashboard_adds_up_by_currency() {
    let (_dir, store) = scanned();
    let d = view::dashboard(&store, now()).unwrap();

    assert_eq!(d.sources, 1);
    assert_eq!(d.last_sync_at.as_deref(), Some("2026-06-30T12:00:00Z"));
    assert_eq!(d.subscriptions, 3, "Delta billed once");
    assert_eq!(d.newsletters, 2);
    assert_eq!(
        d.monthly_spend,
        vec![
            usd(1700),
            Amount {
                minor_units: 999,
                currency: "EUR".into()
            }
        ]
    );
    assert_eq!(d.cancel_all, d.monthly_spend);
    let names: Vec<&str> = d.services.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, vec!["acme Billing", "gamma Billing", "beta Billing"]);
    assert!(d.services[0].price_increase);

    // August 2025 to July 2026: Acme's 11, Beta's and Gamma's 6 each, and 23
    // newsletter issues. Delta is no subscription.
    assert_eq!(d.unsubscribe_all, 23);
    assert_eq!(d.emails_per_year, 11 + 6 + 6 + 23);
    assert_eq!(d.loudest[0].name, "dispatch");
    assert_eq!(d.loudest[0].last_year, 21);
    assert_eq!(d.scanned, 18 + 12 + 1 + 23);
}

#[test]
fn the_subscriptions_list_carries_every_column() {
    let (_dir, store) = scanned();
    let subs = view::subscriptions(&store, now()).unwrap();
    let acme = subs.iter().find(|s| s.name == "acme Billing").unwrap();
    assert_eq!(acme.monthly, Some(usd(1200)));
    assert_eq!(acme.cadence, Some(Cadence::Monthly));
    assert_eq!(acme.last_charge_at.as_deref(), Some("2026-06-05T10:00:00Z"));
    assert_eq!(acme.emails_per_year, 11);
    assert!(acme.price_increase);
    assert!(!acme.is_critical);
    assert_eq!(acme.status, ServiceStatus::Active);

    let gamma = subs.iter().find(|s| s.name == "gamma Billing").unwrap();
    assert!(!gamma.price_increase);
}

#[test]
fn newsletters_report_frequency_and_one_click() {
    let (_dir, store) = scanned();
    let list = view::newsletters(&store).unwrap();
    assert_eq!(list.len(), 2);
    let dispatch = &list[0];
    assert_eq!(
        (dispatch.name.as_str(), dispatch.received),
        ("dispatch", 21)
    );
    assert!(dispatch.one_click);
    // 21 issues over 20 days is about seven a week.
    assert!(
        (dispatch.per_week - 7.35).abs() < 0.01,
        "{}",
        dispatch.per_week
    );
    assert!(!list[1].one_click, "no List-Unsubscribe-Post");
}

#[test]
fn a_service_detail_has_its_histories() {
    let (_dir, store) = scanned();
    let acme = view::subscriptions(&store, now())
        .unwrap()
        .into_iter()
        .find(|s| s.name == "acme Billing")
        .unwrap();
    let d = view::service_detail(&store, acme.id, now())
        .unwrap()
        .unwrap();

    assert_eq!(d.senders, vec!["billing@acme.test"]);
    assert_eq!(d.first_seen.as_deref(), Some("2025-01-05T10:00:00Z"));
    assert_eq!(d.last_charge.as_ref().unwrap().amount, usd(1200));
    assert_eq!(d.spend.len(), 18);
    assert_eq!(d.spend[0].month, "2025-01");
    assert_eq!(d.volume.len(), 12);
    assert_eq!(d.volume[0].month, "2025-08");
    assert_eq!(d.volume[11].month, "2026-07");
    assert_eq!(d.volume[11].messages, 0);
    assert_eq!(d.emails_per_year, 11);
    assert_eq!(d.receipts_per_year, 11);
    assert_eq!(d.price_changes.len(), 1);
    assert_eq!(d.price_changes[0].at, "2026-01-05T10:00:00Z");
    assert_eq!(
        (
            d.price_changes[0].from.minor_units,
            d.price_changes[0].to.minor_units
        ),
        (1000, 1200)
    );
    assert_eq!(d.receipts.len(), 18);
    assert_eq!(d.receipts[0].date.as_deref(), Some("2026-06-05T10:00:00Z"));
    assert_eq!(d.receipts[0].kind, "charge");

    assert!(view::service_detail(&store, 9999, now()).unwrap().is_none());
}
