# Test mail corpora

Type: research
Status: resolved
Blocked by: —
Output: docs/reference/mail-corpus-survey.md

## Question

Where does a realistic test corpus come from, given that no contributor's
real mail may enter the repository?

Cover:

- Public mail archives usable in an MIT project — Enron, Apache and other
  mailing-list archives, SpamAssassin corpora, Ling-Spam. Report license,
  size, format, and whether they contain the receipts, invoices and
  newsletters we need.
- What a synthetic generator would have to produce to exercise the
  deterministic layer: receipt formats from real SaaS vendors, price-change
  mails, `List-Unsubscribe` header variants, multipart and encoded-word
  edge cases.
- How other mail tools fixture their parser tests.
- Whether any corpus carries personal data that makes redistribution in the
  repository a problem.

## Answer

Findings: `docs/reference/mail-corpus-survey.md`.

**No public mail corpus contains modern SaaS receipts, invoices or
price-change notices.** The only public "receipt" datasets are OCR image sets
of paper receipts. The deterministic layer's core job has no public ground
truth.

- Enron (CMU) states no licence at all, only a privacy request. Measured over
  a 78,457-message prefix it is 100% flat `text/plain` with attachments
  stripped, 7 `List-Unsubscribe` headers in total, and 0.07% receipt-shaped
  mail. It exercises neither a MIME parser nor a receipt parser.
- Enron also carries real names and personal addresses. EDRM/Nuix removed over
  10,000 PII items from the PST edition in 2013, including 60 credit-card and
  572 national-ID items. Vendoring it puts unlicensed personal data into git
  history that no fork can erase.
- SpamAssassin's own readme leaves copyright with the original senders, so it
  is not licence-clean. Its content is 2002 mail with zero RFC 8058 headers
  across 6,047 messages.
- Ling-Spam is tokenized bodies with no headers. Enron-Spam inherits the Enron
  problem. TREC corpora 404 at every publisher link. Avocado needs two signed
  agreements and a fee.
- Untroubled (Bruce Guenter) is the one archive with an explicit unrestricted
  grant, and it is current: July 2026 holds 2,161 messages, 1,499 with
  `List-Unsubscribe` and 1,497 with `List-Unsubscribe-Post`. Its billing-shaped
  mail is fraudulent, so it is parser stress input, never receipt ground truth.
- Apache list mbox export works over HTTP and makes a good negative fixture
  set, but carries erasure rights a repository cannot serve.
- Across 18 parser suites in 17 open-source mail projects, **zero** use a
  downloaded corpus for correctness. Eight are inline literals only; ten
  combine small committed raw files with inline cases.

Recommendation carried into ticket 15: a seeded synthetic generator producing
committed `.eml` files with golden JSON as the primary fixture source; no
third-party mail in the repository; opt-in fetchers for Untroubled (stress)
and Apache lists (negative set); contributor bug reports accepted only as
redacted synthetic reconstructions.
