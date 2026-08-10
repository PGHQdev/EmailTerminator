# Mail library maturity: Rust vs TypeScript — survey (August 2026)

All figures read from crates.io, the npm registry, and the GitHub API on
2026-08-10. Download windows differ per registry: crates.io reports the last
90 days, npm reports the last week. Compare within an ecosystem only.

Arms ticket 06 (core language). Product constraints: MIT-licensed,
local-first, mbox-first ingestion, IMAP with app password second.
See CONTEXT.md.

## Job 1 — mbox parsing at Google Takeout scale

| Library | Eco | Latest | Released | License | Adoption | Streaming |
|---|---|---|---|---|---|---|
| [mail-parser `mailbox::mbox`](https://github.com/stalwartlabs/mail-parser/blob/main/src/mailbox/mbox.rs) | Rust | 0.11.5 | 2026-07-08 | Apache-2.0 OR MIT | 1.18M dl/90d, 453 stars | Yes, `BufRead` iterator |
| [mailbox-formats](https://crates.io/crates/mailbox-formats) | Rust | 0.1.3 | 2026-06-04 | MIT OR Apache-2.0 | 1,020 dl total, [0 stars](https://github.com/planetaryescape/mailbox-formats) | Claims mboxo/rd/cl/cl2 |
| [mbox-reader](https://crates.io/crates/mbox-reader) | Rust | 0.2.0 | 2019-09-27 | MIT/Apache-2.0 | 384 dl/90d; [repo archived](https://github.com/djc/mbox-reader) | Dead |
| [mbox-reader](https://www.npmjs.com/package/mbox-reader) | Node | 1.2.0 | 2024-05-31 | MIT | 4.4k dl/wk | Yes, async generator |
| [node-mbox](https://github.com/SpongeData-cz/node-mbox) | Node | 2.0.0 | 2023-04-06 | MIT | 2.6k dl/wk, 5 stars | Yes |

Node's `mbox-reader` is the only library in either ecosystem written for this
exact input. Its [README](https://github.com/postalsys/mbox-reader/blob/master/README.md)
states support for multi-gigabyte files, accepts a gzipped
`gmail-takeout.mbox.gz` via a `gz` option, and yields `X-Gmail-Labels` as a
`labels` array plus `Status`/`X-Status` flags. It documents one limit: it
assumes From-munging, so mboxcl2 files fail.

Rust's usable path is `mail-parser`'s `mailbox::mbox::MessageIterator`. It is
a `BufRead` iterator, holds one message in memory, and unquotes `>From `
per the [qmail mbox spec](http://qmail.org/qmail-manual-html/man5/mbox.html).
Two facts from reading the source: it treats any line beginning with `From `
as a message separator without requiring a preceding blank line, and it has no
gzip layer and no Gmail label extraction. Those are ~40 lines of our own code
over `flate2` plus a header read.

The two dedicated Rust mbox crates are unusable. `mbox-reader` is archived
since 2021. `mailbox-formats` and `list-unsubscribe` were both created
2026-05-16 by one author, have 0 stars, and have no external users.

## Job 2 — MIME parsing

| Library | Eco | Latest | Released | License | Adoption |
|---|---|---|---|---|---|
| [mail-parser](https://crates.io/crates/mail-parser) | Rust | 0.11.5 | 2026-07-08 | Apache-2.0 OR MIT | 1.18M dl/90d, 453 stars |
| [mailparse](https://crates.io/crates/mailparse) | Rust | 0.16.1 | 2025-02-27 | 0BSD | 3.08M dl/90d, 226 stars |
| [email-parser](https://crates.io/crates/email-parser) | Rust | 0.5.0 | 2020-12-17 | MIT | 420 dl/90d; last push 2023-04-29 |
| [rfc2047-decoder](https://crates.io/crates/rfc2047-decoder) | Rust | 1.1.2 | 2026-05-20 | MIT | 1.32M dl/90d |
| [charset](https://crates.io/crates/charset) | Rust | 0.1.5 | 2024-07-21 | Apache-2.0 OR MIT | 4.15M dl/90d |
| [mailparser](https://www.npmjs.com/package/mailparser) | Node | 3.9.15 | 2026-08-07 | MIT | 4.16M dl/wk, 1,672 stars |
| [postal-mime](https://www.npmjs.com/package/postal-mime) | Node | 2.7.6 | 2026-08-07 | MIT-0 | 7.15M dl/wk, 544 stars |
| [libmime](https://www.npmjs.com/package/libmime) | Node | 5.4.2 | 2026-08-07 | MIT | 6.26M dl/wk |
| [iconv-lite](https://www.npmjs.com/package/iconv-lite) | Node | 0.7.3 | 2026-07-03 | MIT | 277M dl/wk |
| [letterparser](https://www.npmjs.com/package/letterparser) | Node | 0.1.8 | 2024-09-19 | BSD-3-Clause-Clear | 38k dl/wk, 51 stars |
| [emailjs-mime-parser](https://www.npmjs.com/package/emailjs-mime-parser) | Node | 2.0.7 | 2019-03-25 | MIT | 13k dl/wk; dormant |

RFC coverage, verified in source or in the project's own README:

| Requirement | Rust | Node |
|---|---|---|
| RFC 2047 encoded words | mail-parser ([README RFC list](https://github.com/stalwartlabs/mail-parser#conformed-rfcs)), mailparse, rfc2047-decoder | libmime `decodeWord`, postal-mime `decodeWords` |
| RFC 2231 parameter continuations | mailparse `parse_param_content` ("RFC 2184, Section 4"), mail-parser README | libmime key regex `/\*((\d+)\*?)?$/`, postal-mime `decodeParameterValueContinuations` |
| RFC 2231 language tag in charset | mail-parser | libmime and postal-mime both strip at `charset.indexOf('*')` |
| Charset breadth | mail-parser: 41 sets per its README, of which 34 are built in and the rest sit behind the optional `encoding_rs` feature. UTF-7 (RFC 2152) is built in | mailparser via iconv-lite, which ships a `utf7` codec; postal-mime via `TextDecoder`, so the WHATWG set only |
| RFC 6532 internationalised headers | mail-parser | Not claimed by either Node parser |

Known correctness gaps:

- mail-parser carries three open parse defects filed in July 2026:
  [#156 MIME boundary matched mid-line, silently truncating multipart body
  parts](https://github.com/stalwartlabs/mail-parser/issues/156),
  [#155 panic in Received-header parsing when input ends with a folded
  line](https://github.com/stalwartlabs/mail-parser/issues/155), and
  [#153 permissive header parsing returns an email address where there should
  be none](https://github.com/stalwartlabs/mail-parser/issues/153). A fourth,
  [#109](https://github.com/stalwartlabs/mail-parser/issues/109), reports the
  charset attribute going out of sync with the actual encoding after decoding.
  Silent truncation of a body part is the one that would corrupt our
  discovery output.
- mailparse's [README](https://github.com/staktrace/mailparse#readme) states it
  "may not follow all the strict requirements in the various specifications",
  and accepts bare LF line endings. It is the parser inside
  [Delta Chat's core](https://github.com/chatmail/core/blob/main/Cargo.toml)
  (`mailparse = "0.16.1"`), which is the strongest real-world hardening signal
  in the Rust set. Its license is 0BSD, which is MIT-compatible and imposes no
  attribution.
- mailparser and postal-mime are the same author's two generations and both
  ship from an active release train (both released 2026-08-07). postal-mime's
  charset support is bounded by `TextDecoder`, so UTF-7 mail decodes wrongly.
- letterparser is BSD-3-Clause-Clear. It is permissive and MIT-compatible, and
  it adds an explicit no-patent-license clause.

Memory behaviour: every parser in both lists parses one whole message from a
buffer. None of them stream a single message's body. That is acceptable, since
the mbox reader bounds the working set to one message.

## Job 3 — IMAP client

| Library | Eco | Latest | Released | License | Adoption | State |
|---|---|---|---|---|---|---|
| [imap (rust-imap)](https://crates.io/crates/imap) | Rust | 3.0.0-alpha.15 | 2025-02-08 | Apache-2.0 OR MIT | 422k dl/90d, 580 stars | Last stable 2.4.1, 2021-01-13 |
| [async-imap](https://crates.io/crates/async-imap) | Rust | 0.11.3 | 2026-07-17 | MIT OR Apache-2.0 | 583k dl/90d, 146 stars | Active |
| [imap-proto](https://crates.io/crates/imap-proto) | Rust | 0.16.7 | 2026-04-21 | MIT OR Apache-2.0 | 1.03M dl/90d | Parser only |
| [imap-codec](https://crates.io/crates/imap-codec) / [imap-types](https://crates.io/crates/imap-types) | Rust | 2.0.0-alpha.9 | 2026-07-19 | MIT OR Apache-2.0 | 24k dl/90d, 50 stars, 57 open issues | Alpha, sans-I/O codec |
| [imap-next](https://crates.io/crates/imap-next) | Rust | 0.3.4 | 2026-02-18 | MIT OR Apache-2.0 | 12k dl/90d, 22 stars | Pre-1.0 |
| [imapflow](https://www.npmjs.com/package/imapflow) | Node | 1.6.6 | 2026-08-07 | MIT | 1.39M dl/wk, 562 stars, 0 open issues | Active |
| [imap (node-imap)](https://www.npmjs.com/package/imap) | Node | 0.8.19 | 2016-12-06 | MIT | 560k dl/wk, 2,226 stars, 192 open issues | Last push 2023-11-10 |
| [imap-simple](https://github.com/chadxz/imap-simple) | Node | 5.1.0 | 2021-06-01 | MIT | 234k dl/wk; repo archived | Dead |
| [emailjs-imap-client](https://www.npmjs.com/package/emailjs-imap-client) | Node | 3.1.0 | 2020-02-14 | MIT | 6.7k dl/wk, 571 stars | Dormant |

This is where the two ecosystems separate.

rust-imap prints a maintenance notice in its own
[README](https://github.com/jonhoo/rust-imap#readme): "This crate is looking
for maintainers". It has shipped no stable release since 2.4.1 in January
2021, and sits on `3.0.0-alpha` five years later. Its 44 open issues include
[#300 wrongful IDLE timeout reset on response](https://github.com/jonhoo/rust-imap/issues/300)
(2025-01), [#307 delay in receiving emails with IDLE in Rust compared to
Node.js](https://github.com/jonhoo/rust-imap/issues/307) (2025-02), and
[#219 program hanging when trying to connect to
gmail](https://github.com/jonhoo/rust-imap/issues/219), open since 2021.
It does support arbitrary FETCH
query strings, so `BODY.PEEK[HEADER.FIELDS (...)]` header-only fetch works,
and `idle()` exists.

async-imap is the credible Rust option. It releases regularly, and
[Delta Chat's core depends on it](https://github.com/chatmail/core/blob/main/Cargo.toml)
at `0.11.3` with `runtime-tokio` and `compress`. Delta Chat runs against
consumer providers at scale, so app-password login, IDLE, and partial fetch are
exercised in production. Outside Delta Chat its reverse-dependency list is
small and hobby-grade.

imap-codec / imap-next are the best-engineered Rust IMAP work, and they are
alpha sans-I/O layers with no client on top.

ImapFlow is a complete client with a commercial maintainer (Postal Systems,
which also sells EmailEngine). Its
[README](https://github.com/postalsys/imapflow#readme) claims automatic
extension negotiation (CONDSTORE, QRESYNC, IDLE, COMPRESS), Gmail labels and
raw search via `X-GM-EXT-1`, and documented Gmail, Outlook, and Yahoo
configuration. Its `FetchQueryObject`
[type definition](https://github.com/postalsys/imapflow/blob/master/lib/imap-flow.d.ts)
gives exactly the fetch shapes v0 wants: `envelope`, `size`, `bodyStructure`,
`bodyParts`, `threadId`, Gmail `labels`, and a partial `source` with `start`
and `maxLength`. It reports 0 open issues.

node-imap has 2,226 stars and 560k weekly downloads on a package last
published in December 2016, with 192 open issues. Treat its download count as
legacy, not as support.

## Job 4 — List-Unsubscribe and RFC 8058 one-click

| Library | Eco | What it gives | Gap |
|---|---|---|---|
| [mail-parser](https://github.com/stalwartlabs/mail-parser/blob/main/src/parsers/fields/list.rs) | Rust | `HeaderName::ListUnsubscribe` parsed by `parse_comma_separared` into `HeaderValue::TextList` | Returns raw comma tokens with `<>` intact. No `ListUnsubscribePost` variant, so RFC 8058's `List-Unsubscribe-Post` falls through to `HeaderName::Other` |
| [list-unsubscribe](https://crates.io/crates/list-unsubscribe) | Rust | Claims RFC 2369 + RFC 8058 parsing into a typed enum. 0.1.3, 2026-06-11, MIT OR Apache-2.0 | 1,457 downloads, [0 stars](https://github.com/planetaryescape/list-unsubscribe), created 2026-05-16, one author. Unproven |
| [mailparser](https://github.com/nodemailer/mailparser/blob/master/lib/mail-parser.js) | Node | Collapses every `List-*` header into a structured `list` object via `parseListHeader` | Runs `addressparser` over the value, which is not the RFC 2369 grammar, and keeps a single `url` and a single `mail` per key. Later values overwrite earlier ones |
| postal-mime, ImapFlow | Node | Raw header string only | Nothing structured |

Neither ecosystem ships a library that implements the RFC 8058 flow. The flow
itself is small: parse the angle-bracket list from `List-Unsubscribe`, confirm
`List-Unsubscribe-Post: List-Unsubscribe=One-Click`, then POST
`List-Unsubscribe=One-Click` as `application/x-www-form-urlencoded` to the
`https` URI. We write this in either language. This job does not move the
decision.

## License summary

Every candidate we would actually pick is MIT-compatible.

| License seen | Libraries | MIT-compatible |
|---|---|---|
| MIT | mailparser, imapflow, libmime, iconv-lite, mbox-reader (npm), lettre, rfc2047-decoder | Yes |
| MIT-0 | postal-mime, nodemailer | Yes, no attribution required |
| Apache-2.0 OR MIT | mail-parser, imap, async-imap, imap-proto, imap-codec, charset | Yes, take the MIT arm |
| 0BSD | mailparse | Yes, no attribution required |
| BSD-3-Clause-Clear | letterparser | Yes, adds an explicit no-patent-grant clause |
| (MIT OR EUPL-1.1+) | mailsplit | Yes on the MIT arm; the package is deprecated in favour of `@zone-eu/mailsplit` |

mail-parser's `LICENSES/` directory holds `MIT.txt` and `Apache-2.0.txt`, and
each source file carries `SPDX-License-Identifier: Apache-2.0 OR MIT`. GitHub
reports `NOASSERTION` for mailparser, postal-mime, and imapflow because their
LICENSE files omit the standard title line; the file bodies are MIT text and
the registry metadata says MIT or MIT-0.

## What we could not verify

- Google's own documentation for the mbox variant Takeout produces. We found
  no Google page stating mboxo, mboxrd, or mboxcl2. This matters: npm
  `mbox-reader` fails on mboxcl2 by its own README, and mail-parser assumes
  `>From ` quoting. Confirm against a real Takeout export before committing.
- Any primary evidence of a Rust IMAP client tested against iCloud or
  Fastmail. rust-imap's tracker carries Gmail and Office 365 reports only.
  Delta Chat's use of async-imap implies broad provider coverage; we did not
  find a published provider matrix.
- ImapFlow's provider claims come from its own README and docs. No public
  per-provider test suite exists. imapflow.com doc URLs we tried returned 404,
  so the fetch-option facts above come from the shipped `.d.ts` and source.
- The exact charset list `TextDecoder` resolves in a Node build, which depends
  on the ICU the binary was compiled with. postal-mime's charset breadth is
  therefore environment-dependent.
- Fix status of mail-parser's three July 2026 parse issues. We read titles and
  state (open), not maintainer triage.
- Whether ImapFlow's `0 open issues` reflects real defect absence or issue
  hygiene by a commercial maintainer.

## Decision consequence

**TypeScript/Node is better supplied for v0. The gap is not uniform: it is
almost entirely IMAP, and it is large.**

- **IMAP — Node wins decisively.** ImapFlow is one actively released,
  MIT-licensed, commercially maintained client that already exposes Gmail
  labels, partial source fetch, and automatic IDLE handling. Rust offers one
  client with a public "looking for maintainers" notice and no stable release
  in five years, and one client that is really Delta Chat's internal
  dependency. Choosing Rust means adopting async-imap and inheriting its API
  shape, or maintaining a rust-imap fork ourselves.
- **MIME parsing — near parity, edge to Rust on breadth.** mail-parser covers
  more RFCs in one crate, including UTF-7 and RFC 6532, than any single Node
  package. Node reaches the same coverage by stacking mailparser + libmime +
  iconv-lite, all from one author and all released the same day. mailparse
  plus Delta Chat gives Rust the better hardening evidence; mail-parser's open
  silent-truncation bug is the one live risk in the Rust set.
- **mbox — Node wins on fit, Rust is close.** npm `mbox-reader` handles the
  Takeout case turnkey, including `.mbox.gz` and `X-Gmail-Labels`.
  mail-parser's mbox iterator is streaming and maintained, and needs a gzip
  wrapper and label handling from us.
- **RFC 8058 — a wash.** Both ecosystems make us write it.

**Where we write our own parser, in either language:** the RFC 8058 one-click
layer, and the `List-Unsubscribe` header grammar itself. mail-parser hands
back raw comma tokens; mailparser runs an address parser over a URL list and
drops duplicates. Neither is the RFC 2369 grammar. Budget one small, well
tested module for header-to-action extraction regardless of the language
choice.

**The one constraint that pulls back:** CONTEXT.md commits v0 to a Tauri
desktop app, so a Rust process exists in the product either way. A Node core
ships as a sidecar binary inside that Tauri app. Ticket 06 should price that
packaging cost against the IMAP maintenance cost named above, since those are
the two real quantities in this decision.

## Sources

Registry and repository metadata read 2026-08-10 via
`https://crates.io/api/v1/crates/<name>`,
`https://registry.npmjs.org/<name>`,
`https://api.npmjs.org/downloads/point/last-week/<name>`, and the GitHub REST
API.

- [mail-parser](https://github.com/stalwartlabs/mail-parser) — [mbox module](https://github.com/stalwartlabs/mail-parser/blob/main/src/mailbox/mbox.rs), [List field parser](https://github.com/stalwartlabs/mail-parser/blob/main/src/parsers/fields/list.rs)
- [mailparse](https://github.com/staktrace/mailparse)
- [rust-imap](https://github.com/jonhoo/rust-imap), [async-imap](https://github.com/chatmail/async-imap), [imap-codec](https://github.com/duesee/imap-codec), [imap-next](https://github.com/duesee/imap-next)
- [Delta Chat core Cargo.toml](https://github.com/chatmail/core/blob/main/Cargo.toml)
- [mailparser](https://github.com/nodemailer/mailparser), [libmime](https://github.com/nodemailer/libmime), [postal-mime](https://github.com/postalsys/postal-mime), [iconv-lite](https://github.com/pillarjs/iconv-lite)
- [ImapFlow](https://github.com/postalsys/imapflow) and its [type definitions](https://github.com/postalsys/imapflow/blob/master/lib/imap-flow.d.ts)
- [mbox-reader (npm)](https://github.com/postalsys/mbox-reader), [node-mbox](https://github.com/SpongeData-cz/node-mbox), [mbox-reader (crate, archived)](https://github.com/djc/mbox-reader)
- [list-unsubscribe](https://crates.io/crates/list-unsubscribe), [mailbox-formats](https://crates.io/crates/mailbox-formats)
- RFCs: [2045](https://datatracker.ietf.org/doc/html/rfc2045), [2046](https://datatracker.ietf.org/doc/html/rfc2046), [2047](https://datatracker.ietf.org/doc/html/rfc2047), [2048](https://datatracker.ietf.org/doc/html/rfc2048), [2049](https://datatracker.ietf.org/doc/html/rfc2049), [2231](https://datatracker.ietf.org/doc/html/rfc2231), [2369](https://datatracker.ietf.org/doc/html/rfc2369), [3501](https://datatracker.ietf.org/doc/html/rfc3501), [8058](https://datatracker.ietf.org/doc/html/rfc8058)
- [qmail mbox specification](http://qmail.org/qmail-manual-html/man5/mbox.html)
