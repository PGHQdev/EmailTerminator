//! What the deterministic layer reads out of one message (PLAN.md 2.7, stage
//! one). The corpus goldens in `core/tests/corpus/` are serialised
//! `Extraction` values, so a change here is a change to every golden.

use serde::{Deserialize, Serialize};

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
    /// True only for exactly `List-Unsubscribe=One-Click`. Whether the
    /// message qualifies for RFC 8058 is decided at M3, not here.
    pub list_unsubscribe_post: bool,
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
    let _ = raw;
    todo!("M1: parse, headers, receipts")
}

#[cfg(test)]
mod tests {
    use super::*;

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
