# EmailTerminator — v0 implementation plan

Date: 2026-08-10, revised 2026-09-26
Status: the working document for building v0

This file is self-contained: the decisions, the reasoning behind them, the
layout, the data model, the milestones, and the release mechanics. There is no
ADR directory and no ticket tracker. When this plan is wrong, edit this plan.

The two other documents that stay authoritative:

- `CONTEXT.md` — product scope, principles, non-goals.
- `DESIGN.md` and `docs/design/` — the 19 screens (S00–S18), the design system,
  and the mockups. Visual source of truth.

---

## Part 1 — Locked decisions

Do not re-open these while building. Each line is the operative fact; the
section named beside it holds the argument and what would reopen it.

| # | Decision | Why |
|---|---|---|
| 1 | Domain logic is Rust. TypeScript exists only in the SvelteKit UI and the licence Worker, which holds no domain logic. | 1.1, 1.7 |
| 2 | The parse path handles 10 GB in under ten minutes with flat memory, measured on an mbox. | 1.1 |
| 3 | v0 is the desktop app alone. No CLI, no localhost UI, no daemon, no bound port. Locked again on 2026-09-26. | 1.2 |
| 4 | Cargo workspace: `core` (no Tauri) and `app` (thin Tauri command layer). | 1.2 |
| 5 | Progress streams over a Tauri `Channel`, never the event system. | 1.2 |
| 6 | Single instance. SQLite has exactly one writer. | 1.2 |
| 7 | Agentic cancellation runs through our Chrome extension using `chrome.debugger` in the user's own profile. | 1.3 |
| 8 | The Rust core drives; the extension only relays CDP over native messaging to a Unix socket (named pipe on Windows). | 1.3 |
| 9 | Three targets at v0: macOS universal, Linux x86_64 AppImage, Windows x64 NSIS. No `.dmg`, `.deb`, `.rpm`, `.msi`, no npm, no Homebrew. | 1.4 |
| 10 | Install to user-owned directories. No administrator prompt at any point. | 1.4 |
| 11 | `latest.json` at the GitHub `releases/latest` URL is the single version source for both installer and updater. | 1.4 |
| 12 | Updates are automatic: check at launch (max once per 24 h), download in background, install at quit, report at next launch. | 1.4 |
| 13 | Browser integration files are written at every launch, unconditionally, into every Chromium-family browser directory. | 1.4 |
| 14 | Extension and app version independently, behind an integer protocol version; the app accepts current and previous. | 1.4 |
| 15 | A `v*` tag builds three platform jobs into a **draft** release; a human publishes. One assemble job is the only writer of `latest.json` and `SHA256SUMS`. | 1.5 |
| 16 | Installer is latest-only. No `--version`, no `ET_VERSION`. Testers use `install-prerelease.sh`. | 1.5 |
| 17 | `CHANGELOG.md` is the single source of release notes, injected into the release body, `latest.json` `notes`, and the app's "what changed" screen. | 1.5 |
| 18 | The minisign key lives in a `release` GitHub Environment restricted to `v*`. Two offline backups exist before the first release. | 1.5 |
| 19 | IMAP comes first. mbox and Maildir import ship in a later v0 milestone. | 1.6 |
| 20 | Source is FSL-1.1-MIT, which becomes MIT two years after each release. The app is free with no locked feature. After a 14-day evaluation an unlicensed copy shows reminders; a licence for $29 or more ($19 or more in launch week) removes them. | 1.7 |
| 21 | Polar sells the licence. One licence covers 3 devices, each identified by a salted hash of the OS machine ID. The Worker issues an Ed25519-signed token bound to that hash; the app renews it every 15 days, with a further 15 days of grace. | 1.7 |
| 22 | Exactly two payloads reach our server: licence activation (always) and cancel stats (opt-in). | 1.7 |
| 23 | The database is SQLCipher, keyed by a random 256-bit key in the OS keychain. | 2.1 |

### 1.1 Why Rust holds the domain logic

Tauri and SvelteKit were fixed before this choice, so both languages exist in
the repository either way. The deciding input was throughput on the scan path.

Benchmarked on an Apple M1 over a 500 MB synthetic mbox of 122,800 messages
mixing `text/plain`, `multipart/alternative`, and `multipart/mixed` with base64
attachments and quoted-printable HTML. Every full-parse path decodes bodies, so
the comparison is like for like.

| Path | 500 MB | msgs/s | 10 GB projected | peak RSS |
|---|---|---|---|---|
| Rust `mail-parser`, full parse | 2.27 s | 54,110 | 0.8 min | 4.5 MB |
| Node `postal-mime`, full parse | 27.4 s | 4,475 | 9.4 min | 127 MB |
| Node `mailparser`, full parse | 125.9 s | 975 | 43 min | 156 MB |
| Node, headers on all + full parse on 10% | 5.47 s | 22,435 | 1.9 min | 113 MB |

Rust is 12× faster than the fastest Node MIME parser at 28× less memory. A
TypeScript core clears the ten-minute budget only by parsing bodies for a
subset; Rust clears it parsing everything, with an order of magnitude spare.

Two findings that shaped the confidence: `mailparser`, the most-downloaded Node
parser, returned nothing for `list-unsubscribe` across all 122,800 messages
while `postal-mime` found every one — unexplained, and recorded because
download counts had been carrying weight. And the Node mail stack traces to a
single author across ImapFlow, postal-mime, `mbox-reader` and mailparser, while
the Rust stack rests on two independent commercially-backed groups, Stalwart
for `mail-parser` and Delta Chat for `async-imap`.

An earlier survey called Rust's IMAP support the decisive weakness. That was
wrong. Verified from crates.io metadata and extracted sources: `async-imap`
0.11.3 with 578,875 downloads per 90 days and eight releases since September
2024; `imap-proto` 0.16.7 at 1,021,885; `imap-codec` and `imap-types` at 1.0.0.
The unmaintained crate is `jonhoo/rust-imap`, which does not generalise. Feature
coverage counted in the sources: MODSEQ, CONDSTORE, VANISHED, QRESYNC, UIDPLUS,
`BODY.PEEK`, IDLE, and Gmail's `X-GM-LABELS`, `X-GM-MSGID`, `X-GM-THRID` — the
exact set that had been credited to ImapFlow as its advantage.

**Consequences carried into the build**: we write the mbox gzip wrapper and
`X-Gmail-Labels` handling ourselves, because `mail-parser`'s mbox iterator
streams but covers neither. We write the RFC 8058 one-click layer and the
`List-Unsubscribe` grammar ourselves, because no library in either ecosystem
implements them. Rule iteration is slower than a dynamic language would give,
which is acceptable only because recipes and playbooks are data files (2.6) —
if receipt rules become compiled Rust, revisit that.

**What would reopen it**: a sustained failure of the Rust mail libraries that
we cannot patch or fork, or a demonstrated need to change receipt-extraction
logic faster than a compile cycle allows.

### 1.2 Why there is no seam

`CONTEXT.md` once described three delivery surfaces — desktop app, headless
CLI, localhost mode. Each one that exists has to be built, tested, secured and
supported.

Facts that closed it: Tauri requires `@sveltejs/adapter-static` and does not
support server-based frontends, so the UI is one static build regardless of
transport. Tauri's own docs say `tauri-plugin-localhost` "brings considerable
security risks and you should only use it if you know what you are doing". The
event system carries JSON strings and is documented as "not designed for low
latency or high throughput situations", which is why progress uses Channels. A
Tauri binary links the platform webview — `libwebkit2gtk` on Linux — so a
single argv-dispatched binary cannot serve a headless machine anyway.

A background daemon was costed properly. The code is modest: a LaunchAgent
plist, a systemd user unit, a Task Scheduler entry, lifecycle handling. The
costs are behavioural — macOS TCC prompts for Desktop, Documents and Downloads
with no window to present them from; users switching it off in Login Items &
Extensions; version skew when the updater replaces the bundle under a running
daemon; and a mandatory uninstaller, which a `curl | sh` install does not have.
Its only v0 benefit is IMAP IDLE while the app is closed, and no designed
screen consumes that.

`core` is a separate crate precisely so that adding a CLI or a daemon later is
an added binary rather than a refactor. Single instance is load-bearing: if two
windows or two data profiles are ever wanted at once, the advisory-lock protocol
this decision avoids has to be designed then.

**What would reopen it**: a user need for headless operation on a server, or a
feature that consumes background sync — price-increase alerts or notifications
are the likely first ones.

### 1.3 Why agentic cancellation goes through an extension

Cancelling a subscription always requires an authenticated session, so the
deciding question was how automation reaches one.

The browser must be headed, because a headless window cannot be handed to a
user, and S09 requires exactly that hand-over. Chrome 136 removed
`--remote-debugging-port` against Chrome's default data directory, because
attackers used it to extract cookies past App-Bound Encryption; Playwright
separately documents that automating the default profile is unsupported. Both
routes to the user's existing session through a launched browser are closed.
A dedicated automation profile does not rescue it: Google refuses sign-in to
browsers "controlled through software automation", which covers a large share
of SaaS logins.

`chrome.debugger` exposes Input, Page and Target, which is sufficient, and
Chromium shows a permanent "started debugging this browser" infobar. For a tool
selling auditability that is a feature, and the UI explains it rather than
working around it.

Shipping practice was checked on a development machine: three native-messaging
hosts were installed, including two of Anthropic's and Apple's password
manager, and none of the three binds a network port. Chrome's message caps are
1 MB host-to-extension and 64 MiB extension-to-host, so our commands fit the
small side and screenshots return on the large side.

OpenAI's Atlas and Perplexity's Comet take the other architecture and ship a
browser. That costs a Chromium fork and a browser-sized download, which is not
proportionate to an experimental feature in a small app.

Neither CDP nor the WebDriver BiDi draft defines a suspend or hand-to-human
command, so pause and hand-over are our own command loop.

**What would reopen it**: Chrome restricting `chrome.debugger` for extensions,
or a Web Store rejection that also makes the sideload path untenable.

### 1.4 Why packaging looks like this

The facts underneath, established by direct check:

- **Quarantine is set by the downloading application, not the OS.** `curl` from
  a terminal, and the Tauri updater's own `tar` plus `rename`, never set it. A
  browser download does, and Homebrew Cask does deliberately with no opt-out.
  This is the whole argument for a shell installer and against a `.dmg` and a
  cask.
- An ad-hoc-signed bundle under quarantine is refused at launch with "damaged
  and can't be opened", whose only fix is `xattr -dr com.apple.quarantine`.
- The Tauri updater's minisign requirement costs nothing, and an Apple-unsigned
  build self-updates in full, silently, from a user-writable directory.
- Windows SmartScreen keys off Mark of the Web, which PowerShell's web cmdlets
  and bundled `curl.exe` do not write. **Microsoft has retracted the claim that
  an EV certificate bypasses SmartScreen**, which Tauri's own documentation
  still repeats — do not buy one on that basis.
