//! Part 9, "Time-series shapes". Each series is `series/<name>/NNN.eml` in
//! date order plus `expected.json`, what recurrence detection must conclude
//! from that directory alone.

use super::receipts::{Kind, Vendor, write};
use super::senders::plain_vendor;
use super::{Cadence, Corpus, Cte, Dt, SeriesExpected, text};
use et_core::extract::{Receipt, ReceiptKind};

struct Series<'a> {
    corpus: &'a mut Corpus,
    name: &'static str,
    count: usize,
}

impl<'a> Series<'a> {
    fn new(corpus: &'a mut Corpus, name: &'static str) -> Self {
        Series {
            corpus,
            name,
            count: 0,
        }
    }

    fn next_stem(&mut self) -> String {
        self.count += 1;
        format!("series/{}/{:03}", self.name, self.count)
    }

    fn receipt(&mut self, v: &Vendor, kind: Kind, at: Dt, monthly: i64) {
        let stem = self.next_stem();
        let mut m = self.corpus.msg(&stem);
        write(&mut m, v, kind, at, monthly);
        self.corpus.put_eml(m);
    }

    /// The previous file again, byte for byte: the same receipt seen in a
    /// second folder.
    fn duplicate_previous(&mut self) {
        let previous = format!("series/{}/{:03}.eml", self.name, self.count);
        let bytes = self.corpus.files[&previous].clone();
        let stem = self.next_stem();
        self.corpus.insert(format!("{stem}.eml"), bytes);
    }

    fn expect(self, expected: SeriesExpected) {
        self.corpus
            .put_json(&format!("series/{}/expected.json", self.name), &expected);
    }
}

fn at(start: Dt, months: i64, rng_hour: u64) -> Dt {
    let dt = start.plus_months(months);
    Dt {
        h: 6 + (rng_hour % 12) as u32,
        mi: (rng_hour * 7 % 60) as u32,
        ..dt
    }
}

fn expected(
    group_key: &str,
    cadence: Option<Cadence>,
    charge_count: u32,
    price_increase: bool,
    latest: Option<i64>,
    currency: Option<&str>,
) -> SeriesExpected {
    SeriesExpected {
        group_key: group_key.into(),
        cadence,
        charge_count,
        price_increase,
        latest_amount_minor_units: latest,
        currency: currency.map(str::to_owned),
    }
}

/// First charge, then monthly renewals.
fn monthly_run(s: &mut Series, v: &Vendor, start: Dt, months: i64, price: i64) {
    for i in 0..months {
        let kind = if i == 0 {
            Kind::FirstCharge
        } else {
            Kind::RenewalMonthly
        };
        s.receipt(v, kind, at(start, i, i as u64 * 5 + 3), price);
    }
}

