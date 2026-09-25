//! Seeded generator for the synthetic corpus (PLAN.md 2.9, Part 9, the M1
//! subset). Every single message is written as `<stem>.eml` beside its golden
//! `<stem>.json`, a pretty-printed `et_core::extract::Extraction`. A series is
//! a directory of `NNN.eml` files plus one `expected.json`.
//!
//! Each file draws from its own PRNG stream, seeded from the corpus seed and
//! its path, so adding a file leaves every other file unchanged.

mod encoded_words;
mod list_unsubscribe;
mod multipart;
mod receipts;
mod senders;
mod series;

use std::collections::BTreeMap;

use et_core::extract::{Extraction, Receipt};
use serde::{Deserialize, Serialize};

/// The corpus seed. Changing it rewrites every Message-ID, boundary and
/// signature in `out/`.
pub const SEED: u64 = 0x4554_2d43_4f52_5055;

/// Payment platforms that relay receipts for a merchant. The detector knows a
/// relay by its sender domain, so these are the real platform domains; the
/// merchants in the bodies are fictional. Order: Stripe, Paddle, Apple,
/// Google Play, PayPal.
pub const RELAY_PLATFORM_DOMAINS: [&str; 5] = [
    "stripe.com",
    "paddle.com",
    "email.apple.com",
    "google.com",
    "paypal.com",
];

/// Every file of the corpus, keyed by its path under `out/`.
pub fn generate(seed: u64) -> BTreeMap<String, Vec<u8>> {
    let mut corpus = Corpus {
        seed,
        files: BTreeMap::new(),
    };
    receipts::generate(&mut corpus);
    senders::generate(&mut corpus);
    list_unsubscribe::generate(&mut corpus);
    encoded_words::generate(&mut corpus);
    multipart::generate(&mut corpus);
    series::generate(&mut corpus);
    corpus.files
}

/// What recurrence detection must conclude from one series directory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeriesExpected {
    pub group_key: String,
    pub cadence: Option<Cadence>,
    pub charge_count: u32,
    pub price_increase: bool,
    pub latest_amount_minor_units: Option<i64>,
    pub currency: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Cadence {
    Monthly,
    Annual,
    Irregular,
}

pub struct Corpus {
    seed: u64,
    files: BTreeMap<String, Vec<u8>>,
}

impl Corpus {
    /// A new message whose files will be `<stem>.eml` and `<stem>.json`.
    pub fn msg(&self, stem: &str) -> Msg {
        let mut rng = Rng::for_path(self.seed, stem);
        let eol = if rng.below(2) == 0 {
            Eol::Crlf
        } else {
            Eol::Lf
        };
        Msg {
            stem: stem.to_owned(),
            rng,
            eol,
            head: Vec::new(),
            body: Vec::new(),
            g: Extraction {
                from_address: None,
                from_name: None,
                subject: None,
                date: None,
                message_id: None,
                list_unsubscribe: None,
                list_unsubscribe_post: false,
                list_id: None,
                is_list: false,
                dkim_domains: Vec::new(),
                receipt: None,
            },
            list_signal: false,
        }
    }

    /// Writes the message and its golden.
    pub fn put(&mut self, msg: Msg) {
        let (stem, eml, golden) = msg.finish();
        self.insert(format!("{stem}.eml"), eml);
        self.put_json(&format!("{stem}.json"), &golden);
    }

    /// Writes the message only, for series members.
    pub fn put_eml(&mut self, msg: Msg) {
        let (stem, eml, _) = msg.finish();
        self.insert(format!("{stem}.eml"), eml);
    }

    pub fn put_json<T: Serialize>(&mut self, path: &str, value: &T) {
        let mut json = serde_json::to_string_pretty(value).expect("golden serialises");
        json.push('\n');
        self.insert(path.to_owned(), json.into_bytes());
    }

    fn insert(&mut self, path: String, bytes: Vec<u8>) {
        let previous = self.files.insert(path.clone(), bytes);
        assert!(previous.is_none(), "two files at {path}");
    }
}

/// splitmix64.
pub struct Rng(u64);

impl Rng {
    fn for_path(seed: u64, path: &str) -> Self {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in path.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
        let mut rng = Rng(seed ^ hash);
        rng.next();
        rng
    }

    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn pick(&mut self, alphabet: &[u8], len: usize) -> String {
        (0..len)
            .map(|_| alphabet[self.below(alphabet.len() as u64) as usize] as char)
            .collect()
    }

