# Map: v0 stack decisions

Label: wayfinder:map

## Destination

The v0 technical stack, decided and recorded as ADRs under `docs/adr/`.
One ADR per resolved decision ticket. No architecture document, no
implementation backlog: the map is done when nothing remains to decide
before a builder starts.

## Notes

- Domain: local-first desktop app. Product scope is fixed by `CONTEXT.md`;
  the surface is fixed by `docs/design/screens.md`. Read both before any
  ticket.
- Locked before charting, so no ticket exists for them: **Tauri** as the
  shell, **SvelteKit** as the UI framework.
- Hard constraints every decision must satisfy: Tier 0 (no model) delivers
  the complete first-run experience; mail never leaves the machine; no
  telemetry of any kind; MIT license, so every dependency must be
  compatible.
- Skills each session consults: `/grilling` and `/domain-modeling` for
  decision tickets, `/research` for research tickets, `/prototype` for
  prototype tickets.
- Each decision ticket closes by writing `docs/adr/NNNN-<slug>.md`, numbered
  from `0001`. Research tickets write to `docs/reference/` and produce no ADR.
- Ticket 06 is the root. Eleven tickets wait on it.

## Decisions so far

<!-- one line per closed ticket: gist + link -->

- [Test mail corpora](issues/05-mail-corpus-survey.md) — no public corpus holds modern SaaS receipts; Enron and SpamAssassin are not licence-clean and Enron carries real PII. A seeded synthetic generator with committed .eml plus golden JSON is the primary fixture source; Untroubled is the one unrestricted archive, usable as stress input only.

- [Shipping a Tauri binary via npm and curl | sh](issues/04-tauri-distribution-precedent.md) — the quarantine-free claim holds, because quarantine is set by the downloading app, not the OS; the GitHub Releases page and Homebrew Cask are the two holes. The Tauri updater uses minisign, so an Apple-unsigned build self-updates in full.

- [Tier 1 local model runtimes](issues/02-local-model-runtime-survey.md) — cheapest Tier 1 is detecting an OpenAI-compatible endpoint the user already runs, which reuses the Tier 2 client; fallback is a 23 MB llama.cpp sidecar fetched on enable. Model licensing bites: Ministral 2410 is research-only, Gemma 3 repos are gated.

- [Mail library maturity: Rust vs TypeScript](issues/01-mail-library-survey.md) — TypeScript is better supplied; the gap is IMAP and it is large (rust-imap unmaintained since 2021 vs ImapFlow at 1.39M downloads/week). MIME is near parity. RFC 8058 is unimplemented in both, so we write it either way.

- [Browser automation options for cancellation](issues/03-browser-automation-survey.md) — headed browser required; Chrome 136 closed default-profile attach; survivors are a dedicated automation profile on the installed Chrome or a `chrome.debugger` extension; `playwright-rust` is dead since 2022, which costs a Rust core.

## Not yet specified

- Diagnostics and error reporting without telemetry — what the "Report an
  issue" screen (S15) can capture, and what a local log looks like. Shape
  depends on 06 and 08.
- What the GitHub Releases page offers, given it is the canonical address and
  a browser download quarantines the bundle. Graduates from 16.
- Linux and Windows support cost, and how much of it CI absorbs. Graduates
  from 16 and 17.
- Libraries for the Gmail bring-your-own OAuth client flow (step 3 of the
  ingestion ladder). Depends on 06.
- Where receipt-format ground truth comes from for real SaaS vendors, given no
  public corpus holds any and no real mail may enter the repository.
  Graduates from 15.
- Default local model and its licence, and the `CONTEXT.md` wording that names
  Ministral and Gemma. Graduates from 11.

## Out of scope

- Landing site stack for emailterminator.com. Separate product surface, no
  runtime coupling to the app, blocks nothing here.
- Module decomposition and any architecture document. The destination is
  stack decisions only.
- Hosted version, mobile apps, virtual cards, email aliasing. Deferred in
  `CONTEXT.md` with their own triggers.
