//! Billing mail: which of the Part 9 kinds a message is, and its amount.

use std::sync::LazyLock;

use regex::{Regex, RegexSet};

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

/// A kind and the phrases that name it, in the subject and in the body. The
/// body list is stricter: a receipt's footer says "request a refund", a
/// refund says "has been refunded".
struct Kind {
    kind: ReceiptKind,
    subject: &'static [&'static str],
    body: &'static [&'static str],
}

/// In the order that decides between kinds: a failed renewal reads "payment"
/// and "renewal", and is a failure first. `Charge` is last and weakest.
const KINDS: &[Kind] = &[
    Kind {
        kind: ReceiptKind::Refund,
        subject: &[r"\brefund"],
        body: &[
            r"\bhas been refunded",
            r"\bwas refunded",
            r"\bwe(?:'ve| have) (?:issued|processed) (?:a|your) (?:partial )?refund",
            r"\brefund(?:ed)? amount",
            r"\bamount refunded",
            r"\brefund of\b",
        ],
    },
    Kind {
        kind: ReceiptKind::PaymentFailed,
        subject: &[
            r"(?:couldn't|could not|unable to|failed to|can't|cannot) (?:process|charge|collect|complete)",
            r"payment (?:failed|declined|was declined|unsuccessful|problem|issue)",
            r"\bdeclined\b",
            r"past due",
            r"overdue",
            r"final (?:notice|reminder|attempt)",
            r"suspend",
        ],
        body: &[
            r"(?:couldn't|could not|unable to|failed to|can't|cannot) (?:process|charge|collect|complete) (?:your|the)",
            r"payment (?:for .{1,60}? )?(?:has )?failed",
            r"\bdeclined the (?:charge|payment)",
            r"(?:payment|card) was declined",
            r"past due",
        ],
    },
    Kind {
        kind: ReceiptKind::Cancellation,
        subject: &[
            r"\bcancel(?:l)?ed\b",
            r"\bcancel(?:l)?ation\b",
            r"sorry to see you go",
            r"subscription (?:has )?ended",
        ],
        body: &[
            r"subscription (?:has been|was|is) cancel(?:l)?ed",
            r"you(?:'ve| have) cancel(?:l)?ed",
            r"cancel(?:l)?ation (?:is )?confirmed",
        ],
    },
    Kind {
        kind: ReceiptKind::TrialEnding,
        subject: &[r"\btrial\b.{0,40}\b(?:ends?|ending|expir)"],
        body: &[r"\btrial\b.{0,60}\b(?:ends|will end|is ending|expires)"],
    },
    Kind {
        kind: ReceiptKind::CardExpiring,
        subject: &[
            r"\bcard\b.{0,40}\bexpir",
            r"\bexpir\w* card",
            r"update your (?:card|payment method)",
        ],
        body: &[r"\bcard\b.{0,60}\bexpires\b"],
    },
    Kind {
        kind: ReceiptKind::Downgrade,
        subject: &[r"\bdowngrad"],
        body: &[r"\bdowngrad"],
    },
    Kind {
        kind: ReceiptKind::Upgrade,
        subject: &[r"\bupgrad", r"plan (?:has )?changed", r"changed your plan"],
        body: &[
            r"\bmoved from\b.{1,80}\bto\b",
            r"\bswitched from\b.{1,80}\bto\b",
            r"\bupgraded\b",
        ],
    },
    Kind {
        kind: ReceiptKind::PriceChange,
        subject: &[
            r"\bprice\b.{0,20}\b(?:change|increase|update)",
            r"\bupdate to your\b.{0,40}\bprice",
            r"\bpricing\b",
            r"\bprices? (?:are|is) changing",
            r"\bnew price",
        ],
        body: &[
            r"\bprice (?:is|will be) (?:changing|increasing)",
            r"\bchanging the price",
            r"\bwill increase (?:to|from)",
            r"\bnew price\b",
        ],
    },
    Kind {
        kind: ReceiptKind::Charge,
        subject: &[
            r"\breceipt\b",
            r"\binvoice\b",
            r"payment (?:received|confirmation|to\b)",
            r"order (?:confirmation|receipt)",
            r"\brenewal\b",
            r"(?:has|have|been) renewed",
            r"subscription renewed",
            r"thank(?:s| you) for your (?:payment|purchase|order)",
            r"you sent a payment",
            r"billing statement",
            r"your purchase",
            r"\border\b",
        ],
        body: &[
            r"thank(?:s| you) for your (?:payment|purchase|order)",
            r"you(?:'ve| have) been charged",
            r"has been charged",
            r"payment received",
            r"you sent a payment",
            r"\bamount paid\b",
        ],
    },
];

