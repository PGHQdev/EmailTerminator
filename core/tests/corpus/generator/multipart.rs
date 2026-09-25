//! Part 9, "Multipart and transport edge cases".

use et_core::extract::{Receipt, ReceiptKind};

use super::{
    Corpus, Cte, Dt, Eol, Msg, Multipart, Part, b64, html, leaf, multipart, text, tiny_pdf,
};

/// A 1x1 transparent PNG.
const PNG: [u8; 67] = [
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
    0x42, 0x60, 0x82,
];

fn shop_headers(m: &mut Msg, subject: &str, at: Dt) {
    m.dkim("northpeak.test", "from:to:subject:date:message-id");
    m.sender("Northpeak Outfitters", "orders@northpeak.test");
    m.to();
    m.subject(subject);
    m.date(at);
    m.message_id("northpeak.test");
}

fn order_text(order: &str, total: &str) -> String {
    format!(
        "Hi Sam,\n\nThanks for your order. We'll email you when it ships.\n\nOrder number: {order}\nTrail Runner 2 (size 10): {total}\nShipping: free\nTotal: {total}\nPayment method: Visa ending in 4242\n"
    )
}

fn order_html(order: &str, total: &str, extra: &str) -> String {
    format!(
        "<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\"></head><body>\n{extra}<p>Hi Sam,</p>\n<p>Thanks for your order. We'll email you when it ships.</p>\n<table>\n<tr><td>Order number</td><td>{order}</td></tr>\n<tr><td>Trail Runner 2 (size 10)</td><td>{total}</td></tr>\n<tr><td>Shipping</td><td>free</td></tr>\n<tr><td><b>Total</b></td><td><b>{total}</b></td></tr>\n</table>\n</body></html>\n"
    )
}

fn order_receipt(minor: i64, currency: &str, order: &str) -> Receipt {
    Receipt {
        kind: ReceiptKind::Charge,
        amount_minor_units: Some(minor),
        currency: Some(currency.into()),
        merchant: None,
        invoice_ref: Some(order.into()),
    }
}

fn plain_notice(m: &mut Msg, subject: &str, at: Dt) {
    m.sender("Town Library", "notices@library.test");
    m.to();
    m.subject(subject);
    m.date(at);
    m.message_id("library.test");
}

fn png_part(cid: &str) -> Part {
    let mut part = leaf("image/png; name=\"logo.png\"", Cte::Base64, &PNG);
    part.headers.push(format!("Content-ID: <{cid}>"));
    part.headers
        .push("Content-Disposition: inline; filename=\"logo.png\"".into());
    part
}

fn pdf_part(name: &str, lines: &[&str]) -> Part {
    let mut part = leaf(
        &format!("application/pdf; name=\"{name}\""),
        Cte::Base64,
        &tiny_pdf(lines),
    );
    part.headers.push(format!(
        "Content-Disposition: attachment; filename=\"{name}\""
    ));
    part
}

pub fn generate(corpus: &mut Corpus) {
    structure(corpus);
    transport(corpus);
    headers(corpus);
    dates(corpus);
}