    pub fn hex(&mut self, len: usize) -> String {
        self.pick(b"0123456789abcdef", len)
    }

    pub fn digits(&mut self, len: usize) -> String {
        self.pick(b"0123456789", len)
    }

    pub fn upper(&mut self, len: usize) -> String {
        self.pick(b"ABCDEFGHJKLMNPQRSTUVWXYZ0123456789", len)
    }

    fn base64ish(&mut self, len: usize) -> String {
        self.pick(
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/",
            len,
        )
    }

    /// Fills a pattern: `#` becomes a digit, `@` an uppercase letter or
    /// digit, anything else stays.
    pub fn fill(&mut self, pattern: &str) -> String {
        pattern
            .chars()
            .map(|c| match c {
                '#' => self.digits(1),
                '@' => self.upper(1),
                other => other.to_string(),
            })
            .collect()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Eol {
    Crlf,
    Lf,
    /// Headers in CRLF, body bytes exactly as given.
    MixedBody,
}

/// One message under construction. Header methods write the raw header and
/// record the value the golden expects. Line breaks are written as `\n` and
/// turned into the message's line ending at the end; a `\n` followed by a
/// space or tab inside a header value is a fold.
pub struct Msg {
    stem: String,
    pub rng: Rng,
    pub eol: Eol,
    head: Vec<u8>,
    body: Vec<u8>,
    pub g: Extraction,
    list_signal: bool,
}

pub const TO: &str = "Sam Rivera <sam@inbox.test>";

impl Msg {
    pub fn header(&mut self, name: &str, value: &str) -> &mut Self {
        self.header_bytes(name, value.as_bytes())
    }

    pub fn header_bytes(&mut self, name: &str, value: &[u8]) -> &mut Self {
        self.head.extend_from_slice(name.as_bytes());
        self.head.extend_from_slice(b": ");
        self.head.extend_from_slice(value);
        self.head.push(b'\n');
        self
    }

    pub fn return_path(&mut self, addr: &str) -> &mut Self {
        self.header("Return-Path", &format!("<{addr}>"))
    }

    /// `From: Display <addr>`, quoting the display name when it needs it.
    pub fn sender(&mut self, display: &str, addr: &str) -> &mut Self {
        let needs_quotes = display.chars().any(|c| "()<>[]:;@\\,.\"".contains(c));
        let raw = if needs_quotes {
            format!("\"{display}\" <{addr}>")
        } else {
            format!("{display} <{addr}>")
        };
        self.sender_raw(&raw, Some(display), addr)
    }

    pub fn sender_raw(&mut self, raw: &str, name: Option<&str>, addr: &str) -> &mut Self {
        self.header("From", raw);
        self.g.from_name = name.map(str::to_owned);
        self.g.from_address = Some(addr.to_lowercase());
        self
    }

    pub fn to(&mut self) -> &mut Self {
        self.header("To", TO)
    }

    pub fn subject(&mut self, subject: &str) -> &mut Self {
        self.subject_raw(subject, subject)
    }

    pub fn subject_raw(&mut self, raw: &str, decoded: &str) -> &mut Self {
        self.header("Subject", raw);
        self.g.subject = Some(decoded.to_owned());
        self
    }

    pub fn date(&mut self, dt: Dt) -> &mut Self {
        self.date_raw(&dt.header(), Some(&dt.utc()))
    }

    pub fn date_raw(&mut self, raw: &str, utc: Option<&str>) -> &mut Self {
        self.header("Date", raw);
        self.g.date = utc.map(str::to_owned);
        self
    }

    /// A fresh `Message-ID` on `domain`.
    pub fn message_id(&mut self, domain: &str) -> &mut Self {
        let id = format!("{}.{}@{domain}", self.rng.hex(16), self.rng.hex(8));
        self.message_id_is(&id)
    }

    pub fn message_id_is(&mut self, id: &str) -> &mut Self {
        self.header("Message-ID", &format!("<{id}>"));
        self.g.message_id = Some(id.to_owned());
        self
    }

    pub fn list_unsubscribe(&mut self, raw: &str) -> &mut Self {
        self.header("List-Unsubscribe", raw);
        self.g.list_unsubscribe = Some(raw.replace('\n', "").trim().to_owned());
        self.list_signal = true;
        self
    }

    pub fn list_unsubscribe_post(&mut self, raw: &str) -> &mut Self {
        self.header("List-Unsubscribe-Post", raw);
        self.g.list_unsubscribe_post = raw.trim() == "List-Unsubscribe=One-Click";
        self
    }

    pub fn list_id(&mut self, raw: &str, id: &str) -> &mut Self {
        self.header("List-Id", raw);
        self.g.list_id = Some(id.to_owned());
        self.list_signal = true;
        self
    }

    pub fn precedence(&mut self, value: &str) -> &mut Self {
        self.header("Precedence", value);
        if value.eq_ignore_ascii_case("bulk") || value.eq_ignore_ascii_case("list") {
            self.list_signal = true;
        }
        self
    }

    /// A `DKIM-Signature` with realistic tags and fake hashes. `d` is written
    /// as given and recorded lowercased.
    pub fn dkim(&mut self, d: &str, signed: &str) -> &mut Self {
        let selector =
            ["s1", "k1", "selector1", "20230601", "pm", "smtpapi"][self.rng.below(6) as usize];
        let t = 1_767_000_000 + self.rng.below(20_000_000);
        let bh = format!("{}=", self.rng.base64ish(43));
        let b = self.rng.base64ish(344);
        let b_folded = b
            .as_bytes()
            .chunks(72)
            .map(|c| std::str::from_utf8(c).expect("ascii"))
            .collect::<Vec<_>>()
            .join("\n\t ");
        let value = format!(
            "v=1; a=rsa-sha256; c=relaxed/relaxed; d={d};\n\ts={selector}; t={t};\n\th={signed};\n\tbh={bh};\n\tb={b_folded}"
        );
        self.header("DKIM-Signature", &value);
        self.g.dkim_domains.push(d.to_lowercase());
        self
    }

    pub fn receipt(&mut self, receipt: Receipt) -> &mut Self {
        self.g.receipt = Some(receipt);
        self
    }

    pub fn boundary(&mut self) -> String {
        format!("----=_Part_{}_{}", self.rng.digits(6), self.rng.hex(12))
    }

    /// Adds `MIME-Version` and the part's own headers, and sets the body.
    pub fn body(&mut self, part: Part) -> &mut Self {
        self.header("MIME-Version", "1.0");
        self.body_raw(&part.headers, part.body)
    }

    /// Header lines written verbatim, then the body bytes verbatim.
    pub fn body_raw(&mut self, headers: &[String], body: Vec<u8>) -> &mut Self {
        for line in headers {
            self.head.extend_from_slice(line.as_bytes());
            self.head.push(b'\n');
        }
        self.body = body;
        self
    }

    fn finish(mut self) -> (String, Vec<u8>, Extraction) {
        self.g.is_list = self.list_signal && self.g.receipt.is_none();
        let mut raw = self.head;
        raw.push(b'\n');
        raw.extend_from_slice(&self.body);
        let raw = match self.eol {
            Eol::Crlf => crlf(&raw),
            Eol::Lf => raw,
            Eol::MixedBody => {
                let mut mixed = crlf(&raw[..raw.len() - self.body.len()]);
                mixed.extend_from_slice(&self.body);
                mixed
            }
        };
        (self.stem, raw, self.g)
    }
}

fn crlf(lf: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(lf.len() + lf.len() / 40);
    for &byte in lf {
        if byte == b'\n' {
            out.push(b'\r');
        }
        out.push(byte);
    }
    out
}

/// A MIME entity: its header lines and its encoded body.
pub struct Part {
    pub headers: Vec<String>,
    pub body: Vec<u8>,
}

#[derive(Clone, Copy)]
pub enum Cte {
    SevenBit,
    EightBit,
    Qp,
    Base64,
}

impl Cte {
    fn name(self) -> &'static str {
        match self {
            Cte::SevenBit => "7bit",
            Cte::EightBit => "8bit",
            Cte::Qp => "quoted-printable",
            Cte::Base64 => "base64",
        }
    }

    fn encode(self, content: &[u8]) -> Vec<u8> {
        match self {
            Cte::SevenBit | Cte::EightBit => {
                let mut body = content.to_vec();
                if !body.ends_with(b"\n") && !body.is_empty() {
                    body.push(b'\n');
                }
                body
            }
            Cte::Qp => {
                let mut text = content.to_vec();
                if !text.ends_with(b"\n") && !text.is_empty() {
                    text.push(b'\n');
                }
                qp(&text).into_bytes()
            }
            Cte::Base64 => b64(content, Some(76), true).into_bytes(),
        }
    }
}

pub fn leaf(content_type: &str, cte: Cte, content: &[u8]) -> Part {
    Part {
        headers: vec![
            format!("Content-Type: {content_type}"),
            format!("Content-Transfer-Encoding: {}", cte.name()),
        ],
        body: cte.encode(content),
    }
}

pub fn text(content: &str, cte: Cte) -> Part {
    leaf("text/plain; charset=utf-8", cte, content.as_bytes())
}

pub fn html(content: &str, cte: Cte) -> Part {
    leaf("text/html; charset=utf-8", cte, content.as_bytes())
}

pub fn multipart(subtype: &str, boundary: &str, parts: Vec<Part>) -> Part {
    Multipart {
        subtype,
        boundary,
        preamble: None,
        epilogue: None,
        close: true,
    }
    .build(parts)
}

pub struct Multipart<'a> {
    pub subtype: &'a str,
    pub boundary: &'a str,
    pub preamble: Option<&'a str>,
    pub epilogue: Option<&'a str>,
    /// False drops the closing delimiter.
    pub close: bool,
}

