//! A whole scan: stub IMAP server → sync → store → rebuild (PLAN.md M1).

mod support {
    pub mod imap_stub;
}

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use et_core::crypt::DbKey;
use et_core::ingest::imap::{self, Account, Options, Security};
use et_core::scan::{ImapScan, rebuild};
use et_core::store::Store;
use support::imap_stub::{self, Provider};

const PASSWORD: &str = "app-password";
const OWNER: &str = "me@example.test";

fn receipt(month: u32, amount: &str) -> Vec<u8> {
    format!(
        "From: Acme Billing <billing@acme.test>\r\n\
         To: {OWNER}\r\n\
         Subject: Your Acme receipt\r\n\
         Date: Mon, 5 {} 2026 10:00:00 +0000\r\n\
         Message-ID: <r{month}@acme.test>\r\n\r\n\
         Pro plan\r\nSubtotal {amount}\r\nTax $0.00\r\nTotal {amount}\r\n",
        ["Jan", "Feb", "Mar", "Apr", "May", "Jun"][month as usize],
    )
    .into_bytes()
}

fn newsletter(n: u32) -> Vec<u8> {
    format!(
        "From: The Dispatch <news@dispatch.test>\r\n\
         Subject: Issue {n}\r\n\
         Date: Sun, {n} Mar 2026 08:00:00 +0000\r\n\
         Message-ID: <issue{n}@dispatch.test>\r\n\
         List-Unsubscribe: <https://dispatch.test/u>\r\n\r\n\
         Read this week's issue.\r\n"
    )
    .into_bytes()
}

fn sent_by_owner() -> Vec<u8> {
    format!(
        "From: Me <{OWNER}>\r\nSubject: hi\r\nDate: Mon, 2 Mar 2026 09:00:00 +0000\r\n\r\nhello\r\n"
    )
    .into_bytes()
}

#[tokio::test]
async fn a_gmail_scan_finds_the_subscription_and_the_newsletter() {
    let stub = imap_stub::start(Provider::Gmail, PASSWORD).await;
    {
        let mut st = stub.state.lock().unwrap();
        for month in 0..6 {
            let amount = if month < 3 { "$10.00" } else { "$12.00" };
            st.deliver("[Gmail]/All Mail", receipt(month, amount));
        }
        for n in 1..=4 {
            st.deliver("[Gmail]/All Mail", newsletter(n));
        }
        st.deliver("[Gmail]/All Mail", sent_by_owner());
    }

    let dir = tempfile::tempdir().unwrap();
    let store =
        Arc::new(Store::open(&dir.path().join("et.db"), &DbKey::generate().unwrap()).unwrap());
    store
        .write(|conn| {
            conn.execute(
                "INSERT INTO source (id, kind, label, config, created_at)
                 VALUES (1, 'imap', 'Gmail', '{}', '2026-09-26T00:00:00Z')",
                [],
            )?;
            Ok(())
        })
        .unwrap();

    let scan = Arc::new(ImapScan::new(store.clone(), 1, OWNER));
    let account = Account {
        host: "127.0.0.1".into(),
        port: stub.port,
        username: OWNER.into(),
        security: Security::PlainLoopback,
    };
    imap::sync(
        &account,
        PASSWORD,
        scan.clone(),
        Arc::new(AtomicBool::new(false)),
        Options::default(),
        |_| {},
    )
    .await
    .unwrap();

    let live = scan.counts();
    assert_eq!(live.scanned, 10, "the owner's own message is skipped");
    assert_eq!(
        (live.senders, live.subscriptions, live.newsletters),
        (2, 1, 1)
    );

    let totals = rebuild(&store).unwrap();
    assert_eq!(
        (totals.senders, totals.subscriptions, totals.newsletters),
        (2, 1, 1)
    );

    let read = store.read().unwrap();
    let (name, cadence, monthly, currency): (String, String, i64, String) = read
        .query_row(
            "SELECT name, cadence, monthly_minor_units, currency FROM service",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(
        (name.as_str(), cadence.as_str(), monthly, currency.as_str()),
        ("Acme Billing", "monthly", 1200, "USD")
    );

    let spend: i64 = read
        .query_row(
            "SELECT sum(spend_minor_units) FROM aggregate WHERE subject_kind = 'service'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(spend, 3 * 1000 + 3 * 1200);

    // A second scan changes nothing and duplicates nothing.
    imap::sync(
        &account,
        PASSWORD,
        scan.clone(),
        Arc::new(AtomicBool::new(false)),
        Options::default(),
        |_| {},
    )
    .await
    .unwrap();
    rebuild(&store).unwrap();
    let messages: i64 = read
        .query_row("SELECT count(*) FROM message", [], |r| r.get(0))
        .unwrap();
    assert_eq!(messages, 10);
}
