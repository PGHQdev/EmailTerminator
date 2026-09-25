//! Regression fixtures for the two mail-parser defects on the parse path
//! (PLAN.md M1). An upgrade that brings either back fails here.

use mail_parser::MessageParser;

/// Upstream #155: a trailing folded `Received` line panicked. Fixed in 0.11.9.
#[test]
fn a_trailing_folded_received_line_does_not_panic() {
    for input in [
        &b"Received:\n\t"[..],
        b"Received: from x (y)\r\n\t",
        b"Received: from x by y;\r\n\t",
    ] {
        let _ = MessageParser::default().parse(input);
    }
}

/// Upstream #156: a boundary string inside a line truncated the part.
/// Carried as a patch in vendor/mail-parser.
#[test]
fn a_boundary_inside_a_line_does_not_end_the_part() {
    let raw: &[u8] = b"Content-Type: multipart/mixed; boundary=\"BND\"\r\n\
\r\n\
--BND\r\n\
Content-Type: text/plain\r\n\
\r\n\
visit --BND for details\r\n\
SECRET\r\n\
--BND--\r\n";
    let msg = MessageParser::default().parse(raw).unwrap();
    assert_eq!(
        msg.body_text(0).as_deref(),
        Some("visit --BND for details\r\nSECRET")
    );
}

#[test]
fn a_boundary_inside_a_quoted_printable_line_does_not_end_the_part() {
    let raw: &[u8] = b"Content-Type: multipart/mixed; boundary=\"BND\"\r\n\
\r\n\
--BND\r\n\
Content-Type: text/plain\r\n\
Content-Transfer-Encoding: quoted-printable\r\n\
\r\n\
Total due: $12=2E00 --BND not a delimiter\r\n\
Thanks\r\n\
--BND--\r\n";
    let msg = MessageParser::default().parse(raw).unwrap();
    assert_eq!(
        msg.body_text(0).as_deref(),
        Some("Total due: $12.00 --BND not a delimiter\r\nThanks")
    );
}

#[test]
fn a_boundary_at_line_start_still_ends_the_part() {
    let raw: &[u8] = b"Content-Type: multipart/alternative; boundary=\"BND\"\r\n\
\r\n\
--BND\r\n\
Content-Type: text/plain\r\n\
\r\n\
plain\r\n\
--BND\r\n\
Content-Type: text/html\r\n\
\r\n\
<p>html</p>\r\n\
--BND--\r\n";
    let msg = MessageParser::default().parse(raw).unwrap();
    assert_eq!(msg.body_text(0).as_deref(), Some("plain"));
    assert_eq!(msg.body_html(0).as_deref(), Some("<p>html</p>"));
}