- No project in a sweep of 584 npm packages keyworded `tauri` ships a desktop
  bundle through npm, and Bun and pnpm 10 gate `postinstall` by default. npm
  also lost its natural artifact when the CLI went (1.2).
- A native-messaging manifest cannot name a path inside an AppImage, so Linux
  needs a generated wrapper on disk.
- An unpacked extension's ID derives from its directory path unless
  `manifest.json` carries a `key`, and `allowed_origins` ships inside the app —
  so both extension IDs must be frozen before the first app release.

**The app says nothing about being unsigned.** On macOS a quarantined
ad-hoc-signed bundle never starts, so in-app copy would reach only users who
were never blocked. On Windows the user has already clicked through SmartScreen
before our code runs.

The full artifact list, install locations, update behaviour, versioning rules
and uninstall policy are in Part 8.

**What signing would buy**, recorded because licence revenue now makes it
affordable: an Apple Developer ID plus notarization (~$99/yr) lets a browser
download launch without the "damaged" refusal, which opens a `.dmg`, a Homebrew
cask and the buyer who never opens a terminal. It also gives the keychain a
stable code identity across updates (Part 6, item 5). Windows Authenticode
only builds SmartScreen reputation slowly per certificate. minisign already
covers the one thing v0 needs: an update that nobody can forge. The shell
installer, not signing, is what keeps an unsigned build clean.

**What would reopen it**: licence revenue covering the Apple fee plus an entity
for the certificate name, which makes a cask and a browser download viable and
changes the whole channel argument; or a user need for managed deployment,
which is what an `.msi` is for.

### 1.5 Why the release pipeline looks like this

- **Standard GitHub-hosted runners are free and unmetered on public
  repositories**, including macOS and Windows. The January 2026 price cuts
  applied to private repositories only. Runner cost is therefore not an argument
  anywhere in this plan, which is why the PR matrix (2.10) is decided by value.
- GitHub excludes draft and pre-release releases from
  `/releases/latest/download/`, which is where the installer and the updater
  both read `latest.json`. That single fact gives us a tester channel for free
  and makes the draft-then-publish flow atomic from a user's point of view.
- `tauri-apps/tauri-action` builds the bundles and can upload the updater JSON,
  but its README does not document what happens when several matrix jobs upload
  `latest.json` to one release concurrently. A lost write there is the worst
  failure this pipeline can produce, so one assemble job is the only writer —
  removing the possibility rather than making it unlikely.
- The `ubuntu-22.04` runner image begins deprecation on 17 September 2026 and is
  unsupported by 17 April 2027. Building Linux inside an `ubuntu:22.04`
  container makes the glibc floor a choice rather than a side effect of
  GitHub's image schedule.
- **The Tauri updater only offers a higher version. No downgrade exists.** That
  is why recovery is fix-forward and why the installer has no pin.
- A piped shell script needs `sh -s --` to receive arguments, and PowerShell's
  `irm | iex` cannot pass arguments at all — which is half the reason the
  installer is latest-only and testers get a separate script.

The gate, runner table, assemble job, signing and rotation, provenance, release
notes, site scripts, extension pipeline and yank procedure are in Part 8.

**What would reopen it**: enough users that a staged rollout of the app itself
is worth having, which means moving `latest.json` off GitHub to something that
can serve a percentage; a tester group large enough to want real nightlies; or
a Chrome Web Store API change that puts a human back at the publish step.

### 1.6 Why the ingestion ladder looks like this

IMAP is the first rung because it is live, it needs no export step, and one
implementation covers most mailboxes. mbox and Maildir import follow in a later
v0 milestone (M5) for offline users and exported archives.

Every official read path for consumer Gmail was costed. The axis that decides
it is developer verification: Google's restricted scopes (`gmail.readonly`,
`mail.google.com`, even `gmail.metadata`) require verification plus an annual
CASA Tier 2 audit, which runs roughly $540–1,800 a year on the precedents we
could confirm.

| Path | Verification burden | User friction | Freshness | v0 |
|---|---|---|---|---|
| IMAP with app password | None | Medium: enable 2SV, generate, paste | Live | Rung 1 |
| Outlook.com through our Microsoft OAuth client | None beyond app registration | Low: sign in | Live | Rung 1 |
| mbox / Maildir import | None | High, manual | Snapshot | Rung 2 |
| Our own verified Google client + CASA | Verification + annual audit | Low | Live | Deferred |
| BYO Google OAuth client | On the user, not us | High: a Google Cloud project | Live, plus history and push | Rejected |

App passwords survived the 2022–2025 "less secure apps" shutdowns and Google
publishes no sunset date; third-party claims of a 2026 phase-out are
unconfirmed. That makes rung 1 the workhorse for Gmail, iCloud, Fastmail and
Yahoo with one implementation.

**Outlook.com is the exception.** Since 16 September 2024 Microsoft refuses
basic authentication, app passwords included, for personal Outlook.com,
Hotmail and Live mailboxes. They need `AUTHENTICATE XOAUTH2`. Microsoft's
identity platform lets us register one public desktop client with PKCE and the
`IMAP.AccessAsUser.All` and `offline_access` scopes. There is no audit
comparable to CASA, so this client ships at v0. An earlier version of this
section counted Outlook among the app-password providers; that was wrong.

**Can the app use its own Google OAuth client for normal users?** Yes, but not
at v0. IMAP over OAuth needs the `https://mail.google.com/` scope, which is
restricted. Until verification passes, an app is capped at 100 test users and
shows the unverified-app warning. Google puts restricted-scope verification at
about six weeks, and the CASA assessment repeats every 12 months. A desktop
client gets no exemption. Neither the restricted-scope verification page nor
the API Services User Data Policy exempts an app that keeps mail on the user's
device; their only exemptions are personal use, testing, internal Workspace
apps and service accounts. In March 2026 Google's community team answered a
local-only Gmail app that the policy "effectively requires a security
assessment" at production scale, and Mimestream, a local-only client, passed
CASA. At the $29 minimum, net of Polar's
fee (about 5% + 50¢), the audit costs roughly 20–67 licences a year. So: apply for verification after
launch, fund the audit from licence revenue, and ship the client as a patch once
it passes. Until then Gmail users take rung 1.

**The user-supplied OAuth client is rejected.** It is the pattern of
developer tools such as rclone, Home Assistant, gmvault and mbsync. It asks a
layperson to create a Google Cloud project, configure a consent screen, enable
the Gmail API, add themselves as a test user and paste a client secret. Our
users are not developers, and an app password is fewer steps for the same live
IMAP access.

Two other escape hatches were rejected. No open-source project lends out its verified
client, and doing so without consent violates Google OAuth policy. Free managed
OAuth middlemen exist, but mail transits their servers, which is incompatible
with local-first.

**RFC 8058 one-click unsubscribe needs no mailbox access at all**, because it is
an HTTPS POST. That is why an imported file on rung 2 still delivers the
product's headline action.

**What would reopen it**: Google publishing an app-password sunset, which makes
our own verified client urgent; or licence revenue covering the audit, which
moves that client out of the deferred row.

### 1.7 Why the licence and the server look like this

The code stays readable so the privacy claim stays auditable. The model is
WinRAR's and Sublime Text's: the app is free and complete, and an unlicensed
copy keeps asking to be paid for.

- **FSL-1.1-MIT.** Anyone may read, build, run and modify the code for any
  purpose except a Competing Use: selling it, or something substantially
  similar, as a commercial product or service. Each release becomes MIT on its
  second anniversary. **FSL has no clause against removing the licence check.**
  A user who patches out the reminders for their own copy breaks no term. It
  stops a competitor from selling our code; it does not stop a personal bypass,
  and with nothing locked a bypass only removes a reminder. FSL is
  source-available, not OSI open source, and the copy in `CONTEXT.md` says so.
- **Every build checks.** There is no feature flag, so a build from source
  shows the same reminders as the installed app. Removing them means editing
  the code.
- **Evaluation.** 14 days with no reminders, counted from first launch. The
  start date lives in the keychain. Erase-all-data restarts it, and we accept
  that.
- **Reminders after day 14**, on an unlicensed copy:
  - a dialog at every launch — buy a licence, enter a key, or continue
    evaluating;
  - the same dialog after every bulk run and after every tenth single action;
  - a permanent "Unregistered" marker in the rail.

  No feature is locked and no button waits on a countdown. A reminder never
  opens during a scan, a critical-service confirmation, or an agent run; it
  waits until the work ends.
- **Hardening is modest on purpose.** The token is verified in several code
  paths with no single `is_licensed()` switch, and release binaries are
  stripped. That turns a one-prompt patch into a longer job. Anything stronger
  costs auditability and stops nobody determined.
- **Polar is the merchant of record.** It handles checkout, sales tax, refunds
  and licence-key issue. The price is pay-what-you-want with a minimum: $19 in
  launch week, then $29. Checkout pre-fills the minimum. Every amount buys the
  same licence.
- **Device fingerprint.** A device is the machine ID the OS already keeps:
  `IOPlatformUUID` on macOS, `/etc/machine-id` on Linux, the `MachineGuid`
  registry value on Windows, read through the `machine-uid` crate. The app
  sends only `HMAC-SHA256(machine ID, key = "emailterminator-device-v1")`, so
  the raw ID never leaves the machine and our hash cannot be joined with any
  other app's. No other hardware data is read. Known limits: a Windows
  reinstall changes `MachineGuid`, and a cloned Linux VM shares its
  `machine-id`. Both are accepted; the first costs a device slot the user can
  free.
- **3 devices per licence.** Polar's activation limit on the licence-key
  benefit is 3. The Worker keeps a D1 row per device hash and its Polar
  activation ID, so a reinstall on the same machine reuses its slot. A fourth
  device is refused with the list of the three, and the user can free one from
  S17 or from Polar's customer portal.
- **Activation.** The app sends the key, the device hash, the app version, the
  OS and the architecture to `api.emailterminator.com/activate`. The Worker
  checks the key with Polar, activates or reuses the device, and returns a
  token signed with our Ed25519 key that names the key, the device hash and an
  expiry 30 days out. The app verifies the signature against the public key
  compiled into the binary, and verifies that the device hash is its own, so a
  token copied to another machine is worthless.
- **Check every 15 days.** From day 15 of a token's life, the app calls
  `/check` with the same fields at launch and then once a day while it runs.
  The Worker asks Polar whether the key and this activation are still valid
  and returns a fresh 30-day token. Offline, the old token keeps working until
  day 30. After that the copy shows the evaluation reminders again, and the
  next successful check silences them. A refunded or revoked key, or a device
  freed in Polar's portal, fails its next check. Nothing is ever locked,
  because an unlicensed copy is still the complete app.
