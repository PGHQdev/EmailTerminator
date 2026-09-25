//! Part 9, "`List-Unsubscribe` variants".

use super::receipts::{Kind, write};
use super::senders::plain_vendor;
use super::{Corpus, Cte, Dt, Msg, html, multipart, text};

const SIGNED_WITH_LIST: &str =
    "from:to:subject:date:message-id:list-unsubscribe:list-unsubscribe-post";

fn newsletter_body(m: &mut Msg) {
    m.body(text(
        "Hi Sam,\n\nThis week: the spring ferry timetable, and the new pier opens.\n\nYou get this because you subscribed at harbor.test.\n",
        Cte::SevenBit,
    ));
}

pub fn generate(corpus: &mut Corpus) {
    let variants: [(&str, &str); 11] = [
        (
            "list_unsubscribe/mailto_only",
            "<mailto:leave-8a1f2c@news.harbor.test>",
        ),
        (
            "list_unsubscribe/https_only",
            "<https://news.harbor.test/u/8a1f2c>",
        ),
        (
            "list_unsubscribe/mailto_then_https",
            "<mailto:leave-8a1f2c@news.harbor.test>, <https://news.harbor.test/u/8a1f2c>",
        ),
        (
            "list_unsubscribe/https_then_mailto",
            "<https://news.harbor.test/u/8a1f2c>, <mailto:leave-8a1f2c@news.harbor.test>",
        ),
        (
            "list_unsubscribe/two_mailto",
            "<mailto:leave-8a1f2c@news.harbor.test>, <mailto:unsubscribe@lists.harbor.test>",
        ),
        (
            "list_unsubscribe/folded_between_uris",
            "<mailto:leave-8a1f2c@news.harbor.test>,\n\t<https://news.harbor.test/u/8a1f2c>",
        ),
        (
            "list_unsubscribe/folded_inside_brackets",
            "<https://news.harbor.test/unsubscribe?list=weekly&\n user=8a1f2c>",
        ),
        (
            "list_unsubscribe/mailto_with_subject",
            "<mailto:unsubscribe@news.harbor.test?subject=unsubscribe>",
        ),
        (
            "list_unsubscribe/mailto_with_body",
            "<mailto:unsubscribe@news.harbor.test?subject=unsubscribe&body=Please%20remove%20sam%40inbox.test>",
        ),
        (
            "list_unsubscribe/no_angle_brackets",
            "https://news.harbor.test/u/8a1f2c, mailto:leave-8a1f2c@news.harbor.test",
        ),
        (
            "list_unsubscribe/folded_before_value",
            "\n <mailto:leave-8a1f2c@news.harbor.test>",
        ),
    ];
    for (day, (stem, value)) in (1..).zip(variants) {
        let mut m = corpus.msg(stem);
        m.dkim(
            "news.harbor.test",
            "from:to:subject:date:message-id:list-unsubscribe",
        );
        signed_newsletter(&mut m, day);
        m.list_unsubscribe(value);
        newsletter_body(&mut m);
        corpus.put(m);
    }

    // A single URI over 998 octets. The sender folds inside it, so the
    // unfolded value carries a space at each fold.
    let token: String = (0..1100)
        .map(|i| b"abcdefghijklmnopqrstuvwxyz0123456789"[i * 7 % 36] as char)
        .collect();
    let folded = format!(
        "<https://t.news.harbor.test/unsubscribe?d={}\n {}\n {}>",
        &token[..400],
        &token[400..900],
        &token[900..]
    );
    let mut m = corpus.msg("list_unsubscribe/uri_over_998_octets");
    m.dkim(
        "news.harbor.test",
        "from:to:subject:date:message-id:list-unsubscribe",
    );
    signed_newsletter(&mut m, 12);
    m.list_unsubscribe(&folded);
    newsletter_body(&mut m);
    corpus.put(m);

    // RFC 8058 cases. The golden records only whether the Post value is
    // exactly right; whether the message qualifies is decided at M3.
    let one_click: [(&str, &str, Option<&str>, &str); 5] = [
        (
            "list_unsubscribe/one_click_valid",
            "<https://news.harbor.test/u/8a1f2c>, <mailto:leave-8a1f2c@news.harbor.test>",
            Some(SIGNED_WITH_LIST),
            "List-Unsubscribe=One-Click",
        ),
        (
            "list_unsubscribe/one_click_http_only",
            "<http://news.harbor.test/u/8a1f2c>",
            Some(SIGNED_WITH_LIST),
            "List-Unsubscribe=One-Click",
        ),
        (
            "list_unsubscribe/one_click_no_dkim",
            "<https://news.harbor.test/u/8a1f2c>",
            None,
            "List-Unsubscribe=One-Click",
        ),
        (
            "list_unsubscribe/one_click_wrong_value",
            "<https://news.harbor.test/u/8a1f2c>",
            Some(SIGNED_WITH_LIST),
            "One-Click",
        ),
        (
            "list_unsubscribe/one_click_wrong_case",
            "<https://news.harbor.test/u/8a1f2c>",
            Some(SIGNED_WITH_LIST),
            "list-unsubscribe=one-click",
        ),
    ];
    for (day, (stem, value, signed, post)) in (13..).zip(one_click) {
        let mut m = corpus.msg(stem);
        if let Some(h) = signed {
            m.dkim("news.harbor.test", h);
        }
        signed_newsletter(&mut m, day);
        m.list_unsubscribe(value);
        m.list_unsubscribe_post(post);
        newsletter_body(&mut m);
        corpus.put(m);
    }

    // The header on a transactional receipt: still a receipt, not a list.
    let v = plain_vendor(
        "kiteworks",
        "Kiteworks",
        "billing@kiteworks.test",
        vec!["kiteworks.test"],
        "Kiteworks Studio",
    );
    let mut m = corpus.msg("list_unsubscribe/on_transactional_receipt");
    write(
        &mut m,
        &v,
        Kind::RenewalMonthly,
        Dt::new(2026, 4, 18, 9, 0, -300),
        v.monthly,
    );
    m.list_unsubscribe("<https://kiteworks.test/email-preferences?u=8a1f2c>");
    corpus.put(m);

    // The Apache-list shape: a full RFC 2369 set plus Precedence: bulk.
    let mut m = corpus.msg("list_unsubscribe/rfc2369_full_set");
    m.return_path("dev-return-4412-sam=inbox.test@quill.apache.test");
    m.header(
        "Mailing-List",
        "contact dev-help@quill.apache.test; run by ezmlm",
    );
    m.precedence("bulk");
    m.header("List-Help", "<mailto:dev-help@quill.apache.test>");
    m.list_unsubscribe("<mailto:dev-unsubscribe@quill.apache.test>");
    m.header("List-Post", "<mailto:dev@quill.apache.test>");
    m.list_id("<dev.quill.apache.test>", "dev.quill.apache.test");
    m.header("Delivered-To", "mailing list dev@quill.apache.test");
    m.dkim("quill.test", "from:to:subject:date:message-id");
    m.sender("Jane Okafor", "jokafor@quill.test");
    m.header("To", "dev@quill.apache.test");
    m.subject("[DISCUSS] Release Quill 2.4.0");
    m.date(Dt::new(2026, 4, 19, 17, 2, 120));
    m.message_id("mail.quill.test");
    m.body(text(
        "Hi all,\n\nI'd like to start the release process for 2.4.0 next week. Objections?\n\nJane\n",
        Cte::SevenBit,
    ));
    corpus.put(m);

    // Precedence: list with no other list header still marks a list.
    let mut m = corpus.msg("list_unsubscribe/precedence_list_only");
    m.sender("Harbor Club Notices", "notices@club.harbor.test");
    m.to();
    m.subject("Harbor Club: marina closed on Saturday");
    m.date(Dt::new(2026, 4, 20, 8, 0, 0));
    m.message_id("club.harbor.test");
    m.precedence("list");
    newsletter_body(&mut m);
    corpus.put(m);

    // List-Id with a display phrase: the golden keeps the identifier only.
    let mut m = corpus.msg("list_unsubscribe/list_id_with_phrase");
    m.dkim(
        "news.harbor.test",
        "from:to:subject:date:message-id:list-id",
    );
    signed_newsletter(&mut m, 21);
    m.list_id(
        "\"Harbor Weekly\" <weekly.news.harbor.test>",
        "weekly.news.harbor.test",
    );
    newsletter_body(&mut m);
    corpus.put(m);

    // No header at all; the only unsubscribe link is in the HTML body.
    let mut m = corpus.msg("list_unsubscribe/html_link_only");
    m.dkim("news.harbor.test", "from:to:subject:date:message-id");
    signed_newsletter(&mut m, 22);
    let boundary = m.boundary();
    m.body(multipart(
        "alternative",
        &boundary,
        vec![
            text(
                "Hi Sam,\n\nThis week: the spring ferry timetable.\n",
                Cte::SevenBit,
            ),
            html(
                "<!DOCTYPE html>\n<html><body>\n<p>Hi Sam,</p>\n<p>This week: the spring ferry timetable.</p>\n<p style=\"font-size:11px\"><a href=\"https://news.harbor.test/u/8a1f2c\">Unsubscribe</a></p>\n</body></html>\n",
                Cte::SevenBit,
            ),
        ],
    ));
    corpus.put(m);
}

/// From, To, Subject, Date, Message-ID of the Harbor newsletter.
fn signed_newsletter(m: &mut Msg, day: u32) {
    m.sender("Harbor Weekly", "weekly@news.harbor.test");
    m.to();
    m.subject("Harbor Weekly: tides, ferries and a new pier");
    m.date(Dt::new(2026, 4, day, 6, 30, 0));
    m.message_id("news.harbor.test");
}
