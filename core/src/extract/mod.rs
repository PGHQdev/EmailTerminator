//! What the deterministic layer reads out of one message (PLAN.md 2.7, stage
//! one). The corpus goldens in `core/tests/corpus/` are serialised
//! `Extraction` values, so a change here is a change to every golden.

mod headers;
mod html;
mod money;
mod receipt;
pub mod unsubscribe;
mod words;

use mail_parser::{MessageParser, MimeHeaders, PartType};
use serde::{Deserialize, Serialize};

pub use headers::{parse_rfc3339_utc, rfc3339_utc};
pub use receipt::is_platform;

/// The facts taken from one raw message. No body text survives past this.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Extraction {
    /// Lowercased `From` address, IDN hosts left as sent.
    pub from_address: Option<String>,
    /// `From` display name after RFC 2047 decoding.
    pub from_name: Option<String>,
    pub subject: Option<String>,
    /// RFC 3339 in UTC, or `None` when `Date` is missing or unreadable.
    pub date: Option<String>,
    /// Without angle brackets.
    pub message_id: Option<String>,
    /// The `List-Unsubscribe` header unfolded, or `None`.
    pub list_unsubscribe: Option<String>,
    /// True only for exactly `List-Unsubscribe=One-Click`.
    pub list_unsubscribe_post: bool,
    /// Every RFC 8058 condition holds, so an unsubscribe can be one POST
    /// (`unsubscribe::one_click`).
    pub one_click: bool,
    /// `List-Id` without angle brackets.
    pub list_id: Option<String>,
    /// Mailing-list or bulk mail: any `List-Id`, `List-Unsubscribe`, or
    /// `Precedence: bulk | list`, unless the message is a receipt.
    pub is_list: bool,
    /// Every DKIM `d=` domain, lowercased, in header order.
    pub dkim_domains: Vec<String>,
    pub receipt: Option<Receipt>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub kind: ReceiptKind,
    /// Integer minor units of `currency`, never a float (PLAN.md 2.5). The
    /// meaning follows `kind`: charged, refunded, due, new price, or credit.
    pub amount_minor_units: Option<i64>,
    /// ISO 4217.
    pub currency: Option<String>,
    /// The merchant named in the message, for receipts a payment platform
    /// relays (Stripe, Paddle, Apple, Google Play, PayPal).
    pub merchant: Option<String>,
    pub invoice_ref: Option<String>,
}

/// The twelve message kinds of PLAN.md Part 9, folded to what spend needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptKind {
    /// First charge, order confirmation, or recurring renewal.
    Charge,
    TrialEnding,
    PaymentFailed,
    CardExpiring,
    Upgrade,
    Downgrade,
    PriceChange,
    Refund,
    Cancellation,
}

impl ReceiptKind {
    /// Kinds whose amount is money that left the account.
    pub fn is_spend(self) -> bool {
        matches!(self, Self::Charge | Self::Upgrade)
    }
}

/// Reads one raw message. Never panics; bytes that do not parse produce an
/// extraction with every field empty.
pub fn extract(raw: &[u8]) -> Extraction {
    let Some(msg) = MessageParser::default().parse(raw) else {
        return Extraction::empty();
    };
    let (from_address, from_name) = headers::from(&msg);
    let subject = headers::subject(&msg);
    let list_unsubscribe = headers::raw(&msg, "List-Unsubscribe");
    let list_id = headers::list_id(&msg);
    let listish = list_unsubscribe.is_some() || list_id.is_some() || headers::is_bulk(&msg);

    let from_domain = from_address
        .as_deref()
        .and_then(|a| a.rsplit_once('@'))
        .map(|(_, d)| d);
    let receipt = receipt::read(&receipt::Input {
        subject: subject.as_deref().unwrap_or(""),
        text: &body_text(&msg),
        from_name: from_name.as_deref(),
        from_domain,
        has_pdf: msg.attachments().any(|part| {
            part.content_type().is_some_and(|ct| {
                ct.ctype().eq_ignore_ascii_case("application")
                    && ct.subtype().is_some_and(|s| s.eq_ignore_ascii_case("pdf"))
            }) || part
                .attachment_name()
                .is_some_and(|n| n.to_ascii_lowercase().ends_with(".pdf"))
        }),
    });

    let list_unsubscribe_post = headers::raw(&msg, "List-Unsubscribe-Post")
        .is_some_and(|v| v == "List-Unsubscribe=One-Click");
    Extraction {
        one_click: unsubscribe::one_click(&msg, list_unsubscribe.as_deref(), list_unsubscribe_post),
        from_address,
        from_name,
        subject,
        date: headers::date(&msg),
        message_id: msg.message_id().map(|id| id.trim().to_owned()),
        list_unsubscribe_post,
        list_unsubscribe,
        list_id,
        is_list: listish && receipt.is_none(),
        dkim_domains: headers::dkim_domains(&msg),
        receipt,
    }
}

impl Extraction {
    fn empty() -> Self {
        Self {
            from_address: None,
            from_name: None,
            subject: None,
            date: None,
            message_id: None,
            list_unsubscribe: None,
            list_unsubscribe_post: false,
            one_click: false,
            list_id: None,
            is_list: false,
            dkim_domains: Vec::new(),
            receipt: None,
        }
    }
}

