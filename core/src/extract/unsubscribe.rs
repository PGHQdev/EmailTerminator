//! The `List-Unsubscribe` grammar (RFC 2369) and the RFC 8058 one-click
//! conditions. No library implements either (PLAN.md 1.1).

use mail_parser::{HeaderName, Message};

/// The URIs of an unfolded `List-Unsubscribe` value, in the sender's order.
///
/// RFC 2369 wants each URI in angle brackets, separated by commas, and says
/// whitespace inside the brackets is to be ignored, which is how a URI folded
/// across lines is put back together. Real senders also omit the brackets, so
/// a value without them is read as a comma-separated list.
pub fn uris(value: &str) -> Vec<String> {
    let clean = |uri: &str| -> Option<String> {
        let uri: String = uri.split_whitespace().collect();
        (!uri.is_empty()).then_some(uri)
    };
    if !value.contains('<') {
        return value.split(',').filter_map(clean).collect();
    }
    let mut found = Vec::new();
    let mut rest = value;
    while let Some(open) = rest.find('<') {
        let Some(close) = rest[open..].find('>') else {
            break;
        };
        found.extend(clean(&rest[open + 1..open + close]));
        rest = &rest[open + close + 1..];
    }
    found
}

/// The first URI with this scheme, compared without case.
pub fn first<'a>(uris: &'a [String], scheme: &str) -> Option<&'a str> {
    uris.iter()
        .find(|uri| {
            uri.split_once(':')
                .is_some_and(|(s, _)| s.eq_ignore_ascii_case(scheme))
        })
        .map(String::as_str)
}

/// RFC 8058 §3.1: an HTTPS URI, `List-Unsubscribe-Post` with exactly
/// `List-Unsubscribe=One-Click`, and a valid DKIM signature that covers both
/// headers.
///
/// The app does not verify signatures itself, because that needs a DNS
/// lookup per sender. It trusts the receiving server's verdict: the first
/// `Authentication-Results` header, which the server adds on top, must report
/// `dkim=pass` for the domain of a signature whose `h=` names both headers.
pub(super) fn one_click(msg: &Message<'_>, list_unsubscribe: Option<&str>, post: bool) -> bool {
    let has_https = list_unsubscribe.is_some_and(|v| first(&uris(v), "https").is_some());
    if !(post && has_https) {
        return false;
    }
    let passed = passed_dkim(msg);
    covering_signers(msg).iter().any(|d| passed.contains(d))
}

/// The `d=` domain of every DKIM signature whose `h=` lists both
/// `List-Unsubscribe` and `List-Unsubscribe-Post`.
fn covering_signers(msg: &Message<'_>) -> Vec<String> {
    let raw = msg.raw_message();
    msg.headers()
        .iter()
        .filter(|h| h.name == HeaderName::DkimSignature)
        .filter_map(|h| {
            let bytes = raw.get(h.offset_start as usize..h.offset_end as usize)?;
            let value: String = String::from_utf8_lossy(bytes).split_whitespace().collect();
            let tag = |name: &str| {
                value.split(';').find_map(|tag| {
                    let (key, val) = tag.split_once('=')?;
                    (key == name).then(|| val.to_ascii_lowercase())
                })
            };
            let signed = tag("h")?;
            let signed: Vec<&str> = signed.split(':').collect();
            (signed.contains(&"list-unsubscribe") && signed.contains(&"list-unsubscribe-post"))
                .then(|| tag("d"))
                .flatten()
        })
        .collect()
}

/// Domains the first `Authentication-Results` header reports `dkim=pass`
/// for, from `header.d` or the domain of `header.i`.
fn passed_dkim(msg: &Message<'_>) -> Vec<String> {
    // Not `headers::raw`: mail-parser returns the last copy of a repeated
    // header, and only the first is the receiving server's.
    let raw = msg.raw_message();
    let Some(value) = msg
        .headers()
        .iter()
        .find(|h| {
            h.name
                .as_str()
                .eq_ignore_ascii_case("Authentication-Results")
        })
        .and_then(|h| raw.get(h.offset_start as usize..h.offset_end as usize))
    else {
        return Vec::new();
    };
    let value = without_comments(&String::from_utf8_lossy(value)).to_ascii_lowercase();
    value
        .split(';')
        .skip(1)
        .filter(|result| {
            result
                .split_whitespace()
                .next()
                .is_some_and(|m| m == "dkim=pass")
        })
        .filter_map(|result| {
            result.split_whitespace().find_map(|prop| {
                if let Some(d) = prop.strip_prefix("header.d=") {
                    Some(d.to_owned())
                } else {
                    prop.strip_prefix("header.i=")
                        .map(|i| i.rsplit_once('@').map_or(i, |(_, d)| d).to_owned())
                }
            })
        })
        .collect()
}

