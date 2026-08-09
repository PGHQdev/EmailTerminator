# Gmail programmatic access — research summary (August 2026)

Compiled from four web-research passes. Informs the v0 ingestion decision.

## Official read paths for consumer Gmail

| Method | Dev verification / CASA | User friction | Freshness |
|---|---|---|---|
| Gmail API restricted scopes (`gmail.readonly`, `mail.google.com`) | Verification + annual CASA Tier 2 | Low | Live, push via Pub/Sub |
| `gmail.metadata` (headers only, no bodies) | Same as above | Low | Live |
| IMAP XOAUTH2 (needs full `mail.google.com` scope) | Verification; CASA possibly waived for on-device-only clients (discretionary) | Low | Live (IDLE) |
| IMAP with app password (requires 2FA) | None | Medium: enable 2SV, generate, paste | Live |
| Google Takeout mbox export | None | High, manual; schedulable every 2 months | Snapshot |
| Apps Script owned by the user (labnol pattern) | None (personal-use exception) | Medium: copy sheet, accept warning | Near-live; 20k Gmail calls/day quota |
| Auto-forward / filter-forward | None | Medium, per account | Live, new mail only |
| Data Portability API | n/a | n/a | No Gmail support as of Aug 2026 |

- App passwords survive the 2022–2025 "less secure apps" shutdowns. Google publishes no sunset date. Third-party claims of a 2026 phase-out are unconfirmed.
- Sources: [restricted-scope verification](https://developers.google.com/identity/protocols/oauth2/production-readiness/restricted-scope-verification), [restricted scopes list](https://support.google.com/cloud/answer/13464325), [XOAUTH2](https://developers.google.com/workspace/gmail/imap/xoauth2-protocol), [app passwords](https://support.google.com/accounts/answer/185833), [Apps Script quotas](https://developers.google.com/apps-script/guides/services/quotas).

## How open-source projects handle it

Three live strategies:

1. **Own verified client + CASA** — Thunderbird desktop (inferred), K-9/Thunderbird Android (confirmed, NetSentries), Mimestream (confirmed, TAC Security, ~$540–1,800/yr recurring). Frictionless UX; annual cost and audit.
2. **User-supplied OAuth client** — rclone (retiring its shared client during 2026), Home Assistant, Gmail MCP servers, gmvault (forced there by Google), mbsync/OfflineIMAP. Dominant 2026 pattern for developer-facing tools. Zero project cost; setup burden on each user.
3. **Shared unverified credentials in the repo** — lieer. Works for niche tools; shared quota, revocation risk (gmvault precedent).

inbox-zero splits: verified client for its hosted product, BYO client for self-hosters.

## Hermes and OpenClaw precedent

- Both ship Gmail via the user's own Google Cloud OAuth client (no shared client ID), with guided/agent-driven setup, and expect the unverified-app warning + "publish to production" step.
- Both bundle a Himalaya IMAP/SMTP skill using Gmail app passwords as the email-only path, recommended when the user wants to skip Google Cloud setup.
- Our ICP already tolerates both flows.

## CASA piggyback options

- No open-source project lends out its verified client; doing so without consent violates Google OAuth policy.
- Free managed-OAuth middlemen exist (Composio ~20k tool calls/mo, Arcade, Smithery), but mail transits their servers: incompatible with local-first positioning.
- Apps Script copy-per-user is the one free, verification-free, Google-side path (labnol's Gmail Unsubscriber precedent); quota-bound, clunky install.
- The "local-only CASA exemption" reading of Google's rule (assessment applies to apps that move data through a third-party server) is contradicted in practice by Mimestream, a local-only client that still passed CASA. Treat the exemption as discretionary; budget for CASA if we ever ship our own client.

## Decision consequence (v0)

Ingestion ladder, lowest friction burden on the project:
1. mbox import (Takeout) — zero-auth baseline, powers discovery + dashboard + RFC 8058 one-click unsubscribe (HTTP POST needs no mailbox access).
2. IMAP with app password — live sync, zero developer verification, works for Gmail/iCloud/Fastmail/Outlook.
3. BYO Google OAuth client (guided setup, Hermes/OpenClaw-style) — optional, for Gmail API features (history sync, push).
4. Own verified client + CASA — deferred until sponsorship funds ~$1–2k/yr; also unlocks the hosted normie version.
