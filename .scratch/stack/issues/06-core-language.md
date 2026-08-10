# Core language: Rust core or TypeScript/Bun core

Type: grilling
Status: resolved
Blocked by: 01
ADR: docs/adr/0001-core-language.md

## Question

Where does the logic live — a Rust core with Tauri as a thin window, or a
TypeScript/Bun core with Tauri's Rust side reduced to a shell?

This is the root of the map. Storage, credentials, the desktop/CLI seam,
the model runtimes, the agent harness, packaging, CI and the test stack all
inherit from it.

Axes to settle:

- Which ecosystem actually supplies the mail libraries (answered by 01).
- The headless CLI and localhost mode ship from the same core
  (`CONTEXT.md`). Which language makes that one binary, and which makes it
  two artifacts?
- npm distribution is a stated v0 channel. A Node/Bun core publishes
  natively; a Rust core needs a binary wrapper.
- Scan performance over a multi-gigabyte Takeout mbox.
- Contributor reach — the project's value is community recipes and
  playbooks, so who can send a pull request matters.
- A split core (Rust for ingestion and parsing, TypeScript for the agent
  layer) is a live third option. Judge it against the cost of two
  toolchains.

The answer names one core language, states what the other side is allowed
to contain, and gives the reason a contributor could not have guessed.

## Answer

**Rust holds the domain logic.** One binary is the desktop core and the headless
CLI, in-process with the Tauri shell. TypeScript is confined to the SvelteKit UI
and to the agentic cancellation driver, which is a separate opt-in process.

ADR: `docs/adr/0001-core-language.md`.

Decided in this order:

1. **IMAP stays in v0.** A Takeout snapshot ages immediately, so the dashboard,
   bulk actions and cancellation confirmation all need live sync.
2. **Contributor reach does not weigh.** Community contribution is data
   (ticket 14), so the core language does not gate it.
3. **The agentic driver may be its own process, fetched on enable.** Browser
   automation bindings therefore stop influencing the core language.
4. **Throughput was measured, not assumed.** On 500 MB / 122,800 synthetic
   messages: Rust `mail-parser` full parse 220 MB/s at 4.5 MB RSS, Node
   `postal-mime` 18.2 MB/s at 127 MB, Node `mailparser` 4.0 MB/s at 156 MB.
   Rust is 12x faster than the best Node parser and uses 28x less memory.
   Projected to 10 GB: 0.8 minutes against 9.4.
5. **The IMAP claim that drove the first draft of this answer was false.**
   Verified from crates.io and extracted sources: `async-imap` 0.11.3
   (2026-07-17, 578k downloads/90d, eight releases since Sept 2024) and
   `imap-proto` 0.16.7 (1.02M downloads/90d) cover MODSEQ, CONDSTORE, QRESYNC,
   VANISHED, UIDPLUS, `BODY.PEEK`, IDLE, and Gmail `X-GM-LABELS` / `X-GM-MSGID`
   / `X-GM-THRID` with a dedicated parser and tests. The unmaintained crate is
   `jonhoo/rust-imap`, and that fact does not generalize. A correction is
   appended to `docs/reference/mail-library-survey.md`.
6. **Maintainership favours Rust.** The whole Node mail stack traces to one
   author. Rust rests on Stalwart and Delta Chat, two independent groups.

Costs accepted: we write the mbox gzip and `X-Gmail-Labels` wrapper, the RFC
8058 layer and the `List-Unsubscribe` grammar; we own `mail-parser` issues #156
(silent multipart truncation) and #155 (panic on a folded `Received` header);
core contributions require Rust.

Acceptance criterion carried forward: a 10 GB mbox scans in under ten minutes
with flat memory.

Two decisions this answer deliberately did not make, now tickets 19 and 20: the
Rust IMAP client crate, and how Rust domain types reach the SvelteKit UI.
