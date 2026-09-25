//! The performance test (PLAN.md 2.9): 1 GB of generated messages through
//! extraction and the encrypted store, with a wall-clock and a peak-memory
//! ceiling. Run in release: `cargo test --release -p et-core --test perf -- --ignored`.

use std::sync::Arc;
use std::time::Instant;

use et_core::crypt::DbKey;
use et_core::ingest::imap::{Fetched, SyncTarget};
use et_core::scan::{ImapScan, rebuild};
use et_core::store::Store;

const TARGET_BYTES: usize = 1 << 30;
/// Scaled from PLAN.md 1.1 (10 GB in under ten minutes) with room for a
/// shared CI runner: 1 GB must finish in 90 s.
const CEILING_SECS: u64 = 90;
/// Flat memory: the scan holds one batch, never the mailbox.
const CEILING_RSS_MB: u64 = 512;

fn message(n: usize) -> Vec<u8> {
    let vendor = n % 997;
    let body = match n % 4 {
        0 => format!("Pro plan\r\nSubtotal $1{vendor}.00\r\nTax $0.00\r\nTotal $1{vendor}.00\r\n"),
        1 => "This week: three stories worth your time.\r\n".repeat(40),
        2 => "<html><body><p>Hello</p><p>Order total: €9,99</p></body></html>\r\n".to_owned(),
        _ => "Meeting notes attached.\r\n".repeat(20),
    };
    let (subject, list) = match n % 4 {
        0 => ("Your receipt", ""),
        1 => ("Issue", "List-Unsubscribe: <https://news.test/u>\r\n"),
        2 => ("Your order confirmation", ""),
        _ => ("Notes", ""),
    };
    let padding = "X".repeat(6_000);
    format!(
        "From: Vendor {vendor} <billing@vendor{vendor}.test>\r\n\
         Subject: {subject} {n}\r\n\
         Date: Mon, {} Jan 2026 10:00:00 +0000\r\n\
         Message-ID: <{n}@vendor{vendor}.test>\r\n\
         {list}\
         Content-Type: multipart/mixed; boundary=\"b\"\r\n\r\n\
         --b\r\nContent-Type: text/plain\r\n\r\n{body}\r\n\
         --b\r\nContent-Type: application/octet-stream\r\nContent-Transfer-Encoding: base64\r\n\r\n{padding}\r\n\
         --b--\r\n",
        n % 28 + 1,
    )
    .into_bytes()
}

fn peak_rss_mb() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|l| l.starts_with("VmHWM:"))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb / 1024)
}

#[test]
#[ignore = "slow; CI runs it in release"]
fn one_gigabyte_scans_inside_the_ceiling() {
    let dir = tempfile::tempdir().unwrap();
    let store =
        Arc::new(Store::open(&dir.path().join("et.db"), &DbKey::generate().unwrap()).unwrap());
    store
        .write(|conn| {
            conn.execute_batch(
                "INSERT INTO source (id, kind, label, config, created_at)
                     VALUES (1, 'imap', 'perf', '{}', '2026-01-01T00:00:00Z');",
            )?;
            Ok(())
        })
        .unwrap();
    let scan = ImapScan::new(store.clone(), 1, "me@example.test");
    scan.resume("INBOX", 1).unwrap();

    let started = Instant::now();
    let (mut bytes, mut n, mut uid) = (0, 0, 0);
    while bytes < TARGET_BYTES {
        let batch: Vec<Fetched> = (0..200)
            .map(|_| {
                n += 1;
                uid += 1;
                let raw = message(n);
                bytes += raw.len();
                Fetched { uid, raw }
            })
            .collect();
        scan.commit("INBOX", 1, batch).unwrap();
    }
    let totals = rebuild(&store).unwrap();
    let elapsed = started.elapsed();

    eprintln!(
        "{n} messages, {} MB in {:.1} s ({:.0} msg/s), peak RSS {:?} MB, {} services",
        bytes >> 20,
        elapsed.as_secs_f64(),
        n as f64 / elapsed.as_secs_f64(),
        peak_rss_mb(),
        totals.subscriptions,
    );
    assert!(elapsed.as_secs() < CEILING_SECS, "took {elapsed:?}");
    if let Some(mb) = peak_rss_mb() {
        assert!(mb < CEILING_RSS_MB, "peak RSS {mb} MB");
    }
    assert_eq!(scan.counts().scanned as usize, n);
}