- **Deactivate** from S17 calls `/deactivate`, frees the Polar activation, and
  deletes the local token.
- **Cancel stats, opt-in.** After a playbook or agent run finishes, the app
  sends the community `data_key` of the service (2.6), the action kind, the
  outcome, and the ISO week. No licence key, no device hash, no sender domain,
  and nothing for a service that matched no community entry, so a personal or
  unknown sender can never leave the machine. Before release, a maintainer exports the aggregates
  to `data/stats.json`, which ships inside the app. S06 and S08 read it. The app
  makes no new request to fetch stats.
- **The server is one Cloudflare Worker** (Hono, TypeScript) with D1, in
  `server/`. It holds no domain logic, which is why locked decision 1 still
  stands. The site stays static.

Abuse of the stats endpoint is accepted at v0: the numbers are advisory and a
maintainer reviews them before they ship.

**What would reopen it**: a fork selling the prebuilt app at scale, which
questions the licence choice; or a demand for per-user stats, which would need
an identifier and a new consent screen.

### Invariants that override any local convenience

- **Mail never leaves the machine.** The outbound requests the app makes on its
  own are: the update check; on a licensed copy, activation and a licence check
  every 15 days; cancel stats, only when the user opts in; the mail provider and its OAuth endpoints; and, when the
  user configures Tier 2, the endpoint they entered.
- **No telemetry beyond the two payloads in 1.7.** No crash phone-home. The
  device hash goes only with licence requests, and an unlicensed copy never
  sends it.
- **No network font, script, or stylesheet at runtime.** Everything the UI
  loads is bundled. This kills the `@import url('https://fonts.googleapis.com…')`
  that ships in the design-system CSS (see 2.4).
- **Tier 0 is complete.** Every feature path must work with no model
  configured. A model call is an enhancement inside a branch that already has
  a deterministic answer.
- **Permissive dependencies only**: MIT, Apache-2.0, BSD, ISC, Zlib, MPL-2.0,
  and OFL for fonts. No GPL or AGPL, which cannot combine with FSL.

---

## Part 2 — The eleven open questions, decided

These were tickets 07, 09, 10, 11, 12, 14, 15, 17, 18, 19, 20. Each is now a
build instruction. Rationale is kept to what a builder needs to not undo it.

### 2.1 Storage and search (was 07)

**SQLCipher through `rusqlite` with the `bundled-sqlcipher-vendored-openssl`
feature.** It compiles SQLCipher and OpenSSL from source, so there is no system
dependency on any of the three platforms and no version drift between them.
SQLCipher Community is BSD-3 and OpenSSL 3 is Apache-2.0, both permissive.

- **Encryption at rest.** SQLCipher encrypts every page with AES-256 and
  authenticates it with HMAC-SHA512. At first launch `core` generates a random
  256-bit key and stores it in the keychain (2.2). The key goes in as a raw key
  (`PRAGMA key = "x'…'"`), which skips SQLCipher's password derivation, because
  the key is already random.
- **Files outside the database** — `sessions/*.jsonl` transcripts, their
  screenshots, and cached scan artefacts — are sealed with XChaCha20-Poly1305
  under a subkey derived from the same key through HKDF-SHA256.
- **What it protects**: a copied data directory, a backup, a cloud-synced
  folder, a lost disk without full-disk encryption. **What it does not**: other
  software that runs as the same user and can ask the keychain for the key. On
  the Linux fallback (2.2) the key file sits beside the database, so there it
  protects only against a copy that leaves the key file behind. S17 says so.
- **A lost key is a designed state.** If the keychain entry is gone, the
  database cannot open. S16 says so and offers a fresh scan; mail on the server
  is untouched.
- **Check at M0** that the SQLCipher build carries FTS5, and that the perf test
  in 2.9 still passes with encryption on.
- SQLCipher was picked over SQLite3MultipleCiphers because `rusqlite` supports
  it as a feature flag; the alternative needs our own build of the C library.

- **One writer thread.** `core` owns a single connection for writes behind a
  channel; readers use a small read-only pool. This matches locked decision 6,
  so no advisory locking exists anywhere.
- **`rusqlite` is blocking, the IMAP client is async** (2.3). Database work
  runs on `tokio::task::spawn_blocking`. Do not reach for `sqlx` to dissolve
  this: it buys an async facade over the same serialised writer and costs the
  compile-time-checked-query workflow against a schema that migrates.
- **Migrations**: numbered `.sql` files in `core/migrations/`, embedded with
  `include_str!`, applied by a stepper keyed on `PRAGMA user_version`. Forward
  only, automatic at launch, one transaction per step. A minor version bump is
  the promise that carries a migration (Part 9).
- **Schema shape** (detail in Part 4): `source`, `message` (metadata plus a
  locator back to the original bytes, never the body), `sender`, `service`,
  `receipt`, `charge`, `action` (the activity log), `setting`.
- **Aggregates are stored, not recomputed.** S03 and S06 read spend and volume
  history for a whole inbox; recomputing per view against message-level rows
  makes the dashboard pay the scan's cost. Aggregates are rebuilt as part of a
  scan and by a migration when the parser changes.
- **Full-text search: FTS5**, an external-content table over subject and
  sender display name, populated by triggers. It backs message-level search
  inside a service detail.
- **The command palette (S18) does not use FTS5.** Its corpus is services,
  senders, screens and actions — thousands of rows at most, and it needs fuzzy
  prefix matching as-you-type. It is an in-memory index in the UI, rebuilt when
  the service and sender lists change.
- **Location**: `dirs::data_dir()/EmailTerminator/` (`~/Library/Application
  Support/…` on macOS, `~/.local/share/…` on Linux, `%APPDATA%\…` on Windows).
  S17 reveals the path and can move it; moving copies then swaps, and never
  deletes the old directory until the copy verifies.

### 2.2 Credential storage (was 09)

**The OS keychain through the `keyring` crate**, with one encrypted-file
fallback.

- macOS Keychain, Windows Credential Manager, Linux Secret Service.
- **Linux fallback**, because Secret Service is genuinely absent on minimal
  desktops: an XChaCha20-Poly1305 encrypted file in the data directory, keyed
  by a random 32-byte key file at mode `0600`. This is convenience with a
  weaker guarantee, and it is the position taken deliberately: a passphrase
  prompt at every launch is not compatible with an app whose first-run promise
  is that it works immediately. S17 states which backend is in use, in plain
  words, so the user can see which one they got.
- **There is no session-less context to design for**, because decision 3
  removed the CLI.
- **Erase-all-data (S17) reaches**: the database, the FTS index, the cached
  scan artefacts, the session transcripts, every keychain entry we created
  except the licence token, the fallback key file, and the settings file. The
  licence token stays, because it holds no mail data and a buyer should not pay
  twice. It does not touch the browser integration files; those go through
  their own control (Part 8).
- Stored items: the database key, IMAP app passwords (per source), the Outlook
  refresh token (per source), Tier 2 provider API keys (per provider), the evaluation start date, and
  the licence token.

### 2.3 Rust IMAP client (was 19)

**`async-imap`.** Verified in 1.1: maintained by the Delta Chat group,
current release 0.11.3, and it carries the exact feature set v0 needs — IDLE,
`BODY.PEEK`, UIDPLUS, CONDSTORE/QRESYNC, and typed Gmail `X-GM-*` accessors
through `imap-proto`.

- **This puts `tokio` in `core`.** Accept it. The alternative is
  `imap-codec` + `imap-next`, which is correctness-first and hands us
  connection handling, reconnect and backoff to write ourselves. It stays the
  named fallback if `async-imap` stalls.
- **We write**: reconnect with exponential backoff, resync after disconnect,
  `AUTHENTICATE XOAUTH2` for Outlook.com, and
  per-provider quirk handling for Gmail, iCloud, Fastmail, Yahoo and Outlook.
  Outlook.com takes OAuth only (1.6); the other four take an app password.
- **Incremental resync is a UID range**, `UID SEARCH UID n:*` from the
  highest UID committed per folder, reset when `UIDVALIDITY` changes. It is
  needed at v0: nothing syncs while the app is closed (1.2), so every launch
  catches up. CONDSTORE and QRESYNC are not used. They report flag changes and
  expunges, and neither matters here: a stored row outlives its message (Part
  4). An earlier version of this section planned them; M1 found they would add
  round trips and change no result.
- A fetched batch and its resume point commit in one transaction, so a dropped
  connection resumes without duplicates.
- Folders open with `EXAMINE`, and fetch is `BODY.PEEK[]<0.2097152>`, never
  `BODY`. Reading a user's mail must not mark it read, and the first 2 MB of a
  message carry its headers and text; the rest of a large attachment is never
  downloaded.
- Gmail reads `\All` alone, so a message in several labels is fetched once.
  Elsewhere every folder except sent, drafts, trash and junk, by special-use
  attribute or common name. The account owner's own mail is skipped.
- TLS is `rustls` with the `ring` provider and `rustls-platform-verifier`, so
  the OS trust store decides, as it does for the user's mail client.

### 2.4 UI composition (was 10)

The old ticket asked for a throwaway prototype. Reading the assets answers it
without one, and these are the facts that decide it:

- `_ds_bundle.js` declares `"components":[]`. It is an empty shim. There is
  nothing to consume.
- The 19 mockups use **inline styles over a screen-level alias layer**
  (`--bg`, `--fg`, `--card`, `--acc`, `--sageSoft`, `--dockBg`, `--b1`…`--b4`),
  mapped onto the design system's ramps. They use none of the system's
  component classes (`.btn`, `.card`, `.table`).
- `Dark A Espresso.dc.html` implements dark **by redefining that alias layer
  only**. The ramps do not move.
- The mockups override the heading font to **Bricolage Grotesque**. The design
  system's README says Caprasimo. The mockups win.
- Both `styles.css` and every mockup pull fonts from `fonts.googleapis.com`.

Therefore:

- **Take the tokens, not the component layer.** `styles.css`'s `:root` ramps,
  space, radius and shadow tokens are copied into
  `ui/src/lib/styles/tokens.css` as the project's own file. The system's
  `.btn`/`.card`/`.table` layer is not imported; the mockups never used it.
- **The alias layer is the theme mechanism.** `ui/src/lib/styles/theme.css`
  defines the aliases on `:root` for light and under `[data-theme="dark"]` for
  dark, lifted verbatim from `Dark A Espresso.dc.html`. Components reference
  aliases (`var(--card)`), never ramps directly. The theme setting in S17
  writes `data-theme` on `<html>`; `system` follows
  `prefers-color-scheme`. **Do not port the `--dock*` aliases.** They are
  leftovers from the rejected "Docked Bar" navigation candidate:
  `Rail.dc.html`, which 12 screens import, references none of them.
