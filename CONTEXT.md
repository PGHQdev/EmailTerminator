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

His $99 one-time charge is kept in shape and cut in price: the prebuilt app
costs $29 or more, once. Two notes are declined on purpose: sender-pays-user schemes and
crypto billing. The hosted version he suggests first is deferred, not refused.

## Positioning

EmailTerminator is a specialized, local-first agent harness that discovers
SaaS subscriptions and newsletters in an inbox, quantifies email volume and
spend, and drives cancellation. It is a focused tool: deterministic parsers
first, domain-specific agent skills second, models third. It is not a
general-purpose agent.

Source-available under FSL-1.1-MIT: anyone can read, build and run it, and
each release becomes MIT two years after it ships. Nobody may sell it as a
competing product. The app is free and complete, like WinRAR or Sublime Text:
after a 14-day evaluation an unlicensed copy reminds the user to buy a licence
for $29 or more, and nothing is ever locked. The project's value is credibility,
distribution, reputation, and licence revenue.

## Users

- **v0 and core identity**: the tech-enthusiast, privacy-conscious user —
  agentic-AI adopters, Hacker News / Product Hunt audiences, everyone who
  resonated with Balaji's request, and Balaji himself. Tolerates setup,
  wants control and auditability.
- **Later**: non-technical users via a hosted version and mobile apps
  (see Deferred).

## Product principles

- Local-first and privacy-preserving by default. Mail never leaves the
  machine. The local database is encrypted at rest.
- **Built in the open, auditable by anyone.** The promise is verifiability:
  every request the app makes is in public source code, listed in the app,
  and small enough to read. Anyone can build the app and compare. The app
  does talk to our server — for the licence and, if the user opts in, cancel
  stats — and it says so plainly.
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
- One product, one price, no locked feature. There are no paid tiers and no
  subscription.

## v0 scope

- **Ingestion ladder**: (1) IMAP first — app password for Gmail, iCloud,
  Fastmail and Yahoo, and our own Microsoft OAuth client for Outlook.com,
  which no longer accepts app passwords; (2) mbox and Maildir import, in a
  later v0 milestone. No Google OAuth client at v0, ours or the user's: ours
  needs a CASA audit, and a user-supplied one asks a layperson to run a Google
  Cloud project.
  See `PLAN.md` 1.6.
- **Dashboard**: every subscription and newsletter, total billed, email
  volume, historical graphs, "unsubscribe all" / "cancel all" with $-saved
  and emails-removed counts.
- **Cancellation**: RFC 8058 one-click unsubscribe; curated per-service
  playbooks; agentic browser cancellation behind an experimental flag.
- **Safeguard**: the critical-services list (AWS, registrars, and similar)
  warns before risky cancellations.
- **Licence**: $29 or more, one-time, through Polar; $19 or more in launch
  week. The app is free; after a 14-day evaluation, an unlicensed copy shows
  reminders until a key is entered. One licence covers 3 devices. A licensed
  copy checks the licence every 15 days and works offline for up to 30. See
  `PLAN.md` 1.7.
- **Telemetry**: two payloads and no others. On a licensed copy, activation
  and the 15-day check send the licence key and a hash of the machine ID. Cancel stats — which community
  playbook ran and how it ended — go only when the user opts in, and they
  help other users see which cancellations work. No crash-report phone-home.
  Feedback flows through user-initiated prefilled GitHub issue links that
  never contain message content.
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
- **Apple-signed builds**: when licence revenue covers the ~$99/yr fee and an
  entity exists for a non-personal name on the certificate.
- **Our own verified Google OAuth client**: when licence revenue covers
  Google's verification and the annual CASA audit (~$540–1,800/yr). Gmail
  users sign in with one click after that. See `PLAN.md` 1.6.

## Non-goals

- Ads, data selling, paid whitelisting of services.
- Telemetry beyond the two payloads above. Never mail content, subjects,
  addresses, or sender domains.
- Sender-pays-user schemes and crypto billing.
- General-purpose agent ambitions.

## Monetization

A one-time, pay-what-you-want licence with a $29 minimum ($19 in launch
week), sold through Polar as merchant of record. Checkout pre-fills the
minimum, so paying more is a choice and never a default. Donations through
GitHub Sponsors stay open before and after a purchase, from the repository and
from S17. Indirect revenue comes through reputation, freelancing, consulting,
and cross-promotion of our other projects (HNTerminal and others). No paid
tiers, no subscription.

## Success metrics

- **Launch week**: Show HN plus a working-demo reply to Balaji's thread.
  Success = HN front page or a Balaji acknowledgment.
- **90 days**: 1,000 GitHub stars; 10 community recipes/playbooks merged;
  one external contributor returns for a second PR.
- **Licences**: activations and the share of installs that buy, read from Polar and the
  activation log. No target is set yet.
- **12 months**: at least one inbound consulting/freelance lead traceable
  to the project; measurable cross-promotion to our other projects.

## Assets

- emailterminator.com (owned; installer + site).
- emailterminator.org / .dev — unregistered as of Aug 2026; register as
  cheap insurance.
- Canonical address: the GitHub repository.
