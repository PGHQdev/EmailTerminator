# Mail library maturity: Rust vs TypeScript

Type: research
Status: resolved
Blocked by: —
Output: docs/reference/mail-library-survey.md

## Question

For Rust and for TypeScript/Node, what mature libraries cover each mail job
v0 needs, and how do the two ecosystems compare?

Jobs to cover:

- mbox parsing at Google Takeout scale — multi-gigabyte files, streaming,
  memory behaviour.
- MIME parsing — multipart, RFC 2047 encoded words, RFC 2231 parameters,
  charset conversion, malformed real-world mail.
- IMAP client — app-password auth, IDLE, partial and header-only fetch,
  and behaviour against Gmail, iCloud, Fastmail, Outlook.
- `List-Unsubscribe` and `List-Unsubscribe-Post` handling for RFC 8058
  one-click.

For each candidate report: maintenance status, last release, license (MIT
compatibility), download or star signal, and known correctness gaps.

This ticket exists to arm ticket 06. State plainly which ecosystem is
better supplied, and where the gap would force us to write a parser.

## Answer

Findings: `docs/reference/mail-library-survey.md`.

**TypeScript/Node is better supplied for v0. The gap is concentrated in IMAP,
and it is large.**

- IMAP, Node: ImapFlow 1.6.6 (2026-08-07, MIT, 1.39M downloads/week, zero open
  issues) ships Gmail `X-GM-EXT-1` labels, automatic IDLE / CONDSTORE / QRESYNC
  negotiation, and partial `source` fetch in its shipped types.
- IMAP, Rust: rust-imap's own README asks for maintainers; no stable release
  since 2.4.1 (2021-01-13); open issues on IDLE (#300, #307) and a Gmail
  connect hang (#219). The one credible alternative, async-imap 0.11.3, is
  effectively Delta Chat's internal dependency, with thin adoption elsewhere.
- mbox: npm `mbox-reader` is the only purpose-built option and handles
  multi-GB files, `.mbox.gz`, and `X-Gmail-Labels`. Rust's
  `mail-parser::mailbox::mbox` streams over `BufRead` but needs a gzip wrapper
  and label handling from us. Both dedicated Rust mbox crates are archived or
  unused.
- MIME: near parity. mail-parser covers more RFCs in one crate (41 charsets,
  UTF-7, RFC 6532); Node reaches the same coverage by stacking mailparser,
  libmime and iconv-lite. Rust has better hardening evidence through Delta Chat.
- Live Rust risk: mail-parser #156 silently truncates multipart body parts on a
  mid-line boundary match; #155 panics on a folded `Received` header.
- RFC 8058 is a wash. Neither ecosystem implements one-click unsubscribe. We
  write that module either way.
- Licensing is a non-issue. Every candidate is MIT-compatible.
- Counterweight recorded for ticket 06: `CONTEXT.md` commits v0 to Tauri, so a
  Rust process exists regardless, and a Node core would ship as a sidecar.

Unverified, and now ticket 18: which mbox variant Google Takeout actually emits.