- **Fonts are vendored.** Bricolage Grotesque and Figtree woff2 files go in
  `ui/static/fonts/` with local `@font-face` rules. The `@import` line is
  deleted. This is not a preference: the network request breaks a stated
  privacy invariant. Both faces are OFL, which bundles into our project as
  long as their licence file ships beside them in `ui/static/fonts/`.
- **No Tailwind.** The global default is Tailwind + Phosphor + BitsUI; here it
  would sit on top of a token system that already owns colour, spacing and
  radius, and produce two vocabularies for one look. Components use scoped
  Svelte `<style>` blocks reading the aliases.
- **Icons: Lucide** (`lucide-svelte`), stroke width 2.75, per the design
  system README. Not Phosphor.
- **BitsUI stays**, for behaviour the design system has none of: dialog focus
  trapping (S10, S11), the command palette (S18), select and combobox (S12,
  S13). Styled with our own scoped CSS.
- **State**: Svelte 5 runes. Long-running work — scan progress (S02), bulk
  action progress (S11), the activity log (S14) — arrives on a Tauri
  `Channel` into a store per stream. No client-side state library.
- **`_adherence.oxlintrc.json` is not adopted as shipped.** It targets React
  (`plugins: ["react"]`, `react/forbid-elements`) and its font rule names
  Caprasimo, which the mockups override. Its rules use `no-restricted-syntax`,
  which oxlint does not implement, and oxlint does not read CSS in any case.
  So `ui/scripts/check-tokens.ts` enforces them on every `.svelte` file as part
  of `bun run lint`: no raw hex colour, no raw `px` value, and no font family
  other than `var(--font-heading)` or `var(--font-body)`. Found at M0.

### 2.5 Type sharing (was 20)

**`specta` + `tauri-specta`.** The in-process Tauri IPC seam (1.2) rules
out a generated OpenAPI client and makes this the fitting option: it emits
both the TypeScript types and typed command bindings, so a renamed command
breaks the UI build rather than failing at runtime.

**Both are at `2.0.0-rc.25`, not a stable release** (checked 2026-08-10). That
is the cost of the typed bindings, and it is accepted with an exit named in
advance: if the rc line breaks against a Tauri release, drop to `ts-rs`, which
is stable and emits types only. Command bindings would then be hand-written
against the committed types, which is the same drift check with more typing.

- Generation runs in a test (`cargo test export_bindings`) and writes
  `ui/src/lib/bindings.ts`. **The output is committed.** CI regenerates and
  runs `git diff --exit-code`; drift fails the build. A reviewer can see the
  seam change in the diff, which a build-time-only generation hides.
- **Representations, fixed once**: timestamps are RFC 3339 UTC strings; money
  is an integer count of minor units plus an ISO 4217 currency code, never a
  float; enums are externally tagged; `Option<T>` maps to `T | null`.
- The old ticket's "how do CLI output types stay consistent" axis is dead
  (1.2).

### 2.6 Community data format (was 14)

**TOML, one file per service**, under `data/services/<domain>.toml`.

- TOML over YAML because indentation errors are the common contribution
  failure, and over JSON because a playbook is prose that a human writes by
  hand.
- **The schema is the Rust `serde` struct** in `et-data/src/lib.rs`. There is
  no second schema document to drift. Validation is `cargo run -p et-data --
  validate`, which deserialises every file, refuses unknown keys, checks
  matcher uniqueness within each collection, and checks that every link is a
  well-formed `https://` URL, printing file and line on failure. It does not
  fetch the links: a network check in CI fails on a vendor's outage or bot
  wall, not on the contribution. CI runs it on every pull request, and
  `core/build.rs` runs the same parser, so data that does not validate does not
  build.
- **A service entry** is `name`, `domains`, optional `senders` (address
  patterns, `*` only before the `@`), optional `unsubscribe` (the vendor's
  email-preferences page, used when a sender's mail has no `List-Unsubscribe`
  header), and an optional `[playbook]` with `source` (the vendor's help page
  the steps come from), `checked` (the date someone last compared them),
  optional `minutes`, and `[[playbook.steps]]` of `text` and optional `link`.
- **Matching**: each entry declares matchers explicitly — a list of domains
  (the common case) and optional sender-address patterns. Never inferred from
  the filename. Matcher collisions across files are a validation error. The
  most specific matcher wins: a sender pattern beats a domain, and a longer
  domain beats a shorter one. A sender an entry names groups under that entry,
  so AWS billing from `amazon.com` is its own service beside Amazon's shop.
- **One format serves S08 and S09.** A playbook step carries `text` (shown to
  the user) and, from M7, an optional `action` (a declarative instruction the
  agent can execute: navigate, click a described target, fill, wait, confirm).
  A step with no `action` is a human-only step, and the agent stops there and
  hands over. M4 leaves `action` out of the schema, so no entry can declare
  one before an executor exists to test it.
- **Four collections**: `data/services/` (cancellation playbooks and
  unsubscribe recipes, keyed per service), `data/critical.toml` (the
  critical-services warning list), `data/providers.toml` (Tier 2 endpoint
  presets), and `data/stats.json` (cancel-stats aggregates per service, exported
  from the Worker before a release, 1.7). The file stem of a service entry is
  its `data_key`, the only service identifier cancel stats may carry.
- **Bundle-only at v0.** Entries ship inside the app; a corrected playbook is a
  patch release (Part 9). Runtime fetching is deferred — it would add an
  outbound request to a product whose privacy statement enumerates them.

### 2.7 Agent-skill harness (was 12)

**A skill is a data file the core interprets** — the same TOML playbook format
from 2.6. It is not a plugin and not a prompt bundle. The 90-day goal in
`CONTEXT.md` is community contributions, and a data file is the only format
where contributing needs nothing but a text editor.

- **The order — parsers, then skills, then models — is enforced by the
  pipeline, not by convention.** `core` runs three stages against each
  candidate: deterministic extraction, then playbook matching, then, only for
  what remains unresolved, a model call. A stage that produces an answer ends
  the pipeline for that item. Stage three is skipped entirely when no model is
  configured, which is what makes Tier 0 complete rather than degraded.
- **Capabilities are declared per skill and enforced by the executor.** A step
  declares what it needs; the executor grants only browser access, and only
  through the core's CDP driver (1.3). No skill gets filesystem access, an
  arbitrary network call, or a process. There is no sandbox to escape because
  there is no code to run: a skill is data.
- **No agent framework.** The loop is: match a step, execute the declared
  action through the CDP driver, observe, advance or hand over. An existing
  framework would bring a tool-calling abstraction, a planner and a prompt
  format for a loop whose steps are already written down by a human.
- **Evidence for S14**: every skill run opens an `action` row and writes a
  transcript file under `sessions/<action-id>.jsonl` — one line per step, with
  the step, the outcome, and a screenshot path where one was taken. The
  activity log's evidence link opens that transcript. For non-agentic actions
  the evidence link points at the message the action was derived from.

### 2.8 Intelligence runtime (was 11)

**One client, two tiers, one trait.**

- **Tier 2** is an OpenAI-compatible chat client written against `reqwest`,
  plus a second implementation for the Anthropic Messages API. OpenRouter,
  DeepSeek, local servers and "custom endpoint" are all the first
  implementation with a different base URL — presets live in
  `data/providers.toml` (2.6).
- **Tier 1 is discovery first.** Probe the OpenAI-compatible endpoints a user
  is likely to already run (Ollama, LM Studio, llama.cpp server) on their
  default ports. If one answers, Tier 1 *is* the Tier 2 client pointed at
  localhost, and we ship no runtime at all.
- **A local endpoint is not automatically a local model.** Ollama now ships
  cloud models, so an endpoint on `localhost` can forward mail content to a
  vendor. Set `OLLAMA_NO_CLOUD=1` when we drive Ollama, and treat any
  discovered endpoint as Tier 2 for the purposes of the privacy statement
  unless we can confirm the model is resident. Getting this wrong breaks the
  first invariant while appearing to honour it.
- **Fallback when nothing answers**: fetch a `llama.cpp` server binary on
  enable (~23 MB) and manage it as a child process. Fetched, never bundled —
  it must not be in the installer for a feature most users never turn on.
- **Model licensing is settled, and the rule that settles it is gating.** A
  gated Hugging Face repository needs an account and per-repository approval,
  which no unattended download can satisfy. Every `google/gemma-3-*` repo
  reports `"gated": "manual"`, including the official GGUF ones, as does
  `meta-llama/Llama-3.2-3B-Instruct`. Ministral 8B Instruct 2410 is worse: MRL
  §3.2 restricts it to research purposes, so it cannot ship in a product we sell
  at all.

  The current generation clears both problems — Gemma 4 moved to Apache-2.0,
  and Qwen 3.5 and Ministral 3 are Apache-2.0 and ungated at the vendor org.
  Pick the default from this set:

  | Model | 4-bit file | Size | Context |
  |---|---|---|---|
  | Qwen3.5 0.8B | Q4_K_M | 0.53 GB | — |
  | Qwen3.5 2B | Q4_K_M | 1.28 GB | — |
  | Ministral 3 3B | Q4_K_M | 2.15 GB | 256K |
  | Qwen3.5 4B | Q4_K_M | 2.74 GB | 262,144 |
  | Gemma 4 E2B | q4_0 QAT | 3.35 GB | 128K |
  | Gemma 4 E4B | q4_0 QAT | 5.15 GB | 128K |
  | Ministral 3 8B | Q4_K_M | 5.20 GB | 256K |

  **Verify ungated status at download time anyway**, since a vendor can flip a
  repository to gated after we ship. A gated response is a designed failure in
  S13 that names the model and offers another, never a silent hang.
- **"Test connection" (S13)** performs one real completion with a one-token
  cap and reports latency and the model name the endpoint reported. Listing
  models is not enough — an endpoint that lists but refuses to infer is the
  failure users actually hit.
- **Degradation is structural.** Every call site holds `Option<&dyn
  ModelClient>` and has a Tier 0 branch that is the tested default path. A
  model absent, unreachable, rate-limited or out of credit is the same code
  path as "no model configured", surfaced as a status in S13 and never as a
  blocking error in a scan.

### 2.9 Test stack (was 15)

- **Core**: `cargo test`, with `insta` snapshots for parser goldens. A parser
  change shows as a snapshot diff, which is the review artefact.
- **Corpus**: a seeded synthetic generator (`core/tests/corpus/`) producing
  `.eml` plus golden JSON, committed. **Part 9 is its specification** — every
  shape it must emit, and the whole test surface for a deterministic layer that
  promises to work with zero models. This is the primary fixture source: no
  public corpus is both licence-clean and modern, Enron carries real PII, and
  the Untroubled archive is stress input only, fetched on demand and never
  committed.
- **Regression rule**: every parser bug fixed adds its message to the fixture
  set as a redacted synthetic reconstruction. Real mail never enters the
  repository.
