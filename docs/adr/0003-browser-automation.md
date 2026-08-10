# 3. Agentic cancellation runs through a Chrome extension

Date: 2026-08-10

Status: Accepted

## Context

`CONTEXT.md` puts agentic browser cancellation in v0 behind an experimental
flag. Screen S09 requires live progress, a pause, and hand-over of control to
the user. Cancelling a subscription always requires an authenticated session, so
the deciding question is how the automation reaches one.

`docs/reference/browser-automation-survey.md` established the constraints:

- The browser must be headed. A headless window cannot be handed to a user.
- Chrome 136 removed `--remote-debugging-port` against Chrome's default data
  directory, because attackers used it to extract cookies past App-Bound
  Encryption. Playwright separately documents that automating the default
  profile is unsupported. Both routes to the user's existing session through a
  launched browser are closed.
- Google refuses sign-in to browsers "controlled through software automation",
  so a dedicated automation profile cannot use "Sign in with Google", which
  covers a large share of SaaS logins.
- `chrome.debugger` exposes Input, Page and Target, which is sufficient, and
  Chromium shows a permanent "started debugging this browser" infobar.
- Neither CDP nor the WebDriver BiDi draft defines a suspend or hand-to-human
  command.

Shipping practice was checked directly on a development machine. Three
native-messaging hosts were installed:

| Host | Binary | Origins |
|---|---|---|
| `com.anthropic.claude_browser_extension` | `/Applications/Claude.app/Contents/Helpers/chrome-native-host` | 3 extension IDs |
| `com.anthropic.claude_code_browser_extension` | `~/.claude/chrome/chrome-native-host`, a wrapper that execs the versioned binary with `--chrome-native-host` | 1 extension ID |
| `com.apple.passwordmanager` | helper inside a system app bundle | 2 extension IDs |

Claude Code's manifest is written into four browsers' directories: Chrome,
Brave, Edge and Chromium. None of the three binds a network port.

OpenAI's Atlas and Perplexity's Comet take the other architecture entirely and
ship a browser. That costs a Chromium fork and a browser-sized download, which
is not proportionate to an experimental feature in a small app.

## Decision

Agentic cancellation ships in v0, driven through a **Chrome extension using
`chrome.debugger` inside the user's real, already-logged-in profile**.

- **The Rust core drives.** The extension attaches the debugger and relays CDP
  messages. Every step, wait, selector and model call happens in `core`.
- **Transport is native messaging plus a local socket.** Chrome spawns a thin
  native-messaging host, which relays to the running app over a Unix domain
  socket, or a named pipe on Windows. No TCP and no bound port. Chrome's message
  caps are 1 MB host to extension and 64 MiB extension to host, so our commands
  fit the small side and screenshots return on the large side.
- **The host is a small helper binary in the workspace,** not the app binary.
  The manifest points at a stable wrapper path so an update does not break the
  link.
- **Distribution is one Chrome Web Store listing with a sideloaded unpacked
  build as an equally supported path.** `allowed_origins` lists both IDs.
- **The app writes the manifest into every installed Chromium-family browser's
  directory:** Chrome, Brave, Edge, Vivaldi, Chromium. One listing, several
  paths.

## Consequences

- **Playwright is out of this project.** `chrome.debugger` speaks CDP through
  the extension API, so CDP sequences are hand-written in whatever language
  drives them. The dead `playwright-rust` binding stops being an argument
  anywhere, including retroactively in ADR 1.
- **Pause and hand-over are our own command loop,** since no protocol provides
  them.
- **The debugging infobar stays visible.** For a tool selling auditability that
  is a feature, and the UI should explain it rather than work around it.
- **The extension is a second release artifact** with its own version and a
  handshake with the app on connect. Tickets 16 and 21 own building, versioning
  and publishing it.
- Automation opens its own window in the user's profile and never takes over an
  existing tab.
- If the extension is absent, the agentic path is unavailable and S06 offers
  one-click unsubscribe and playbooks. This is a designed state, not an error.
- Review latency or rejection for a `chrome.debugger` plus `nativeMessaging`
  extension degrades the feature. It does not block a release, because the
  sideload path is supported.
- Ticket 12 inherits a concrete answer to "what may a skill touch": the browser,
  through the core's CDP driver, never directly.

### What would reopen this

Chrome restricting `chrome.debugger` for extensions, or a Web Store rejection
that also makes the sideload path untenable.
