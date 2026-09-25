//! RFC 2047 encoded words in an unstructured header, decoded the way real mail
//! needs: adjacent words in one charset join before decoding, so a multi-byte
//! character split across two words survives, and base64 without its padding
//! still decodes.

use std::sync::LazyLock;

use mail_parser::decoders::charsets::map::charset_decoder;
use regex::Regex;

static WORD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"=\?([^?\s]+)\?([QqBb])\?([^?\s]*)\?=").expect("encoded-word pattern")
});

pub(super) fn decode(raw: &str) -> String {
    let mut out = String::new();
    // Bytes of consecutive words in one charset, not yet decoded.
    let mut pending: Option<(String, Vec<u8>)> = None;
    let mut last = 0;
    for caps in WORD.captures_iter(raw) {
        let whole = caps.get(0).expect("match");
        let between = &raw[last..whole.start()];
        let charset = caps[1].split('*').next().unwrap_or("").to_ascii_lowercase();
        let bytes = match &caps[2] {
            "Q" | "q" => q_decode(&caps[3]),
            _ => b_decode(&caps[3]),
        };
        // Whitespace between two encoded words is dropped (RFC 2047 6.2).
        let adjacent = pending.is_some() && between.trim().is_empty();
        match &mut pending {
            Some((open, buffer)) if adjacent && *open == charset => buffer.extend(bytes),
            _ => {
                flush(&mut out, pending.take());
                if !adjacent {
                    out.push_str(between);
                }
                pending = Some((charset, bytes));
            }
        }
        last = whole.end();
    }
    flush(&mut out, pending);
    out.push_str(&raw[last..]);
    out
}

fn flush(out: &mut String, pending: Option<(String, Vec<u8>)>) {
    if let Some((charset, bytes)) = pending {
        match charset_decoder(charset.as_bytes()) {
            Some(decoder) => out.push_str(&decoder(&bytes)),
            None => out.push_str(&String::from_utf8_lossy(&bytes)),
        }
    }
}

fn q_decode(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let hex = |b: u8| (b as char).to_digit(16).map(|d| d as u8);
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'_' => out.push(b' '),
            b'=' if i + 2 < bytes.len() => match (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                (Some(high), Some(low)) => {
                    out.push(high << 4 | low);
                    i += 2;
                }
                _ => out.push(b'='),
            },
            byte => out.push(byte),
        }
        i += 1;
    }
    out
}

/// Base64 that tolerates missing or broken padding: every complete group of
/// six bits is kept.
fn b_decode(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in text.bytes() {
        let value = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => continue,
        };
        acc = (acc << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_passes_through() {
        assert_eq!(decode("Your receipt"), "Your receipt");
    }

    #[test]
    fn adjacent_words_join_and_drop_the_space_between() {
        assert_eq!(
            decode("=?utf-8?Q?Your_order?= =?utf-8?Q?_has_shipped?="),
            "Your order has shipped"
        );
    }

    #[test]
    fn a_character_split_across_two_words_survives() {
        assert_eq!(
            decode("=?utf-8?Q?Caf=C3?= =?utf-8?Q?=A9_cr=C3=A8me?="),
            "Café crème"
        );
        // お支払い: the second word starts inside a UTF-8 sequence.
        let raw = "=?utf-8?B?44GK5pSv5omV?= =?utf-8?B?44GE44Gu56K66KqN?=";
        assert_eq!(decode(raw), "お支払いの確認");
    }

    #[test]
    fn broken_padding_keeps_every_whole_byte() {
        assert_eq!(decode("=?utf-8?B?SGVsbG8gd29ybGQ?="), "Hello world");
    }

    #[test]
    fn a_stray_marker_stays_literal() {
        assert_eq!(decode("Save 20% =? today"), "Save 20% =? today");
    }

    #[test]
    fn legacy_charsets_decode() {
        assert_eq!(decode("=?iso-8859-1?Q?Caf=E9?="), "Café");
        assert_eq!(decode("=?koi8-r?B?8NLJ18XU?="), "Привет");
    }
}