- **IMAP is faked by a stub server** on loopback
  (`core/tests/support/imap_stub.rs`) that models one mailbox per provider
  profile (Gmail, iCloud, Fastmail, Yahoo, Outlook): their folder names,
  special-use attributes and sign-in refusal text. It records every command
  and can drop the connection mid-fetch. A model replaced the planned
  transcript replay at M1, because a replay breaks whenever the client's
  command order changes. Provider HTTP endpoints — OAuth token endpoints, Polar, and
  our Worker — are faked with `wiremock`.
- **Server**: `vitest` against the Worker with Miniflare's local D1.
- **UI**: `vitest` plus `@testing-library/svelte` for component logic. The
  bindings file (2.5) makes the seam type-checked, so there are no UI tests
  that assert command shapes.
- **No end-to-end webview automation at v0.** `tauri-driver` on three platforms
  is a large maintenance surface for a release that already has a mandatory
  human smoke test before publish (Part 6). The smoke test is a written
  checklist in Part 6, not an improvised click-around.
- **Performance is a test.** From M1, 1 GB of generated messages runs through
  the parse path in CI with an asserted ceiling on wall-clock and peak RSS,
  scaled from the 1.1 measurement and taken with SQLCipher on. From M5 the same
  test reads a generated mbox. The 10 GB acceptance criterion is checked by hand
  before a release.

### 2.10 CI: the pull-request pipeline (was 17)

Standard GitHub runners are free and unmetered on public repositories
(1.5), so the matrix is decided by value, not by minutes.

`.github/workflows/pr.yml`, on every pull request:

| Job | Runner | Runs |
|---|---|---|
| `core-linux` | `ubuntu-latest`, in an `ubuntu:22.04` container | `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, the perf test |
| `core-macos` | `macos-latest` | `cargo test` |
| `core-windows` | `windows-latest` | `cargo test` |
| `ui` | `ubuntu-latest` | `bun install --frozen-lockfile`, `svelte-check`, `oxlint`, `vitest` |
| `bindings-drift` | `ubuntu-latest` | regenerate bindings, `git diff --exit-code` |
| `data` | `ubuntu-latest` | `cargo run -p et-data -- validate` |
| `server` | `ubuntu-latest` | `bun install --frozen-lockfile`, `tsc --noEmit`, `oxlint`, `vitest` in `server/` |

- **The Linux job uses the same `ubuntu:22.04` container as the release
  build** (Part 9), so glibc and WebKitGTK cannot drift between
  what a pull request tested and what a release ships.
- All seven are required checks. Caching is `Swatinem/rust-cache` plus Bun's
  lockfile cache; an external contributor's first pull request pays one cold
  Rust build and nothing after.
- No `cargo build` of the Tauri bundle on a pull request. Bundling is the
  release pipeline's job and it proves nothing a `cargo test` plus a UI build
  does not.

### 2.11 Google Takeout sample (was 18)

Not a decision — a chore, and it blocks nothing. Do it early anyway, because
it is the only thing with a multi-day queue in front of it.

Request a Gmail-only Takeout export now. When it lands, record it in this
section, without copying any message content: the
mbox variant and its `From ` separator form, whether it is gzipped, which
Gmail headers appear (`X-Gmail-Labels`, `X-GM-THRID`), total size and message
count, and any line-ending or escaping quirk. Note where the file sits on
disk. **The file never enters the repository.**

---

## Part 3 — Repository layout

```
Cargo.toml                  workspace: core, app, native-host, et-data
core/                       the domain. No Tauri dependency.
  src/
    ingest/                 IMAP client, Outlook OAuth, mbox and Maildir readers
    extract/                one message to facts: headers, list signals, receipts
    scan/                   store fetched mail; rebuild senders, services, charges, rollups
    source/                 connected mailboxes and files (S12)
    view/                   what the screens read: S03–S06 queries over the rollups (M2)
    local/                  the data directory: where it is, move, erase (S17, M2)
    setting/                key/value preferences
    store/                  rusqlite, schema, migrations, FTS
    action/                 unsubscribe, playbook execution, bulk runner
    skill/                  the three-stage pipeline and the step executor
    cdp/                    the CDP driver and socket transport
    model/                  ModelClient trait, OpenAI-compatible, Anthropic
    data/                   loader for data/ (services, critical, providers, stats)
    licence/                evaluation clock, reminders, activation, Ed25519 token check
    crypt/                  SQLCipher keying, HKDF subkeys, sealed files
  migrations/               NNN-name.sql, applied by user_version
  tests/corpus/             seeded generator plus goldens
  tests/support/            the IMAP stub server
vendor/mail-parser/         mail-parser with the #156 patch (M1)
app/                        the Tauri binary. Thin command layer over core.
  src/                      commands, Channel streams, browser-file writer
  tauri.conf.json           version omitted; inherits from Cargo.toml
native-host/                the chrome-native-host helper binary
et-data/                    the data schema (lib) and its validator (bin)
extension/                  Chrome extension (MV3), its own version
ui/                         SvelteKit, adapter-static
  src/lib/styles/           tokens.css, theme.css
  src/lib/bindings.ts       generated by tauri-specta, committed
  static/fonts/             vendored woff2
