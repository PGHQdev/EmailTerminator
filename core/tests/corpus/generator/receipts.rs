//! Part 9, "Receipt and billing shapes": ten vendor templates, each sending
//! the twelve message kinds (refund split into full and partial), plus the
//! transport variations that only make sense on a receipt.

use et_core::extract::{Receipt, ReceiptKind};

use super::{
    Corpus, Cte, Dt, Eol, Msg, Part, RELAY_PLATFORM_DOMAINS, html, leaf, multipart, text, tiny_pdf,
};

#[derive(Clone, Copy)]
pub enum Money {
    /// `$12.00`
    Dollar,
    /// `US$12.00`
    UsDollar,
    /// `USD 1,234.56`
    IsoPrefix,
    /// `12.00 USD`
    IsoSuffix,
    /// `$15.00 USD`
    DollarIsoSuffix,
    /// `1.234,56 €`
    EuroSuffix,
    /// `€12`, with decimals only when there are cents
    EuroPrefix,
    /// `¥1,200`
    Yen,
}

impl Money {
    pub fn fmt(self, minor: i64) -> String {
        let sign = if minor < 0 { "-" } else { "" };
        let abs = minor.abs();
        let (int, frac) = (abs / 100, abs % 100);
        let en = format!("{}.{frac:02}", group(int, ','));
        let body = match self {
            Money::Dollar => format!("${en}"),
            Money::UsDollar => format!("US${en}"),
            Money::IsoPrefix => format!("USD {en}"),
            Money::IsoSuffix => format!("{en} USD"),
            Money::DollarIsoSuffix => format!("${en} USD"),
            Money::EuroSuffix => format!("{},{frac:02} €", group(int, '.')),
            Money::EuroPrefix if frac == 0 => format!("€{}", group(int, ',')),
            Money::EuroPrefix => format!("€{en}"),
            Money::Yen => format!("¥{}", group(abs, ',')),
        };
        format!("{sign}{body}")
    }

    pub fn currency(self) -> &'static str {
        match self {
            Money::EuroSuffix | Money::EuroPrefix => "EUR",
            Money::Yen => "JPY",
            _ => "USD",
        }
    }
}

fn group(n: i64, sep: char) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(sep);
        }
        out.push(c);
    }
    out
}

