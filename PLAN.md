# EmailTerminator — v0 implementation plan

Date: 2026-08-10
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
| 1 | Domain logic is Rust. TypeScript exists only in the SvelteKit UI. | 1.1 |
| 2 | A 10 GB mbox scans in under ten minutes with flat memory. | 1.1 |
| 3 | v0 is the desktop app alone. No CLI, no localhost UI, no daemon, no bound port. | 1.2 |
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

**What would reopen it**: Apple or Microsoft signing money arriving, which makes
a cask and a browser download viable and changes the whole channel argument; or
a user need for managed deployment, which is what an `.msi` is for.

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

### 1.6 Why the ingestion ladder has three rungs

Every official read path for consumer Gmail was costed. The axis that decides
it is developer verification: Google's restricted scopes (`gmail.readonly`,
`mail.google.com`, even `gmail.metadata`) require verification plus an annual
CASA Tier 2 audit, which runs roughly $540–1,800 a year on the precedents we
could confirm.

| Path | Verification burden | User friction | Freshness |
|---|---|---|---|
| Google Takeout mbox export | None | High, manual; schedulable every 2 months | Snapshot |
| IMAP with app password | None | Medium: enable 2SV, generate, paste | Live |
| BYO OAuth client | On the user, not us | Medium: guided Google Cloud setup | Live, plus history and push |
| Own verified client + CASA | Verification + annual audit | Low | Live |

App passwords survived the 2022–2025 "less secure apps" shutdowns and Google
publishes no sunset date; third-party claims of a 2026 phase-out are
unconfirmed. That makes rung 2 the workhorse, and it covers Gmail, iCloud,
Fastmail and Outlook with one implementation.

The user-supplied OAuth client is the dominant 2026 pattern for developer-facing
tools — rclone, Home Assistant, gmvault, mbsync — and Hermes and OpenClaw both
ship exactly this shape with guided setup, expecting the unverified-app warning
and the "publish to production" step. Our users tolerate it.

Two escape hatches were rejected. No open-source project lends out its verified
client, and doing so without consent violates Google OAuth policy. Free managed
OAuth middlemen exist, but mail transits their servers, which is incompatible
with local-first.

The "local-only apps are exempt from CASA" reading is contradicted in practice:
Mimestream is a local-only client and still passed CASA. Treat the exemption as
discretionary and budget for the audit if we ever ship our own client — that is
rung 4, deferred until sponsorship funds it, and it is the same unlock as the
hosted version.

**RFC 8058 one-click unsubscribe needs no mailbox access at all**, because it is
an HTTPS POST. That is why rung 1 already delivers the product's headline
action.

### Invariants that override any local convenience

- **Mail never leaves the machine.** The only outbound requests the app makes
  on its own are the update check and, when the user configures Tier 2, calls
  to the endpoint they entered.
- **No telemetry of any kind.** No crash phone-home, no identifier we invent.
- **No network font, script, or stylesheet at runtime.** Everything the UI
  loads is bundled. This kills the `@import url('https://fonts.googleapis.com…')`
  that ships in the design-system CSS (see 2.4).
- **Tier 0 is complete.** Every feature path must work with no model
  configured. A model call is an enhancement inside a branch that already has
  a deterministic answer.
- **MIT-compatible dependencies only.**

---

## Part 2 — The eleven open questions, decided

These were tickets 07, 09, 10, 11, 12, 14, 15, 17, 18, 19, 20. Each is now a
build instruction. Rationale is kept to what a builder needs to not undo it.

### 2.1 Storage and search (was 07)

**SQLite through `rusqlite` with the `bundled` and `fts5` features.** Bundled
compiles SQLite from source, so there is no system dependency on any of the
three platforms and no version drift between them. SQLite is public domain.

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
  scan artefacts, every keychain entry we created, the fallback key file, and
  the settings file. It does not touch the browser integration files; those go
  through their own control (Part 9).