/// RFC 5322 comments, which `Authentication-Results` uses for free text.
fn without_comments(value: &str) -> String {
    let mut depth = 0usize;
    value
        .chars()
        .filter(|c| match c {
            '(' => {
                depth += 1;
                false
            }
            ')' => {
                depth = depth.saturating_sub(1);
                false
            }
            _ => depth == 0,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mail_parser::MessageParser;

    #[test]
    fn bracketed_uris_keep_their_order() {
        assert_eq!(
            uris("<https://a.test/u>, <mailto:u@a.test?subject=unsubscribe>"),
            vec!["https://a.test/u", "mailto:u@a.test?subject=unsubscribe"]
        );
        assert_eq!(uris("<mailto:a@x.test>,<mailto:b@x.test>").len(), 2);
    }

    #[test]
    fn a_fold_inside_the_brackets_is_removed() {
        assert_eq!(
            uris("<https://a.test/u?list=w& user=8a>"),
            vec!["https://a.test/u?list=w&user=8a"]
        );
    }

    #[test]
    fn missing_brackets_still_read_as_a_list() {
        assert_eq!(
            uris("https://a.test/u, mailto:u@a.test"),
            vec!["https://a.test/u", "mailto:u@a.test"]
        );
    }

    #[test]
    fn comments_and_an_unclosed_bracket_are_ignored() {
        assert_eq!(
            uris("(web) <https://a.test/u>, <mailto:"),
            vec!["https://a.test/u"]
        );
    }

    #[test]
    fn schemes_match_without_case() {
        let found = uris("<MAILTO:u@a.test>, <HTTPS://a.test/u>");
        assert_eq!(first(&found, "https"), Some("HTTPS://a.test/u"));
        assert_eq!(first(&found, "mailto"), Some("MAILTO:u@a.test"));
        assert_eq!(first(&found, "http"), None);
    }

    fn message(results: &str, signed: &str) -> Vec<u8> {
        format!(
            "Authentication-Results: mx.inbox.test;\r\n dkim=pass (2048-bit key) header.d=news.test header.s=k1;\r\n spf=pass\r\n\
Authentication-Results: {results}\r\n\
DKIM-Signature: v=1; a=rsa-sha256; d=news.test; s=k1;\r\n h={signed}; b=abc\r\n\
From: News <news@news.test>\r\n\
List-Unsubscribe: <https://news.test/u/1>\r\n\
List-Unsubscribe-Post: List-Unsubscribe=One-Click\r\n\r\nHi\r\n"
        )
        .into_bytes()
    }

    fn check(raw: &[u8]) -> bool {
        let msg = MessageParser::default().parse(raw).unwrap();
        one_click(&msg, Some("<https://news.test/u/1>"), true)
    }

    #[test]
    fn a_passing_signature_over_both_headers_qualifies() {
        assert!(check(&message(
            "forged; dkim=fail",
            "from:list-unsubscribe:list-unsubscribe-post"
        )));
    }

    #[test]
    fn a_signature_that_skips_the_post_header_does_not() {
        assert!(!check(&message(
            "x; dkim=pass header.d=news.test",
            "from:list-unsubscribe"
        )));
    }

    #[test]
    fn only_the_first_results_header_is_trusted() {
        let raw = message("x", "from:list-unsubscribe:list-unsubscribe-post");
        let raw =
            String::from_utf8(raw)
                .unwrap()
                .replacen("dkim=pass (2048-bit key)", "dkim=fail", 1);
        assert!(!check(raw.as_bytes()));
    }

    #[test]
    fn header_i_names_the_domain_too() {
        let raw = String::from_utf8(message("x", "list-unsubscribe:list-unsubscribe-post"))
            .unwrap()
            .replacen("header.d=news.test", "header.i=@news.test", 1);
        assert!(check(raw.as_bytes()));
    }
}