fn structure(corpus: &mut Corpus) {
    let mut m = corpus.msg("multipart/alternative_text_html");
    shop_headers(
        &mut m,
        "Your Northpeak order NP-100234",
        Dt::new(2026, 6, 1, 14, 0, -360),
    );
    let boundary = m.boundary();
    m.body(multipart(
        "alternative",
        &boundary,
        vec![
            text(&order_text("NP-100234", "$84.50"), Cte::Qp),
            html(&order_html("NP-100234", "$84.50", ""), Cte::Qp),
        ],
    ));
    m.receipt(order_receipt(8450, "USD", "NP-100234"));
    corpus.put(m);

    let mut m = corpus.msg("multipart/related_cid_images");
    m.dkim(
        "news.northpeak.test",
        "from:to:subject:date:message-id:list-unsubscribe",
    );
    m.sender("Northpeak Outfitters", "news@news.northpeak.test");
    m.to();
    m.subject("New trail shoes are in");
    m.date(Dt::new(2026, 6, 2, 10, 0, -360));
    m.message_id("news.northpeak.test");
    m.list_unsubscribe("<https://news.northpeak.test/unsubscribe/8a1f2c>");
    let outer = m.boundary();
    let inner = m.boundary();
    m.body(multipart(
        "related",
        &outer,
        vec![
            multipart(
                "alternative",
                &inner,
                vec![
                    text("Hi Sam,\n\nNew trail shoes are in. See them on our site.\n", Cte::SevenBit),
                    html(
                        "<!DOCTYPE html>\n<html><body>\n<img src=\"cid:logo@northpeak.test\" alt=\"Northpeak\">\n<p>Hi Sam,</p>\n<p>New trail shoes are in.</p>\n<img src=\"cid:hero@northpeak.test\" alt=\"\">\n</body></html>\n",
                        Cte::SevenBit,
                    ),
                ],
            ),
            png_part("logo@northpeak.test"),
            png_part("hero@northpeak.test"),
        ],
    ));
    corpus.put(m);

    let mut m = corpus.msg("multipart/mixed_pdf_invoice");
    m.dkim("fieldnote.test", "from:to:subject:date:message-id");
    m.sender("Fieldnote Accounting", "invoices@fieldnote.test");
    m.to();
    m.subject("Invoice FN-2026-0042 (paid)");
    m.date(Dt::new(2026, 6, 3, 9, 0, 0));
    m.message_id("fieldnote.test");
    let boundary = m.boundary();
    m.body(multipart(
        "mixed",
        &boundary,
        vec![
            text(
                "Hi Sam,\n\nThanks for your payment. The invoice is attached.\n\nInvoice: FN-2026-0042\nAmount paid: $240.00\nPaid on: Jun 3, 2026\n",
                Cte::SevenBit,
            ),
            pdf_part(
                "FN-2026-0042.pdf",
                &["Fieldnote Accounting", "Invoice FN-2026-0042", "Amount paid: $240.00"],
            ),
        ],
    ));
    m.receipt(order_receipt(24000, "USD", "FN-2026-0042"));
    corpus.put(m);

    // alternative inside mixed inside related.
    let mut m = corpus.msg("multipart/alternative_in_mixed_in_related");
    shop_headers(
        &mut m,
        "Your Northpeak order NP-100251",
        Dt::new(2026, 6, 4, 14, 0, -360),
    );
    let (b1, b2, b3) = (m.boundary(), m.boundary(), m.boundary());
    m.body(multipart(
        "related",
        &b1,
        vec![
            multipart(
                "mixed",
                &b2,
                vec![
                    multipart(
                        "alternative",
                        &b3,
                        vec![
                            text(&order_text("NP-100251", "$129.00"), Cte::Qp),
                            html(
                                &order_html(
                                    "NP-100251",
                                    "$129.00",
                                    "<img src=\"cid:logo@northpeak.test\" alt=\"Northpeak\">\n",
                                ),
                                Cte::Qp,
                            ),
                        ],
                    ),
                    pdf_part("NP-100251.pdf", &["Order NP-100251", "Total: $129.00"]),
                ],
            ),
            png_part("logo@northpeak.test"),
        ],
    ));
    m.receipt(order_receipt(12900, "USD", "NP-100251"));
    corpus.put(m);

    let mut m = corpus.msg("multipart/missing_closing_boundary");
    shop_headers(
        &mut m,
        "Your Northpeak order NP-100260",
        Dt::new(2026, 6, 5, 14, 0, -360),
    );
    let boundary = m.boundary();
    m.body(
        Multipart {
            subtype: "alternative",
            boundary: &boundary,
            preamble: None,
            epilogue: None,
            close: false,
        }
        .build(vec![
            text(&order_text("NP-100260", "$42.00"), Cte::SevenBit),
            html(&order_html("NP-100260", "$42.00", ""), Cte::SevenBit),
        ]),
    );
    m.receipt(order_receipt(4200, "USD", "NP-100260"));
    corpus.put(m);

    // The boundary string appears inside body lines; only a line that starts
    // with it is a delimiter, so the total after it must survive.
    let mut m = corpus.msg("multipart/boundary_in_body_text");
    shop_headers(
        &mut m,
        "Your Northpeak order NP-100277",
        Dt::new(2026, 6, 6, 14, 0, -360),
    );
    m.body(multipart(
        "mixed",
        "NPB",
        vec![text(
            "Hi Sam,\n\nThanks for your order. Reference: see --NPB in your account.\n\nOrder number: NP-100277\nTotal: $19.00\n",
            Cte::SevenBit,
        )],
    ));
    m.receipt(order_receipt(1900, "USD", "NP-100277"));
    corpus.put(m);

    let mut m = corpus.msg("multipart/preamble_and_epilogue");
    plain_notice(
        &mut m,
        "Your hold is ready",
        Dt::new(2026, 6, 7, 9, 0, -360),
    );
    let boundary = m.boundary();
    m.body(
        Multipart {
            subtype: "mixed",
            boundary: &boundary,
            preamble: Some("This is a multi-part message in MIME format.\nIf you can read this, your mail client does not support MIME."),
            epilogue: Some("This epilogue is not part of any body part.\nTotal: $999.00 (ignore me)"),
            close: true,
        }
        .build(vec![text(
            "Hi Sam,\n\nThe book you reserved is ready at the front desk.\n",
            Cte::SevenBit,
        )]),
    );
    corpus.put(m);

    let mut m = corpus.msg("multipart/html_only");
    shop_headers(
        &mut m,
        "Your Northpeak order NP-100290",
        Dt::new(2026, 6, 8, 14, 0, -360),
    );
    m.body(html(&order_html("NP-100290", "$30.00", ""), Cte::Qp));
    m.receipt(order_receipt(3000, "USD", "NP-100290"));
    corpus.put(m);
}