/// Tax rates are in thousandths of a percent: 8875 is 8.875%.
#[derive(Clone, Copy)]
pub enum Tax {
    None,
    UsSales { region: &'static str, rate: i64 },
    VatExclusive { rate: i64, number: &'static str },
    VatInclusive { rate: i64, number: &'static str },
    Gst { rate: i64 },
    ReverseCharge { customer_vat: &'static str },
}

fn pct(rate: i64) -> String {
    let frac = format!("{:03}", rate % 1000);
    let frac = frac.trim_end_matches('0');
    if frac.is_empty() {
        format!("{}%", rate / 1000)
    } else {
        format!("{}.{frac}%", rate / 1000)
    }
}

fn round_div(n: i64, d: i64) -> i64 {
    (n + d / 2) / d
}

impl Tax {
    /// The total the customer pays on `sub`.
    pub fn total(self, sub: i64) -> i64 {
        match self {
            Tax::UsSales { rate, .. } | Tax::VatExclusive { rate, .. } | Tax::Gst { rate } => {
                sub + round_div(sub * rate, 100_000)
            }
            Tax::None | Tax::VatInclusive { .. } | Tax::ReverseCharge { .. } => sub,
        }
    }

    fn lines(self, money: Money, sub: i64) -> Vec<Line> {
        let f = |x| money.fmt(x);
        let total = self.total(sub);
        match self {
            Tax::None => vec![kv("Total", f(total))],
            Tax::UsSales { region, rate } => vec![
                kv("Subtotal", f(sub)),
                kv(
                    &format!("Sales tax ({region} {})", pct(rate)),
                    f(total - sub),
                ),
                kv("Total", f(total)),
            ],
            Tax::VatExclusive { rate, number } => vec![
                kv("Subtotal", f(sub)),
                kv(&format!("VAT ({})", pct(rate)), f(total - sub)),
                kv("Total", f(total)),
                p(&format!("VAT number: {number}")),
            ],
            Tax::VatInclusive { rate, number } => {
                let net = round_div(sub * 100_000, 100_000 + rate);
                vec![
                    kv("Total", f(sub)),
                    kv(&format!("Includes VAT ({})", pct(rate)), f(sub - net)),
                    p(&format!("VAT number: {number}")),
                ]
            }
            Tax::Gst { rate } => vec![
                kv("Subtotal", f(sub)),
                kv(&format!("GST ({})", pct(rate)), f(total - sub)),
                kv("Total", f(total)),
            ],
            Tax::ReverseCharge { customer_vat } => vec![
                kv("Subtotal", f(sub)),
                kv("VAT 0% (reverse charge)", f(0)),
                kv("Total", f(total)),
                p(&format!(
                    "Customer VAT ID: {customer_vat}. VAT is due from the customer under the reverse-charge rule."
                )),
            ],
        }
    }
}

#[derive(Clone, Copy)]
pub enum Period {
    /// `Jan 1 – Feb 1, 2026`
    Words,
    /// `01/01/2026 - 02/01/2026`
    MonthFirst,
    /// `01/02/2026 - 01/03/2026`
    DayFirst,
    /// "the next 12 months" for annual plans, words otherwise
    Next12,
}

impl Period {
    fn line(self, start: Dt, months: i64) -> Line {
        let end = start.plus_months(months);
        let slash = |d: Dt, day_first: bool| {
            if day_first {
                format!("{:02}/{:02}/{}", d.d, d.mo, d.y)
            } else {
                format!("{:02}/{:02}/{}", d.mo, d.d, d.y)
            }
        };
        match self {
            Period::Next12 if months == 12 => p("Your plan now covers the next 12 months."),
            Period::Words | Period::Next12 => {
                let range = if start.y == end.y {
                    format!(
                        "{} {} – {} {}, {}",
                        start.month_name(),
                        start.d,
                        end.month_name(),
                        end.d,
                        end.y
                    )
                } else {
                    format!("{} – {}", start.words(), end.words())
                };
                kv("Billing period", range)
            }
            Period::MonthFirst => kv(
                "Billing period",
                format!("{} - {}", slash(start, false), slash(end, false)),
            ),
            Period::DayFirst => kv(
                "Billing period",
                format!("{} - {}", slash(start, true), slash(end, true)),
            ),
        }
    }
}

#[derive(Clone, Copy)]
pub enum Layout {
    Text(Cte),
    Alternative(Cte),
    HtmlOnly(Cte),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Relay {
    Direct,
    Stripe,
    Paddle,
    Apple,
    GooglePlay,
    PayPal,
}

pub struct Vendor {
    pub key: &'static str,
    pub display: &'static str,
    pub address: String,
    pub bounce: String,
    pub dkim: Vec<&'static str>,
    pub relay: Relay,
    pub merchant: Option<&'static str>,
    /// The name the service goes by in subjects and account notices.
    pub brand: &'static str,
    pub product: &'static str,
    pub money: Money,
    pub monthly: i64,
    pub annual: i64,
    /// Monthly price of the plan above, for the upgrade.
    pub plus: i64,
    /// Monthly price of the plan below, for the downgrade.
    pub lite: i64,
    /// Monthly price after the price change.
    pub raised: i64,
    pub tax: Tax,
    pub period: Period,
    /// Label and `#`/`@` pattern of the printed invoice identifier.
    pub invoice: Option<(&'static str, &'static str)>,
    pub card: &'static str,
    pub layout: Layout,
    pub eol: Eol,
    pub offset: i32,
}

impl Vendor {
    fn domain(&self) -> &str {
        self.address.split('@').nth(1).expect("address has a host")
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    FirstCharge,
    RenewalMonthly,
    RenewalAnnual,
    TrialEnding,
    FailedFirst,
    FailedFinal,
    CardExpiring,
    Upgrade,
    Downgrade,
    PriceChange,
    RefundFull,
    RefundPartial,
    Cancellation,
}

const KINDS: [Kind; 13] = [
    Kind::FirstCharge,
    Kind::RenewalMonthly,
    Kind::RenewalAnnual,
    Kind::TrialEnding,
    Kind::FailedFirst,
    Kind::FailedFinal,
    Kind::CardExpiring,
    Kind::Upgrade,
    Kind::Downgrade,
    Kind::PriceChange,
    Kind::RefundFull,
    Kind::RefundPartial,
    Kind::Cancellation,
];

impl Kind {
    fn slug(self) -> &'static str {
        match self {
            Kind::FirstCharge => "01_first_charge",
            Kind::RenewalMonthly => "02_renewal_monthly",
            Kind::RenewalAnnual => "03_renewal_annual",
            Kind::TrialEnding => "04_trial_ending",
            Kind::FailedFirst => "05_payment_failed_first",
            Kind::FailedFinal => "06_payment_failed_final",
            Kind::CardExpiring => "07_card_expiring",
            Kind::Upgrade => "08_upgrade",
            Kind::Downgrade => "09_downgrade",
            Kind::PriceChange => "10_price_change",
            Kind::RefundFull => "11_refund_full",
            Kind::RefundPartial => "11_refund_partial",
            Kind::Cancellation => "12_cancellation",
        }
    }

    fn receipt_kind(self) -> ReceiptKind {
        match self {
            Kind::FirstCharge | Kind::RenewalMonthly | Kind::RenewalAnnual => ReceiptKind::Charge,
            Kind::TrialEnding => ReceiptKind::TrialEnding,
            Kind::FailedFirst | Kind::FailedFinal => ReceiptKind::PaymentFailed,
            Kind::CardExpiring => ReceiptKind::CardExpiring,
            Kind::Upgrade => ReceiptKind::Upgrade,
            Kind::Downgrade => ReceiptKind::Downgrade,
            Kind::PriceChange => ReceiptKind::PriceChange,
            Kind::RefundFull | Kind::RefundPartial => ReceiptKind::Refund,
            Kind::Cancellation => ReceiptKind::Cancellation,
        }
    }

    fn charges(self) -> bool {
        matches!(
            self,
            Kind::FirstCharge | Kind::RenewalMonthly | Kind::RenewalAnnual | Kind::Upgrade
        )
    }

    /// Local calendar date of this kind's message.
    fn date(self) -> (i64, u32, u32) {
        match self {
            Kind::TrialEnding => (2025, 12, 29),
            Kind::FirstCharge => (2026, 1, 1),
            Kind::RenewalMonthly => (2026, 2, 1),
            Kind::RefundFull => (2026, 2, 3),
            Kind::RefundPartial => (2026, 2, 4),
            Kind::Upgrade => (2026, 2, 16),
            Kind::Downgrade => (2026, 3, 16),
            Kind::PriceChange => (2026, 4, 2),
            Kind::FailedFirst => (2026, 5, 1),
            Kind::FailedFinal => (2026, 5, 8),
            Kind::CardExpiring => (2026, 5, 20),
            Kind::Cancellation => (2026, 6, 1),
            Kind::RenewalAnnual => (2026, 6, 15),
        }
    }
}

pub enum Line {
    P(String),
    Kv(String, String),
}

fn p(s: &str) -> Line {
    Line::P(s.to_owned())
}

fn kv(k: &str, v: String) -> Line {
    Line::Kv(k.to_owned(), v)
}

fn render_text(lines: &[Line]) -> String {
    let mut out = String::from("Hi Sam,\n");
    let mut previous_kv = false;
    for line in lines {
        match line {
            Line::P(s) => {
                out.push('\n');
                out.push_str(s);
                out.push('\n');
                previous_kv = false;
            }
            Line::Kv(k, v) => {
                if !previous_kv {
                    out.push('\n');
                }
                out.push_str(&format!("{k}: {v}\n"));
                previous_kv = true;
            }
        }
    }
    out
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn render_html(title: &str, brand: &str, lines: &[Line]) -> String {
    let mut out = format!(
        "<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\"><title>{}</title></head>\n<body style=\"font-family:Helvetica,Arial,sans-serif;color:#222\">\n<h1 style=\"font-size:20px\">{}</h1>\n<p>Hi Sam,</p>\n",
        escape(title),
        escape(brand)
    );
    let mut in_table = false;
    for line in lines {
        match line {
            Line::P(s) => {
                if in_table {
                    out.push_str("</table>\n");
                    in_table = false;
                }
                out.push_str(&format!("<p>{}</p>\n", escape(s)));
            }
            Line::Kv(k, v) => {
                if !in_table {
                    out.push_str("<table role=\"presentation\" cellpadding=\"4\">\n");
                    in_table = true;
                }
                out.push_str(&format!(
                    "<tr><td>{}</td><td style=\"text-align:right\">{}</td></tr>\n",
                    escape(k),
                    escape(v)
                ));
            }
        }
    }
    if in_table {
        out.push_str("</table>\n");
    }
    out.push_str("</body></html>\n");
    out
}

pub fn body(m: &mut Msg, layout: Layout, subject: &str, brand: &str, lines: &[Line]) {
    let part = match layout {
        Layout::Text(cte) => text(&render_text(lines), cte),
        Layout::HtmlOnly(cte) => html(&render_html(subject, brand, lines), cte),
        Layout::Alternative(cte) => {
            let boundary = m.boundary();
            multipart(
                "alternative",
                &boundary,
                vec![
                    text(&render_text(lines), cte),
                    html(&render_html(subject, brand, lines), cte),
                ],
            )
        }
    };
    m.body(part);
}

/// Return-Path, signatures, From, To, Subject, Date, Message-ID.
pub fn envelope(m: &mut Msg, v: &Vendor, subject: &str, at: Dt) {
    m.eol = v.eol;
    m.return_path(&v.bounce);
    for d in &v.dkim {
        m.dkim(
            d,
            "from:to:subject:date:message-id:mime-version:content-type",
        );
    }
    m.sender(v.display, &v.address);
    m.to();
    m.subject(subject);
    m.date(at);
    let domain = v.domain().to_owned();
    m.message_id(&domain);
}

/// Where a relayed message names its merchant. Platform account notices
/// about the card name no merchant.
fn seller_line(v: &Vendor, kind: Kind) -> Option<Line> {
    let merchant = v.merchant?;
    let charge_like = kind.charges() || kind == Kind::TrialEnding;
    Some(match v.relay {
        Relay::Direct => return None,
        Relay::Stripe if charge_like => p(&format!("Receipt from {merchant}")),
        Relay::Stripe => p(merchant),
        Relay::Paddle if charge_like => p(&format!("Thanks for your purchase from {merchant}.")),
        Relay::Paddle => p(&format!("About your purchase from {merchant}.")),
        Relay::Apple | Relay::GooglePlay | Relay::PayPal if kind == Kind::CardExpiring => {
            return None;
        }
        Relay::Apple => kv("App", merchant.to_owned()),
        Relay::GooglePlay => kv("Seller", merchant.to_owned()),
        Relay::PayPal if matches!(kind, Kind::RefundFull | Kind::RefundPartial) => {
            p(&format!("Refund from {merchant}"))
        }
        Relay::PayPal if charge_like => p(&format!("Payment to {merchant}")),
        Relay::PayPal => p(&format!("Automatic payments to {merchant}")),
    })
}

fn receipt_subject(v: &Vendor, at: Dt, fallback: String) -> String {
    match (v.relay, v.merchant) {
        (Relay::Apple, _) => "Your receipt from Apple.".to_owned(),
        (Relay::GooglePlay, _) => format!("Your Google Play Order Receipt from {}", at.words()),
        (Relay::PayPal, Some(m)) => format!("Receipt for your payment to {m}"),
        _ => fallback,
    }
}

/// Writes one vendor message of `kind` at `at`, with the monthly price
/// `monthly` (series vary it).
pub fn write(m: &mut Msg, v: &Vendor, kind: Kind, at: Dt, monthly: i64) {
    let f = |x| v.money.fmt(x);
    let product = v.product;
    let seller = v.merchant.unwrap_or(v.brand);
    let invoice = v
        .invoice
        .map(|(label, pattern)| (label, m.rng.fill(pattern)));
    let invoice_line = || invoice.as_ref().map(|(label, id)| kv(label, id.clone()));
    let total = v.tax.total(monthly);
    let mut lines: Vec<Line> = seller_line(v, kind).into_iter().collect();
    let (subject, amount, with_invoice) = match kind {
        Kind::FirstCharge | Kind::RenewalMonthly | Kind::RenewalAnnual => {
            let (subject, intro, sub, months) = match kind {
                Kind::FirstCharge => (
                    format!("Your {seller} receipt"),
                    format!("Thanks for subscribing to {product}."),
                    monthly,
                    1,
                ),
                Kind::RenewalMonthly => (
                    format!("Your {product} subscription has renewed"),
                    format!("Your monthly subscription to {product} has renewed."),
                    monthly,
                    1,
                ),
                _ => (
                    format!("Your annual {product} renewal"),
                    format!("Your annual subscription to {product} has renewed."),
                    v.annual,
                    12,
                ),
            };
            lines.push(p(&intro));
            lines.push(kv("Date", at.words()));
            lines.push(v.period.line(at, months));
            lines.extend(v.tax.lines(v.money, sub));
            lines.extend(invoice_line());
            lines.push(kv("Payment method", v.card.to_owned()));
            (
                receipt_subject(v, at, subject),
                Some(v.tax.total(sub)),
                true,
            )
        }
        Kind::TrialEnding => {
            let end = at.plus_days(3).words();
            lines.push(p(&format!("Your free trial of {product} ends on {end}.")));
            lines.push(kv(&format!("Amount due on {end}"), f(total)));
            lines.push(p(&format!(
                "We'll charge your {} unless you cancel before then.",
                v.card
            )));
            (
                format!("Your {product} trial ends in 3 days"),
                Some(total),
                false,
            )
        }
        Kind::FailedFirst => {
            lines.push(p(&format!(
                "Your payment for {product} failed. Your bank declined the charge to your {}.",
                v.card
            )));
            lines.push(kv("Amount due", f(total)));
            lines.extend(invoice_line());
            lines.push(p(
                "We'll try again in 3 days. Update your payment method to keep your subscription.",
            ));
            (
                format!("We couldn't process your {seller} payment"),
                Some(total),
                true,
            )
        }
        Kind::FailedFinal => {
            lines.push(p(&format!("We still can't collect payment for {product}.")));
            lines.push(kv("Amount due", f(total)));
            lines.extend(invoice_line());
            lines.push(p(&format!(
                "If payment fails again on {}, we'll suspend your subscription.",
                at.plus_days(7).words()
            )));
            (
                format!("Final notice: your {product} subscription will be suspended"),
                Some(total),
                true,
            )
        }
        Kind::CardExpiring => {
            let target = if v.relay != Relay::Direct && v.merchant.is_some() && lines.is_empty() {
                "your subscriptions".to_owned()
            } else {
                product.to_owned()
            };
            lines.push(p(&format!(
                "The {} on your {} account expires at the end of this month.",
                v.card, v.brand
            )));
            lines.push(p(&format!(
                "Update your card to avoid an interruption to {target}."
            )));
            ("Your card on file expires soon".to_owned(), None, false)
        }
        Kind::Upgrade => {
            let new_half = round_div(v.plus, 2);
            let old_half = round_div(monthly, 2);
            let sub = new_half - old_half;
            lines.push(p(&format!(
                "You moved from {product} to {product} Plus today."
            )));
            lines.push(kv(
                &format!("{product} Plus, 15 days (prorated)"),
                f(new_half),
            ));
            lines.push(kv(
                &format!("Unused time on {product}, 15 days"),
                f(-old_half),
            ));
            lines.extend(v.tax.lines(v.money, sub));
            lines.extend(invoice_line());
            lines.push(kv("Payment method", v.card.to_owned()));
            (
                receipt_subject(v, at, format!("You've upgraded to {product} Plus")),
                Some(v.tax.total(sub)),
                true,
            )
        }
        Kind::Downgrade => {
            let credit = round_div(monthly - v.lite, 2);
            lines.push(p(&format!(
                "You moved from {product} to {product} Lite today."
            )));
            lines.push(kv("New price", format!("{} per month", f(v.lite))));
            lines.push(kv("Credit added to your account", f(credit)));
            lines.push(p("We'll apply the credit to your next invoices."));
            (
                format!("Your plan has changed to {product} Lite"),
                Some(credit),
                false,
            )
        }
        Kind::PriceChange => {
            let effective = Dt { d: 1, ..at }.plus_months(1).words();
            lines.push(p(&format!("We're changing the price of {product}.")));
            lines.push(kv("Current price", format!("{} per month", f(monthly))));
            lines.push(kv(
                &format!("New price from {effective}"),
                format!("{} per month", f(v.raised)),
            ));
            lines.push(p(
                "You don't need to do anything. To cancel, visit your account settings.",
            ));
            (
                format!("An update to your {product} price"),
                Some(v.raised),
                false,
            )
        }
        Kind::RefundFull => {
            lines.push(p(&format!("We've refunded your payment for {product}.")));
            lines.push(kv("Refund amount", f(total)));
            lines.extend(invoice_line());
            lines.push(kv("Refunded to", v.card.to_owned()));
            lines.push(p("Refunds take 5-10 business days to appear."));
            (format!("Your refund from {seller}"), Some(total), true)
        }
        Kind::RefundPartial => {
            let part = round_div(total, 2);
            lines.push(p(&format!(
                "We've refunded part of your payment for {product}."
            )));
            lines.push(kv("Original charge", f(total)));
            lines.push(kv("Refund amount", f(part)));
            lines.extend(invoice_line());
            lines.push(kv("Refunded to", v.card.to_owned()));
            (
                format!("Partial refund for your {product} payment"),
                Some(part),
                true,
            )
        }
        Kind::Cancellation => {
            lines.push(p(&format!("Your subscription to {product} is cancelled.")));
            lines.push(p(&format!(
                "You keep access until {}. You won't be charged again.",
                at.plus_days(20).words()
            )));
            (
                format!("Your {product} subscription has been cancelled"),
                None,
                false,
            )
        }
    };
    envelope(m, v, &subject, at);
    body(m, v.layout, &subject, v.brand, &lines);
    let merchant = seller_line(v, kind).and(v.merchant).map(str::to_owned);
    m.receipt(Receipt {
        kind: kind.receipt_kind(),
        amount_minor_units: amount,
        currency: amount.map(|_| v.money.currency().to_owned()),
        merchant,
        invoice_ref: if with_invoice {
            invoice.map(|(_, id)| id)
        } else {
            None
        },
    });
}

fn bounce(domain: &str) -> String {
    format!("bounces+sam=inbox.test@{domain}")
}

pub fn vendors() -> Vec<Vendor> {
    let [stripe, paddle, apple, google, paypal] = RELAY_PLATFORM_DOMAINS;
    vec![
        Vendor {
            key: "gitforge",
            display: "GitForge",
            address: "billing@gitforge.test".into(),
            bounce: bounce("em.gitforge.test"),
            dkim: vec!["gitforge.test"],
            relay: Relay::Direct,
            merchant: None,
            brand: "GitForge",
            product: "GitForge Team",
            money: Money::Dollar,
            monthly: 1200,
            annual: 12000,
            plus: 2100,
            lite: 400,
            raised: 1500,
            tax: Tax::UsSales {
                region: "NY",
                rate: 8875,
            },
            period: Period::Words,
            invoice: Some(("Invoice number", "GF-2026-#####")),
            card: "Visa ending in 4242",
            layout: Layout::Alternative(Cte::Qp),
            eol: Eol::Crlf,
            offset: -480,
        },
        Vendor {
            key: "cloudbarn",
            display: "Cloudbarn Billing",
            address: "no-reply@billing.cloudbarn.test".into(),
            bounce: "0100019a2b3c4d5e-1f2a3b4c-5d6e-7f80-9a0b-1c2d3e4f5a6b-000000@amazonses.com"
                .into(),
            dkim: vec!["billing.cloudbarn.test", "amazonses.com"],
            relay: Relay::Direct,
            merchant: None,
            brand: "Cloudbarn",
            product: "Cloudbarn Business Support",
            money: Money::IsoPrefix,
            monthly: 123_456,
            annual: 1_234_560,
            plus: 180_000,
            lite: 40_000,
            raised: 130_000,
            tax: Tax::None,
            period: Period::MonthFirst,
            invoice: Some(("Invoice ID", "##########")),
            card: "Mastercard ending in 5454",
            layout: Layout::Text(Cte::SevenBit),
            eol: Eol::Lf,
            offset: 0,
        },
        Vendor {
            key: "streamly",
            display: "Streamly",
            address: "info@account.streamly.test".into(),
            bounce: bounce("mail.streamly.test"),
            dkim: vec!["streamly.test"],
            relay: Relay::Direct,
            merchant: None,
            brand: "Streamly",
            product: "Streamly Standard",
            money: Money::EuroPrefix,
            monthly: 1200,
            annual: 12000,
            plus: 1800,
            lite: 800,
            raised: 1400,
            tax: Tax::VatInclusive {
                rate: 23_000,
                number: "IE3668997OH",
            },
            period: Period::Next12,
            invoice: None,
            card: "Visa ending in 1881",
            layout: Layout::Alternative(Cte::Qp),
            eol: Eol::Crlf,
            offset: 0,
        },
        Vendor {
            key: "pagewise",
            display: "Pagewise",
            address: "receipts@pagewise.test".into(),
            bounce: bounce("pagewise.test"),
            dkim: vec!["pagewise.test"],
            relay: Relay::Direct,
            merchant: None,
            brand: "Pagewise",
            product: "Pagewise Team",
            money: Money::EuroSuffix,
            monthly: 1200,
            annual: 123_456,
            plus: 2000,
            lite: 600,
            raised: 1500,
            tax: Tax::ReverseCharge {
                customer_vat: "DE811569869",
            },
            period: Period::DayFirst,
            invoice: Some(("Invoice number", "PW-######")),
            card: "Mastercard ending in 0005",
            layout: Layout::Text(Cte::EightBit),
            eol: Eol::Lf,
            offset: 60,
        },
        Vendor {
            key: "tunebox",
            display: "Tunebox",
            address: "no-reply@tunebox.test".into(),
            bounce: bounce("em1234.tunebox.test"),
            dkim: vec!["tunebox.test"],
            relay: Relay::Direct,
            merchant: None,
            brand: "Tunebox",
            product: "Tunebox Premium",
            money: Money::UsDollar,
            monthly: 1200,
            annual: 12000,
            plus: 1800,
            lite: 600,
            raised: 1300,
            tax: Tax::Gst { rate: 9000 },
            period: Period::Words,
            invoice: Some(("Order ID", "TB-########")),
            card: "Amex ending in 1005",
            layout: Layout::Text(Cte::Base64),
            eol: Eol::Crlf,
            offset: 480,
        },
        Vendor {
            key: "stripe",
            display: "Lumen Analytics",
            address: format!("invoice+statements@{stripe}"),
            bounce: format!("bounce+acct_1Abc@{stripe}"),
            dkim: vec![stripe],
            relay: Relay::Stripe,
            merchant: Some("Lumen Analytics"),
            brand: "Lumen Analytics",
            product: "Lumen Analytics Growth",
            money: Money::Dollar,
            monthly: 4900,
            annual: 49000,
            plus: 9900,
            lite: 1900,
            raised: 5900,
            tax: Tax::UsSales {
                region: "CA",
                rate: 7250,
            },
            period: Period::Words,
            invoice: Some(("Receipt number", "####-####")),
            card: "Visa ending in 4242",
            layout: Layout::Alternative(Cte::Qp),
            eol: Eol::Crlf,
            offset: -480,
        },
        Vendor {
            key: "paddle",
            display: "Paddle",
            address: format!("help@{paddle}"),
            bounce: format!("bounces@mail.{paddle}"),
            dkim: vec![paddle],
            relay: Relay::Paddle,
            merchant: Some("Inkwell Fonts"),
            brand: "Inkwell Fonts",
            product: "Inkwell Fonts Studio",
            money: Money::IsoSuffix,
            monthly: 1200,
            annual: 12000,
            plus: 2400,
            lite: 600,
            raised: 1500,
            tax: Tax::VatExclusive {
                rate: 20_000,
                number: "GB 123 4567 89",
            },
            period: Period::DayFirst,
            invoice: Some(("Order number", "#########")),
            card: "Visa ending in 1111",
            layout: Layout::Text(Cte::Qp),
            eol: Eol::Lf,
            offset: 0,
        },
        Vendor {
            key: "apple",
            display: "Apple",
            address: format!("no_reply@{apple}"),
            bounce: format!("bounce@{apple}"),
            dkim: vec![apple],
            relay: Relay::Apple,
            merchant: Some("Skyline Weather"),
            brand: "Apple",
            product: "Skyline Weather Premium",
            money: Money::Yen,
            monthly: 1200,
            annual: 12000,
            plus: 2000,
            lite: 600,
            raised: 1500,
            tax: Tax::None,
            period: Period::Words,
            invoice: Some(("Order ID", "M@@@@@@@@@")),
            card: "Visa ending in 0310",
            layout: Layout::HtmlOnly(Cte::Base64),
            eol: Eol::Crlf,
            offset: 540,
        },
        Vendor {
            key: "google_play",
            display: "Google Play",
            address: format!("googleplay-noreply@{google}"),
            bounce: format!("3xYz-googleplay-noreply=google.com@{google}"),
            dkim: vec![google],
            relay: Relay::GooglePlay,
            merchant: Some("Puzzle Den"),
            brand: "Google Play",
            product: "Puzzle Den Premium",
            money: Money::UsDollar,
            monthly: 499,
            annual: 4999,
            plus: 999,
            lite: 199,
            raised: 599,
            tax: Tax::None,
            period: Period::MonthFirst,
            invoice: Some(("Order number", "GPA.####-####-####-#####")),
            card: "Mastercard ending in 5454",
            layout: Layout::Alternative(Cte::SevenBit),
            eol: Eol::Lf,
            offset: -300,
        },
        Vendor {
            key: "paypal",
            display: "PayPal",
            address: format!("service@{paypal}"),
            bounce: format!("bounce@{paypal}"),
            dkim: vec![paypal],
            relay: Relay::PayPal,
            merchant: Some("Northwind Hosting"),
            brand: "PayPal",
            product: "Northwind Hosting VPS-2",
            money: Money::DollarIsoSuffix,
            monthly: 1500,
            annual: 15000,
            plus: 3000,
            lite: 700,
            raised: 1800,
            tax: Tax::None,
            period: Period::Words,
            invoice: Some(("Invoice ID", "NH-#####")),
            card: "Visa ending in 1111",
            layout: Layout::Alternative(Cte::Qp),
            eol: Eol::Lf,
            offset: -420,
        },
    ]
}

pub fn generate(corpus: &mut Corpus) {
    let vendors = vendors();
    for v in &vendors {
        for kind in KINDS {
            let mut m = corpus.msg(&format!("receipts/{}/{}", v.key, kind.slug()));
            let (y, mo, d) = kind.date();
            let at = Dt::new(
                y,
                mo,
                d,
                6 + m.rng.below(14) as u32,
                m.rng.below(60) as u32,
                v.offset,
            );
            write(&mut m, v, kind, at, v.monthly);
            corpus.put(m);
        }
    }
    let by_key = |key: &str| vendors.iter().find(|v| v.key == key).expect("vendor");
    qp_encoded_period(corpus, by_key("gitforge"));
    discount_to_zero(corpus, by_key("gitforge"));
    qp_soft_break(corpus, by_key("stripe"));
    amount_only_in_html(corpus, by_key("streamly"));
    amount_only_in_pdf(corpus, by_key("cloudbarn"));
}

/// `$12=2E00`: the period is quoted-printable encoded.
fn qp_encoded_period(corpus: &mut Corpus, v: &Vendor) {
    let mut m = corpus.msg("receipts/gitforge/qp_encoded_period");
    let invoice = m.rng.fill("GF-2026-#####");
    let at = Dt::new(2026, 3, 1, 9, 12, v.offset);
    envelope(&mut m, v, "Your GitForge receipt", at);
    let content = format!(
        "Hi Sam,\n\nThanks for your payment to GitForge. Your billing address is in a state\nwith no sales tax, so none was charged.\n\nPlan: GitForge Team (monthly)\nTotal: $12=2E00\nInvoice number: {invoice}\nPayment method: Visa ending in 4242\n"
    );
    m.body(Part {
        headers: vec![
            "Content-Type: text/plain; charset=utf-8".into(),
            "Content-Transfer-Encoding: quoted-printable".into(),
        ],
        body: content.into_bytes(),
    });
    m.receipt(Receipt {
        kind: ReceiptKind::Charge,
        amount_minor_units: Some(1200),
        currency: Some("USD".into()),
        merchant: None,
        invoice_ref: Some(invoice),
    });
    corpus.put(m);
}

/// A zero-amount invoice from a 100% discount.
fn discount_to_zero(corpus: &mut Corpus, v: &Vendor) {
    let mut m = corpus.msg("receipts/gitforge/discount_100_percent");
    let invoice = m.rng.fill("GF-2026-#####");
    let at = Dt::new(2026, 1, 1, 10, 0, v.offset);
    let subject = "Your GitForge receipt";
    envelope(&mut m, v, subject, at);
    let lines = vec![
        p("Thanks for subscribing to GitForge Team."),
        kv("Date", at.words()),
        v.period.line(at, 1),
        kv("Subtotal", "$12.00".into()),
        kv("Discount (LAUNCH100, 100% off)", "-$12.00".into()),
        kv("Sales tax (NY 8.875%)", "$0.00".into()),
        kv("Total", "$0.00".into()),
        kv("Invoice number", invoice.clone()),
    ];
    body(&mut m, v.layout, subject, v.brand, &lines);
    m.receipt(Receipt {
        kind: ReceiptKind::Charge,
        amount_minor_units: Some(0),
        currency: Some("USD".into()),
        merchant: None,
        invoice_ref: Some(invoice),
    });
    corpus.put(m);
}

/// `$12=\n.00`: a soft line break inside the amount, in a bare-LF file.
fn qp_soft_break(corpus: &mut Corpus, v: &Vendor) {
    let mut m = corpus.msg("receipts/stripe/qp_soft_break");
    let receipt_number = m.rng.fill("####-####");
    let at = Dt::new(2026, 3, 1, 16, 45, v.offset);
    envelope(&mut m, v, "Your receipt from Lumen Analytics", at);
    m.eol = Eol::Lf;
    let content = format!(
        "Receipt from Lumen Analytics\n\nLumen Analytics Starter, 1 seat, billed monthly. Tax exempt customer, n=\no tax applied. Amount paid: $12=\n.00\nDate paid: {}\nReceipt number: {receipt_number}\nPayment method: Visa ending in 4242\n",
        at.words()
    );
    m.body(Part {
        headers: vec![
            "Content-Type: text/plain; charset=utf-8".into(),
            "Content-Transfer-Encoding: quoted-printable".into(),
        ],
        body: content.into_bytes(),
    });
    m.receipt(Receipt {
        kind: ReceiptKind::Charge,
        amount_minor_units: Some(1200),
        currency: Some("USD".into()),
        merchant: Some("Lumen Analytics".into()),
        invoice_ref: Some(receipt_number),
    });
    corpus.put(m);
}

/// The text part is empty; only the HTML part carries the amount.
fn amount_only_in_html(corpus: &mut Corpus, v: &Vendor) {
    let mut m = corpus.msg("receipts/streamly/amount_only_in_html");
    let at = Dt::new(2026, 7, 1, 7, 3, v.offset);
    let subject = "Your annual Streamly Standard renewal";
    envelope(&mut m, v, subject, at);
    let lines = vec![
        p("Your annual subscription to Streamly Standard has renewed."),
        kv("Date", at.words()),
        v.period.line(at, 12),
        kv("Total", "€120".into()),
        kv("Includes VAT (23%)", "€22.44".into()),
        p("VAT number: IE3668997OH"),
    ];
    let boundary = m.boundary();
    m.body(multipart(
        "alternative",
        &boundary,
        vec![
            text("", Cte::SevenBit),
            html(&render_html(subject, v.brand, &lines), Cte::Qp),
        ],
    ));
    m.receipt(Receipt {
        kind: ReceiptKind::Charge,
        amount_minor_units: Some(12000),
        currency: Some("EUR".into()),
        merchant: None,
        invoice_ref: None,
    });
    corpus.put(m);
}

/// Neither body part names the amount; the PDF does, and M1 does not read
/// PDFs, so the golden amount is null.
fn amount_only_in_pdf(corpus: &mut Corpus, v: &Vendor) {
    let mut m = corpus.msg("receipts/cloudbarn/amount_only_in_pdf");
    let invoice = m.rng.fill("##########");
    let at = Dt::new(2026, 2, 2, 8, 0, v.offset);
    envelope(&mut m, v, "Cloudbarn invoice available", at);
    let pdf = tiny_pdf(&[
        "Cloudbarn Web Services",
        &format!("Invoice ID: {invoice}"),
        "Billing period: 01/01/2026 - 02/01/2026",
        "Total: USD 1,234.56",
    ]);
    let boundary = m.boundary();
    let mut attachment = leaf(
        &format!("application/pdf; name=\"{invoice}.pdf\""),
        Cte::Base64,
        &pdf,
    );
    attachment.headers.push(format!(
        "Content-Disposition: attachment; filename=\"{invoice}.pdf\""
    ));
    m.body(multipart(
        "mixed",
        &boundary,
        vec![
            text(
                "Hello,\n\nYour Cloudbarn invoice for January 2026 is attached. You can also view\nit in the Billing console.\n\nCloudbarn Billing\n",
                Cte::SevenBit,
            ),
            attachment,
        ],
    ));
    m.receipt(Receipt {
        kind: ReceiptKind::Charge,
        amount_minor_units: None,
        currency: None,
        merchant: None,
        invoice_ref: None,
    });
    corpus.put(m);
}