data/                       community data: services/, critical.toml, providers.toml, stats.json
server/                     Cloudflare Worker (Hono, D1): /activate, /check, /deactivate, /stats. Bun.
site/                       landing page, install scripts. Cloudflare on push to main.
.github/workflows/          pr.yml, release.yml, extension.yml, yank.yml
.github/FUNDING.yml         GitHub Sponsors; the same link S17 opens
CHANGELOG.md                single source of release notes
```

`ui` and `server` use Bun (`bun install`, `bunx`). `@sveltejs/adapter-static` is required —
Tauri does not support a server-based frontend (1.2).

**Two on-disk directories exist and must not be merged.** The data directory is
`dirs::data_dir()/EmailTerminator/`, which is platform-specific and movable by
the user (2.1). The browser-integration wrapper lives at the fixed path
`~/.emailterminator/bin/` (`%LOCALAPPDATA%\EmailTerminator\bin\` on Windows),
because a native-messaging manifest names an absolute path that must survive an
app update and a data-location change (1.4).

### Dependency baseline

Checked against crates.io and npm on 2026-08-10; the rows M0 pinned were
re-checked on 2026-09-26. Pin each row when its milestone adds it, and let
Dependabot move them.

| Dependency | Version | Note |
|---|---|---|
| `mail-parser` | 0.11.9 | Vendored with the #156 patch; `full_encoding` for legacy charsets (M1) |
| `async-imap` / `tokio` | 0.11.3 / 1.53.1 | `runtime-tokio`, no async-std |
| `rustls` / `tokio-rustls` / `rustls-platform-verifier` | 0.23.45 / 0.26.5 / 0.7.1 | `ring` provider, OS trust store |
| `regex` | 1.13.1 | Amount and receipt phrases |
| `psl` | 2.1.238 | Registrable domains for grouping senders |
| `chacha20poly1305` | 0.11.0 | The Linux secrets file (2.2) |
| `tauri-plugin-opener` | 2.5.5 | Provider guides, later S15 and checkout |
| `lucide-svelte` | 1.0.1 | Icons, stroke 2.75 |
| `rusqlite` | 0.40.2 | Feature `bundled-sqlcipher-vendored-openssl`; FTS5 confirmed at M0 (2.1) |
| `chacha20poly1305`, `hkdf`, `ed25519-dalek` | pin at M0 | Sealed files and the licence token (1.7, 2.1) |
| `machine-uid`, `hmac`, `sha2` | pin at M0 | Device hash (1.7) |
| `hono` | 4.13.9 | `server/` only |
| `tauri` | 2.11.6 | With `tauri-plugin-single-instance` 2.4.5 |
| `keyring` | 4.2.0 | Default `v1` feature |
| `specta` / `tauri-specta` | 2.0.0-rc.25 | Release candidate — see 2.5. `core` derives `specta::Type` on the view types too (M2) |
| `insta` | 1.48.0 | |
| `wiremock` | 0.6.5 | |
| `dirs` | 7.0.0 | |
| `@sveltejs/adapter-static` | 3.0.10 | |
| `bits-ui` | 2.19.3 | Command palette and the erase confirmation (M2) |
| `tauri-plugin-dialog` | 2.7.3 | S17's folder picker, called from Rust only; the web view has no dialog permission (M2) |
| `lucide-svelte` | 1.0.1 | |

---

## Part 4 — Data model

Tables, with the columns that carry weight. M1's DDL is
`core/migrations/001-initial.sql`; a later milestone adds its tables in its
own migration.

- **`source`** — `id`, `kind` (`imap` | `outlook` | `gmail` | `mbox` | `maildir`; `gmail` is
  reserved for the deferred verified Google client), `label`,
  `last_sync_at`, `message_count`, plus per-kind config. Feeds S12. `last_sync_at`
  only advances while the app is open (1.2); S12 must not imply otherwise.
- **`mailbox`** — one row per IMAP folder: `source_id`, `name`, `uid_validity`,
  `highest_uid`. The resume point of 2.3.
- **`message`** — `id`, `source_id`, `mailbox_id`, `message_id` header,
  `sender_id`, `subject`, `date`, `list_unsubscribe` (raw),
  `list_unsubscribe_post` (bool), `list_id`, `is_list`, `dkim_domains`,
  `one_click` (RFC 8058 holds, M3),
  `locator` (UID for IMAP, byte offset for mbox, file name for Maildir),
  unique per source and mailbox.
  **No body is stored.** The
  locator is how evidence links re-read the original from its source.
  **A locator can stop resolving** — an imported mbox is moved or deleted, an
  IMAP message is expunged. That is a designed state in S14, not an error: the
  row keeps its subject, sender and date, and the evidence link reports that
  the original is no longer reachable.
- **`sender`** — `id`, `address`, `display_name`, `domain`, `service_id`,
  `first_seen`, `last_seen`, `message_count`, `classification` (`service` |
  `newsletter` | `other`), `confidence`, `classified_by` (`parser` |
  `playbook` | `model`), `unsubscribed_at`. A service has many senders (`billing@`, `news@`), so
  the link sits on the sender; it was planned the other way round until M1.
- **`service`** — `id`, `name`, `group_key` (the registrable domain, or the
  merchant for receipts a payment platform relays), `data_key` (the
  `data/services/` entry it matched), `is_critical`, `status` (`active` |
  `canceling` | `canceled`), `cadence`, `monthly_minor_units`, `currency`.
- **`receipt`** / **`charge`** — a receipt is what one message says: `kind`,
  `amount_minor_units`, `currency`, `merchant`, `invoice_ref`,
  `extracted_by`. A charge is money that left the account, after one receipt
  in two folders is merged. Price-increase flags in S04 and price history in
  S06 are derived from `charge` ordered by date.
- **`aggregate`** — per-service and per-sender monthly rollups of spend and
  volume, rebuilt by a scan. This is what S03 and S06 read. **Rollups are keyed
  by currency.** A mailbox holding USD and EUR receipts produces a row per
  currency, and S03's headline figure names the dominant one with the others
  listed beside it. There is no conversion: a rate needs a network call, and
  the privacy statement enumerates every outbound request we make.
- **`action`** — the activity log (S14): `id`, `kind` (`unsubscribe` |
  `playbook` | `agent` | `sync`), `target` (a name, kept as text), `at`,
  `outcome` (`succeeded` | `failed` | `needs_you`), `detail`, `request` (what
  was sent), and the evidence: a `message` id plus copies of its subject,
  sender and date, which outlive the message row. M7 adds the
  `sessions/<id>.jsonl` path. A sweep writes one row per sender, so there is
  no `bulk` row.
- **`setting`** — key/value. Appearance, sweep behaviour, update behaviour,
  data location, active tier, cancel-stats opt-in. The evaluation start and the
  licence token live in the keychain (2.2), not here.

FTS5 external-content table over `message.subject` and `sender.display_name`,
kept current by triggers.

---

## Part 5 — Milestones

Each milestone ends with something demonstrable and its own acceptance check.
The order is driven by one rule: IMAP is the first rung (1.6), so a live
mailbox proves the product first, and file import follows once the results
and actions exist.

### M0 — Skeleton

Cargo workspace with the four crates; SvelteKit with `adapter-static`; Tauri
v2 with the single-instance plugin; `tokens.css` and `theme.css` extracted with
fonts vendored; the SQLCipher store keyed from the keychain, with the FTS5
check from 2.1; `server/` scaffolded as an empty Hono Worker; `pr.yml` running
all seven jobs; `CHANGELOG.md` with an `Unreleased` section; FSL-1.1-MIT `LICENSE`.

Done when: an empty window opens on all three platforms, the database file is
unreadable without the key, and every CI job is green on a pull request.

### M1 — Scan a mailbox over IMAP

IMAP with app passwords through `async-imap`, credential storage, `BODY.PEEK`
fetch, full sync and incremental resync (2.3), MIME parse, sender aggregation,
deterministic subscription and newsletter detection, receipt extraction for the
first ten vendor formats, the schema and its migration stepper, aggregates, and
the scan progress `Channel`.

Handle `mail-parser`'s two defects here — issue #156 silent multipart
truncation and #155 panic on a folded `Received` header, both on the parse path
(1.1). Done at M1: 0.11.9 fixes #155 upstream; #156 is still open, so
`vendor/mail-parser/` carries a patch that accepts a boundary only at the
start of a line. `core/tests/mail_parser_defects.rs` holds a fixture for each.

Screens: S01 (IMAP path only), S02, plus S16's IMAP auth failure state.

Done when: the stub server (2.9) replays every provider transcript, a real
Gmail app-password account scans without a panic, and a disconnect mid-sync
resumes without duplicating rows.

Checked after M3, 2026-09-26: a real Gmail account connected with an app
password and scanned. A second personal `@gmail.com` account had no App
passwords page ("not available for your account") with 2-Step Verification
on, passkeys set, no Advanced Protection. Adding an authenticator app, which
Google then listed as a second step, did not bring the page back. Google
documents three blockers (security keys as the only second step, an
organisation account, Advanced Protection) and this account has none, so
some Gmail accounts cannot use IMAP for reasons Google does not state. Their
only path at v0 is a Takeout mbox import (M5). The Gmail form says both.

### M2 — See the results

Dashboard, subscriptions list, newsletters list, service detail, command
palette, general settings, report an issue. All read paths over M1's data. This
is where the design system becomes real components.

Screens: S03, S04, S05, S06, S18, S15, plus S16's empty states and lost-key
state. **S17 opens here but does not finish here**: its appearance,
data-location, encryption-backend and erase-all-data controls land in M2, sweep
behaviour in M3, the browser integration control in M7, the licence and
cancel-stats sections in M8, and the update section in M9.

Done when: every one of those screens renders from a scanned mailbox and
matches its mockup in light and dark.

Found at M2:

- **"Per year" is the last twelve months of the rollups**: this month and the
  eleven before it. S03, S04, S05 and S06 all read it from `aggregate`, so the
  figures agree across screens.
- **The price-increase flag is stored** on `service` (migration 002) and S06's
  price history comes from `summary::price_changes`, which uses the same plan
  split as the flag. Rows from an M1 scan read 0 until the next scan.
- **Moving and erasing finish at the next launch.** Windows refuses to delete a
  file an open connection holds, so the app records the request, restarts,
  and `local::prepare` completes it before the database opens. A move copies,
  opens the copy with the key and compares it, then points the default
  directory at it with a `location` file; the original goes on the next start.
- **Numbers and dates are formatted in English**, to match the English copy.
- **Actions show but stay disabled** until their milestone: bulk cancel and
  unsubscribe (M3), playbooks (M4), the agent (M7). S06's "view email" link
  and S14's log arrive with M3's evidence links; the "worth it" card needs
  open counts, which no source provides yet.
- **Design preview**: `bun run dev` in a plain browser answers every command
  from `ui/src/lib/dev/mock.ts` with the mockups' figures (`?mock=empty`,
  `?mock=clean`, `?mock=locked`, `?theme=dark`). The build does not include it.
- **Windows test binaries** need the Common-Controls manifest that
  `tauri-build` gives only the app binary; `app/build.rs` adds it to every
  target.

### M3 — Act

RFC 8058 one-click unsubscribe and the `List-Unsubscribe` header grammar —
both written by us, since no library in either ecosystem implements them
(1.1). The activity log, the bulk runner with per-item progress over a
`Channel`, and the critical-services confirmation.

Screens: S07, S10, S11, S14, plus S16's critical-service warning and S17's
sweep-behaviour settings — which are this milestone's defaults made editable:
whether critical services are excluded from a bulk run, and the size at which a
bulk run asks for confirmation.

Done when: a bulk unsubscribe over a scanned mailbox reports per-item outcomes,
excludes critical services by default, and writes an evidence link for each.

### M4 — Community data

The TOML format, the loader, `et-data validate` in CI, the playbook screen,
and enough seed entries to be useful — the twenty highest-volume SaaS senders
plus the critical list (AWS, registrars, payment processors, domain and DNS
providers).

Screens: S08.

Done when: a contributor can add a service in one file and CI rejects a
malformed one with a file and line.

Found at M4:

- **Seed playbooks come from each vendor's own help page**, read on
  2026-09-26 and named in `source`. Nobody has walked them in a live account,
  so S08 says "checked", not the mockup's "verified by the community".
  Peacock and Max are missing: their help sites refused a non-US address.
  Crunchyroll and Headspace took their places.
- **Most seed domains are the brand's registrable domain.** Few vendors
  document their billing sender. `critical.toml` uses exact addresses where a
  domain is shared: AWS, Google Cloud and Azure mail from `amazon.com`,
  `google.com` and `microsoft.com`.
- **The `List-Unsubscribe` host matcher is not built.** No seed entry needed
  it; it returns with the first entry that does.
- **S08 records the cancellation when the user says it is done.** The
  mockup's "we watch for the confirmation email" needs a later scan to match
  a cancellation receipt to the service, which no milestone owns. Finishing
  writes a `playbook` row to S14 and sets the service to cancelled.
- **S10 names the action**: "cancel <name>" before a playbook, "unsubscribe
  <name>" before a sweep.
- **The schema lives in `et-data` and `core` depends on it**, so the CI data
  job still builds without SQLCipher.

### M5 — More sources

The Outlook.com OAuth client (1.6), then the sources screen. Then file
import: the mbox reader (including the gzip wrapper and `X-Gmail-Labels`, which `mail-parser`
does not cover) and the Maildir reader.

Screens: S01 (all paths), S12, plus S16's mbox-parse-failure error state.

Done when: all five target providers (Gmail, iCloud, Fastmail, Yahoo,
Outlook) sync incrementally; a 1 GB generated mbox scans inside the CI ceiling;
and a real Takeout export (2.11) scans without a panic.

### M6 — Intelligence

The `ModelClient` trait, the OpenAI-compatible and Anthropic implementations,
Tier 1 endpoint discovery, the `llama.cpp` fallback fetch, and the tier
picker. Then the model stage of the skill pipeline, for long-tail
classification and messy receipts only.

Screens: S13.

Done when: the full test suite passes with no model configured and with each
tier configured, and the Tier 0 result set is unchanged by a tier being on.

### M7 — Agentic cancellation

The extension (MV3, `chrome.debugger` plus `nativeMessaging`), the
native-messaging host binary, the socket transport, the protocol handshake with
its integer version, the CDP driver in `core`, the step executor, pause and
hand-over, and the browser-integration file writer that runs at every launch.

Freeze both extension IDs before anything ships: reserve the Web Store item
with a draft upload, and pin the unpacked build's ID with a `key` in
`manifest.json` (1.4).

Screens: S09, both unavailable states in S06 and S09, S17's
browser-integration control, and S16's agent-blocked state and experimental
marker.

Done when: a cancellation runs end to end in a real logged-in profile, pause
and hand-over work, and both unavailable states render correctly with the
extension removed and with a deliberately mismatched protocol version.

### M8 — Licence and stats

The Polar product and its licence-key benefit with an activation limit of 3;
the Worker's `/activate`, `/check`, `/deactivate` and `/stats` routes on D1;
the Ed25519 signing key as a Worker secret; the evaluation clock, the
reminders, the device hash, activation, the 15-day check and its grace;
the cancel-stats opt-in and its sender; the export script that writes
`data/stats.json`; and the S06/S08 lines that read it.

Screens: S17's licence and cancel-stats sections, S16's evaluation reminder,
licence-invalid state and "Unregistered" marker, the stats lines in S06 and
S08.

Done when: an unlicensed copy shows no reminder before day 15, shows every
reminder in 1.7 from day 15 with every feature still working, never interrupts
a scan, a critical confirmation or an agent run, and goes quiet with a Polar
test key; a fourth device is refused and a freed slot accepts it; a licensed
copy runs 30 days with the network off and then shows reminders; a token
copied to another machine is rejected; a refunded key fails its next check;
and a stats payload
captured in a test holds nothing beyond the four fields in 1.7.

### M9 — Ship

The release pipeline exactly as Part 8 specifies: the gate, three platform
jobs, the single assemble job writing `latest.json` and `SHA256SUMS`, build
provenance attestation, the `ext-v*` extension workflow with staged rollout,
and the `workflow_dispatch` yank. The landing site with the price and the Polar
checkout link, `install.sh`, `install.ps1`, `install-prerelease.sh`,
`install-prerelease.ps1`, `uninstall.sh`, `uninstall.ps1`. The updater plugin
wired to S17's three-way setting and the "what changed" screen.

Screens: S00, plus S16's update-ready state.

Done when: `0.1.0` installs from the site on all three platforms, and a
`0.1.1` tag reaches an installed copy through the updater without a prompt.

Found at M3:

- **The one-click verdict trusts the receiving server.** RFC 8058 needs a
  valid DKIM signature over both list headers; verifying one needs a DNS
  lookup per sender. The first `Authentication-Results` header must report
  `dkim=pass` for the domain of a signature whose `h=` names both headers.
  Mail without that header is never one-click. The verdict is stored per
  message; migration 003 fetches each folder again from its oldest candidate.
- **mail-parser returns the last copy of a repeated header**, so code that
  needs the first reads the header list itself.
- **The POST is written over the IMAP client's TLS stack**, with no HTTP
  crate: one request, no cookies, no redirects followed. Only a 2xx answer
  counts as done.
- **Four routes, one request.** One-click sends the POST. An HTTPS or HTTP
  page, or a `mailto:` address, is handed to the user as "needs you", with a
  link to finish. A message with no header fails. The app sends no email.
- **A service is unsubscribed through its senders that send list mail.**
  Receipts carry no list headers, so billing mail keeps arriving.
- **Nothing sets `is_critical` until `data/critical.toml` (M4).** S10 and the
  exclusion are built and tested with the flag set by hand.
- **Sweep settings are two switches**: confirm before bulk actions (with the
  size from which a sweep is reviewed anyway, 10 by default) and leave
  critical services out. A sweep below that size skips S11, and each
  critical item in it still asks S10. The mockup's "keep watching after
  unsubscribe" needs a re-send on later scans that no milestone owns, and
  "update playbooks automatically" contradicts 2.6's bundle-only data, so
  neither is built; S07's line about stragglers went with the first.
- **Scans log a `sync` row**, so S14's sync filter has rows.
- **async-imap ends a FETCH stream without an error when the connection
  closes**, so a short batch now sends a NOOP before it commits. CI caught
  it on macOS; locally the close arrived as an error instead.

### Sizing

Wall-clock for an AI agent executing the work, not human effort. Ranges, not
commitments — M1 and M7 carry the most unknowns, because one meets real mail
and the other meets a real browser.

| Milestone | Estimate | What drives the spread |
|---|---|---|
| M0 Skeleton | 3–4 h | Three-platform CI going green, and SQLCipher's OpenSSL build on Windows |
| M1 Scan over IMAP | 2–3 d | Receipt formats, the two `mail-parser` defects, provider quirks |
| M2 See the results | 2–3 d | Eight dense screens against mockups, light and dark |
| M3 Act | 1 d | |
| M4 Community data | 1 d | Twenty seed entries is research, not code |
| M5 More sources | 2 d | The Outlook OAuth flow, plus the mbox and Maildir readers |
| M6 Intelligence | 1 d | |
| M7 Agentic cancellation | 2–3 d | Extension, host, socket and CDP against a real profile |
| M8 Licence and stats | 1 d | Polar's API in test mode |
| M9 Ship | 1–2 d | Plus the hardware checks in Part 6, which are serial |

Roughly two and a half to three and a half weeks of agent wall-clock. **Three
things do not respond to effort** and should start now if they can: the
Takeout export queue (2.11), Chrome Web Store review for the extension, and the
hardware confirmations in Part 6.

---

## Part 6 — Before the first release

Blocking items, each with a named owner action. None of these is discoverable
from code review.

**Must be confirmed on hardware** (1.4 and 1.5):

1. `tauri build --target universal-apple-darwin` ad-hoc signs the merged
   binary. Apple silicon refuses unsigned arm64 code, so a failure here means
   no macOS release at all.
2. `appimagetool` runs inside the `ubuntu:22.04` container with
   `APPIMAGE_EXTRACT_AND_RUN=1`. Fallback is the `ubuntu-22.04` runner until
   April 2027.
3. macOS App Management does not prompt during a self-update in
   `~/Applications`.
4. Chrome on Windows accepts our wrapper path as a native-messaging host.
5. The macOS keychain still hands the database key to the app after a
   self-update. An ad-hoc signature changes with every build, and the keychain
   ties an item's access list to the code signature. If it prompts, every
   update shows a keychain dialog, and a "Deny" locks the user out of their
   database until they allow it. Test before `0.1.0`; the fix, if needed, is a
   keychain access group or a stable self-signed identity.

**Must exist before the pipeline can finish**:

6. The minisign key, password-protected, in the `release` Environment, with two
   offline backups. Losing it strands every installed copy permanently.
7. Chrome Web Store client ID, client secret and refresh token. The refresh
   token is revocable and will fail silently one day, so the extension workflow
   fails loudly when the API rejects it.
8. Both extension IDs frozen (M7).
9. A tag protection rule restricting `v*` to maintainers.
10. The Ed25519 licence key as a Worker secret, with two offline backups. Its
    public half is compiled into every build, so losing the private
    half means no new activation until an app update ships a new public key.
11. The Polar product as pay-what-you-want with its licence-key benefit: minimum
    and default $19 for launch week, then $29, the Polar API token
    as a Worker secret, and the Worker deployed on `api.emailterminator.com`.
12. The Microsoft app registration for Outlook.com, public client with PKCE.

**Human smoke test before publishing any draft release**, on each platform:
install from the script, first run reaches S01 with the evaluation running, connect
a test IMAP account, dashboard renders, one unsubscribe completes and appears in
the activity log, a Polar test key activates, settings open, quit and relaunch
retains data.

---

## Part 7 — Explicitly not in v0

Virtual cards, email aliasing, a hosted version, mobile apps, a CLI,
Apple-signed builds, a Homebrew cask, an `.msi`, npm, a background daemon,
nightly builds, a second release channel, runtime-fetched community data,
end-to-end webview automation, our own verified Google OAuth client, and any
telemetry beyond the two payloads in 1.7. Each has a trigger recorded in `CONTEXT.md` or
in the Part 1 section that removed it.

---

## Part 8 — Packaging and release reference

The mechanics behind locked decisions 9–18. M9 builds this; the reasoning is in
1.4 and 1.5.

### Artifacts per release

- `EmailTerminator_<ver>_universal.app.tar.gz` and `.sig`
- `EmailTerminator_<ver>_amd64.AppImage`, `.AppImage.tar.gz` and `.sig`
- `EmailTerminator_<ver>_x64-setup.exe` and `.sig`
- `emailterminator-extension-<extver>.zip`
- `latest.json`
- `SHA256SUMS`

No `.dmg`, `.deb`, `.rpm` or `.msi`. A `.dmg` serves only the browser download,
which is the path that quarantines the bundle. A `.deb` splits Linux users into
a group that cannot self-update. An `.msi` needs an administrator install that
no v0 user has asked for.

Windows on ARM runs the x64 build under emulation, so there is no second
Windows installer.

### Channels and install locations

Two channels: the shell installer from emailterminator.com, and the GitHub
Releases page as a documented fallback. Every release asset is public, because
the installer fetches from there.

| OS | Location |
|---|---|
| macOS | `~/Applications/EmailTerminator.app`, creating the directory if absent |
| Linux | `~/Applications/EmailTerminator.AppImage`, with a `.desktop` file and icon under `~/.local/share` |
| Windows | the Tauri NSIS per-user default under `%LOCALAPPDATA%\Programs` |

No administrator prompt at any point. This keeps macOS updates on the silent
`rename` path and off the AppleScript admin fallback.

The installer script lives in this repository and is deployed to the site by
the release tag, so its URL is stable while its content is versioned. The bytes
live on GitHub Releases. The first install trusts TLS alone, which is what the
Bun, Deno and Scoop installers do, because minisign is absent from a stock
machine; every update after it is minisign-verified.

Release notes lead with the installer line, then carry a "downloaded the file
directly?" section with the per-OS repair step: `xattr -dr
com.apple.quarantine` on macOS, "More info → Run anyway" on Windows, `chmod +x`
for the AppImage.

### Updates

1. Check at launch, at most once per 24 hours.
2. Download in the background.
3. Install during quit, and only when the download already finished, so
   quitting never waits on the network.
4. Show what changed at the next launch.

The setting offers Automatic (default), Notify only, and Off. S01's privacy
statement names the check and what it sends: our version, operating system,
architecture, and the IP that any HTTPS request carries. The update check
carries nothing we invent; the device hash goes only with licence requests
(1.7).

The cost of that line, stated so nobody is surprised later: adoption signal is
limited to GitHub asset download counts, `latest.json` hits, and activation
counts. **We will not know crash rates.** An
opt-in report that shows its exact payload before sending is the cheap answer
if that ever matters, and it is a separate decision.

### Versioning

The `app` crate's `Cargo.toml` holds the version and `tauri.conf.json` omits
the field so it inherits. App tags are `v<version>`, extension tags are
`ext-v<version>`, and CI fails a tag whose number disagrees with the file it
names. The first release is `0.1.0`.

| Bump | Trigger | Promise |
|---|---|---|
| patch | Fixes, copy, playbook and recipe data | No schema migration, no protocol change, no new screen |
| minor | New user-visible capability, schema migration, or extension protocol bump | Migration runs once, forward only, automatic at launch |
| `1.0.0` | All three ingestion paths shipped, agentic cancellation out of the experimental flag, extension live in the Web Store | Stated stability |

### Browser integration files

Written at every launch, unconditionally:

- a wrapper at a fixed per-user path — `~/.emailterminator/bin/chrome-native-host`
  on macOS and Linux, which execs the helper in the bundle or the AppImage with
  a `--native-host` argument; a small `.exe` under
  `%LOCALAPPDATA%\EmailTerminator\bin\` on Windows;
- the native-messaging manifest in every installed Chromium-family browser
  directory — Chrome, Brave, Edge, Vivaldi, Chromium — and, on Windows, the
  registry key that points at it.

Installation order therefore never matters: a user who installs the extension
first finds a working host, and a user who enables the feature first finds the
extension when it arrives. The per-launch write also repairs a moved or updated
app. The files are inert without our extension, because `allowed_origins` names
only our two IDs. S17 removes them.

### Uninstall

`curl -fsSL emailterminator.com/uninstall.sh | sh`, plus NSIS uninstaller hooks
on Windows. It removes the bundle or AppImage, the wrapper, every manifest it
finds, the registry key and the desktop entry.

**Application data and keychain entries survive by default** and go only with
`--purge` or a yes at an interactive prompt. Deleting a scan that took ten
minutes, because someone reinstalled to fix a bug, is the worse failure. S17's
erase-all-data remains the data-only action.

### The release pipeline

A `v*` tag starts a release; tag creation is restricted to maintainers by a tag
protection rule. Three platform jobs upload into a **draft** release, and a
human smoke-tests and publishes.

**Gate**, one job that the builds declare `needs: gate`. It asserts the tag
equals the version in the `app` crate's `Cargo.toml`; the tagged commit is on
`main`; `CHANGELOG.md` has a section for that version; and the test suite
passes, once, on Linux. The commit-on-`main` assertion catches the realistic
accident, which is tagging a local branch. Repeating the full three-OS matrix
would re-test code byte-identical to what the pull request already tested.

| Target | Runner | Notes |
|---|---|---|
| macOS universal | `macos-latest` | arm64 host; `rustup target add x86_64-apple-darwin`, built with `--target universal-apple-darwin` |
| Linux x86_64 AppImage | `ubuntu-latest`, job in an `ubuntu:22.04` container | `APPIMAGE_EXTRACT_AND_RUN=1`, because `appimagetool` has no FUSE in a container |
| Windows x64 NSIS | `windows-latest` | native |

`tauri-action` per platform, with `uploadUpdaterJson: false` and
`uploadUpdaterSignatures: true`. One assemble job runs after all three and is
the only writer of the release's shared files. It writes `latest.json` with
explicit `darwin-universal`, `linux-x86_64` and `windows-x86_64` keys plus the
version, signatures and `notes`; publishes `SHA256SUMS`; and renders the
release body from the template.

### Signing, secrets and rotation

The minisign key is password-protected. It and its password are secrets in a
`release` GitHub Environment restricted to `v*` tags, so no branch workflow can
sign anything. Two offline backups exist, one in the maintainer's password
manager and one encrypted copy held elsewhere.

**Rotation, written before it is needed**: ship a release signed by the old key
whose bundled `tauri.conf.json` carries the new public key, wait for installs to
take it, then sign subsequent releases with the new key. Rotation is possible
only while the old key still exists. Losing it strands every installed copy on
its current version permanently.

Other secrets: Chrome Web Store client ID, client secret and refresh token. The
refresh token is revocable and will fail silently one day, so the extension
workflow must fail loudly when the API rejects it.

### Provenance and checksums

Every artifact gets an `actions/attest-build-provenance` attestation, verifiable
with `gh attestation verify`. The installer verifies its download against
`SHA256SUMS`.

Stated plainly, because the limit matters: a checksum fetched from the host that
served the bytes detects a corrupt or truncated download. It does not defend
against a compromise of GitHub. The attestation is the artifact-level claim a
reader can check without trusting us, which is the one that suits this
product's pitch.

### Release notes

`CHANGELOG.md` is the single source, one section per version. The release commit
moves `Unreleased` under the new number. The assemble job injects that section
into three places: the release body, the `notes` field of `latest.json`, and
through it the app's "what changed" screen at the first launch after an update.
The gate fails a tag whose section is missing.

The text is written for a user who did not ask to read our commit log, which is
why it is not generated from commit messages.

### Site and scripts

The landing site is a directory in this repository, built by Cloudflare's Git
integration on push to `main`. **There is no release trigger.** Anything
version-specific on the site is read from `latest.json` at runtime, so a site
build can never display a version that disagrees with what the installer
fetches.

The site serves `install.sh`, `install.ps1`, `install-prerelease.sh`,
`install-prerelease.ps1`, `uninstall.sh` and `uninstall.ps1`, plus the price
and a link to the Polar checkout. All are static files, so the site needs no
Function and no server-side logic. Activation and stats live in `server/` on
their own host (1.7). A push to `main` that touches `server/` deploys it with
`wrangler deploy`.

**Installer scope is latest only.** `install-prerelease.sh` resolves the newest
GitHub pre-release through the API and installs it, so it is still "latest of a
channel" and one script still means one thing. Semver ordering does the rest: a
tester on `0.2.0-rc1` is not moved by a `0.1.9` stable release and is moved when
`0.2.0` ships, with no channel setting in the app.

There are no scheduled nightly builds. Nightlies would put the signing key on an
automatic schedule, and nobody is testing daily.

### Extension pipeline

An `ext-v*` tag builds the zip, attaches it to its own GitHub release, uploads
it to the Chrome Web Store, and publishes automatically once review passes.
Publish uses a staged rollout percentage, which is the safety valve now that no
human stands between a tag and every store user — and it is the only brake.

**The copy shipped inside the app is pinned to the newest `ext-v*` tag**, not
built from the app's own commit. A version number that identifies two different
builds is a support trap that only surfaces while someone is already debugging
something else. A release is therefore not reproducible from its tag alone, so
the assemble job records both versions in the release body.

Distribution is one Web Store listing as the main path, with the unpacked build
also shipped inside the app; the sideload flow opens `chrome://extensions` and
puts the folder path on the clipboard.