- Stored items: IMAP app passwords (per source), Gmail OAuth client secret and
  refresh token (per source), Tier 2 provider API keys (per provider).

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
  and per-provider quirk handling for Gmail, iCloud, Fastmail and Outlook —
  all four are app-password targets at v0.
- **CONDSTORE/QRESYNC are used when the server advertises them**, with a
  UID-range fallback when it does not. Incremental resync is needed at v0, not
  later: nothing syncs while the app is closed (1.2), so every launch
  performs a catch-up and a full re-fetch each time is not viable.
- Fetch is `BODY.PEEK`, never `BODY`. Reading a user's mail must not mark it
  read.

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
  privacy invariant. Both faces are OFL, which bundles into an MIT project as
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
  Caprasimo, which the mockups override. Port its two useful rules into the
  project's oxlint config, applied to `.svelte`: no raw hex colour, no raw
  `px` literal. Correct the font list to Bricolage Grotesque and Figtree.

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
- **The schema is the Rust `serde` struct.** There is no second schema
  document to drift. Validation is `cargo run -p et-data -- validate`, a
  workspace binary that deserialises every file, checks matcher uniqueness and
  link reachability of the declared form, and prints file and line on failure.
  CI runs it on every pull request, so a malformed contribution fails before a
  maintainer reads it.
- **Matching**: each entry declares matchers explicitly — a list of domains
  (the common case), optional sender-address patterns, optional
  `List-Unsubscribe` host. Never inferred from the filename. Matcher collisions
  across files are a validation error.
- **One format serves S08 and S09.** A playbook step carries `text` (shown to
  the user) and an optional `action` (a declarative instruction the agent can
  execute: navigate, click a described target, fill, wait, confirm). A step
  with no `action` is a human-only step, and the agent stops there and hands
  over.
- **Three collections**: `data/services/` (cancellation playbooks and
  unsubscribe recipes, keyed per service), `data/critical.toml` (the
  critical-services warning list), `data/providers.toml` (Tier 2 endpoint
  presets).
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
  §3.2 restricts it to research purposes, so it cannot ship in an MIT product
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
- **IMAP is faked by a scripted stub server** on loopback that replays recorded
  response transcripts, one per provider quirk (Gmail, iCloud, Fastmail,
  Outlook). Provider HTTP endpoints are faked with `wiremock`.
- **UI**: `vitest` plus `@testing-library/svelte` for component logic. The
  bindings file (2.5) makes the seam type-checked, so there are no UI tests
  that assert command shapes.
- **No end-to-end webview automation at v0.** `tauri-driver` on three platforms
  is a large maintenance surface for a release that already has a mandatory
  human smoke test before publish (Part 6). The smoke test is a written
  checklist in Part 6, not an improvised click-around.
- **Performance is a test.** A 1 GB generated mbox scans in CI with an asserted
  ceiling on wall-clock and peak RSS, scaled from the 1.1 measurement. The
  10 GB acceptance criterion is checked by hand before a release.

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

- **The Linux job uses the same `ubuntu:22.04` container as the release
  build** (Part 9), so glibc and WebKitGTK cannot drift between
  what a pull request tested and what a release ships.
- All six are required checks. Caching is `Swatinem/rust-cache` plus Bun's
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
    ingest/                 mbox reader, IMAP client, Gmail OAuth
    parse/                  MIME, List-Unsubscribe grammar, receipt extraction
    detect/                 subscription and newsletter classification
    store/                  rusqlite, schema, migrations, FTS
    action/                 unsubscribe, playbook execution, bulk runner
    skill/                  the three-stage pipeline and the step executor
    cdp/                    the CDP driver and socket transport
    model/                  ModelClient trait, OpenAI-compatible, Anthropic
    data/                   loader for data/ (services, critical, providers)
  migrations/               NNN-name.sql, applied by user_version
  tests/corpus/             seeded generator plus goldens
