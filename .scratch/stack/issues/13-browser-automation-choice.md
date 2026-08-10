# Browser automation choice

Type: grilling
Status: resolved
Blocked by: —
ADR: docs/adr/NNNN-browser-automation.md (number assigned at resolution, in acceptance order)

## Question

Which browser automation stack drives agentic cancellation, given it ships
behind an experimental flag?

Rewired 2026-08-10: the 12 edge was backwards. The decisive factors here —
session reuse, a headed browser, hand-over, install weight — are not harness
questions, and research 03 armed all of them. Ticket 12 now waits on this one.

Axes:

- Install weight. A bundled browser download for an experimental feature is
  hard to justify. Is it fetched on first use instead?
- Cancellation needs a logged-in session. Does the choice reuse the user's
  own browser profile, or does the user log in inside our browser?
- S09 requires live progress, pause, and hand-over to the user. Which
  option supports hand-over at all?
- What the failure path is when a site blocks automation, and how that
  becomes the "needs you" result.
- Whether the same stack is reused later for virtual-card swaps
  (`CONTEXT.md` v0.1) without being designed for it now.

## Answer

**Agentic cancellation ships in v0, driven through a Chrome extension using
`chrome.debugger` inside the user's real, already-logged-in profile.**

ADR: `docs/adr/0003-browser-automation.md`.

- The Rust core drives; the extension attaches the debugger and relays CDP.
- Chrome spawns a thin native-messaging host, which relays to the running app
  over a Unix domain socket, or a named pipe on Windows. No TCP, no bound port.
  The app stays the single SQLite writer.
- One Chrome Web Store listing, with a sideloaded unpacked build as an equally
  supported path. `allowed_origins` names both IDs.
- The app writes the native-messaging manifest into every installed
  Chromium-family browser's directory. One listing, several paths.

Decided in this order:

1. **Agentic stays in v0.** The launch metric is an HN front page or a Balaji
   acknowledgment, and an agent cancelling a real subscription is the demo that
   earns either.
2. **Extension over a dedicated automation profile.** Google refuses sign-in to
   browsers controlled through software automation, so a dedicated profile
   breaks "Sign in with Google" across a large share of SaaS. The extension runs
   where the user is already authenticated.
3. **Logic in the core, not the extension.** The extension updates through Web
   Store review measured in days; the app self-updates via minisign. Playbooks
   are repo data interpreted by the core, and Tier 2 keys live in the OS
   keychain the extension cannot reach.
4. **Native messaging, not a localhost WebSocket.** Keeps ADR 0002's no-port
   commitment. Message caps are 1 MB host to extension and 64 MiB back, so the
   limit falls away from screenshots.
5. **Store plus sideload.** Review latency or rejection for a `chrome.debugger`
   plus `nativeMessaging` extension degrades the feature instead of blocking a
   release.

Verified against shipping practice on a development machine: the Claude desktop
app, Claude Code and Apple's password manager all use native messaging with a
thin host binary and multiple IDs in `allowed_origins`, and Claude Code writes
its manifest into Chrome, Brave, Edge and Chromium. None binds a port. The
manifest points at a stable wrapper path so updates do not break the link, which
we copy.

The other architecture — OpenAI's Atlas and Perplexity's Comet ship their own
browser — costs a Chromium fork and a browser-sized download, which is not
proportionate to an experimental feature.

**Playwright is out of this project.** CDP through the extension API means
hand-written sequences whatever the language, so the dead `playwright-rust`
binding stops being an argument anywhere.