impl Multipart<'_> {
    pub fn build(self, parts: Vec<Part>) -> Part {
        let mut body = Vec::new();
        if let Some(preamble) = self.preamble {
            body.extend_from_slice(preamble.as_bytes());
            body.push(b'\n');
        }
        for part in parts {
            body.extend_from_slice(format!("--{}\n", self.boundary).as_bytes());
            for line in &part.headers {
                body.extend_from_slice(line.as_bytes());
                body.push(b'\n');
            }
            body.push(b'\n');
            body.extend_from_slice(&part.body);
            if !part.body.ends_with(b"\n") {
                body.push(b'\n');
            }
        }
        if self.close {
            body.extend_from_slice(format!("--{}--\n", self.boundary).as_bytes());
        }
        if let Some(epilogue) = self.epilogue {
            body.extend_from_slice(epilogue.as_bytes());
            body.push(b'\n');
        }
        Part {
            headers: vec![format!(
                "Content-Type: multipart/{};\n boundary=\"{}\"",
                self.subtype, self.boundary
            )],
            body,
        }
    }
}

/// Quoted-printable (RFC 2045 6.7), keeping `\n` as the hard line break.
pub fn qp(input: &[u8]) -> String {
    let mut out = String::new();
    for (i, line) in input.split(|&b| b == b'\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let mut len = 0;
        for (j, &byte) in line.iter().enumerate() {
            let last = j + 1 == line.len();
            let literal = ((33..=126).contains(&byte) && byte != b'=')
                || ((byte == b' ' || byte == b'\t') && !last);
            let token = if literal {
                (byte as char).to_string()
            } else {
                format!("={byte:02X}")
            };
            if len + token.len() > 75 {
                out.push_str("=\n");
                len = 0;
            }
            out.push_str(&token);
            len += token.len();
        }
    }
    out
}

