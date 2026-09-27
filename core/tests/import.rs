//! M5: mbox and Maildir files import through the same store path as IMAP,
//! and a stopped import keeps what it read and resumes (DESIGN.md S16).

use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use et_core::crypt::DbKey;
use et_core::ingest::{maildir, mbox};
use et_core::scan::{ImportError, MailScan, rebuild};
use et_core::store::Store;

fn store(kind: &str) -> (tempfile::TempDir, Arc<Store>) {
    let dir = tempfile::tempdir().unwrap();
    let store =
        Arc::new(Store::open(&dir.path().join("et.db"), &DbKey::generate().unwrap()).unwrap());
    let kind = kind.to_owned();
    store
        .write(move |conn| {
            conn.execute(
                "INSERT INTO source (id, kind, label, config, created_at)
                 VALUES (1, ?1, 'file', '{}', '2026-01-01T00:00:00Z')",
                [kind],
            )?;
            Ok(())
        })
        .unwrap();
    (dir, store)
}

fn receipt(month: u32) -> String {
    format!(
        "From: Netflix <info@account.netflix.com>\n\
         Subject: Your receipt\n\
         Date: Mon, 5 {} 2026 10:00:00 +0000\n\
         Message-ID: <{month}@netflix.test>\n\n\
         Monthly plan\nTotal $15.49\n",
        ["Jan", "Feb", "Mar", "Apr", "May", "Jun"][month as usize],
    )
}

fn mbox_of(messages: &[String]) -> Vec<u8> {
    messages
        .iter()
        .map(|m| format!("From x Mon Jan  5 10:00:00 2026\n{m}\n"))
        .collect::<String>()
        .into_bytes()
}

fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    e.write_all(bytes).unwrap();
    e.finish().unwrap()
}

fn import_mbox(scan: &MailScan, path: &std::path::Path) -> Result<(), ImportError> {
    let file = mbox::open(path).unwrap();
    scan.import(
        "mbox",
        file.map(|m| {
            m.map_err(|e| match e {
                mbox::MboxError::Stopped { message, reason } => {
                    ImportError::Stopped { message, reason }
                }
                other => ImportError::Stopped {
                    message: 0,
                    reason: other.to_string(),
                },
            })
        }),
        &AtomicBool::new(false),
        |_| {},
    )
}

#[test]
fn an_mbox_import_finds_the_subscription() {
    let (_dir, store) = store("mbox");
    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.write_all(&mbox_of(&(0..4).map(receipt).collect::<Vec<_>>()))
        .unwrap();
    let scan = MailScan::new(store.clone(), 1, "");
    import_mbox(&scan, file.path()).unwrap();
    assert_eq!(scan.counts().scanned, 4);
    let totals = rebuild(&store).unwrap();
    assert_eq!(totals.subscriptions, 1);
}

#[test]
fn a_truncated_import_keeps_its_messages_and_a_retry_adds_only_new_ones() {
    let (_dir, store) = store("mbox");
    let messages: Vec<String> = (0..600)
        .map(|n| {
            format!(
                "Subject: {n}\nMessage-ID: <{n}@x.test>\n\n{}\n",
                "y".repeat(400)
            )
        })
        .collect();
    let zipped = gzip(&mbox_of(&messages));
    let mut cut = tempfile::NamedTempFile::new().unwrap();
    cut.write_all(&zipped[..zipped.len() * 2 / 3]).unwrap();

    let scan = MailScan::new(store.clone(), 1, "");
    let stopped = import_mbox(&scan, cut.path()).unwrap_err();
    let ImportError::Stopped { message, .. } = stopped else {
        panic!("expected a stop, got {stopped:?}");
    };
    let kept = scan.counts().scanned;
    assert_eq!(kept, message - 1);
    let count = |store: &Store| -> u64 {
        store
            .read()
            .unwrap()
            .query_row("SELECT count(*) FROM message", [], |r| r.get::<_, i64>(0))
            .unwrap() as u64
    };
    assert_eq!(count(&store), kept);

    // The whole file arrives; the retry stores only what the cut lost.
    let mut whole = tempfile::NamedTempFile::new().unwrap();
    whole.write_all(&zipped).unwrap();
    let retry = MailScan::new(store.clone(), 1, "");
    import_mbox(&retry, whole.path()).unwrap();
    assert_eq!(retry.counts().scanned, 600 - kept);
    assert_eq!(count(&store), 600);
}

#[test]
fn a_maildir_import_skips_sent_mail() {
    let (_dir, store) = store("maildir");
    let root = tempfile::tempdir().unwrap();
    for (n, folder) in ["cur", "cur", ".Sent/cur", ".Receipts/new"]
        .iter()
        .enumerate()
    {
        let dir = root.path().join(folder);
        std::fs::create_dir_all(&dir).unwrap();
        // Windows forbids `:` in a file name; its Maildir tools write `!`.
        let info = if cfg!(windows) { "!" } else { ":" };
        std::fs::write(dir.join(format!("{n}.x.host{info}2,S")), receipt(n as u32)).unwrap();
    }
    let scan = MailScan::new(store.clone(), 1, "");
    let files = maildir::open(root.path()).unwrap();
    assert_eq!(files.total(), 3);
    scan.import("maildir", files.map(Ok), &AtomicBool::new(false), |_| {})
        .unwrap();
    assert_eq!(scan.counts().scanned, 3);
}