app/                        the Tauri binary. Thin command layer over core.
  src/                      commands, Channel streams, browser-file writer
  tauri.conf.json           version omitted; inherits from Cargo.toml
native-host/                the chrome-native-host helper binary
et-data/                    the data validator binary
extension/                  Chrome extension (MV3), its own version
ui/                         SvelteKit, adapter-static
  src/lib/styles/           tokens.css, theme.css
  src/lib/bindings.ts       generated by tauri-specta, committed
  static/fonts/             vendored woff2
data/                       community data: services/, critical.toml, providers.toml
site/                       landing page, install scripts. Cloudflare on push to main.
.github/workflows/          pr.yml, release.yml, extension.yml, yank.yml
CHANGELOG.md                single source of release notes
```

`ui` uses Bun (`bun install`, `bunx`). `@sveltejs/adapter-static` is required —
Tauri does not support a server-based frontend (1.2).

**Two on-disk directories exist and must not be merged.** The data directory is
`dirs::data_dir()/EmailTerminator/`, which is platform-specific and movable by
the user (2.1). The browser-integration wrapper lives at the fixed path
`~/.emailterminator/bin/` (`%LOCALAPPDATA%\EmailTerminator\bin\` on Windows),
because a native-messaging manifest names an absolute path that must survive an
app update and a data-location change (1.4).

### Dependency baseline

Checked against crates.io and npm on 2026-08-10. Pin these at M0 and let
Dependabot move them.

| Dependency | Version | Note |
|---|---|---|
| `mail-parser` | 0.11.5 | Pin exactly; carries issues #155 and #156 (M1) |
| `async-imap` | 0.11.3 | Brings `tokio` into `core` (2.3) |
| `rusqlite` | 0.40.2 | Features `bundled`, `fts5` |
| `keyring` | 4.1.6 | |
| `specta` / `tauri-specta` | 2.0.0-rc.25 | Release candidate — see 2.5 |
| `insta` | 1.48.0 | |
| `wiremock` | 0.6.5 | |
| `dirs` | 6.0.0 | |
| `@sveltejs/adapter-static` | 3.0.10 | |
| `bits-ui` | 2.18.1 | |
| `lucide-svelte` | 1.0.1 | |

---

## Part 4 — Data model

Tables, with the columns that carry weight. Full DDL lives in
`core/migrations/001-initial.sql`.

- **`source`** — `id`, `kind` (`mbox` | `imap` | `gmail`), `label`,
  `last_sync_at`, `message_count`, plus per-kind config. Feeds S12. `last_sync_at`
  only advances while the app is open (1.2); S12 must not imply otherwise.
- **`message`** — `id`, `source_id`, `message_id` header, `sender_id`,
  `subject`, `date`, `list_unsubscribe` (raw), `list_unsubscribe_post` (bool),
  `locator` (byte offset for mbox, UID for IMAP). **No body is stored.** The
  locator is how evidence links re-read the original from its source.
  **A locator can stop resolving** — an imported mbox is moved or deleted, an
  IMAP message is expunged. That is a designed state in S14, not an error: the
  row keeps its subject, sender and date, and the evidence link reports that
  the original is no longer reachable.
- **`sender`** — `id`, `address`, `display_name`, `domain`, `first_seen`,
  `last_seen`, `message_count`, `classification` (`service` | `newsletter` |
  `other`), `confidence`, `classified_by` (`parser` | `playbook` | `model`).
- **`service`** — `id`, `sender_id`, `name`, `data_key` (the `data/services/`
  entry it matched), `is_critical`, `status` (`active` | `canceling` |
  `canceled`), `cadence`, `monthly_minor_units`, `currency`.
- **`receipt`** / **`charge`** — extracted amounts with `message_id`,
  `amount_minor_units`, `currency`, `charged_at`, `extracted_by`. Price-increase
  flags in S04 and price history in S06 are derived from `charge` ordered by
  date.
- **`aggregate`** — per-service and per-sender monthly rollups of spend and
  volume, rebuilt by a scan. This is what S03 and S06 read. **Rollups are keyed
  by currency.** A mailbox holding USD and EUR receipts produces a row per
  currency, and S03's headline figure names the dominant one with the others
  listed beside it. There is no conversion: a rate needs a network call, and
  the privacy statement enumerates every outbound request we make.
- **`action`** — the activity log (S14): `id`, `kind` (`unsubscribe` |
  `playbook` | `agent` | `bulk`), `target`, `started_at`, `finished_at`,
  `outcome`, `evidence` (a `message` id or a `sessions/<id>.jsonl` path).
- **`setting`** — key/value. Appearance, sweep behaviour, update behaviour,
  data location, active tier.

FTS5 external-content table over `message.subject` and `sender.display_name`,
kept current by triggers.

---

## Part 5 — Milestones

Each milestone ends with something demonstrable and its own acceptance check.
The order is driven by one rule: the mbox path is the zero-auth baseline, so it
proves the product before any credential exists anywhere.

### M0 — Skeleton

Cargo workspace with the four crates; SvelteKit with `adapter-static`; Tauri
v2 with the single-instance plugin; `tokens.css` and `theme.css` extracted with
fonts vendored; `pr.yml` running all six jobs; `CHANGELOG.md` with an
`Unreleased` section; MIT `LICENSE`.

Done when: an empty window opens on all three platforms and every CI job is
green on a pull request.

### M1 — Scan an mbox

mbox reader (including the gzip wrapper and `X-Gmail-Labels`, which
`mail-parser` does not cover), MIME parse, sender aggregation, deterministic
subscription and newsletter detection, receipt extraction for the first ten
vendor formats, the schema and its migration stepper, aggregates, and the scan
progress `Channel`.

Handle `mail-parser`'s two open defects here — issue #156 silent multipart
truncation and #155 panic on a folded `Received` header, both on the scan path
(1.1). Pin a known-good version, carry a patch, and add a fixture for
each so an upgrade cannot regress them silently.

Screens: S01 (mbox path only), S02, plus S16's mbox-parse-failure error state.

Done when: a 1 GB generated mbox scans inside the CI ceiling, and a real
Takeout export (2.11) scans without a panic.

### M2 — See the results

Dashboard, subscriptions list, newsletters list, service detail, command
palette, general settings, report an issue. All read paths over M1's data. This
is where the design system becomes real components.

Screens: S03, S04, S05, S06, S18, S15, plus S16's empty states. **S17 opens
here but does not finish here**: its appearance, data-location and
erase-all-data controls land in M2, sweep behaviour in M3, the browser
integration control in M7, and the update section in M8.

Done when: every one of those screens renders from a scanned mbox and matches
its mockup in light and dark.

### M3 — Act

RFC 8058 one-click unsubscribe and the `List-Unsubscribe` header grammar —
both written by us, since no library in either ecosystem implements them
(1.1). The activity log, the bulk runner with per-item progress over a
`Channel`, and the critical-services confirmation.

Screens: S07, S10, S11, S14, plus S16's critical-service warning and S17's
sweep-behaviour settings — which are this milestone's defaults made editable:
whether critical services are excluded from a bulk run, and the size at which a
bulk run asks for confirmation.

Done when: a bulk unsubscribe over a scanned mbox reports per-item outcomes,
excludes critical services by default, and writes an evidence link for each.

### M4 — Community data

The TOML format, the loader, `et-data validate` in CI, the playbook screen,
and enough seed entries to be useful — the twenty highest-volume SaaS senders
plus the critical list (AWS, registrars, payment processors, domain and DNS
providers).

Screens: S08.

Done when: a contributor can add a service in one file and CI rejects a
malformed one with a file and line.

### M5 — Live mail

IMAP with app passwords through `async-imap`, credential storage, incremental
resync, and the sources screen. Then the bring-your-own Gmail OAuth client
flow with guided setup — the third rung of the ingestion ladder, and the one
`CONTEXT.md` puts in v0.

Screens: S01 (all three paths), S12, plus S16's IMAP auth failure state.

Done when: all four target providers (Gmail, iCloud, Fastmail, Outlook) sync
incrementally, and a disconnect mid-sync resumes without duplicating rows.

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

### M8 — Ship

The release pipeline exactly as Part 8 specifies: the gate, three platform
jobs, the single assemble job writing `latest.json` and `SHA256SUMS`, build
provenance attestation, the `ext-v*` extension workflow with staged rollout,
and the `workflow_dispatch` yank. The landing site, `install.sh`,
`install.ps1`, `install-prerelease.sh`, `install-prerelease.ps1`,
`uninstall.sh`, `uninstall.ps1`. The updater plugin wired to S17's three-way
setting and the "what changed" screen.

Screens: S00, plus S16's update-ready state.

Done when: `0.1.0` installs from the site on all three platforms, and a
`0.1.1` tag reaches an installed copy through the updater without a prompt.

### Sizing

Wall-clock for an AI agent executing the work, not human effort. Ranges, not
commitments — M1 and M7 carry the most unknowns, because one meets real mail
and the other meets a real browser.

| Milestone | Estimate | What drives the spread |
|---|---|---|
| M0 Skeleton | 2–3 h | Three-platform CI going green on the first try, or not |
| M1 Scan an mbox | 1–2 d | Receipt formats and the two `mail-parser` defects |
| M2 See the results | 2–3 d | Eight dense screens against mockups, light and dark |
| M3 Act | 1 d | |
| M4 Community data | 1 d | Twenty seed entries is research, not code |
| M5 Live mail | 2–3 d | Four providers' quirks, plus the OAuth guided setup |
| M6 Intelligence | 1 d | |
| M7 Agentic cancellation | 2–3 d | Extension, host, socket and CDP against a real profile |
| M8 Ship | 1–2 d | Plus the hardware checks in Part 6, which are serial |

Roughly two to three weeks of agent wall-clock. **Three things do not respond
to effort** and should start now if they can: the Takeout export queue (2.11),
Chrome Web Store review for the extension, and the four hardware confirmations
in Part 6.

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

**Must exist before the pipeline can finish**:

5. The minisign key, password-protected, in the `release` Environment, with two
   offline backups. Losing it strands every installed copy permanently.
6. Chrome Web Store client ID, client secret and refresh token. The refresh
   token is revocable and will fail silently one day, so the extension workflow
   fails loudly when the API rejects it.
7. Both extension IDs frozen (M7).
8. A tag protection rule restricting `v*` to maintainers.

**Human smoke test before publishing any draft release**, on each platform:
install from the script, first run reaches S01, import a small mbox, dashboard
renders, one unsubscribe completes and appears in the activity log, settings
open, quit and relaunch retains data.

---

## Part 7 — Explicitly not in v0

Virtual cards, email aliasing, a hosted version, mobile apps, Apple-signed
builds, a Homebrew cask, an `.msi`, npm, a background daemon, nightly builds, a
second release channel, runtime-fetched community data, end-to-end webview
automation, and any telemetry. Each has a trigger recorded in `CONTEXT.md` or
in the Part 1 section that removed it.

---

## Part 8 — Packaging and release reference

The mechanics behind locked decisions 9–18. M8 builds this; the reasoning is in
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
architecture, and the IP that any HTTPS request carries. Nothing we invent is
transmitted, which keeps the no-telemetry line intact.

The cost of that line, stated so nobody is surprised later: adoption signal is
limited to GitHub asset download counts and `latest.json` hits, both aggregate
and collected by GitHub rather than by us. **We will not know crash rates.** An
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
`install-prerelease.ps1`, `uninstall.sh` and `uninstall.ps1`. All are static
files, so the site needs no Function and no server-side logic.

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
