# Test mail corpora — research summary (August 2026)

Compiled from publisher pages and from direct measurement of the archives.
Informs the fixture strategy for the deterministic layer.

All counts below come from downloading the archive and counting headers on
10 August 2026. Enron figures come from a 78,457-message prefix of the 1.7 GB
stream. Every other figure covers the whole named set.

## Candidate corpora

| Corpus | Publisher | Stated terms | Size | Format | Era |
|---|---|---|---|---|---|
| [Enron Email Dataset](https://www.cs.cmu.edu/~enron/) | William Cohen, CMU (from CALO / SRI / FERC) | None. Page states a privacy request only | 1.7 GB gz, ~0.5M messages, ~150 users | maildir tree, one plain-text message per file | 1998–2002 |
| [SpamAssassin public corpus](https://spamassassin.apache.org/old/publiccorpus/) | Justin Mason, hosted by the ASF | "Copyright for the text in the messages remains with the original senders" | ~12 MB bz2, 6,047 messages | raw single-message files in `spam/`, `easy_ham/`, `hard_ham/` dirs | 2002–2003 |
| [Untroubled spam archive](http://untroubled.org/spam/) | Bruce Guenter | "Permission is hereby granted to use this archive without restriction" | 1998 → 2026-08, monthly `.7z`; 3.3 MB for 2026-07 | raw single-message files, `YYYY/MM/` | 1998–current |
| [Ling-Spam](https://www2.aueb.gr/users/ion/data/lingspam_public.tar.gz) | Ion Androutsopoulos, AUEB | None. Readme requires acknowledgement and notification | 11.5 MB gz, 2,893 messages × 4 variants | tokenised text, `Subject:` line plus body | 1997–2000 |
| [Enron-Spam](https://www2.aueb.gr/users/ion/data/enron-spam/) | Metsis, Androutsopoulos, Paliouras, AUEB | None. Readme requires citation | raw and preprocessed variants | raw messages plus token files | 1999–2005 |
| [Apache list archives](https://lists.apache.org/) | Apache Software Foundation | Public archive. No corpus licence. Removal requests go to [privacy@apache.org](https://privacy.apache.org/faq/committers.html) | mbox per list per month over an HTTP API | mbox | 1995–current |
| [EDRM data sets](https://edrm.net/resources/data-sets-2/) | EDRM Global Inc. | Site footer: CC BY 4.0 "except where otherwise noted" | I18N set 176 MB zip | Ubuntu localisation list archives, 23 languages | 2010 |
| [TREC 2005/2006/2007 spam](https://trec.nist.gov/data/spam.html) | Cormack, University of Waterloo | Unknown | Unknown | Unknown | 2005–2007 |
| [Avocado Research Email Collection](https://catalog.ldc.upenn.edu/LDC2015T03) | Linguistic Data Consortium | Two signed licence agreements plus a fee | 279 accounts | XML metadata plus extracted text | 2000s |

TREC spam corpora are unreachable. Every corpus link on the NIST page points
at `plg.uwaterloo.ca/~gvcormac/`, and all three paths return 404 as of
10 August 2026. Treat the TREC spam corpora as unavailable from the publisher.

## What the archives actually contain

Measured over the fetched archives. `LU` is `List-Unsubscribe`, `LU-Post` is
the RFC 8058 `List-Unsubscribe-Post` header.

| Set | Messages | LU | LU-Post | multipart | RFC 2047 encoded word |
|---|---|---|---|---|---|
| Untroubled 2026-07 | 2,161 | 1,499 | 1,497 | 1,629 `multipart/alternative` | 62 |
| SpamAssassin `easy_ham_2` | 1,400 | 771 | 0 | 56 | 13 |
| SpamAssassin `hard_ham` | 250 | 40 | 0 | 42 | 3 |
| SpamAssassin `spam_2` | 1,396 | 89 | 0 | 196 | 20 |
| Apache `httpd dev` 2025-01 mbox | 76 | 76 | 0 | 29 | 6 |
| Enron (78,457-message prefix) | 78,457 | 7 | 0 | 252 | 9 |

Reading of that table:

- **Untroubled is the only archive with modern header shapes.** 69% of the
  July 2026 messages carry `List-Unsubscribe`, and 1,497 of those 1,499 also
  carry `List-Unsubscribe-Post`. That matches the [Google bulk-sender rules
  in force since 1 February 2024](https://support.google.com/a/answer/81126),
  which require one-click unsubscribe above 5,000 messages per day.
  Content-transfer-encodings in the same month: 2,466 quoted-printable,
  100 base64, 139 7bit, 22 8bit.
- **The Untroubled semantics are fraudulent.** The subjects that look like
  billing mail are scams: "Your Subscription at C0STC0 Has Lapsed",
  "We couldn't renew your cloud storage. Review your payment method." The
  headers are real-world shaped. The vendor, the amount and the plan are
  invented by a spammer. Use it to exercise the parser. Do not use it as
  ground truth for receipt extraction, and do not use it to train sender
  reputation.
- **SpamAssassin is 2002 mail.** Its `List-Unsubscribe` values are 2002-era
  newsletter and ezmlm links (505 `http:`, 249 `https:`, 20 `mailto:` in
  `easy_ham_2`). Zero messages in the whole corpus carry
  `List-Unsubscribe-Post`, because RFC 8058 is from January 2017. A 2002 spam
  and ham corpus is not evidence about modern SaaS receipts. One message in
  `hard_ham` has a billing-shaped subject ("Automated 30 day renewal
  reminder 2002-05-27"). The rest are CNET, Lockergnome and Register
  newsletters. Each directory also holds a `cmds` shell file. The readme
  claims 1,397 messages in `spam_2`; the archive holds 1,396.
- **Enron is not MIME.** 78,437 of the 78,457 sampled messages declare
  `Content-Type: text/plain`, 73,637 of them `charset=us-ascii`, and every
  one of those 78,437 carries the `X-Folder`, `X-Origin` and `X-FileName`
  headers added by the PST extraction tool. Attachments were stripped by the
  publisher. 7 messages in the sample carry `List-Unsubscribe`. 55 have a
  receipt-shaped subject and 61 come from an amazon, paypal or ebay sender,
  which is 0.07% of the sample and all of it from 1999–2002. Enron exercises
  neither a MIME parser nor a receipt parser.
- **Ling-Spam is not mail.** The files hold a lowercased, tokenised body and
  a `Subject:` line. There are no headers at all. It cannot test anything in
  our deterministic layer.
- **Apache list mail is clean, modern, and the wrong genre.** The mbox export
  works: `https://lists.apache.org/api/mbox.lua?list=dev&domain=httpd.apache.org&date=2025-01`
  returns 1.1 MB of `application/mbox`. 74 of 76 messages carry a DKIM
  signature and all 76 carry an ezmlm `list-unsubscribe: <mailto:...>`. It is
  a good negative fixture set: mailing-list mail that our layer must classify
  as a list rather than as a paid subscription. It contains no commerce.

Nothing public contains real SaaS receipts. The receipt datasets that do
exist are OCR image sets of paper receipts, for example
[ReceiptSense](https://arxiv.org/html/2406.04493v2) and ICDAR SROIE. They are
photographs, not messages.

## Personal data and redistribution

**Enron is the sharp case.** The corpus is real mail of about 150 named
people, most of whom were not accused of anything. The publisher states no
licence. The publisher's own words: "In using this dataset, please be
sensitive to the privacy of the people involved (and remember that many of
these people were certainly not involved in any of the actions which
precipitated the investigation)", and some messages were removed "as part of
a redaction effort due to requests from affected employees"
([CMU](https://www.cs.cmu.edu/~enron/)). Sender addresses in the sample are
real and personal: `@enron.com` dominates, then `@hotmail.com`, `@aol.com`,
`@yahoo.com`, plus outside counsel at `@bracepatt.com`.

The PST edition carried worse. EDRM and Nuix removed more than 10,000
high-risk items from it in 2013, including "60 items containing credit card
numbers", "572 containing Social Security or other national identity
numbers" and "532 containing information of a highly personal nature such as
medical or legal matters"
([EDRM](https://edrm.net/2013/05/nuix-and-edrm-republish-enron-data-set-cleansed-of-more-than-10000-items-containing-private-health-and-financial-information/)).
The CMU edition has no attachments, so it avoids the spreadsheets. It keeps
the names, the home addresses typed into message bodies and the personal
mailbox addresses.

Consequence for an MIT repository: vendoring Enron gives the project a
permanent, unlicensed copy of identifiable personal data, in git history,
mirrored by every fork. The upstream publisher honours removal requests. A
git repository cannot honour one. A privacy-first product that ships other
people's unredacted mail contradicts its own README.

The other corpora, ranked by redistribution risk:

| Corpus | Redistribution position | Personal data |
|---|---|---|
| Untroubled | Explicit grant, unrestricted | Publisher's own bait addresses in `To:` and in unsubscribe URLs. Third-party sender identities are forged |
| SpamAssassin | No grant. "Copyright for the text in the messages remains with the original senders" | Partly obfuscated headers. Publisher asks that the mail never be replayed into a live mail system |
| Ling-Spam / Enron-Spam | No grant. Acknowledgement and citation requested | Ling-Spam: linguists' list posts, stripped to tokens. Enron-Spam: the Enron problem again |
| Apache list archives | No corpus licence. Per-message copyright with the authors | Real names and addresses of living contributors. ASF routes erasure requests to `privacy@apache.org` |
| EDRM I18N | Site footer claims CC BY 4.0 except where noted | Ubuntu translator list posts. Real names and addresses |
| Avocado | Signed agreements and a fee. Not redistributable | Real employee mail of a defunct company |

Only Untroubled carries a redistribution grant broad enough to vendor
without a licence question. It is spam.

## How open-source mail tools fixture parser tests

Surveyed from the repositories themselves. Counts come from GitHub tree and
contents listings and from raw file fetches.

| Project | Parser fixture strategy | Message files | Typical size |
|---|---|---|---|
| [CPython `email`](https://github.com/python/cpython/tree/main/Lib/test/test_email/data) | Files for the legacy suite, inline for every modern module | 48 `msg_*.txt` plus 17 media payloads | 136 B – 9 KB, median ~600 B |
| [stalwartlabs/mail-parser](https://github.com/stalwartlabs/mail-parser/tree/main/resources/eml) | Golden-file snapshots plus JSON header tables | 107 `.eml` with 214 expected JSON | 51 B – 19.8 KB |
| [nodemailer/mailparser](https://github.com/nodemailer/mailparser/tree/master/test/fixtures) | Mostly inline | 10 `.eml`, 3 of them ~1 MB stress inputs | 548 B – 44 KB |
| [dovecot/core](https://github.com/dovecot/core/blob/main/src/lib-mail/test-message-parser.c) | Inline C literals only | 0 | — |
| [cyrus-imapd](https://github.com/cyrusimap/cyrus-imapd/tree/master/cassandane/data/mime) | Inline for unit tests, generated messages for integration | 11 | 169 B – 970 B |
| [comm-central](https://github.com/mozilla/releases-comm-central/tree/master/mailnews/test/data) (Thunderbird) | Files for tree parsing, inline for headers | 47 `.eml` plus 56 raw messages named after bugs | small |
| [thunderbird-android](https://github.com/thunderbird/thunderbird-android/blob/main/mail/common/src/test/java/com/fsck/k9/mail/internet/MimeMessageParseTest.java) (K-9) | Inline Java literals only | 0 in the MIME library | — |
| [Go `net/mail`](https://github.com/golang/go/tree/master/src/net/mail) | Table-driven raw string literals | 0, no `testdata` | — |
| [emersion/go-message](https://github.com/emersion/go-message) | Inline | 0 | — |
| [axllent/mailpit](https://github.com/axllent/mailpit/tree/develop/internal/storage/testdata) | Files named after the structure under test | 6 `.eml` | 548 B – 41 KB |
| [notmuch](https://github.com/notmuch/notmuch/tree/master/test/corpora) | Committed Maildir corpus for correctness, downloaded corpus for performance | 320 blobs | small |
| [roundcubemail](https://github.com/roundcube/roundcubemail/tree/master/tests/src) | Mixed, mostly inline | 20 `.eml` repo-wide | ~1.3 KB |
| [php-mime-mail-parser](https://github.com/php-mime-mail-parser/php-mime-mail-parser/tree/main/tests/mails) | One file per GitHub issue | 46 | small |
| [rjarry/aerc](https://github.com/rjarry/aerc/tree/master/lib/rfc822/testdata/message) | 2 files for RFC 822, 93 for threading | 2 plus 93 | small |
| [pimalaya/himalaya](https://github.com/pimalaya/himalaya) | None. It does no MIME parsing | 0 | — |
| [offlineimap3](https://github.com/OfflineIMAP/offlineimap3) | None. It syncs opaque bytes | 0 | — |
| [isync/mbsync](https://github.com/gburd/isync/blob/master/src/run-tests.pl) (GitHub mirror) | Messages generated at run time by the test script | 0 | — |

The dominant pattern is small hand-picked raw messages committed to the
repository, or inline string literals in the test source. Across the 18 test
suites in those 17 projects, Cyrus counting twice because its unit tests and
its Cassandane integration tests differ, 8 use inline literals alone, 10
combine small committed files with inline cases, and **zero use a downloaded
corpus for parser correctness**. notmuch is the single project that downloads a corpus,
and it uses it only for time and memory tests
([`performance-test/Makefile.local`](https://github.com/notmuch/notmuch/blob/master/performance-test/Makefile.local)),
with the tarball excluded by `.gitignore` and verified against a GPG
signature that is committed.

Four secondary rules held across projects:

1. **The split follows the test layer.** Header and token parsing is inline
   and table-driven. Whole-message parsing uses files. Stalwart holds 432
   JSON header cases against 107 `.eml`. Thunderbird's `test_header.js` is
   1,304 lines with zero file reads, while `test_mime_tree.js` reads 19
   files. CPython's `test__header_value_parser.py` is 147 KB with zero file
   reads.
2. **One fixture per defect, named after the defect.** comm-central has
   `bugmail1` through `bug513543`. php-mime-mail-parser has `issue84`
   through `issue408`.
3. **RFC 5322 Appendix A is the common seed.** Go's first address case is
   annotated `// RFC 5322, Appendix A.1.1`. Stalwart's `address.json` case 1
   is the same `John Doe <jdoe@machine.example>`, and it keeps a whole
   `resources/eml/rfc/` suite.
4. **Fixtures travel between projects with a licence file attached.** The
   2010 Hunnysoft MIME samples became Stalwart's `legacy` suite under a
   per-directory
   [`COPYING`](https://github.com/stalwartlabs/mail-parser/blob/main/resources/eml/legacy/COPYING),
   and are now vendored inside Thunderbird's
   [`third_party/rust/mail-parser`](https://github.com/mozilla/releases-comm-central/tree/master/third_party/rust/mail-parser). Dovecot's
   inline cases became Stalwart's `malformed` and `thirdparty` suites under
   an [MIT
   `COPYING`](https://github.com/stalwartlabs/mail-parser/blob/main/resources/eml/malformed/COPYING).

Two counter-examples show the provenance failure mode. aerc's
[`lib/jwz/testdata/`](https://github.com/rjarry/aerc/tree/master/lib/jwz/testdata)
holds 93 files with SpamAssassin's `NNNN.<md5>` naming and
`netnoteinc.com` headers from August 2002. The origin is documented nowhere
in aerc. notmuch's `test/corpora/lkml/cur` holds 210 real Linux Kernel
Mailing List messages with intact `Received:` chains and real posters'
addresses, and the corpus
[README](https://github.com/notmuch/notmuch/blob/master/test/corpora/README)
describes only the `default` and `broken` directories. Both are third-party
mail sitting in a repository with no stated licence. Neither project set out
to do that. That is the trap to avoid.

Stalwart is the one project that stores expected output as golden files, 214
JSON snapshots that the test regenerates on mismatch. Every other project
asserts field by field in the test source.

## What a synthetic generator must produce

The deterministic layer claims to work with zero models configured. That
makes fixtures the whole test surface. A seeded generator, emitting raw
`.eml` files plus an expected-result JSON per file, must cover the following.

### Receipt and billing shapes

Twelve message kinds, per vendor template:

1. First charge / order confirmation.
2. Recurring renewal, monthly.
3. Recurring renewal, annual.
4. Trial ending in N days.
5. Payment failed, first dunning notice.
6. Payment failed, final notice before suspension.
7. Card expiring.
8. Plan upgrade with proration.
9. Plan downgrade with credit.
10. Price-change notice, effective on a future date.
11. Refund and partial refund.
12. Cancellation confirmation.

Field variation inside them:

- Amount forms: `$12.00`, `US$12.00`, `USD 12.00`, `12,00 €`, `€12`,
  `¥1,200` with no decimal part, `12.00 USD` after the number.
- Locale decimal separators: `1,234.56` and `1.234,56`.
- Tax lines: no tax, US sales tax, EU VAT with a VAT number, GST, reverse
  charge with a zero amount.
- Period expressed as `Jan 1 – Feb 1, 2026`, as `01/02/2026 - 01/03/2026`
  under both day-first and month-first reading, and as "next 12 months".
- Invoice identifiers, card last-four, and a zero-amount invoice on a 100%
  discount.
- One amount split by a quoted-printable soft break, `$12=\n.00`, and one
  amount with an encoded period, `$12=2E00`.
- Amount present only in the HTML part, with the text part empty.
- Amount present only in a PDF attachment, with neither body part carrying it.

### Sender shapes

- `From` display name against a different envelope sender, so `Return-Path`
  and `From` disagree.
- Per-function subdomains: `billing@`, `receipts@`, `no-reply@`,
  `invoice+acct123@`.
- ESP relays where the DKIM `d=` is `sendgrid.net`, `amazonses.com` or
  `mailgun.org` while the `From` domain is the vendor.
- Vendor changing sending domain between two months of the same
  subscription, to test recurrence grouping.
- Two unrelated vendors on one shared ESP domain, which must not merge.
- IDN and punycode sender hosts.

### `List-Unsubscribe` variants

- `mailto:` only.
- `https:` only.
- Both, in each order.
- Two `mailto:` values.
- Folded across lines per RFC 5322, including a fold inside the URI's angle
  brackets.
- `mailto:` with `?subject=unsubscribe` and with a `body=` parameter.
- Missing angle brackets, which real senders emit.
- A URI longer than 998 octets, forcing a fold.
- `List-Unsubscribe-Post: List-Unsubscribe=One-Click` present and correct.
  RFC 8058 requires the HTTPS URI, the exact key/value pair, and a DKIM
  signature covering both headers
  ([RFC 8058 §3.1](https://www.rfc-editor.org/rfc/rfc8058.txt)).
- `List-Unsubscribe-Post` present with a `http:` URI only, which is invalid
  and must not be offered as one-click.
- `List-Unsubscribe-Post` present with no DKIM signature, likewise invalid.
- `List-Unsubscribe-Post` with a wrong value.
- The header present on a transactional receipt, where offering unsubscribe
  would be wrong.
- Full RFC 2369 sets with `List-Id`, `List-Help`, `List-Post` and
  `Precedence: bulk`, which is the Apache-list shape and must classify as a
  list.
- No unsubscribe header at all, with an unsubscribe link only in the HTML
  body.

### RFC 2047 encoded words

- `Q` and `B` encoding, in `Subject` and in the `From` display name.
- Charsets: `utf-8`, `iso-8859-1`, `windows-1252`, `shift_jis`, `gb2312`,
  `koi8-r`, plus one unregistered charset name.
- Adjacent encoded words separated by whitespace, where the whitespace is
  dropped on decode.
- An encoded word split across a fold.
- A multi-byte character split across two encoded words.
- An encoded word over the 75-character limit, which real senders emit.
- Broken base64 padding and a stray `=?` that is not an encoded word.
- RFC 2231 continuations for an attachment name:
  `filename*0*=utf-8''...` and `filename*1*=...`.

### Multipart and transport edge cases

- `multipart/alternative` with text and HTML.
- `multipart/related` with `cid:` images referenced from the HTML.
- `multipart/mixed` carrying a PDF invoice.
- `alternative` nested inside `mixed` inside `related`.
- Missing closing boundary.
- The boundary string appearing as literal body text.
- Non-empty preamble and epilogue.
- CRLF and bare-LF line endings, and one file mixing both.
- `8bit` content declared as `7bit`.
- Declared charset disagreeing with the actual bytes.
- `Content-Type` with no `charset` parameter.
- HTML-only message with no text part.
- Duplicate `Subject` and duplicate `From`.
- Missing `Date`, missing `Message-ID`, and a duplicate `Message-ID` across
  two files.
- Obsolete and broken date forms: `-0000`, `GMT`, `UT`, two-digit years.
- Base64 body with no padding and with wrapped lines.

### Container edge cases

- mbox with `From ` lines inside bodies, in both `mboxo` and `mboxrd`
  escaping.
- Google Takeout mbox carrying `X-Gmail-Labels` and `X-GM-THRID`.
- maildir input alongside mbox input.
- A single message over 25 MB.

### Time-series shapes

Recurrence detection needs whole sequences, not single messages:

- 24 consecutive monthly receipts from one vendor.
- An annual plan across 3 years.
- A subscription that stops after 7 months.
- A price increase mid-sequence.
- Two overlapping subscriptions from one vendor.
- A one-off purchase that must not read as recurring.
- Duplicate delivery of the same receipt to two folders.

## Not verified

- **TREC 2005/2006/2007 spam corpora.** Every publisher link returns 404.
  Licence, size and format stay unknown. No mirror was used.
- **EDRM Enron data set terms.** The data set page redirects to a generic
  data sets index, and the Enron entry is gone. The CC BY 4.0 line is the
  site-wide footer. It says nothing specific about that data set.
- **`lore.kernel.org`.** It returns 403 to a plain HTTP client. Its archives
  are public-inbox git repositories, and this survey ran no git command, so
  its contents and its stated terms stay unchecked.
- **Ling-Spam and Enron-Spam licence.** Both readmes are silent on
  licensing. Silence is not permission. The authors were not contacted.
- **Full Enron corpus.** The figures above cover a 78,457-message prefix. The
  publisher states about 0.5M messages. The proportions should hold. The
  absolute counts do not.
- **Avocado fee and exact redistribution clause.** The LDC catalogue page
  names two agreements and a fee behind a login. The agreements stay unread.
- **Whether the ASF ever relicensed the SpamAssassin corpus.** The directory
  holds `readme.html`, the tarballs and an `obsolete/` directory. There is no
  `LICENSE` file. The readme's sentence about sender copyright is the only
  stated position.
- **isync/mbsync and aerc upstream.** Both live outside GitHub, on
  SourceForge and `git.sr.ht`. The survey used GitHub mirrors, which may lag.
- **The origin of aerc's 93 threading fixtures and of notmuch's `lkml`
  corpus.** Both came from the message headers, because neither repository
  states a provenance.

## Decision consequence (v0)

Ship a synthetic generator as the primary fixture source, and use public
corpora only as read-only, downloaded-on-demand parser stress input.

1. **Commit synthetic fixtures.** Hand-written and generator-emitted `.eml`
   files under the repository, with an expected-output JSON beside each,
   following Stalwart's golden-file shape. Split by test layer as every
   surveyed project does: inline table cases for header parsing,
   whole-message files for MIME and receipt extraction. Keep the files small,
   one per behaviour, named after the behaviour or the issue. This is the
   only source that can cover SaaS receipts, price-change notices and RFC
   8058 one-click at all, because no public corpus contains them.
2. **Commit no third-party mail.** No Enron, no SpamAssassin, no Apache list
   mail in the repository. Enron is unlicensed personal data. SpamAssassin
   leaves copyright with the senders. Apache list mail carries erasure rights
   that a git history cannot serve. If a third-party fixture ever does enter
   the tree, it enters with a per-directory `COPYING` naming its source, the
   way Stalwart and Thunderbird do it. aerc and notmuch show what happens
   without that rule.
3. **Add an opt-in corpus fetcher for stress runs.** A developer-only command
   downloads Untroubled's latest month into an ignored directory and runs the
   parser over it for crash and timeout coverage. Untroubled is the only
   archive with both an unrestricted grant and modern RFC 8058 headers. Follow
   notmuch's shape: the archive stays out of git, the fetch is a separate make
   target, and the download is checked before use. Assert on "does not crash"
   and on header extraction. Never on receipt semantics, because the billing
   content is fraudulent. Never send these messages, because both publishers
   ask that their corpora stay out of live mail systems.
4. **Fetch Apache list mbox as the negative set.** One month of one list,
   fetched on demand, asserting that our layer classifies it as a mailing
   list and never as a paid subscription.
5. **Accept contributor fixtures only as redacted synthetic derivatives.**
   A contributor who hits a parser bug on their own mail submits a
   reconstructed message with invented names, addresses, amounts and URLs
   that reproduces the bug. Real mail never enters a pull request. State this
   in `CONTRIBUTING.md` before the first external contribution arrives.

Cost of the choice: our fixtures encode our own assumptions about what a
Stripe or a Vercel receipt looks like, so a template drift in the real world
shows up as a field bug in the wild rather than as a red test. The mitigation
is the community-maintained data path the product already has. Receipt
templates become pull-requestable data files, in the same way as unsubscribe
recipes.
