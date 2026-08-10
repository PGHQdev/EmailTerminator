# Browser automation options for cancellation

Type: research
Status: resolved
Blocked by: —
Output: docs/reference/browser-automation-survey.md

## Question

What are the practical options for agent-driven cancellation in a
subscription's own web UI, and what does each cost in install weight and
reliability?

Candidates: Playwright, Puppeteer, CDP directly (chromiumoxide,
rust-headless-chrome), WebDriver BiDi, and driving the user's own signed-in
Chrome through an extension or remote-debugging port.

For each report: whether a browser is downloaded and how large, whether the
user's existing session and cookies can be reused (cancellation almost
always needs a logged-in account), observed bot-detection behaviour on
account and billing pages, license, and bindings available in Rust and in
TypeScript.

Also report how the "pause / take over control" action in screen S09 could
be delivered by each option.

## Answer

Findings: `docs/reference/browser-automation-survey.md`.

- S09 forces a **headed** browser (a headless window cannot be handed to the
  user) against an **already-authenticated** session.
- Chrome 136 removed the cheapest session-reuse path: `--remote-debugging-port`
  no longer attaches to Chrome's default data directory. Playwright separately
  documents that automating the default profile is unsupported. Both
  "reuse the real profile" routes are closed.
- Two honest implementations of "uses your saved session" survive: a Chrome
  extension using `chrome.debugger` inside the real profile, or a dedicated
  automation profile the user signs into once.
- `chrome.debugger` exposes Input, Page and Target, which is sufficient, and
  Chromium shows a permanent "started debugging this browser" infobar — which
  suits our auditability stance rather than fighting it.
- Pause and hand-over are our own command loop. Neither CDP nor the WebDriver
  BiDi draft defines a suspend or hand-to-human command.
- Install weight: Chrome for Testing is 178–192 MB compressed, Playwright's
  own browsers 180–281 MB installed. Driving the user's **installed** Chrome
  makes the download zero.
- Bot detection, from first-party sources: Google refuses sign-in to browsers
  "controlled through software automation"; `navigator.webdriver` is
  spec-mandated; Chromium puts "Headless" in the user agent. Claims about
  specific billing pages are marked as community observation.
- Bindings: TypeScript is first-party across every option. Rust has
  chromiumoxide, headless_chrome, fantoccini and thirtyfour, but
  `playwright-rust` last shipped in 2022 — a Rust core means writing waits,
  selectors and frame logic by hand. **This is direct input to ticket 06.**
- Ranking: (1) dedicated persistent profile on the installed Chrome,
  (2) extension plus a native-messaging host, (3) WebDriver BiDi if
  cross-browser ever matters. Rejected: attaching to the debugging port,
  the Tauri WebView, safaridriver, Selenium classic.