### Withdrawing a release

Recovery is always fix-forward, because the updater has no downgrade and the
installer has no pin.

A `workflow_dispatch` yank takes a tag and flips that release to draft in one
click. `/releases/latest/download/latest.json` then falls back to the previous
published release, which stops new installs and stops update checks offering the
bad build to anyone who has not taken it. The useful window is the minutes
before most users next launch the app, which is exactly when nobody remembers
the right `gh` invocation.

The fix ships as a higher version, and its changelog entry names the withdrawn
one.

---

## Part 9 — Corpus generator specification

The deterministic layer claims to work with zero models configured. That
makes fixtures the whole test surface. A seeded generator, emitting raw
`.eml` files plus an expected-result JSON per file, must cover the following.

### Receipt and billing shapes

Twelve message kinds, per vendor template:

1. First charge / order confirmation.
2. Recurring renewal, monthly.
3. Recurring renewal, annual.
4. Trial ending in N days.
5. Payment failed, first dunning notice.
6. Payment failed, final notice before suspension.
7. Card expiring.
8. Plan upgrade with proration.
9. Plan downgrade with credit.
10. Price-change notice, effective on a future date.
11. Refund and partial refund.
12. Cancellation confirmation.

Field variation inside them:

- Amount forms: `$12.00`, `US$12.00`, `USD 12.00`, `12,00 €`, `€12`,
  `¥1,200` with no decimal part, `12.00 USD` after the number.