fn transport(corpus: &mut Corpus) {
    for (stem, eol) in [
        ("multipart/line_endings_crlf", Eol::Crlf),
        ("multipart/line_endings_lf", Eol::Lf),
    ] {
        let mut m = corpus.msg(stem);
        m.eol = eol;
        plain_notice(
            &mut m,
            "Library hours this week",
            Dt::new(2026, 6, 9, 9, 0, 0),
        );
        m.body(text(
            "Hi Sam,\n\nWe open late on Thursday.\n",
            Cte::SevenBit,
        ));
        corpus.put(m);
    }

    // Headers in CRLF, body lines alternating LF and CRLF.
    let mut m = corpus.msg("multipart/line_endings_mixed");
    m.eol = Eol::MixedBody;
    plain_notice(
        &mut m,
        "Library closed on Monday",
        Dt::new(2026, 6, 10, 9, 0, 0),
    );
    m.body_raw(
        &["Content-Type: text/plain; charset=us-ascii".into()],
        b"Hi Sam,\n\r\nThe library is closed on Monday for a holiday.\nWe reopen on Tuesday.\r\n"
            .to_vec(),
    );
    corpus.put(m);

    let mut m = corpus.msg("multipart/8bit_declared_as_7bit");
    m.dkim("pagewise.test", "from:to:subject:date:message-id");
    m.sender("Pagewise", "receipts@pagewise.test");
    m.to();
    m.subject("Your Pagewise receipt");
    m.date(Dt::new(2026, 6, 11, 10, 0, 120));
    m.message_id("pagewise.test");
    m.body(Part {
        headers: vec![
            "Content-Type: text/plain; charset=utf-8".into(),
            "Content-Transfer-Encoding: 7bit".into(),
        ],
        body: "Hallo Sam,\n\nthanks for your payment for Pagewise Plus.\n\nInvoice number: PW-770012\nTotal: 12,00 €\n"
            .as_bytes()
            .to_vec(),
    });
    m.receipt(order_receipt(1200, "EUR", "PW-770012"));
    corpus.put(m);

    // Declares iso-8859-1 but carries UTF-8 bytes.
    let mut m = corpus.msg("multipart/charset_mismatch");
    plain_notice(
        &mut m,
        "Greetings from the reading group",
        Dt::new(2026, 6, 12, 9, 0, 0),
    );
    m.body(Part {
        headers: vec![
            "Content-Type: text/plain; charset=iso-8859-1".into(),
            "Content-Transfer-Encoding: 8bit".into(),
        ],
        body: "Grüße, Sam! The reading group meets at the café on Friday.\n"
            .as_bytes()
            .to_vec(),
    });
    corpus.put(m);

    let mut m = corpus.msg("multipart/content_type_without_charset");
    plain_notice(
        &mut m,
        "Your card was renewed",
        Dt::new(2026, 6, 13, 9, 0, 0),
    );
    m.body(Part {
        headers: vec!["Content-Type: text/plain".into()],
        body: b"Hi Sam,\n\nYour library card is renewed until June 2028.\n".to_vec(),
    });
    corpus.put(m);

    let content = order_text("NP-100301", "$64.00");
    for (stem, order, wrap, pad) in [
        ("multipart/base64_no_padding", "NP-100301", Some(76), false),
        (
            "multipart/base64_wrapped_short_lines",
            "NP-100302",
            Some(40),
            true,
        ),
    ] {
        let mut m = corpus.msg(stem);
        shop_headers(
            &mut m,
            &format!("Your Northpeak order {order}"),
            Dt::new(2026, 6, 14, 14, 0, -360),
        );
        let content = content.replace("NP-100301", order);
        // A length that leaves a remainder, so padding would be needed.
        let content = if content.len() % 3 == 0 {
            format!("{content}\n")
        } else {
            content
        };
        m.body(Part {
            headers: vec![
                "Content-Type: text/plain; charset=utf-8".into(),
                "Content-Transfer-Encoding: base64".into(),
            ],
            body: b64(content.as_bytes(), wrap, pad).into_bytes(),
        });
        m.receipt(order_receipt(6400, "USD", order));
        corpus.put(m);
    }
}

