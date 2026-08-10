# 1. Rust for the core

Date: 2026-08-10

Status: Accepted

## Context

The v0 stack needs one language to hold the domain logic: mbox and IMAP
ingestion, MIME parsing, subscription and newsletter detection, receipt
extraction, storage, and the unsubscribe and cancellation actions.

Two constraints were fixed before this decision. Tauri is the desktop shell and
SvelteKit is the UI framework. Both put a second language in the project
regardless of what the core is written in: Tauri means Rust exists in the app,
and SvelteKit means TypeScript exists in the repository.

Live IMAP sync is in v0 scope, so the quality of the IMAP client is a first-order
input. Contributor reach is not an input: recipes, playbooks and the
critical-services list are data files (see ADR 9), so contributing to them
requires no core language. Browser automation bindings are not an input either:
the agentic cancellation driver runs as a separate opt-in process fetched on
enable, so its language is independent of the core.

The candidates were a Rust core in-process with Tauri, a TypeScript core on Bun
shipped as a sidecar, and a split with Rust owning ingestion and TypeScript
owning rules.

### Measured throughput

Benchmarked on an Apple M1, over a 500 MB synthetic mbox of 122,800 messages
mixing `text/plain`, `multipart/alternative`, `multipart/mixed` with base64
attachments, and quoted-printable HTML, all carrying RFC 8058 headers. Every
full-parse path decodes bodies, so the comparison is like for like.

| Path | 500 MB | msgs/s | MB/s | 10 GB projected | peak RSS |
|---|---|---|---|---|---|
| Rust `mail-parser`, full parse | 2.27 s | 54,110 | 220 | 0.8 min | 4.5 MB |
| Rust, mbox split only | 0.22 s | 548,471 | 2,234 | 0.1 min | 3.4 MB |
| Node `postal-mime`, full parse | 27.4 s | 4,475 | 18.2 | 9.4 min | 127 MB |
| Node `mailparser`, full parse | 125.9 s | 975 | 4.0 | 43 min | 156 MB |
| Node, split plus headers only | 2.41 s | 50,884 | 207 | 0.8 min | 103 MB |
| Node, headers on all plus full parse on 10% | 5.47 s | 22,435 | 91 | 1.9 min | 113 MB |

Rust is 12x faster than the fastest Node MIME parser and holds 28x less memory.
A TypeScript core clears a ten-minute budget for a 10 GB mailbox only by parsing
bodies for a candidate subset; Rust clears it parsing everything.

`mailparser`, the most-downloaded Node parser, returned nothing for
`list-unsubscribe` across all 122,800 messages while `postal-mime` found every
one. That is unexplained and was not chased down. It is recorded because
download counts were carrying weight in the original library survey.

### Verified IMAP support

`docs/reference/mail-library-survey.md` concluded that Rust's IMAP support was
the decisive weakness. That conclusion was wrong, and the correction is appended
to the survey. Verified from crates.io metadata and extracted crate sources on
2026-08-10:

- `async-imap` 0.11.3, published 2026-07-17, 578,875 downloads in 90 days, eight
  releases since September 2024, repository `chatmail/async-imap`.
- `imap-proto` 0.16.7, published 2026-04-21, 1,021,885 downloads in 90 days.
- `imap-codec` and `imap-types` reached 1.0.0 on 2026-07-19.
- The unmaintained crate is `jonhoo/rust-imap`, in alpha since 2022. That does
  not generalize to the ecosystem.

Feature coverage counted in the sources: MODSEQ and CONDSTORE, VANISHED and
QRESYNC, UIDPLUS, `BODY.PEEK`, IDLE, and Gmail's `X-GM-LABELS`, `X-GM-MSGID` and
`X-GM-THRID`. `imap-proto/src/parser/gmail.rs` parses the Gmail extensions with
tests, and `async-imap` exposes them as typed accessors on FETCH responses.
These are the exact features that had been credited to ImapFlow as its
advantage.

### Maintainership

The Node mail stack traces to a single author: ImapFlow, postal-mime,
`mbox-reader` and mailparser. The Rust stack rests on two independent,
commercially backed groups: Stalwart for `mail-parser`, Delta Chat for
`async-imap`. On bus factor Rust is ahead.

## Decision

**Rust holds the domain logic.** One binary is both the desktop app's core and
the headless CLI, running in-process with the Tauri shell.

TypeScript is confined to two places:

1. The SvelteKit UI.
2. The agentic cancellation driver, which is a separate process behind the
   experimental flag and fetched when the user enables it.

Nothing else in the product is written in TypeScript. Mail, parsing, storage,
rules, actions and model calls are Rust.

## Consequences

- Domain types are defined once in Rust and generated into TypeScript for the
  UI. The generator is not chosen here; it is a separate decision, because the
  choice depends on the shape of the app-to-UI seam.
- The IMAP client crate is not chosen here. `async-imap` is verified viable;
  `imap-codec` and `imap-next` are the alternatives. That is a separate
  decision.
- We write the mbox gzip wrapper and `X-Gmail-Labels` handling ourselves.
  `mail-parser`'s mbox iterator streams but does not cover them.
- We write the RFC 8058 one-click layer and the `List-Unsubscribe` header
  grammar ourselves. No library in either ecosystem implements them.
- `mail-parser` carries two open defects that are now ours to manage: issue #156
  silently truncates multipart body parts on a mid-line boundary match, and
  issue #155 panics on a folded `Received` header. Both sit directly on the scan
  path.
- Rule iteration is slower than a dynamic language would give. This is
  acceptable only because recipes, playbooks and the critical-services list are
  data files. If receipt-extraction rules end up as compiled Rust code rather
  than data, that assumption needs revisiting.
- Contributors to the core need Rust. Contributors to the community data need
  nothing but a text editor, which is where the project's stated 90-day
  contribution goal lives.

### Acceptance criterion

A 10 GB mbox scans in under ten minutes with flat memory. The measurement above
projects 0.8 minutes, so the budget has an order of magnitude of headroom. If a
real Takeout export misses the target, that is a bug in our code, not grounds to
revisit this decision.

### What would reopen this

A sustained failure of the Rust mail libraries that we cannot patch or fork, or
a demonstrated need to change receipt-extraction logic faster than a compile
cycle allows.
