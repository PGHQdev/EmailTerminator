# Browser automation for agentic cancellation — research summary (August 2026)

Compiled from primary sources only: vendor documentation, project repositories,
package registries, the CDP and WebDriver specifications, and Chromium source.
Informs screen S09 (agentic cancellation, experimental).

Package and size figures were read from the npm registry, crates.io, and
Google's Chrome for Testing storage bucket on 10 August 2026.

## What S09 demands

The mockup fixes three requirements. The agent "used your saved session".
The user can "take over control", which "opens the live browser window exactly
where the agent is". Every step is logged.

Two consequences follow. The browser must be **headed**, because a headless
window cannot be handed over. The session must be **authenticated before the
run starts**, because a cancellation page is behind a login wall.

## Decisive axis 1 — can the user's existing session be reused?

| Mechanism | Reuses the user's live Chrome profile? | Blocking constraint |
|---|---|---|
| Extension using `chrome.debugger` | Yes, the real profile and its cookies | Extension must be installed; a permanent infobar appears |
| `--remote-debugging-port` on the user's Chrome | No, since Chrome 136 | Chrome refuses the switch with the default user data directory ([Chrome 136 change](https://developer.chrome.com/blog/remote-debugging-port)) |
| Playwright/Puppeteer `userDataDir` pointed at Chrome's real profile | Not supported | "automating the default Chrome user profile is not supported … may result in pages not loading or the browser exiting" ([Playwright](https://playwright.dev/docs/api/class-browsertype)) |
| Dedicated persistent profile, user signs in once inside it | Yes, after a one-time login | The one-time login is visible product friction |
| Exported `storageState` (cookies, localStorage, IndexedDB) | Only what was captured | Playwright cannot persist sessionStorage; state expires ([auth docs](https://playwright.dev/docs/auth)) |
| WebDriver classic / BiDi drivers | No by default | geckodriver "will create a new profile" in the temp folder ([moz:firefoxOptions](https://developer.mozilla.org/en-US/docs/Web/WebDriver/Reference/Capabilities/firefoxOptions)); ChromeDriver exposes `user-data-dir` and `debuggerAddress` ([capabilities](https://developer.chrome.com/docs/chromedriver/capabilities)) |
| safaridriver | No | "Automation window always start from a clean slate and cannot access Safari's normal browsing history, AutoFill data, or other sensitive information" ([WebKit](https://webkit.org/blog/6900/webdriver-support-in-safari-10/)) |

Two facts dominate this axis.

Chrome 136 removed the cheapest path. Google states that
`--remote-debugging-port` and `--remote-debugging-pipe` no longer work against
the default data directory, because "Since App-Bound Encryption was enabled
we've seen an increase in attackers using Chrome Remote Debugging to extract
cookies". Google's recommendation for automation is Chrome for Testing
([source](https://developer.chrome.com/blog/remote-debugging-port)).

Google's own MCP server repeats the security warning for the port we would have
to open: "Any application on your machine can connect to this port and control
the browser. Make sure that you are not browsing any sensitive websites while
the debugging port is open"
([chrome-devtools-mcp](https://github.com/ChromeDevTools/chrome-devtools-mcp)).

So "used your saved session" has two honest implementations: a browser
extension inside the real profile, or a dedicated automation profile the user
signs into once.

## Decisive axis 2 — handing control back mid-run

| Option | Handover mechanism | Quality |
|---|---|---|
| Extension + `chrome.debugger` | `chrome.debugger.detach`; the tab is already the user's own window | Best. The user is already looking at their own browser |
| Playwright / Puppeteer, headed, persistent profile | Stop issuing commands, leave the window open and focused | Good. The window is a normal Chrome window |
| chromiumoxide / headless_chrome, headed | Same: stop sending CDP commands | Good, but the app must build the "focus this window" step itself |
| WebDriver classic / BiDi | Stop sending commands, do not call `session.delete` | Weak. Drivers own the session lifetime and may reap it |
| safaridriver | A "glass pane" blocks user interaction during a session; the search field turns orange ([WebKit](https://webkit.org/blog/6900/webdriver-support-in-safari-10/)) | Rejected. The design blocks handover by intent |
| Tauri's own WebView | Native window, trivial handover | Cookies are the app's, not the user's; see below |

Pause is easier than handover. Every option above is a command loop the app
owns, so "pause" is a flag in our code, not a protocol feature. Neither the
[CDP](https://chromedevtools.github.io/devtools-protocol/) nor the
[WebDriver BiDi editor's draft](https://w3c.github.io/webdriver-bidi/) defines a
suspend or hand-over-to-human command.

## Option comparison

| Option | Browser download | Library weight | License | Rust binding | TypeScript binding |
|---|---|---|---|---|---|
| Playwright | Bundled Chromium, or `channel: 'chrome'` for the installed browser | `playwright` 4.84 MB + `playwright-core` 12.82 MB unpacked | Apache-2.0 | None official; `playwright-rust` 0.0.20, last published 2022-08-20 | First-party |
| Puppeteer | Chrome for Testing, or `puppeteer-core` + `executablePath` for none | `puppeteer-core` 5.49 MB + `chromium-bidi` 9.06 MB + `devtools-protocol` 3.55 MB + `@puppeteer/browsers` 0.42 MB | Apache-2.0 | None | First-party |
| CDP direct — chromiumoxide | Only with the `fetcher` feature | Crate only | MIT OR Apache-2.0 | First-party (0.9.1, 2026-02-25) | n/a |
| CDP direct — headless_chrome | Only with the `fetch` feature | Crate only | MIT | First-party (1.0.22, 2026-06-11) | n/a |
| CDP direct — TypeScript | None | `chrome-remote-interface` 1.93 MB (MIT); types `devtools-protocol` 3.55 MB (BSD-3-Clause) | MIT | n/a | Community |
| WebDriver BiDi | ChromeDriver 8.5 MB (mac-arm64) / 10.6 MB (linux64), plus a browser | `webdriverio` 1.47 MB, or `selenium-webdriver` 16.99 MB | Apache-2.0 (ChromeDriver, Selenium); MIT (WebdriverIO) | `thirtyfour` 0.37.4, `bidi` feature flag | WebdriverIO, Selenium, Puppeteer `protocol: 'webDriverBiDi'` |
| WebDriver classic | Same driver binaries | as above | as above | `fantoccini` 0.22.1, `thirtyfour` 0.37.4 (both MIT OR Apache-2.0) | Selenium, WebdriverIO |
| User's Chrome via remote-debugging port | None | Any CDP client | n/a | chromiumoxide `Browser::connect` | Puppeteer `browserURL`, Playwright `connectOverCDP` |
| Extension + `chrome.debugger` | None | Extension source only | our own | Native messaging host in Rust | Extension is TS |

Browser payloads, measured as `Content-Length` on Google's own bucket for
Chrome for Testing 151.0.7922.77:

| Artifact | mac-arm64 | linux64 | win64 |
|---|---|---|---|
| `chrome` | 178.4 MB | 184.3 MB | 191.8 MB |
| `chrome-headless-shell` | 94.4 MB | 114.6 MB | — |
| `chromedriver` | 8.5 MB | 10.6 MB | — |

Those are compressed archives. Playwright's documentation gives installed sizes
in its own example listing: "281M chromium-XXXXXX / 187M firefox-XXXX / 180M
webkit-XXXX", and states browsers "will take a few hundred megabytes of disk
space when installed" ([browsers](https://playwright.dev/docs/browsers)).
`playwright install --only-shell` avoids the full Chromium download, and
`channel: 'chrome'` avoids any download by using the installed browser.

## The extension route in detail

`chrome.debugger` is the only supported way for an extension to speak CDP inside
the user's own profile. Chrome documents the `"debugger"` permission and access
to a fixed subset of CDP domains: Accessibility, Audits, CacheStorage, Console,
CSS, Database, Debugger, DOM, DOMDebugger, DOMSnapshot, Emulation, Fetch, IO,
**Input**, Inspector, Log, Network, Overlay, **Page**, Performance, Profiler,
Runtime, Storage, **Target**, Tracing, WebAudio, WebAuthn
([API reference](https://developer.chrome.com/docs/extensions/reference/api/debugger)).
Input, Page, and Target are the three we need, so the subset is sufficient.

Attaching is visible and permanent. Chromium's resource file defines the
infobar text as `"<CLIENT_NAME>" started debugging this browser`, with the
comment that the label "does not disappear until the user dismisses it, even if
the debugger is detached"
([generated_resources.grd](https://chromium.googlesource.com/chromium/src/+/refs/heads/main/chrome/app/generated_resources.grd)).
For a privacy-first product this is an asset, not a defect: the user always sees
when we are driving.

The desktop app talks to the extension through native messaging: a JSON host
manifest with `allowed_origins` pinned to our extension ID, `type: "stdio"`,
1 MB limit on messages from the host and 64 MiB toward it, and the
`"nativeMessaging"` permission
([native messaging](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging)).
A Rust host binary fits this transport with no extra dependency.

A cheaper extension variant exists: a content script with host permissions, no
`debugger` permission, no infobar. It is weaker for a reason the DOM Standard
states outright — events dispatched from page script have `isTrusted` set to
false ([DOM Standard](https://dom.spec.whatwg.org/)). Retention flows that
check `isTrusted` would reject those clicks. `chrome.debugger` Input events do
not go through `dispatchEvent`, but see "Not verified" below.

## Bot detection

Documented, first-party statements only:

- **Google blocks automated sign-in.** Google lists browsers that "Are being
  controlled through software automation rather than a human" and browsers that
  "Are embedded in a different application" among those it will not sign in
  ([support article](https://support.google.com/accounts/answer/7675428)).
  This matters directly: many SaaS cancellation flows sit behind "Sign in with
  Google".
- **Cloudflare targets headless browsers by name of category.** Its JavaScript
  Detections engine exists to catch "headless browsers (browsers controlled by
  software, with no visible window or human operator) and other automation
  tools" ([bot score](https://developers.cloudflare.com/bots/concepts/bot-score/)).
  Cloudflare names no automation library in that page.
- **The automation flag is standardised.** The WebDriver specification defines
  the webdriver-active flag, "set to true when the user agent is under remote
  control", exposed as `navigator.webdriver`, so that "alternate code paths can
  be triggered during automation" ([W3C WebDriver](https://w3c.github.io/webdriver/)).
  CDP can override it: `Emulation.setAutomationOverride` "Allows overriding the
  automation flag"
  ([CDP Emulation](https://chromedevtools.github.io/devtools-protocol/tot/Emulation/)).
- **Headless changes the user agent.** Chromium's user agent code inserts the
  string "Headless" into the product token when the headless switch is present
  ([user_agent_utils.cc](https://chromium.googlesource.com/chromium/src/+/refs/heads/main/components/embedder_support/user_agent_utils.cc)).
  Running headed removes this signal.
- **Chrome's own automation switch is an indicator by design.** Chromium
  defines `--enable-automation` with the comment "Enable indication that browser
  is controlled by automation"
  ([content_switches.cc](https://chromium.googlesource.com/chromium/src/+/refs/heads/main/content/public/common/content_switches.cc)).

Marked as **community observation, not documented behaviour**: every claim that
a specific billing or account page fingerprints Playwright, Puppeteer, or CDP;
every ranking of which stealth patch defeats which vendor; every claim about
detection rates. Puppeteer's official FAQ says nothing about bot detection,
blocking, or stealth. Playwright's documentation likewise carries no
detection guidance. Stealth forks exist —
`puppeteer-extra-plugin-stealth` (MIT, last published 2023-04-11) and
`patchright` (Apache-2.0, 2026-06-23) — and neither publishes a first-party
efficacy claim we can cite.

The practical reading: headed Chrome, a real profile, and human-paced input
remove the documented signals. Nothing beyond that is verifiable from primary
sources.

## Options the ticket did not name

- **Chrome DevTools MCP** (Google, Apache-2.0, `chrome-devtools-mcp` 11.92 MB
  unpacked). Built on Puppeteer, offers `--browserUrl`, `--isolated`,
  `--channel`, `--executablePath`, and connects to a running Chrome to
  "maintain the same application state when alternating between manual site
  testing and agent-driven testing"
  ([repo](https://github.com/ChromeDevTools/chrome-devtools-mcp)). It is a
  ready-made tool surface for a BYOK model, and it inherits the Chrome 136
  port restriction and the port warning quoted above.
- **Stagehand** (`@browserbasehq/stagehand`, MIT, 9.66 MB unpacked). An
  LLM-driven layer over Playwright. It pulls `openai` and `@google/genai` as
  dependencies, which conflicts with our provider-agnostic BYOK design.
- **chromium-bidi** (Google, Apache-2.0, 9.06 MB unpacked). "an implementation
  of the WebDriver BiDi protocol … implemented as a JavaScript layer translating
  between BiDi and CDP, running inside a Chrome tab"
  ([repo](https://github.com/GoogleChromeLabs/chromium-bidi)). Already a
  Puppeteer dependency. It shows that BiDi on Chrome is CDP underneath.
- **Selenium** (`selenium-webdriver`, Apache-2.0, 16.99 MB unpacked). The
  heaviest JavaScript client, with no advantage over WebdriverIO for us.
- **Tauri's own WebView.** Zero extra install weight, and handover is native.
  Tauri's WebDriver support is the blocker: driving `tauri-driver` directly,
  "only Windows and Linux are supported on desktop, as macOS has no WKWebView
  driver tool available"
  ([Tauri](https://v2.tauri.app/develop/tests/webdriver/)). We ship macOS first.
  The WebView also has its own cookie jar, so it reuses no existing session.
- **Firefox.** geckodriver accepts `args: ["-profile", "/path"]` to use an
  existing profile and creates a temporary one otherwise
  ([moz:firefoxOptions](https://developer.mozilla.org/en-US/docs/Web/WebDriver/Reference/Capabilities/firefoxOptions)).
  Viable as a second engine, not as the first.

## Protocol stability

CDP publishes no compatibility promise for the version everyone uses. The
official site describes tip-of-tree as: "It changes frequently and can break at
any time … There is no backwards compatibility support guaranteed." Stable 1.3
is "tagged at Chrome 64" and is a subset
([CDP](https://chromedevtools.github.io/devtools-protocol/)).

WebDriver BiDi is the standards-track answer. ChromeDriver "implements the W3C
WebDriver and WebDriver BiDi standards"
([ChromeDriver](https://developer.chrome.com/docs/chromedriver)), and the spec
is still an Editor's Draft, dated 5 August 2026
([W3C](https://w3c.github.io/webdriver-bidi/)). It buys cross-browser stability
and costs a driver binary plus a fresh-profile default.

## Not verified

- Whether CDP `Input.dispatchMouseEvent` produces events with `isTrusted` true.
  No primary source states this. Do not assume it.
- Whether the Chrome Web Store applies extra review to the `debugger`
  permission. The registration page confirms a "one-time registration fee" but
  does not state the amount.
- The licence terms of Chrome for Testing binaries. They are Google Chrome
  builds, not Chromium builds; we did not read the applicable terms.
- Chromium's single-process-per-user-data-dir lock. The documentation file we
  expected in Chromium's `docs/` tree does not exist at that path. Playwright's
  warning about the default profile is the only first-party statement we have.
- Bot-detection behaviour on any named subscription vendor's account or billing
  page. No primary source publishes this. Measure it per playbook instead.
- Real disk weight of a Rust CDP client's compiled binary. Neither crate
  publishes a figure.
- ChromeDriver's `webSocketUrl` capability is not listed on the ChromeDriver
  capabilities page, although BiDi support is claimed on the landing page.

## Decision consequence (v0, experimental flag)

The constraint is our own: the default install must not carry a browser, and
the agentic path is opt-in behind a flag. Every candidate below adds zero bytes
to the default install; the ranking is about what happens after the user turns
the flag on.

1. **Dedicated persistent Chrome profile, driven headed, using the installed
   Chrome.** Playwright `channel: 'chrome'` + `launchPersistentContext`, or
   Puppeteer `puppeteer-core` + `executablePath` + `userDataDir`, or
   chromiumoxide if the core is Rust. Zero browser download. The user signs in
   once inside that profile, which is exactly S09's "used your saved session".
   Pause is our flag; handover is "stop driving, focus the window". Falls back
   to a lazy Chrome for Testing download (178–192 MB) only when no Chrome is
   installed.
2. **Chrome extension with `chrome.debugger` plus a Rust native-messaging
   host.** The only option that reuses the real profile with no second login,
   and the best handover, because the window is already the user's. The
   permanent "started debugging this browser" infobar matches our auditability
   stance. The cost is distribution: an extension to publish and a second
   install step. Build this second, once the first path proves the agent logic.
3. **WebDriver BiDi through ChromeDriver.** Take this when cross-browser
   support or protocol stability starts to matter more than handover fidelity.
   Adds an 8.5–10.6 MB driver and a version-matching burden.
4. **Attaching to the user's running Chrome over `--remote-debugging-port`.**
   Rejected for v0. Chrome 136 blocks it against the default profile, and
   Google's own documentation calls the open port a hazard.
5. **Tauri WebView, safaridriver, Selenium classic.** Rejected: no macOS
   driver, deliberate session isolation, and no advantage respectively.

Language note: TypeScript has first-party bindings for every option. Rust has
first-party CDP clients (chromiumoxide, headless_chrome) and WebDriver clients
(fantoccini, thirtyfour) but no maintained Playwright or Puppeteer binding;
`playwright-rust` last shipped in August 2022. Choosing Rust for the core means
choosing CDP or WebDriver directly, and writing the waiting, selector, and frame
logic that Playwright would otherwise supply.

## Sources

- Playwright: [browsers](https://playwright.dev/docs/browsers),
  [BrowserType](https://playwright.dev/docs/api/class-browsertype),
  [authentication](https://playwright.dev/docs/auth)
- Puppeteer: [configuration](https://pptr.dev/guides/configuration),
  [ConnectOptions](https://pptr.dev/api/puppeteer.connectoptions),
  [FAQ](https://github.com/puppeteer/puppeteer/blob/main/docs/faq.md)
- Chrome: [remote debugging change in Chrome 136](https://developer.chrome.com/blog/remote-debugging-port),
  [chrome.debugger](https://developer.chrome.com/docs/extensions/reference/api/debugger),
  [native messaging](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging),
  [ChromeDriver](https://developer.chrome.com/docs/chromedriver),
  [ChromeDriver capabilities](https://developer.chrome.com/docs/chromedriver/capabilities),
  [headless mode](https://developer.chrome.com/docs/chromium/headless),
  [Web Store registration](https://developer.chrome.com/docs/webstore/register)
- Chrome for Testing: [last-known-good-versions-with-downloads.json](https://googlechromelabs.github.io/chrome-for-testing/)
- Chromium source: [content_switches.cc](https://chromium.googlesource.com/chromium/src/+/refs/heads/main/content/public/common/content_switches.cc),
  [user_agent_utils.cc](https://chromium.googlesource.com/chromium/src/+/refs/heads/main/components/embedder_support/user_agent_utils.cc),
  [generated_resources.grd](https://chromium.googlesource.com/chromium/src/+/refs/heads/main/chrome/app/generated_resources.grd)
- Specifications: [Chrome DevTools Protocol](https://chromedevtools.github.io/devtools-protocol/),
  [CDP Emulation domain](https://chromedevtools.github.io/devtools-protocol/tot/Emulation/),
  [W3C WebDriver](https://w3c.github.io/webdriver/),
  [W3C WebDriver BiDi](https://w3c.github.io/webdriver-bidi/),
  [DOM Standard](https://dom.spec.whatwg.org/)
- Repositories: [chromiumoxide](https://github.com/mattsse/chromiumoxide),
  [rust-headless-chrome](https://github.com/rust-headless-chrome/rust-headless-chrome),
  [chromium-bidi](https://github.com/GoogleChromeLabs/chromium-bidi),
  [chrome-devtools-mcp](https://github.com/ChromeDevTools/chrome-devtools-mcp)
- Registries: [crates.io API](https://crates.io/api/v1/crates/chromiumoxide),
  [npm registry](https://registry.npmjs.org/playwright)
- Other vendors: [Cloudflare bot score](https://developers.cloudflare.com/bots/concepts/bot-score/),
  [Google sign-in security](https://support.google.com/accounts/answer/7675428),
  [WebKit WebDriver](https://webkit.org/blog/6900/webdriver-support-in-safari-10/),
  [Mozilla moz:firefoxOptions](https://developer.mozilla.org/en-US/docs/Web/WebDriver/Reference/Capabilities/firefoxOptions),
  [Tauri WebDriver](https://v2.tauri.app/develop/tests/webdriver/)