fn headers(corpus: &mut Corpus) {
    let mut m = corpus.msg("multipart/duplicate_subject_and_from");
    m.sender("Town Library", "notices@library.test");
    m.header("From", "Library Robot <robot@library.test>");
    m.to();
    m.subject("Your hold expires tomorrow");
    m.header("Subject", "Second subject that must be ignored");
    m.date(Dt::new(2026, 6, 15, 9, 0, 0));
    m.message_id("library.test");
    m.body(text(
        "Hi Sam,\n\nPlease collect your hold by tomorrow.\n",
        Cte::SevenBit,
    ));
    corpus.put(m);

    let mut m = corpus.msg("multipart/missing_date");
    m.sender("Town Library", "notices@library.test");
    m.to();
    m.subject("A message with no Date header");
    m.message_id("library.test");
    m.body(text(
        "Hi Sam,\n\nThis message has no Date header.\n",
        Cte::SevenBit,
    ));
    corpus.put(m);

    let mut m = corpus.msg("multipart/missing_message_id");
    m.sender("Town Library", "notices@library.test");
    m.to();
    m.subject("A message with no Message-ID header");
    m.date(Dt::new(2026, 6, 16, 9, 0, 0));
    m.body(text(
        "Hi Sam,\n\nThis message has no Message-ID header.\n",
        Cte::SevenBit,
    ));
    corpus.put(m);

    for (stem, subject, day) in [
        ("multipart/duplicate_message_id_a", "Event reminder", 17),
        (
            "multipart/duplicate_message_id_b",
            "Event reminder (resent)",
            18,
        ),
    ] {
        let mut m = corpus.msg(stem);
        m.sender("Town Library", "notices@library.test");
        m.to();
        m.subject(subject);
        m.date(Dt::new(2026, 6, day, 9, 0, 0));
        m.message_id_is("20260617090000.reminder-4412@library.test");
        m.body(text(
            "Hi Sam,\n\nStory time starts at 10 on Saturday.\n",
            Cte::SevenBit,
        ));
        corpus.put(m);
    }
}

fn dates(corpus: &mut Corpus) {
    let sunday = Dt::new(2026, 3, 1, 9, 30, 0);
    let wd = |dt: Dt| dt.weekday();
    let y99 = Dt::new(1999, 3, 1, 9, 30, 0);
    let cases: [(&str, String, Option<&str>); 9] = [
        (
            "date_zone_minus_0000",
            "Sun, 01 Mar 2026 09:30:00 -0000".into(),
            Some("2026-03-01T09:30:00Z"),
        ),
        (
            "date_zone_gmt",
            "Sun, 01 Mar 2026 09:30:00 GMT".into(),
            Some("2026-03-01T09:30:00Z"),
        ),
        (
            "date_zone_ut",
            "Sun, 01 Mar 2026 09:30:00 UT".into(),
            Some("2026-03-01T09:30:00Z"),
        ),
        (
            "date_zone_est",
            "Sun, 01 Mar 2026 09:30:00 EST".into(),
            Some("2026-03-01T14:30:00Z"),
        ),
        (
            "date_two_digit_year_2000s",
            format!("{}, 01 Mar 26 09:30:00 +0100", wd(sunday)),
            Some("2026-03-01T08:30:00Z"),
        ),
        (
            "date_two_digit_year_1900s",
            format!("{}, 01 Mar 99 09:30:00 -0500", wd(y99)),
            Some("1999-03-01T14:30:00Z"),
        ),
        (
            "date_no_weekday_no_seconds",
            "1 Mar 2026 09:30 +0200".into(),
            Some("2026-03-01T07:30:00Z"),
        ),
        (
            "date_trailing_comment",
            "Sun, 01 Mar 2026 09:30:00 +0000 (UTC)".into(),
            Some("2026-03-01T09:30:00Z"),
        ),
        ("date_unreadable", "sometime next week".into(), None),
    ];
    for (stem, raw, utc) in cases {
        let mut m = corpus.msg(&format!("multipart/{stem}"));
        m.sender("Town Library", "notices@library.test");
        m.to();
        m.subject("Date header test");
        m.date_raw(&raw, utc);
        m.message_id("library.test");
        m.body(text(
            "Hi Sam,\n\nThe Date header of this message is the test.\n",
            Cte::SevenBit,
        ));
        corpus.put(m);
    }
}
