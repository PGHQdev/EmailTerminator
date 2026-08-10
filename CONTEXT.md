# EmailTerminator — Strategy

August 2026

## Origin

A direct answer to Balaji's "THE EMAIL TERMINATOR" product request
([@balajis, 12 Jul 2024](https://x.com/balajis/status/1811590332657598604)):

> THE EMAIL TERMINATOR
>
> Wanted: an AI tool that processes your inbox, lists every SaaS subscription
> and newsletter, shows total amount billed and total number of emails sent by
> that service, and allows you to one-click cancel.

A [second tweet](https://x.com/balajis/status/1811596254918701373) gave 17
implementation notes. The ones this product answers, in his order: build a
privacy-preserving local version over mbox files; curate the cancellation
process heavily, because many services make it hard on purpose; virtual cards
so you need not log in everywhere; per-service relay addresses; big
"unsubscribe all" and "cancel all" buttons that state the emails removed and
the dollars saved; warn before cancelling things like AWS that break live
sites; graph email volume and price history to expose creeping increases;
resist ads and paid whitelisting; and combine deterministic mbox parsing,
hard-coded heuristics, and AI parsing of unstructured content.

Three of his notes are declined on purpose: a $99 one-time charge (the free
product is the whole product), sender-pays-user schemes, and crypto billing.
The hosted version he suggests first is deferred, not refused.

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
  2. Local model (Gemma 4 / Qwen 3.5 / Ministral 3 class, all Apache-2.0
     and ungated) — long-tail classification, messy receipt extraction.
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
  idiom). No shared OAuth client, no CASA burden. See `PLAN.md` 1.6.
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
- **Form**: Tauri desktop app with an embedded web UI. The domain logic is
  a Rust library the app links in-process. No CLI, no localhost mode, no
  daemon. macOS, Linux and Windows all ship at v0. See `PLAN.md` 1.2 and 1.4.
- **Distribution**: shell installer (`curl | sh`, PowerShell one-liner)
  from emailterminator.com, with the GitHub Releases page as a documented
  fallback. The installer path sets no quarantine and no Mark of the Web,
  so an unsigned build launches clean. Installs land in user-owned
  directories and update themselves silently. No npm and no Homebrew cask
  while unsigned. See `PLAN.md` 1.4 and Part 9.

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