- Locale decimal separators: `1,234.56` and `1.234,56`.
- Tax lines: no tax, US sales tax, EU VAT with a VAT number, GST, reverse
  charge with a zero amount.
- Period expressed as `Jan 1 – Feb 1, 2026`, as `01/02/2026 - 01/03/2026`
  under both day-first and month-first reading, and as "next 12 months".
- Invoice identifiers, card last-four, and a zero-amount invoice on a 100%
  discount.
- One amount split by a quoted-printable soft break, `$12=\n.00`, and one
  amount with an encoded period, `$12=2E00`.
- Amount present only in the HTML part, with the text part empty.
- Amount present only in a PDF attachment, with neither body part carrying it.

### Sender shapes

- `From` display name against a different envelope sender, so `Return-Path`
  and `From` disagree.
- Per-function subdomains: `billing@`, `receipts@`, `no-reply@`,
  `invoice+acct123@`.
- ESP relays where the DKIM `d=` is `sendgrid.net`, `amazonses.com` or
  `mailgun.org` while the `From` domain is the vendor.
- Vendor changing sending domain between two months of the same
  subscription, to test recurrence grouping.
- Two unrelated vendors on one shared ESP domain, which must not merge.
- IDN and punycode sender hosts.

### `List-Unsubscribe` variants

- `mailto:` only.
- `https:` only.
- Both, in each order.
- Two `mailto:` values.
- Folded across lines per RFC 5322, including a fold inside the URI's angle
  brackets.
- `mailto:` with `?subject=unsubscribe` and with a `body=` parameter.
- Missing angle brackets, which real senders emit.
- A URI longer than 998 octets, forcing a fold.
- `List-Unsubscribe-Post: List-Unsubscribe=One-Click` present and correct.
  RFC 8058 requires the HTTPS URI, the exact key/value pair, and a DKIM
  signature covering both headers
  ([RFC 8058 §3.1](https://www.rfc-editor.org/rfc/rfc8058.txt)).
- `List-Unsubscribe-Post` present with a `http:` URI only, which is invalid
  and must not be offered as one-click.
- `List-Unsubscribe-Post` present with no DKIM signature, likewise invalid.
- `List-Unsubscribe-Post` with a wrong value.
- The header present on a transactional receipt, where offering unsubscribe
  would be wrong.
- Full RFC 2369 sets with `List-Id`, `List-Help`, `List-Post` and
  `Precedence: bulk`, which is the Apache-list shape and must classify as a
  list.
- No unsubscribe header at all, with an unsubscribe link only in the HTML
  body.

### RFC 2047 encoded words

- `Q` and `B` encoding, in `Subject` and in the `From` display name.
- Charsets: `utf-8`, `iso-8859-1`, `windows-1252`, `shift_jis`, `gb2312`,
  `koi8-r`, plus one unregistered charset name.
- Adjacent encoded words separated by whitespace, where the whitespace is
  dropped on decode.
- An encoded word split across a fold.
- A multi-byte character split across two encoded words.
- An encoded word over the 75-character limit, which real senders emit.
- Broken base64 padding and a stray `=?` that is not an encoded word.
- RFC 2231 continuations for an attachment name:
  `filename*0*=utf-8''...` and `filename*1*=...`.

### Multipart and transport edge cases

- `multipart/alternative` with text and HTML.
- `multipart/related` with `cid:` images referenced from the HTML.
- `multipart/mixed` carrying a PDF invoice.
- `alternative` nested inside `mixed` inside `related`.
- Missing closing boundary.
- The boundary string appearing as literal body text.
- Non-empty preamble and epilogue.
- CRLF and bare-LF line endings, and one file mixing both.
- `8bit` content declared as `7bit`.
- Declared charset disagreeing with the actual bytes.
- `Content-Type` with no `charset` parameter.
- HTML-only message with no text part.
- Duplicate `Subject` and duplicate `From`.
- Missing `Date`, missing `Message-ID`, and a duplicate `Message-ID` across
  two files.
- Obsolete and broken date forms: `-0000`, `GMT`, `UT`, two-digit years.
- Base64 body with no padding and with wrapped lines.

### Container edge cases

- mbox with `From ` lines inside bodies, in both `mboxo` and `mboxrd`
  escaping.
- Google Takeout mbox carrying `X-Gmail-Labels` and `X-GM-THRID`.
- maildir input alongside mbox input.
- A single message over 25 MB.

### Time-series shapes

Recurrence detection needs whole sequences, not single messages:

- 24 consecutive monthly receipts from one vendor.
- An annual plan across 3 years.
- A subscription that stops after 7 months.
- A price increase mid-sequence.
- Two overlapping subscriptions from one vendor.
- A one-off purchase that must not read as recurring.
- Duplicate delivery of the same receipt to two folders.
