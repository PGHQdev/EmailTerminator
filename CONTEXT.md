# EmailTerminator — Strategy

August 2026

## Origin

A direct answer to Balaji's "THE EMAIL TERMINATOR" product request (Jul 2024).
Full tweet text: docs/reference/balaji-email-terminator-tweet.md

## Positioning

EmailTerminator is a specialized, local-first agent harness that discovers
SaaS subscriptions and newsletters in an inbox, quantifies email volume and
spend, and drives cancellation. It is a focused tool: deterministic parsers
first, domain-specific agent skills second, models third. It is not a
general-purpose agent.

Fully open source under the MIT license. The project's value is credibility,
distribution, and reputation.

## Users

- **v0 and core identity**: the tech-enthusiast, privacy-conscious user —
  agentic-AI adopters, Hacker News / Product Hunt audiences, everyone who
  resonated with Balaji's request, and Balaji himself. Tolerates setup,
  wants control and auditability.
- **Later**: non-technical users via a hosted version and mobile apps
  (see Deferred).

## Product principles

- Local-first and privacy-preserving by default. Mail never leaves the
  machine.
- **Works with zero models configured.** The deterministic layer alone
  (headers, List-Unsubscribe, sender heuristics, receipt parsing) produces
  discovery, spend, volume, and one-click unsubscribe. Models are optional
  tiers above it:
  1. No model — the complete first-run experience.
  2. Local model (Gemma / Qwen / Ministral class) — long-tail
     classification, messy receipt extraction.
  3. BYOK cloud model — agentic browser cancellation. Providers:
     OpenAI-compatible, Anthropic-compatible, OpenRouter, DeepSeek, and
     similar endpoints.
- Community-maintained knowledge: unsubscribe recipes, cancellation
  playbooks, and the critical-services warning list live in the repo as
  pull-requestable data.
- The free product is the whole product.

## v0 scope

- **Ingestion ladder**: (1) mbox import (Google Takeout / Mail.app) as the
  zero-auth baseline; (2) IMAP with app password for live sync;
  (3) optional BYO Google OAuth client with guided setup (Hermes/OpenClaw
  idiom). No shared OAuth client, no CASA burden.
  Research: docs/reference/gmail-access-research.md
- **Dashboard**: every subscription and newsletter, total billed, email
  volume, historical graphs, "unsubscribe all" / "cancel all" with $-saved
  and emails-removed counts.
- **Cancellation**: RFC 8058 one-click unsubscribe; curated per-service
  playbooks; agentic browser cancellation behind an experimental flag.
- **Safeguard**: the critical-services list (AWS, registrars, and similar)
  warns before risky cancellations.
- **Feedback**: no telemetry, no crash-report phone-home. Feedback flows
  through user-initiated prefilled GitHub issue links that never contain
  message content.
- **Form**: Tauri desktop app with an embedded web UI, plus a headless
  CLI / localhost mode from the same core. macOS first, Linux second,
  Windows when cheap.
- **Distribution**: shell installer (`curl | sh`, PowerShell one-liner)
  and npm, from emailterminator.com. Never a browser download of the .app
  (keeps installs quarantine-free). Unsigned at launch.

## Deferred (with triggers)

- **v0.1 — Virtual cards**: BYO privacy.com / Lithic account; card-swap
  automation reuses the cancellation browser skills.
- **Email aliasing** (per-service relay addresses): after virtual cards;
  user brings a relay account.
- **Hosted version + mobile apps**: after the desktop core proves demand;
  they absorb the CASA and app-store costs when funded.
- **Apple-signed builds**: at first sponsorship money (~$99/yr; requires
  an entity for a non-personal name on the certificate).

## Non-goals

- Ads, data selling, paid whitelisting of services.
- Telemetry of any kind.
- Sender-pays-user schemes and crypto billing.
- General-purpose agent ambitions.

## Monetization

Tips, donations, and sponsorships; indirect revenue through reputation,
freelancing, consulting, and cross-promotion of our other projects
(HNTerminal and others). Nothing else. No paid tiers.

## Success metrics

- **Launch week**: Show HN plus a working-demo reply to Balaji's thread.
  Success = HN front page or a Balaji acknowledgment.
- **90 days**: 1,000 GitHub stars; 10 community recipes/playbooks merged;
  one external contributor returns for a second PR.
- **12 months**: at least one inbound consulting/freelance lead traceable
  to the project; measurable cross-promotion to our other projects.

## Assets

- emailterminator.com (owned; installer + site).
- emailterminator.org / .dev — unregistered as of Aug 2026; register as
  cheap insurance.
- Canonical address: the GitHub repository.
