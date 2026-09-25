//! Billing mail: which of the Part 9 kinds a message is, and its amount.

use std::sync::LazyLock;

use regex::Regex;

use super::money::{self, Money};
use super::{Receipt, ReceiptKind};

/// Payment platforms that send receipts on a merchant's behalf. The sender
/// domain names the platform; the merchant is read from the message.
const PLATFORMS: &[&str] = &[
    "stripe.com",
    "paddle.com",
    "email.apple.com",
    "apple.com",
    "google.com",
    "paypal.com",
];

pub fn is_platform(domain: &str) -> bool {
    PLATFORMS
        .iter()
        .any(|p| domain == *p || domain.ends_with(&format!(".{p}")))
}

/// Phrases in the order that decides between kinds: a failed renewal reads
/// "payment" and "renewal", and is a failure first.
const KINDS: &[(ReceiptKind, &[&str])] = &[
    (ReceiptKind::Refund, &["refund", "refunded"]),
    (
        ReceiptKind::PaymentFailed,
        &[
            "payment failed",
            "payment was declined",
            "payment declined",
            "card was declined",
            "unable to process your payment",
            "couldn't process your payment",
            "could not process your payment",
            "unsuccessful payment",
            "payment unsuccessful",
            "past due",
            "overdue",
            "final notice",
            "action required: update your payment",
        ],
    ),
    (
        ReceiptKind::Cancellation,
        &[
            "subscription has been canceled",
            "subscription has been cancelled",
            "subscription canceled",
            "subscription cancelled",
            "cancellation confirmed",
            "cancellation confirmation",
            "you've canceled",
            "you have canceled",
            "you've cancelled",
            "you have cancelled",
            "we're sorry to see you go",
        ],
    ),
    (
        ReceiptKind::TrialEnding,
        &[
            "trial ends",
            "trial will end",
            "trial is ending",
            "trial ending",
            "trial expires",
        ],
    ),
    (
        ReceiptKind::CardExpiring,
        &[
            "card expires",
            "card is expiring",
            "card will expire",
            "expiring card",
            "card expiring",
        ],
    ),
    (
        ReceiptKind::PriceChange,
        &[
            "price change",
            "price increase",
            "pricing update",
            "price update",
            "new price",
            "prices are changing",
            "price is changing",
            "will increase to",
        ],
    ),
    (ReceiptKind::Downgrade, &["downgrade", "downgraded"]),
    (ReceiptKind::Upgrade, &["upgrade", "upgraded"]),
    (
        ReceiptKind::Charge,
        &[
            "receipt",
            "invoice",
            "payment received",
            "payment confirmation",
            "order confirmation",
            "your order",
            "renewal",
            "renewed",
            "has been charged",
            "you've been charged",
            "thanks for your payment",
            "thank you for your payment",
            "thank you for your purchase",
            "payment to",
            "you sent a payment",
            "billing statement",
        ],
    ),
];

/// Labels that name the right amount for a kind, strongest first.
fn labels(kind: ReceiptKind) -> &'static [&'static str] {
    match kind {
        ReceiptKind::Charge | ReceiptKind::Upgrade => &[
            "amount paid",
            "amount charged",
            "total paid",
            "grand total",
            "total charged",
            "order total",
            "you paid",
            "charged",
            "total",
            "amount",
        ],
        ReceiptKind::Refund => &["refund amount", "amount refunded", "refunded", "refund"],
        ReceiptKind::PaymentFailed | ReceiptKind::TrialEnding => &[
            "amount due",
            "total due",
            "balance due",
            "outstanding",
            "will be charged",
            "you'll be charged",
            "due",
            "total",
            "amount",
        ],
        ReceiptKind::PriceChange => &["new price", "increase to", "will be", " to "],
        ReceiptKind::Downgrade => &["credit", "credited", "prorated"],
        ReceiptKind::CardExpiring | ReceiptKind::Cancellation => &[],
    }
}

/// Lines that carry an amount that is never the one wanted.
const NOT_TOTAL: &[&str] = &[
    "subtotal",
    "sub-total",
    "tax",
    "vat",
    "gst",
    "discount",
    "previous balance",
    "was ",
    "old price",
    "current price",
];

static INVOICE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:invoice|receipt|order|transaction)\s*(?:number|no\.?|id|#)\s*[:#]?\s*([A-Z0-9][A-Z0-9-]{2,})",
    )
    .expect("invoice pattern")
});

static MERCHANT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i:payment to|purchase from|purchased from|receipt from|order from|subscription to|paid to|charge from|you sent .{1,40}? to)\s+([A-Z0-9][\w&.'-]*(?:[ ][A-Z0-9][\w&.'-]*){0,4})",
    )
    .expect("merchant pattern")
});

pub(super) struct Input<'a> {
    pub subject: &'a str,
    pub text: &'a str,
    pub from_name: Option<&'a str>,
    pub from_domain: Option<&'a str>,
}

pub(super) fn read(input: &Input<'_>) -> Option<Receipt> {
    let subject = input.subject.to_lowercase();
    let body = input.text.to_lowercase();
    let kind = kind(&subject, &body)?;

    let amounts = money::find_all(input.text);
    let amount = choose(kind, input.text, &amounts);
    let needs_amount = !matches!(kind, ReceiptKind::CardExpiring | ReceiptKind::Cancellation);
    // A charge named only in the body ("see your invoice") with no amount is
    // not a receipt; every other kind stands on its phrase.
    if kind == ReceiptKind::Charge && amount.is_none() && !subject_says(&subject, kind) {
        return None;
    }

    let platform = input.from_domain.is_some_and(is_platform);
    Some(Receipt {
        kind,
        amount_minor_units: amount.filter(|_| needs_amount).map(|m| m.minor_units),
        currency: amount
            .filter(|_| needs_amount)
            .map(|m| m.currency.to_owned()),
        merchant: platform.then(|| merchant(input)).flatten(),
        invoice_ref: INVOICE
            .captures(input.text)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_owned()),
    })
}

