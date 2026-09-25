//! Part 9, "RFC 2047 encoded words". Non-UTF-8 charsets are written as
//! precomputed bytes, since the corpus adds no encoding crate.

use super::{Corpus, Cte, Dt, leaf, multipart, text, tiny_pdf};

/// Stem, raw `Subject`, decoded subject.
const SUBJECTS: [(&str, &str, &str); 17] = [
    (
        "subject_q_utf8",
        "=?utf-8?Q?Caf=C3=A9_ouvert_ce_week-end?=",
        "Café ouvert ce week-end",
    ),
    (
        "subject_b_utf8",
        "=?UTF-8?B?UmVjaG51bmcgZsO8ciBNw6Ryeg==?=",
        "Rechnung für März",
    ),
    (
        "charset_iso_8859_1",
        "=?iso-8859-1?Q?Gr=FC=DFe_aus_M=FCnchen?=",
        "Grüße aus München",
    ),
    (
        "charset_windows_1252",
        "=?windows-1252?Q?=80_=93Deals=94_this_week?=",
        "€ “Deals” this week",
    ),
    (
        "charset_shift_jis",
        "=?shift_jis?B?gqiSbYLngrmBRoKykL+LgQ==?=",
        "お知らせ：ご請求",
    ),
    ("charset_gb2312", "=?gb2312?B?1cu1pc2o1qo=?=", "账单通知"),
    (
        "charset_koi8_r",
        "=?koi8-r?B?8NLJ18XUIMnaIO3P08vX2Q==?=",
        "Привет из Москвы",
    ),
    // ASCII-only payload, so any fallback decoding gives the same text.
    (
        "charset_unregistered",
        "=?x-unknown-legacy?Q?Your_weekly_summary?=",
        "Your weekly summary",
    ),
    (
        "adjacent_words_whitespace_dropped",
        "=?utf-8?Q?Your_order?= =?utf-8?Q?_has_shipped?=",
        "Your order has shipped",
    ),
    (
        "word_split_across_fold",
        "=?utf-8?Q?Your_invoice_for?=\n =?utf-8?Q?_M=C3=A4rz_is_ready?=",
        "Your invoice for März is ready",
    ),
    (
        "word_then_plain_text",
        "=?utf-8?Q?Caf=C3=A9?= menu for spring",
        "Café menu for spring",
    ),
    (
        "multibyte_split_q",
        "=?utf-8?Q?Caf=C3?= =?utf-8?Q?=A9_cr=C3=A8me?=",
        "Café crème",
    ),
    (
        "multibyte_split_b",
        "=?utf-8?B?44GK5pSv5omV4w==?= =?utf-8?B?gYTjga7norroqo0=?=",
        "お支払いの確認",
    ),
    (
        "word_over_75_chars",
        "=?utf-8?Q?Your_monthly_statement_from_Harbor_Savings_is_ready_to_view_online_=E2=80=94_log_in?=",
        "Your monthly statement from Harbor Savings is ready to view online — log in",
    ),
    (
        "broken_base64_padding",
        "=?utf-8?B?SGVsbG8gd29ybGQ?=",
        "Hello world",
    ),
    (
        "stray_marker_not_a_word",
        "Save 50% =? today only",
        "Save 50% =? today only",
    ),
    (
        "stray_marker_in_word_position",
        "Price =?drop?= this week",
        "Price =?drop?= this week",
    ),
];

pub fn generate(corpus: &mut Corpus) {
    for (day, (stem, raw, decoded)) in (1..).zip(SUBJECTS) {
        let mut m = corpus.msg(&format!("encoded_words/{stem}"));
        m.sender("Mailroom", "hello@mailroom.test");
        m.to();
        m.subject_raw(raw, decoded);
        m.date(Dt::new(2026, 5, day, 12, 0, 0));
        m.message_id("mailroom.test");
        m.body(text(
            "Hi Sam,\n\nThe subject line of this message is the test.\n",
            Cte::SevenBit,
        ));
        corpus.put(m);
    }

    for (day, (stem, raw)) in (20..).zip([
        (
            "from_name_q",
            "=?utf-8?Q?Zo=C3=AB_M=C3=BCller?= <zoe@mueller.test>",
        ),
        (
            "from_name_b",
            "=?utf-8?B?Wm/DqyBNw7xsbGVy?= <zoe@mueller.test>",
        ),
    ]) {
        let mut m = corpus.msg(&format!("encoded_words/{stem}"));
        m.sender_raw(raw, Some("Zoë Müller"), "zoe@mueller.test");
        m.to();
        m.subject("Photos from Saturday");
        m.date(Dt::new(2026, 5, day, 18, 40, 120));
        m.message_id("mueller.test");
        m.body(text(
            "Hi Sam,\n\nThe photos are in the album.\n",
            Cte::SevenBit,
        ));
        corpus.put(m);
    }

    // RFC 2231 continuations for an attachment name, "Rechnung_März_2026.pdf".
    let mut m = corpus.msg("encoded_words/rfc2231_filename_continuation");
    m.sender("Kanzlei Weber", "post@kanzlei-weber.test");
    m.to();
    m.subject("Ihre Unterlagen");
    m.date(Dt::new(2026, 5, 22, 9, 15, 60));
    m.message_id("kanzlei-weber.test");
    let boundary = m.boundary();
    let mut attachment = leaf(
        "application/pdf;\n name*0*=utf-8''Rechnung_M%C3%A4rz;\n name*1*=_2026.pdf",
        Cte::Base64,
        &tiny_pdf(&["Unterlagen"]),
    );
    attachment.headers.push(
        "Content-Disposition: attachment;\n filename*0*=utf-8''Rechnung_M%C3%A4rz;\n filename*1*=_2026.pdf"
            .into(),
    );
    m.body(multipart(
        "mixed",
        &boundary,
        vec![
            text(
                "Guten Tag,\n\nanbei die angeforderten Unterlagen.\n\nKanzlei Weber\n",
                Cte::SevenBit,
            ),
            attachment,
        ],
    ));
    corpus.put(m);
}
