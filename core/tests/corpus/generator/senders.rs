//! Part 9, "Sender shapes".

use super::receipts::{Kind, Layout, Money, Period, Relay, Tax, Vendor, write};
use super::{Corpus, Cte, Dt, Eol, Msg, text};

/// A plain direct vendor for sender tests: `$` prices, no tax.
pub fn plain_vendor(
    key: &'static str,
    display: &'static str,
    address: &str,
    dkim: Vec<&'static str>,
    product: &'static str,
) -> Vendor {
    Vendor {
        key,
        display,
        address: address.to_owned(),
        bounce: format!("bounces@{}", address.split('@').nth(1).expect("host")),
        dkim,
        relay: Relay::Direct,
        merchant: None,
        brand: display,
        product,
        money: Money::Dollar,
        monthly: 900,
        annual: 9000,
        plus: 1900,
        lite: 400,
        raised: 1100,
        tax: Tax::None,
        period: Period::Words,
        invoice: Some(("Invoice number", "INV-######")),
        card: "Visa ending in 4242",
        layout: Layout::Text(Cte::Qp),
        eol: Eol::Crlf,
        offset: -300,
    }
}

fn charge(corpus: &mut Corpus, stem: &str, v: &Vendor, at: Dt) {
    let mut m = corpus.msg(stem);
    write(&mut m, v, Kind::FirstCharge, at, v.monthly);
    corpus.put(m);
}

/// A non-billing message with a plain text body.
fn notice(m: &mut Msg, display: &str, address: &str, subject: &str, at: Dt, body: &str) {
    m.sender(display, address);
    m.to();
    m.subject(subject);
    m.date(at);
    let domain = address.split('@').nth(1).expect("host").to_lowercase();
    m.message_id(&domain);
    m.body(text(body, Cte::SevenBit));
}