/// What an evidence link shows of an original: the headers the app read,
/// then the text. Built on demand from a re-read message and never stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub headers: Vec<Field>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct Field {
    pub name: String,
    pub value: String,
}

/// The preview shows this much text; the rest of a long message is cut.
const PREVIEW_CHARS: usize = 20_000;

pub fn preview(raw: &[u8]) -> Preview {
    let Some(msg) = MessageParser::default().parse(raw) else {
        return Preview {
            headers: Vec::new(),
            text: String::new(),
        };
    };
    let headers = [
        "From",
        "To",
        "Date",
        "Subject",
        "List-Unsubscribe",
        "List-Unsubscribe-Post",
        "Authentication-Results",
    ]
    .into_iter()
    .filter_map(|name| {
        let value = headers::raw(&msg, name)?;
        Some(Field {
            name: name.to_owned(),
            value: words::decode(&value),
        })
    })
    .collect();
    let text = body_text(&msg);
    let text = match text.char_indices().nth(PREVIEW_CHARS) {
        Some((cut, _)) => format!("{}…", &text[..cut]),
        None => text,
    };
    Preview {
        headers,
        text: text.trim().to_owned(),
    }
}

/// Every text part, then every HTML part read as text, so an amount that
/// only the HTML carries is still found.
fn body_text(msg: &mail_parser::Message<'_>) -> String {
    let mut text = String::new();
    for part in msg.text_bodies().chain(msg.html_bodies()) {
        let chunk = match &part.body {
            PartType::Text(t) => t.to_string(),
            PartType::Html(h) => html::to_text(h),
            _ => continue,
        };
        if !text.contains(chunk.trim()) {
            text.push_str(&chunk);
            text.push('\n');
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_newsletter_reads_as_a_list() {
        let raw = b"From: =?utf-8?Q?The_Dispatch?= <News@Dispatch.test>\r\n\
Subject: Issue 12\r\n\
Date: Sun, 1 Mar 2026 10:30:00 +0100\r\n\
Message-ID: <issue-12@dispatch.test>\r\n\
List-Unsubscribe: <https://dispatch.test/u/1>,\r\n <mailto:u@dispatch.test>\r\n\
List-Unsubscribe-Post: List-Unsubscribe=One-Click\r\n\
List-Id: The Dispatch <dispatch.test>\r\n\
DKIM-Signature: v=1; a=rsa-sha256; d=Dispatch.test; s=k1;\r\n h=from:list-unsubscribe:list-unsubscribe-post; b=abc\r\n\
\r\n\
Hello.\r\n";
        let e = extract(raw);
        assert_eq!(e.from_address.as_deref(), Some("news@dispatch.test"));
        assert_eq!(e.from_name.as_deref(), Some("The Dispatch"));
        assert_eq!(e.date.as_deref(), Some("2026-03-01T09:30:00Z"));
        assert_eq!(e.message_id.as_deref(), Some("issue-12@dispatch.test"));
        assert_eq!(
            e.list_unsubscribe.as_deref(),
            Some("<https://dispatch.test/u/1>, <mailto:u@dispatch.test>")
        );
        assert!(e.list_unsubscribe_post);
        assert!(
            !e.one_click,
            "no Authentication-Results reports the signature"
        );
        assert_eq!(e.list_id.as_deref(), Some("dispatch.test"));
        assert_eq!(e.dkim_domains, vec!["dispatch.test"]);
        assert!(e.is_list);
        assert!(e.receipt.is_none());
    }

    #[test]
    fn a_preview_decodes_headers_and_keeps_the_text() {
        let p = preview(
            b"From: =?utf-8?Q?Caf=C3=A9?= <news@cafe.test>\r\nSubject: Menu\r\n\r\nSoup today.\r\n",
        );
        assert_eq!(
            p.headers,
            vec![
                Field {
                    name: "From".into(),
                    value: "Caf\u{e9} <news@cafe.test>".into()
                },
                Field {
                    name: "Subject".into(),
                    value: "Menu".into()
                },
            ]
        );
        assert_eq!(p.text, "Soup today.");
    }

    #[test]
    fn bytes_that_do_not_parse_give_an_empty_extraction() {
        assert_eq!(extract(b""), Extraction::empty());
    }

    #[test]
    fn the_golden_format_round_trips() {
        let extraction = Extraction {
            from_address: Some("billing@acme.test".into()),
            from_name: Some("Acme".into()),
            subject: Some("Your receipt".into()),
            date: Some("2026-01-01T09:00:00Z".into()),
            message_id: Some("abc@acme.test".into()),
            list_unsubscribe: None,
            list_unsubscribe_post: false,
            one_click: false,
            list_id: None,
            is_list: false,
            dkim_domains: vec!["acme.test".into()],
            receipt: Some(Receipt {
                kind: ReceiptKind::Charge,
                amount_minor_units: Some(1200),
                currency: Some("USD".into()),
                merchant: None,
                invoice_ref: Some("INV-1".into()),
            }),
        };
        let json = serde_json::to_string(&extraction).unwrap();
        assert!(json.contains("\"kind\":\"charge\""));
        assert_eq!(
            serde_json::from_str::<Extraction>(&json).unwrap(),
            extraction
        );
    }
}
