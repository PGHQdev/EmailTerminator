//! Amounts as printed in billing mail, read into integer minor units.

use std::sync::LazyLock;

use regex::Regex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Money {
    pub minor_units: i64,
    pub currency: &'static str,
    /// Byte range of the match in the searched text.
    pub start: usize,
    pub end: usize,
}

/// Symbols and codes, longest first so `US$` wins over `$`.
const SIGNS: &[(&str, &str)] = &[
    ("US$", "USD"),
    ("CA$", "CAD"),
    ("AU$", "AUD"),
    ("NZ$", "NZD"),
    ("HK$", "HKD"),
    ("C$", "CAD"),
    ("A$", "AUD"),
    ("S$", "SGD"),
    ("R$", "BRL"),
    ("$", "USD"),
    ("€", "EUR"),
    ("£", "GBP"),
    ("¥", "JPY"),
    ("₹", "INR"),
];

const CODES: &[&str] = &[
    "USD", "EUR", "GBP", "JPY", "CAD", "AUD", "NZD", "CHF", "SEK", "NOK", "DKK", "INR", "BRL",
    "SGD", "HKD", "MXN", "PLN", "CZK", "ZAR", "KRW", "CNY",
];

/// ISO 4217 currencies without a minor unit.
const ZERO_DECIMAL: &[&str] = &["JPY", "KRW"];

static AMOUNT: &str = r"\d{1,3}(?:[.,\x{a0} ]\d{3})+(?:[.,]\d{1,2})?|\d+(?:[.,]\d{1,2})?";

static PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    let signs = SIGNS
        .iter()
        .map(|(s, _)| regex::escape(s))
        .chain(CODES.iter().map(|c| format!(r"\b{c}")))
        .collect::<Vec<_>>()
        .join("|");
    let suffixes = ["€", "£", "¥"]
        .iter()
        .map(|s| regex::escape(s))
        .chain(CODES.iter().map(|c| format!(r"{c}\b")))
        .collect::<Vec<_>>()
        .join("|");
    Regex::new(&format!(
        r"(?P<pre>{signs})\s?(?P<pa>{AMOUNT})|(?P<sa>{AMOUNT})\s?(?P<suf>{suffixes})"
    ))
    .expect("money pattern")
});

/// Every amount in `text`, in order. A sign written after a number belongs to
/// the next number instead when digits follow it: in "Jan 1, 2026 ¥1,200" the
/// yen is 1,200's, not 2026's.
pub fn find_all(text: &str) -> Vec<Money> {
    let mut found = Vec::new();
    let mut at = 0;
    while let Some(caps) = PATTERN.captures_at(text, at) {
        let whole = caps.get(0).expect("match");
        let (sign, amount) = match (caps.name("pre"), caps.name("pa")) {
            (Some(sign), Some(amount)) => (sign, amount),
            _ => {
                let (Some(sign), Some(amount)) = (caps.name("suf"), caps.name("sa")) else {
                    at = whole.end();
                    continue;
                };
                if text[sign.end()..]
                    .trim_start()
                    .starts_with(|c: char| c.is_ascii_digit())
                {
                    at = sign.start();
                    continue;
                }
                (sign, amount)
            }
        };
        at = whole.end();
        let Some(currency) = currency_of(sign.as_str()) else {
            continue;
        };
        if let Some(minor_units) = minor_units(amount.as_str(), currency) {
            found.push(Money {
                minor_units,
                currency,
                start: whole.start(),
                end: whole.end(),
            });
        }
    }
    found
}

fn currency_of(sign: &str) -> Option<&'static str> {
    SIGNS
        .iter()
        .find(|(s, _)| *s == sign)
        .map(|(_, code)| *code)
        .or_else(|| CODES.iter().find(|c| **c == sign).copied())
}

/// Reads `1,234.56`, `1.234,56`, `12,00`, `1 234,56` and `1,200` by the
/// position of the last separator: two or fewer digits after it make it the
/// decimal mark, three make it a thousands mark.
fn minor_units(amount: &str, currency: &str) -> Option<i64> {
    let cleaned: String = amount
        .chars()
        .filter(|c| !matches!(c, ' ' | '\u{a0}'))
        .collect();
    let (whole, fraction) = match cleaned.rfind([',', '.']) {
        Some(at) if cleaned.len() - at - 1 <= 2 => (&cleaned[..at], &cleaned[at + 1..]),
        _ => (cleaned.as_str(), ""),
    };
    let whole: i64 = whole
        .chars()
        .filter(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .ok()?;
    if ZERO_DECIMAL.contains(&currency) {
        return Some(whole);
    }
    let cents: i64 = match fraction.len() {
        0 => 0,
        1 => fraction.parse::<i64>().ok()? * 10,
        _ => fraction.parse().ok()?,
    };
    whole.checked_mul(100)?.checked_add(cents)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(text: &str) -> (i64, &'static str) {
        let found = find_all(text);
        assert_eq!(found.len(), 1, "{text:?} gave {found:?}");
        (found[0].minor_units, found[0].currency)
    }

    #[test]
    fn every_amount_form_in_the_corpus_spec() {
        assert_eq!(one("Total: $12.00"), (1200, "USD"));
        assert_eq!(one("Total: US$12.00"), (1200, "USD"));
        assert_eq!(one("Total: USD 12.00"), (1200, "USD"));
        assert_eq!(one("Summe: 12,00 €"), (1200, "EUR"));
        assert_eq!(one("Total €12"), (1200, "EUR"));
        assert_eq!(one("合計 ¥1,200"), (1200, "JPY"));
        assert_eq!(one("Total 12.00 USD"), (1200, "USD"));
    }

    #[test]
    fn both_decimal_conventions() {
        assert_eq!(one("$1,234.56"), (123456, "USD"));
        assert_eq!(one("1.234,56 €"), (123456, "EUR"));
        assert_eq!(one("€1 234,56"), (123456, "EUR"));
        assert_eq!(one("$1,200"), (120000, "USD"));
        assert_eq!(one("£9.5"), (950, "GBP"));
    }

    #[test]
    fn several_amounts_come_back_in_order() {
        let found = find_all("Subtotal $10.00, tax $0.80, total $10.80");
        let values: Vec<i64> = found.iter().map(|m| m.minor_units).collect();
        assert_eq!(values, vec![1000, 80, 1080]);
    }

    #[test]
    fn plain_numbers_are_not_money() {
        assert!(find_all("Order 12345 shipped on 2026-01-05, 3 items").is_empty());
        assert!(find_all("USDA approved").is_empty());
    }

    #[test]
    fn a_year_does_not_take_the_next_amounts_sign() {
        assert_eq!(one("Amount due on Jan 1, 2026 ¥1,200"), (1200, "JPY"));
        assert_eq!(one("Due Jan 1, 2026 €12,00"), (1200, "EUR"));
    }

    #[test]
    fn zero_is_an_amount() {
        assert_eq!(one("Amount due: $0.00"), (0, "USD"));
    }
}