pub fn generate(corpus: &mut Corpus) {
    // Return-Path names the ESP's bounce host; From names the shop.
    let mut m = corpus.msg("senders/return_path_differs");
    m.return_path("bounce-mc.us21_4471.90213-sam=inbox.test@mail221.suw11.rsgsv.test");
    m.dkim("acmerobotics.test", "from:to:subject:date:message-id");
    notice(
        &mut m,
        "Acme Robotics Store",
        "orders@acmerobotics.test",
        "Your order has shipped",
        Dt::new(2026, 2, 11, 15, 4, -300),
        "Hi Sam,\n\nYour order AR-10442 has shipped and should arrive on Friday.\n\nAcme Robotics Store\n",
    );
    corpus.put(m);

    // Per-function local parts.
    charge(
        corpus,
        "senders/function_billing",
        &plain_vendor(
            "quarry",
            "Quarry",
            "billing@quarry.test",
            vec!["quarry.test"],
            "Quarry Pro",
        ),
        Dt::new(2026, 1, 5, 9, 0, -300),
    );
    charge(
        corpus,
        "senders/function_receipts",
        &plain_vendor(
            "lattice",
            "Lattice Notes",
            "receipts@lattice.test",
            vec!["lattice.test"],
            "Lattice Notes Plus",
        ),
        Dt::new(2026, 1, 6, 9, 0, -300),
    );
    charge(
        corpus,
        "senders/function_invoice_plus_tag",
        &plain_vendor(
            "ledgerly",
            "Ledgerly",
            "invoice+acct123@ledgerly.test",
            vec!["ledgerly.test"],
            "Ledgerly Solo",
        ),
        Dt::new(2026, 1, 7, 9, 0, -300),
    );
    let mut m = corpus.msg("senders/function_no_reply");
    m.dkim("lattice.test", "from:to:subject:date:message-id");
    notice(
        &mut m,
        "Lattice Notes",
        "no-reply@lattice.test",
        "New sign-in to your Lattice Notes account",
        Dt::new(2026, 1, 8, 22, 17, -300),
        "Hi Sam,\n\nWe noticed a new sign-in from Firefox on Linux. If this was you, you can\nignore this message.\n\nLattice Notes\n",
    );
    corpus.put(m);

    // ESP relays: the only signature is the ESP's.
    let mut v = plain_vendor(
        "orbit",
        "Orbit Maps",
        "billing@orbitmaps.test",
        vec!["sendgrid.net"],
        "Orbit Maps Pro",
    );
    v.bounce = "bounces+2231417-8f2a-sam=inbox.test@em4417.orbitmaps.test".into();
    charge(
        corpus,
        "senders/esp_sendgrid",
        &v,
        Dt::new(2026, 1, 9, 9, 0, -300),
    );

    let mut m = corpus.msg("senders/esp_amazonses");
    m.return_path("0100019a4e2f1b7c-6c1d2e3f-4a5b-6c7d-8e9f-0a1b2c3d4e5f-000000@amazonses.com");
    m.dkim(
        "amazonses.com",
        "from:to:subject:date:message-id:list-unsubscribe:list-unsubscribe-post",
    );
    m.sender("Fernleaf Journal", "newsletter@fernleaf.test");
    m.to();
    m.subject("This week at Fernleaf: spring planting");
    m.date(Dt::new(2026, 3, 19, 11, 0, 0));
    m.message_id("email.amazonses.com");
    m.list_unsubscribe("<https://fernleaf.test/unsubscribe?u=sam%40inbox.test&t=8a1f>");
    m.list_unsubscribe_post("List-Unsubscribe=One-Click");
    m.body(text(
        "Hi Sam,\n\nSpring planting starts now. Read the guide on our site.\n\nFernleaf Journal\n",
        Cte::SevenBit,
    ));
    corpus.put(m);

    let mut m = corpus.msg("senders/esp_mailgun");
    m.return_path("bounce+5f1c2a.9a0e-sam=inbox.test@mg.pinecone.test");
    m.dkim("mailgun.org", "from:to:subject:date:message-id");
    notice(
        &mut m,
        "Pinecone Support",
        "support@pinecone.test",
        "Your support ticket 55120 was updated",
        Dt::new(2026, 3, 2, 13, 30, 0),
        "Hi Sam,\n\nAn agent replied to ticket 55120. Reply to this email to answer.\n\nPinecone Support\n",
    );
    corpus.put(m);

    // One subscription whose vendor moves to a new registrable domain
    // between two months. series/sending_domain_change carries the grouping
    // expectation.
    for (stem, address, month) in [
        (
            "senders/domain_change_month_1",
            "billing@brightline.test",
            3,
        ),
        (
            "senders/domain_change_month_2",
            "billing@brightlinehq.test",
            4,
        ),
    ] {
        let domain = address.split('@').nth(1).expect("host");
        charge(
            corpus,
            stem,
            &plain_vendor(
                "brightline",
                "Brightline",
                address,
                vec![domain],
                "Brightline VPN",
            ),
            Dt::new(2026, month, 3, 8, 0, -300),
        );
    }

    // Two unrelated newsletters on one shared sending domain; grouping by
    // domain alone would merge them.
    for (stem, display, local, list) in [
        (
            "senders/shared_domain_alpha",
            "Alpha Weekly",
            "alphaweekly",
            "Issue 41: the state of home robotics",
        ),
        (
            "senders/shared_domain_beta",
            "The Beta Digest",
            "betadigest",
            "Beta Digest: five links for Friday",
        ),
    ] {
        let mut m = corpus.msg(stem);
        m.dkim(
            "letterpost.test",
            "from:to:subject:date:message-id:list-id:list-unsubscribe",
        );
        m.sender(display, &format!("{local}@letterpost.test"));
        m.to();
        m.subject(list);
        m.date(Dt::new(2026, 3, 20, 7, 30, 0));
        m.message_id("letterpost.test");
        m.list_id(
            &format!("<{local}.letterpost.test>"),
            &format!("{local}.letterpost.test"),
        );
        m.list_unsubscribe(&format!(
            "<https://letterpost.test/{local}/unsubscribe?r=sam>, <mailto:{local}+unsubscribe@letterpost.test>"
        ));
        m.body(text(
            &format!("Hi Sam,\n\nThis week's {display} is below.\n\n(The issue text.)\n"),
            Cte::SevenBit,
        ));
        corpus.put(m);
    }

    // IDN host as UTF-8 (RFC 6532), and the same host as punycode in
    // uppercase, which the golden lowercases.
    let mut m = corpus.msg("senders/idn_utf8_host");
    m.dkim("xn--caf-crme-60ag.test", "from:to:subject:date:message-id");
    m.header_bytes("From", "Café Crème <nouvelles@café-crème.test>".as_bytes());
    m.g.from_name = Some("Café Crème".into());
    m.g.from_address = Some("nouvelles@café-crème.test".into());
    m.to();
    m.subject("Our spring menu is here");
    m.date(Dt::new(2026, 3, 21, 10, 0, 60));
    m.message_id("xn--caf-crme-60ag.test");
    m.body(text(
        "Bonjour Sam,\n\nOur spring menu starts on Monday.\n\nCafé Crème\n",
        Cte::EightBit,
    ));
    corpus.put(m);

    let mut m = corpus.msg("senders/idn_punycode_host");
    m.dkim("XN--BCHER-KVA.TEST", "from:to:subject:date:message-id");
    m.sender_raw(
        "Buecher Versand <Versand@XN--BCHER-KVA.TEST>",
        Some("Buecher Versand"),
        "Versand@XN--BCHER-KVA.TEST",
    );
    m.to();
    m.subject("Your order is on its way");
    m.date(Dt::new(2026, 3, 22, 10, 0, 60));
    m.message_id("xn--bcher-kva.test");
    m.body(text(
        "Hi Sam,\n\nYour books left our warehouse today.\n",
        Cte::SevenBit,
    ));
    corpus.put(m);
}
