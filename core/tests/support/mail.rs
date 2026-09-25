//! Synthetic mail and a store filled the way a scan fills it.

use std::sync::Arc;

use et_core::ingest::imap::{Fetched, SyncTarget};
use et_core::scan::{ImapScan, rebuild};
use et_core::store::Store;

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

pub fn receipt(vendor: &str, month: usize, total: &str) -> Vec<u8> {
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

pub fn newsletter(name: &str, day: u32, one_click: bool) -> Vec<u8> {
    // RFC 8058 wants the POST header and a passing signature over both
    // list headers; the receiving server's verdict comes first.
    let signed = if one_click {
        format!(
            "Authentication-Results: mx.example.test; dkim=pass header.d={name}.test\r\n\
             DKIM-Signature: v=1; a=rsa-sha256; d={name}.test; s=k1;\r\n \
             h=from:subject:list-unsubscribe:list-unsubscribe-post; bh=x; b=y\r\n"
        )
    } else {
        String::new()
    };
    let post = if one_click {
        "List-Unsubscribe-Post: List-Unsubscribe=One-Click\r\n"
    } else {
        ""
    };
    format!(
        "{signed}From: {name} <news@{name}.test>\r\n\
         Subject: Issue {day}\r\n\
         Date: Sun, {day} Jun 2026 08:00:00 +0000\r\n\
         Message-ID: <{name}{day}@{name}.test>\r\n\
         List-Unsubscribe: <https://{name}.test/u>, <mailto:u@{name}.test>\r\n\
         {post}\r\n\
         This week's issue.\r\n"
    )
    .into_bytes()
}

/// Stores `mail` as source 1's INBOX, UIDs from 1, then rebuilds.
pub fn scan(store: &Arc<Store>, mail: Vec<Vec<u8>>) {
    let scan = ImapScan::new(store.clone(), 1, "me@example.test");
    scan.resume("INBOX", 1).unwrap();
    let batch = mail
        .into_iter()
        .zip(1..)
        .map(|(raw, uid)| Fetched { uid, raw })
        .collect();
    scan.commit("INBOX", 1, batch).unwrap();
    rebuild(store).unwrap();
}