struct Compiled {
    kind: ReceiptKind,
    subject: RegexSet,
    body: RegexSet,
}

static COMPILED: LazyLock<Vec<Compiled>> = LazyLock::new(|| {
    KINDS
        .iter()
        .map(|k| Compiled {
            kind: k.kind,
            subject: RegexSet::new(k.subject).expect("subject phrases"),
            body: RegexSet::new(k.body).expect("body phrases"),
        })
        .collect()
});

/// A plan change whose body speaks of a credit is a downgrade, whatever its
/// subject says.
static CREDIT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"credit (?:added|applied)|credited|credit to your account|\bdowngrad")
        .expect("credit")
});

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
            "total",
            "charged",
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
    "unused time",
    "was ",
    "old price",
    "current price",
];

static INVOICE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:invoice|receipt|order|transaction)(?:\s*(?:number|no\.?|id|#))?(?:\s*[:#]\s*|\t\s*)([A-Z0-9][A-Z0-9.-]{2,}[A-Z0-9])",
    )
    .expect("invoice pattern")
});

/// A labelled line naming the merchant, the most reliable place for it.
static MERCHANT_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?mi)^[ \t]*(?:seller|merchant|sold by|app|developer)[ \t]*[:\t][ \t]*(\S[^\n]*?)[ \t]*$",
    )
    .expect("merchant line")
});

static MERCHANT_PHRASE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i:payments? to|purchase from|purchased from|receipt from|refund from|order from|paid to|charge from|you sent .{1,40}? to)\s+([A-Z0-9][\w&.'-]*(?:[ ][A-Z0-9][\w&.'-]*){0,4})",
    )
    .expect("merchant phrase")
});

pub(super) struct Input<'a> {
    pub subject: &'a str,
    pub text: &'a str,
    pub from_name: Option<&'a str>,
    pub from_domain: Option<&'a str>,
    /// A PDF attachment may hold the amount the text lacks.
    pub has_pdf: bool,
}

pub(super) fn read(input: &Input<'_>) -> Option<Receipt> {
    let subject = input.subject.to_lowercase();
    let body = input.text.to_lowercase();
    let mut kind = kind(&subject, &body)?;
    if kind == ReceiptKind::Upgrade && CREDIT.is_match(&body) {
        kind = ReceiptKind::Downgrade;
    }

    let amounts = money::find_all(input.text);
    let amount = choose(kind, input.text, &amounts);
    let needs_amount = !matches!(kind, ReceiptKind::CardExpiring | ReceiptKind::Cancellation);
    // A charge needs an amount, or a subject that names the bill and a PDF
    // that holds the amount (M1 does not read PDFs).
    if kind == ReceiptKind::Charge && amount.is_none() && !(input.has_pdf && names_a_bill(&subject))
    {
        return None;
    }
    let amount = amount.filter(|_| needs_amount);

    let platform = input.from_domain.is_some_and(is_platform);
    Some(Receipt {
        kind,
        amount_minor_units: amount.map(|m| m.minor_units),
        currency: amount.map(|m| m.currency.to_owned()),
        merchant: platform.then(|| merchant(input)).flatten(),
        invoice_ref: INVOICE
            .captures(input.text)
            .or_else(|| INVOICE.captures(input.subject))
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_owned()),
    })
}

fn names_a_bill(subject: &str) -> bool {
    subject.contains("invoice") || subject.contains("receipt")
}

/// The subject decides when it names a specific kind; otherwise the body
/// does; a generic "receipt" or "invoice" is a charge.
fn kind(subject: &str, body: &str) -> Option<ReceiptKind> {
    let specific = &COMPILED[..COMPILED.len() - 1];
    specific
        .iter()
        .find(|c| c.subject.is_match(subject))
        .or_else(|| specific.iter().find(|c| c.body.is_match(body)))
        .map(|c| c.kind)
        .or_else(|| {
            COMPILED
                .last()
                .filter(|c| c.subject.is_match(subject) || c.body.is_match(body))
                .map(|c| c.kind)
        })
}

