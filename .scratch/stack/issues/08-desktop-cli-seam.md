# Desktop and CLI seam

Type: grilling
Status: resolved
Blocked by: 06
ADR: docs/adr/0002-no-seam-desktop-only.md

## Question

How does one core serve both the Tauri window and the headless CLI or
localhost mode?

Options: Tauri IPC commands with the CLI linking the core as a library; a
localhost HTTP server that both the webview and the CLI call; or a library
core with two thin front ends and no protocol between them.

Axes:

- `CONTEXT.md` requires the CLI and localhost mode to come from the same
  core. Which option makes divergence impossible rather than merely
  discouraged?
- A localhost port on a privacy-first app is an attack surface. What binds,
  and what authenticates?
- Long-running work — a scan of a large mbox, a bulk unsubscribe run — must
  stream progress to S02 and S11. Which option streams naturally?
- The SvelteKit UI must run in the webview and, in localhost mode, in a
  normal browser. Does the seam choice force two builds?

## Answer

**There is no seam. v0 is the desktop app alone** — no CLI, no localhost web UI,
no daemon, no bound port.

ADR: `docs/adr/0002-no-seam-desktop-only.md`.

- Cargo workspace of two crates: `core` (the domain, no Tauri dependency,
  testable headless) and `app` (the Tauri binary, a thin command layer).
- The UI calls `core` through Tauri commands. Scan and bulk progress stream over
  a Tauri `Channel`; the event system is JSON-only and documented as unsuitable
  for throughput.
- One instance, enforced by the single-instance plugin. SQLite has one writer,
  so no advisory-lock protocol is needed.

Decided in this order:

1. **Localhost web UI is out of v0.** It duplicates an existing UI and adds a
   bound port and a second audited path into destructive actions, on a tool
   whose pitch is that mail never leaves the machine.
2. **No daemon.** Costed honestly: modest code (a LaunchAgent plist, a systemd
   user unit, a Task Scheduler entry, lifecycle handling), and four sharp
   behavioural edges — TCC prompts a headless process cannot present, users
   disabling it in macOS Login Items, updater version skew, and a mandatory
   uninstaller a `curl | sh` install does not have. Its only v0 benefit is IMAP
   IDLE while the app is closed, which no designed screen consumes.
3. **CLI dropped from v0.** This contradicts `CONTEXT.md` and removes npm's
   natural artifact; both recorded as consequences.
4. **`core` stays a separate crate.** Costs one directory, keeps Tauri types out
   of mail parsing, and makes a later CLI or daemon an added binary rather than
   an extraction.

Facts that drove it: Tauri requires `adapter-static` and no server frontend; its
own `tauri-plugin-localhost` docs warn of "considerable security risks"; a Tauri
binary links `libwebkit2gtk` on Linux, so one argv-dispatched binary cannot
serve a headless machine.