pub fn b64(input: &[u8], wrap: Option<usize>, pad: bool) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut flat = String::new();
    for chunk in input.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |acc, (i, &b)| acc | u32::from(b) << (16 - 8 * i));
        for i in 0..=chunk.len() {
            flat.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
        if pad {
            for _ in chunk.len()..3 {
                flat.push('=');
            }
        }
    }
    match wrap {
        None => flat,
        Some(width) => {
            let mut out = String::new();
            for line in flat.as_bytes().chunks(width) {
                out.push_str(std::str::from_utf8(line).expect("ascii"));
                out.push('\n');
            }
            out
        }
    }
}

/// A local wall-clock time with its UTC offset in minutes.
#[derive(Clone, Copy, Debug)]
pub struct Dt {
    pub y: i64,
    pub mo: u32,
    pub d: u32,
    pub h: u32,
    pub mi: u32,
    pub s: u32,
    pub off: i32,
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const WEEKDAYS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];

impl Dt {
    pub fn new(y: i64, mo: u32, d: u32, h: u32, mi: u32, off: i32) -> Self {
        Dt {
            y,
            mo,
            d,
            h,
            mi,
            s: 0,
            off,
        }
    }

    pub fn days(&self) -> i64 {
        days_from_civil(self.y, self.mo, self.d)
    }