fn choose<'a>(kind: ReceiptKind, text: &str, amounts: &'a [Money]) -> Option<&'a Money> {
    if amounts.is_empty() {
        return None;
    }
    let lower = text.to_lowercase();
    let line_start = |m: &Money| lower[..m.start].rfind('\n').map_or(0, |i| i + 1);
    let line_of = |m: &Money| {
        let end = lower[m.end..].find('\n').map_or(lower.len(), |i| m.end + i);
        &lower[line_start(m)..end]
    };
    let before = |m: &Money| &lower[line_start(m)..m.start];
    let excluded = |m: &Money| {
        kind != ReceiptKind::PriceChange && NOT_TOTAL.iter().any(|n| before(m).contains(n))
    };

    // Strongest label before the amount on its own line; the later amount on
    // a tie, because totals follow line items.
    let labels = labels(kind);
    let mut best: Option<(usize, &Money)> = None;
    for m in amounts.iter().filter(|m| !excluded(m)) {
        let label_text = if kind == ReceiptKind::PriceChange {
            line_of(m)
        } else {
            before(m)
        };
        if let Some(rank) = labels.iter().position(|l| label_text.contains(l))
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
        .filter(|m| !excluded(m))
        .max_by_key(|m| m.minor_units)
        .or_else(|| amounts.last())
}

fn merchant(input: &Input<'_>) -> Option<String> {
    let domain = input.from_domain?;
    if let Some(line) = MERCHANT_LINE.captures(input.text).and_then(|c| c.get(1)) {
        return Some(line.as_str().to_owned());
    }
    // Stripe sends as the merchant: the display name is the merchant's.
    if domain == "stripe.com" || domain.ends_with(".stripe.com") {
        return input.from_name.map(str::to_owned);
    }
    MERCHANT_PHRASE
        .captures(input.text)
        .or_else(|| MERCHANT_PHRASE.captures(input.subject))
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
            has_pdf: false,
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
    fn a_plan_change_with_a_credit_is_a_downgrade() {
        let r = receipt(
            "Your plan has changed",
            "You moved from Team to Team Lite.\nNew price: $4.00 per month\nCredit added to your account: $3.00\n",
        )
        .unwrap();
        assert_eq!(r.kind, ReceiptKind::Downgrade);
        assert_eq!(r.amount_minor_units, Some(300));
    }

    #[test]
    fn a_generic_receipt_subject_yields_to_an_upgrade_body() {
        let r = receipt(
            "Your receipt",
            "You moved from Premium to Premium Plus today.\nUnused time: -$6.00\nTotal: $4.90\n",
        )
        .unwrap();
        assert_eq!(r.kind, ReceiptKind::Upgrade);
        assert_eq!(r.amount_minor_units, Some(490));
    }

    #[test]
    fn card_expiry_carries_no_amount() {
        let r = receipt(
            "Your card on file expires soon",
            "Card ending 4242 expires 04/26.\n",
        )
        .unwrap();
        assert_eq!(r.kind, ReceiptKind::CardExpiring);
        assert_eq!(r.amount_minor_units, None);
    }

    #[test]
    fn mail_that_only_sounds_like_billing_is_not_a_receipt() {
        assert!(
            receipt(
                "This week in startups",
                "How to write an invoice that gets paid.\n"
            )
            .is_none()
        );
        assert!(receipt("Your order has shipped", "It arrives Tuesday.\n").is_none());
        assert!(receipt("Your card was renewed", "Your library card is renewed.\n").is_none());
    }

    #[test]
    fn invoice_references_are_read() {
        let r = receipt("Invoice", "Invoice number: INV-2026-0042\nTotal $5.00\n").unwrap();
        assert_eq!(r.invoice_ref.as_deref(), Some("INV-2026-0042"));
        let r = receipt(
            "Receipt",
            "Order number: GPA.3014-3407-9502-92873\nTotal $5.00\n",
        )
        .unwrap();
        assert_eq!(r.invoice_ref.as_deref(), Some("GPA.3014-3407-9502-92873"));
    }

    #[test]
    fn platform_receipts_name_the_merchant() {
        let stripe = read(&Input {
            subject: "Your receipt from Acme Analytics",
            text: "Amount paid $49.00\n",
            from_name: Some("Acme Analytics"),
            from_domain: Some("stripe.com"),
            has_pdf: false,
        })
        .unwrap();
        assert_eq!(stripe.merchant.as_deref(), Some("Acme Analytics"));

        let play = read(&Input {
            subject: "Your Google Play Order Receipt",
            text: "Seller: Puzzle Den\nThanks for subscribing to Puzzle Den Premium.\nTotal: US$4.99\n",
            from_name: Some("Google Play"),
            from_domain: Some("google.com"),
            has_pdf: false,
        })
        .unwrap();
        assert_eq!(play.merchant.as_deref(), Some("Puzzle Den"));

        let paypal = read(&Input {
            subject: "Receipt for your payment",
            text: "You sent a payment of $15.00 USD to Tiny Tools Ltd.\n",
            from_name: Some("PayPal"),
            from_domain: Some("paypal.com"),
            has_pdf: false,
        })
        .unwrap();
        assert_eq!(paypal.merchant.as_deref(), Some("Tiny Tools Ltd"));
    }
}