pub fn generate(corpus: &mut Corpus) {
    let mut v = plain_vendor(
        "hostlane",
        "Hostlane",
        "billing@hostlane.test",
        vec!["hostlane.test"],
        "Hostlane Starter",
    );
    v.monthly = 1200;
    let mut s = Series::new(corpus, "monthly_24_months");
    monthly_run(&mut s, &v, Dt::new(2024, 6, 5, 0, 0, -300), 24, v.monthly);
    s.expect(expected(
        "hostlane.test",
        Some(Cadence::Monthly),
        24,
        false,
        Some(1200),
        Some("USD"),
    ));

    let mut v = plain_vendor(
        "atlasbook",
        "Atlasbook",
        "receipts@atlasbook.test",
        vec!["atlasbook.test"],
        "Atlasbook Annual",
    );
    v.annual = 9900;
    let mut s = Series::new(corpus, "annual_3_years");
    for year in 0..3 {
        s.receipt(
            &v,
            Kind::RenewalAnnual,
            at(Dt::new(2023, 9, 10, 0, 0, 0), year * 12, year as u64 + 1),
            v.monthly,
        );
    }
    s.expect(expected(
        "atlasbook.test",
        Some(Cadence::Annual),
        3,
        false,
        Some(9900),
        Some("USD"),
    ));

    let mut v = plain_vendor(
        "snapclip",
        "Snapclip",
        "billing@snapclip.test",
        vec!["snapclip.test"],
        "Snapclip Pro",
    );
    v.monthly = 799;
    let start = Dt::new(2025, 3, 14, 0, 0, -480);
    let mut s = Series::new(corpus, "stops_after_7_months");
    monthly_run(&mut s, &v, start, 7, v.monthly);
    s.receipt(
        &v,
        Kind::Cancellation,
        at(start, 7, 2).plus_days(-4),
        v.monthly,
    );
    s.expect(expected(
        "snapclip.test",
        Some(Cadence::Monthly),
        7,
        false,
        Some(799),
        Some("USD"),
    ));

    let mut v = plain_vendor(
        "brewbox",
        "Brewbox",
        "billing@brewbox.test",
        vec!["brewbox.test"],
        "Brewbox Coffee Club",
    );
    v.monthly = 1000;
    v.raised = 1200;
    let start = Dt::new(2025, 5, 20, 0, 0, -300);
    let mut s = Series::new(corpus, "price_increase_mid_sequence");
    monthly_run(&mut s, &v, start, 5, 1000);
    s.receipt(&v, Kind::PriceChange, at(start, 5, 9).plus_days(-10), 1000);
    s.receipt(&v, Kind::RenewalMonthly, at(start, 5, 5), 1000);
    for i in 6..12 {
        s.receipt(&v, Kind::RenewalMonthly, at(start, i, i as u64), 1200);
    }
    s.expect(expected(
        "brewbox.test",
        Some(Cadence::Monthly),
        12,
        true,
        Some(1200),
        Some("USD"),
    ));

    // Two subscriptions from one vendor, one directory each. Both share the
    // group key. Fed both directories together, the detector must report two
    // subscriptions, told apart by plan name and amount, each monthly; it
    // must not merge them into one subscription with two charges a month.
    for (name, product, price, day) in [
        ("overlap_driftwood_pro", "Driftwood Pro", 1500, 3),
        (
            "overlap_driftwood_storage",
            "Driftwood Storage 1 TB",
            400,
            20,
        ),
    ] {
        let mut v = plain_vendor(
            "driftwood",
            "Driftwood",
            "billing@driftwood.test",
            vec!["driftwood.test"],
            product,
        );
        v.monthly = price;
        let mut s = Series::new(corpus, name);
        monthly_run(&mut s, &v, Dt::new(2025, 9, day, 0, 0, 0), 8, price);
        s.expect(expected(
            "driftwood.test",
            Some(Cadence::Monthly),
            8,
            false,
            Some(price),
            Some("USD"),
        ));
    }

    let mut s = Series::new(corpus, "one_off_purchase");
    order(
        &mut s,
        KESTREL,
        "KK-2025-0917",
        "$45.00",
        Dt::new(2025, 11, 28, 19, 12, -300),
    );
    s.expect(expected(
        "kestrelkites.test",
        None,
        1,
        false,
        Some(4500),
        Some("USD"),
    ));

    let mut s = Series::new(corpus, "irregular_purchases");
    for (id, amount, dt) in [
        ("ORD-50110", "$18.00", Dt::new(2025, 2, 3, 20, 1, -300)),
        ("ORD-51377", "$62.50", Dt::new(2025, 4, 22, 12, 40, -240)),
        ("ORD-53920", "$9.99", Dt::new(2025, 8, 1, 8, 5, -240)),
        ("ORD-55812", "$45.00", Dt::new(2025, 11, 28, 19, 12, -300)),
    ] {
        order(&mut s, LUMENFIELD, id, amount, dt);
    }
    s.expect(expected(
        "lumenfield.test",
        Some(Cadence::Irregular),
        4,
        false,
        Some(4500),
        Some("USD"),
    ));

    let mut v = plain_vendor(
        "quillpad",
        "Quillpad",
        "billing@quillpad.test",
        vec!["quillpad.test"],
        "Quillpad Plus",
    );
    v.monthly = 500;
    let start = Dt::new(2025, 10, 8, 0, 0, 60);
    let mut s = Series::new(corpus, "duplicate_in_two_folders");
    for i in 0..6 {
        let kind = if i == 0 {
            Kind::FirstCharge
        } else {
            Kind::RenewalMonthly
        };
        s.receipt(&v, kind, at(start, i, i as u64 + 11), 500);
        if i == 3 {
            s.duplicate_previous();
        }
    }
    s.expect(expected(
        "quillpad.test",
        Some(Cadence::Monthly),
        6,
        false,
        Some(500),
        Some("USD"),
    ));

    // One subscription that moves to a new registrable domain after three
    // months. No domain covers the whole run, so the group key is the
    // merchant name both halves share.
    let start = Dt::new(2026, 1, 3, 0, 0, -300);
    let mut s = Series::new(corpus, "sending_domain_change");
    for i in 0..6 {
        let domain = if i < 3 {
            "brightline.test"
        } else {
            "brightlinehq.test"
        };
        let v = plain_vendor(
            "brightline",
            "Brightline",
            &format!("billing@{domain}"),
            vec![domain],
            "Brightline VPN",
        );
        let kind = if i == 0 {
            Kind::FirstCharge
        } else {
            Kind::RenewalMonthly
        };
        s.receipt(&v, kind, at(start, i, i as u64 + 2), v.monthly);
    }
    s.expect(expected(
        "Brightline",
        Some(Cadence::Monthly),
        6,
        false,
        Some(900),
        Some("USD"),
    ));
}

const LUMENFIELD: (&str, &str) = ("Lumenfield Prints", "lumenfield.test");
const KESTREL: (&str, &str) = ("Kestrel Kites", "kestrelkites.test");

/// A one-off shop order: an order confirmation, which extracts as a charge.
fn order(s: &mut Series, (shop, domain): (&str, &str), id: &str, amount: &str, dt: Dt) {
    let stem = s.next_stem();
    let mut m = s.corpus.msg(&stem);
    m.dkim(domain, "from:to:subject:date:message-id");
    m.sender(shop, &format!("orders@{domain}"));
    m.to();
    m.subject(&format!("Order confirmation {id}"));
    m.date(dt);
    m.message_id(domain);
    m.body(text(
        &format!(
            "Hi Sam,\n\nThanks for your order. Your order ships in 3-5 days.\n\nOrder number: {id}\nTotal: {amount}\nPayment method: Visa ending in 4242\n"
        ),
        Cte::SevenBit,
    ));
    let minor = amount
        .trim_start_matches('$')
        .replace('.', "")
        .parse()
        .expect("amount");
    m.receipt(Receipt {
        kind: ReceiptKind::Charge,
        amount_minor_units: Some(minor),
        currency: Some("USD".into()),
        merchant: None,
        invoice_ref: Some(id.into()),
    });
    s.corpus.put_eml(m);
}