fn subject_says(subject: &str, kind: ReceiptKind) -> bool {
    KINDS
        .iter()
        .find(|(k, _)| *k == kind)
        .is_some_and(|(_, phrases)| phrases.iter().any(|p| subject.contains(p)))
}

/// The subject decides when it names a kind; the body decides otherwise, and
/// a body-only charge needs an amount (checked by the caller).
fn kind(subject: &str, body: &str) -> Option<ReceiptKind> {
    let first_in = |text: &str| {
        KINDS
            .iter()
            .find(|(_, phrases)| phrases.iter().any(|p| text.contains(p)))
            .map(|(kind, _)| *kind)
    };
    first_in(subject).or_else(|| first_in(body))
}

fn choose<'a>(kind: ReceiptKind, text: &str, amounts: &'a [Money]) -> Option<&'a Money> {
    if amounts.is_empty() {
        return None;
    }
    let lower = text.to_lowercase();
    let line_of = |m: &Money| {
        let start = lower[..m.start].rfind('\n').map_or(0, |i| i + 1);
        let end = lower[m.end..].find('\n').map_or(lower.len(), |i| m.end + i);
        &lower[start..end]
    };
    let before = |m: &Money| {
        let line = line_of(m);
        let line_start = lower[..m.start].rfind('\n').map_or(0, |i| i + 1);
        &line[..m.start - line_start]
    };

    // Strongest label on the amount's own line, the later amount on a tie.
    let labels = labels(kind);
    let mut best: Option<(usize, &Money)> = None;
    for m in amounts {
        let line = line_of(m);
        if kind != ReceiptKind::PriceChange && NOT_TOTAL.iter().any(|n| before(m).contains(n)) {
            continue;
        }
        if let Some(rank) = labels.iter().position(|l| line.contains(l))
            && best.is_none_or(|(r, _)| rank <= r)
        {
            best = Some((rank, m));
        }
    }
    if let Some((_, m)) = best {
        return Some(m);
    }
    // No label: the largest amount that is not a tax or subtotal line.
    amounts
        .iter()
        .filter(|m| !NOT_TOTAL.iter().any(|n| before(m).contains(n)))
        .max_by_key(|m| m.minor_units)
        .or_else(|| amounts.last())
}

fn merchant(input: &Input<'_>) -> Option<String> {
    let domain = input.from_domain?;
    // Stripe sends as the merchant: the display name is the merchant's.
    if domain == "stripe.com" || domain.ends_with(".stripe.com") {
        return input.from_name.map(str::to_owned);
    }
    MERCHANT
        .captures(input.text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim_end_matches(['.', ',']).to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt(subject: &str, text: &str) -> Option<Receipt> {
        read(&Input {
            subject,
            text,
            from_name: None,
            from_domain: Some("vendor.test"),
        })
    }

    #[test]
    fn a_receipt_takes_the_total_over_subtotal_and_tax() {
        let r = receipt(
            "Your receipt from Vendor",
            "Pro plan  $10.00\nSubtotal $10.00\nTax $0.80\nTotal $10.80\n",
        )
        .unwrap();
        assert_eq!(r.kind, ReceiptKind::Charge);
        assert_eq!(r.amount_minor_units, Some(1080));
        assert_eq!(r.currency.as_deref(), Some("USD"));
    }

    #[test]
    fn a_failed_renewal_is_a_failure() {
        let r = receipt("Payment failed for your renewal", "Amount due: €19,99\n").unwrap();
        assert_eq!(r.kind, ReceiptKind::PaymentFailed);
        assert_eq!(r.amount_minor_units, Some(1999));
    }

    #[test]
    fn a_price_change_reports_the_new_price() {
        let r = receipt(
            "An update to your plan's price",
            "Your price is changing from $10.00 to $12.00 on March 1.\n",
        )
        .unwrap();
        assert_eq!(r.kind, ReceiptKind::PriceChange);
        assert_eq!(r.amount_minor_units, Some(1200));
    }

    #[test]
    fn card_expiry_carries_no_amount() {
        let r = receipt(
            "Your card expires soon",
            "Card ending 4242 expires 04/26.\n",
        )
        .unwrap();
        assert_eq!(r.kind, ReceiptKind::CardExpiring);
        assert_eq!(r.amount_minor_units, None);
    }

    #[test]
    fn a_newsletter_mentioning_an_invoice_is_not_a_receipt() {
        assert!(
            receipt(
                "This week in startups",
                "How to write an invoice that gets paid.\n"
            )
            .is_none()
        );
    }

    #[test]
    fn invoice_references_are_read() {
        let r = receipt("Invoice", "Invoice number: INV-2026-0042\nTotal $5.00\n").unwrap();
        assert_eq!(r.invoice_ref.as_deref(), Some("INV-2026-0042"));
    }

    #[test]
    fn platform_receipts_name_the_merchant() {
        let stripe = read(&Input {
            subject: "Your receipt from Acme Analytics",
            text: "Amount paid $49.00\n",
            from_name: Some("Acme Analytics"),
            from_domain: Some("stripe.com"),
        })
        .unwrap();
        assert_eq!(stripe.merchant.as_deref(), Some("Acme Analytics"));

        let paypal = read(&Input {
            subject: "Receipt for your payment",
            text: "You sent a payment of $15.00 USD to Tiny Tools Ltd.\n",
            from_name: Some("PayPal"),
            from_domain: Some("paypal.com"),
        })
        .unwrap();
        assert_eq!(paypal.merchant.as_deref(), Some("Tiny Tools Ltd"));
    }
}