    pub fn weekday(&self) -> &'static str {
        WEEKDAYS[self.days().rem_euclid(7) as usize]
    }

    pub fn month_name(&self) -> &'static str {
        MONTHS[self.mo as usize - 1]
    }

    /// `Sun, 01 Mar 2026 09:30:00 +0000`.
    pub fn header(&self) -> String {
        format!(
            "{}, {:02} {} {} {:02}:{:02}:{:02} {}",
            self.weekday(),
            self.d,
            self.month_name(),
            self.y,
            self.h,
            self.mi,
            self.s,
            self.zone()
        )
    }

    pub fn zone(&self) -> String {
        let sign = if self.off < 0 { '-' } else { '+' };
        let abs = self.off.abs();
        format!("{sign}{:02}{:02}", abs / 60, abs % 60)
    }

    /// RFC 3339 in UTC with seconds and `Z`.
    pub fn utc(&self) -> String {
        let secs = self.days() * 86_400
            + i64::from(self.h) * 3600
            + i64::from(self.mi) * 60
            + i64::from(self.s)
            - i64::from(self.off) * 60;
        let (y, mo, d) = civil_from_days(secs.div_euclid(86_400));
        let rem = secs.rem_euclid(86_400);
        format!(
            "{y:04}-{mo:02}-{d:02}T{:02}:{:02}:{:02}Z",
            rem / 3600,
            rem % 3600 / 60,
            rem % 60
        )
    }

    pub fn plus_days(&self, n: i64) -> Self {
        let (y, mo, d) = civil_from_days(self.days() + n);
        Dt { y, mo, d, ..*self }
    }

    /// Same day of month `n` months on, clamped to the month's length.
    pub fn plus_months(&self, n: i64) -> Self {
        let index = self.y * 12 + i64::from(self.mo) - 1 + n;
        let y = index.div_euclid(12);
        let mo = (index.rem_euclid(12) + 1) as u32;
        let (ny, nmo) = if mo == 12 { (y + 1, 1) } else { (y, mo + 1) };
        let month_len = (days_from_civil(ny, nmo, 1) - days_from_civil(y, mo, 1)) as u32;
        Dt {
            y,
            mo,
            d: self.d.min(month_len),
            ..*self
        }
    }

    /// `Jan 1, 2026`.
    pub fn words(&self) -> String {
        format!("{} {}, {}", self.month_name(), self.d, self.y)
    }
}

/// Days since 1970-01-01 (Howard Hinnant's algorithm).
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(m);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// A small, valid PDF with one page of text in an uncompressed stream.
pub fn tiny_pdf(lines: &[&str]) -> Vec<u8> {
    let mut content = String::from("BT /F1 11 Tf 72 720 Td 14 TL\n");
    for line in lines {
        let escaped = line
            .replace('\\', "\\\\")
            .replace('(', "\\(")
            .replace(')', "\\)");
        content.push_str(&format!("({escaped}) '\n"));
    }
    content.push_str("ET\n");
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>".to_owned(),
        format!("<< /Length {} >>\nstream\n{content}endstream", content.len()),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
    ];
    let mut pdf = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut offsets = Vec::new();
    for (i, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", i + 1).as_bytes());
    }
    let xref = pdf.len();
    pdf.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    pdf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_convert_to_utc() {
        assert_eq!(
            Dt::new(2026, 3, 1, 9, 30, 0).header(),
            "Sun, 01 Mar 2026 09:30:00 +0000"
        );
        assert_eq!(
            Dt::new(2026, 3, 1, 1, 30, 120).utc(),
            "2026-02-28T23:30:00Z"
        );
        assert_eq!(
            Dt::new(2025, 12, 31, 20, 0, -300).utc(),
            "2026-01-01T01:00:00Z"
        );
        assert_eq!(Dt::new(2026, 1, 31, 0, 0, 0).plus_months(1).d, 28);
        assert_eq!(Dt::new(2028, 1, 31, 0, 0, 0).plus_months(1).d, 29);
        assert_eq!(Dt::new(2026, 11, 15, 0, 0, 0).plus_months(3).y, 2027);
    }

    #[test]
    fn encoders_match_the_rfcs() {
        assert_eq!(b64(b"Hello world", None, true), "SGVsbG8gd29ybGQ=");
        assert_eq!(b64(b"Hello world", None, false), "SGVsbG8gd29ybGQ");
        assert_eq!(
            qp("Total: 12,00 €\n".as_bytes()),
            "Total: 12,00 =E2=82=AC\n"
        );
        assert_eq!(qp(b"a = b \n"), "a =3D b=20\n");
    }
}
