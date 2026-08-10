# 2. No seam: v0 is the desktop app alone

Date: 2026-08-10

Status: Accepted

Supersedes part of the v0 form described in `CONTEXT.md` (see Consequences).

## Context

ADR 1 put the domain logic in Rust. This decision settles how the UI reaches it,
and whether anything else reaches it at all.

`CONTEXT.md` described the v0 form as "Tauri desktop app with an embedded web UI,
plus a headless CLI / localhost mode from the same core". Those are three
delivery surfaces, and each one that exists has to be built, tested, secured and
supported.

Facts established while deciding:

- Tauri requires `@sveltejs/adapter-static` and does not support server-based
  frontends, so the UI is one static build regardless of transport
  ([Tauri SvelteKit guide](https://v2.tauri.app/start/frontend/sveltekit/)).
- Tauri's own documentation for `tauri-plugin-localhost` warns that it "brings
  considerable security risks and you should only use it if you know what you
  are doing" ([plugin docs](https://v2.tauri.app/plugin/localhost/)).
- Tauri's event system carries JSON strings and is documented as "not designed
  for low latency or high throughput situations"; channels are the mechanism
  for streaming ([calling the frontend from Rust](https://v2.tauri.app/develop/calling-frontend/)).
- A Tauri binary links the platform webview, `libwebkit2gtk` on Linux, so a
  single argv-dispatched binary cannot serve a headless machine.

A background daemon was considered and costed. The code is modest: a LaunchAgent
plist, a systemd user unit, a Task Scheduler entry, and lifecycle handling. The
real costs are behavioural: a background process hitting macOS TCC prompts for
Desktop, Documents and Downloads with no window to present them from; users
switching it off in System Settings → Login Items & Extensions, where macOS
lists background items; version skew when the Tauri updater replaces the bundle
under a running daemon; and a mandatory uninstaller, which a `curl | sh` install
does not have today. Against that, its only v0 benefit is IMAP IDLE while the
app is closed, and no designed screen consumes it.

## Decision

**v0 ships the desktop app and nothing else.** No CLI, no localhost web UI, no
daemon, no bound port.

The domain logic is a library the app links in-process. There is no protocol
between them.

Layout is a Cargo workspace of two crates:

- `core` — the domain. No Tauri dependency. Testable without a webview.
- `app` — the Tauri binary. A thin command layer over `core`.

The UI calls `core` through Tauri commands. Scan and bulk-action progress stream
over a Tauri `Channel`, not the event system.

Exactly one instance runs, enforced by the single-instance plugin: a second
launch focuses the existing window. SQLite therefore has one writer, and no
advisory-lock protocol is needed.

## Consequences

- **`CONTEXT.md` is now wrong about the v0 form.** "Plus a headless CLI /
  localhost mode from the same core" does not describe v0. The strategy document
  needs correcting.
- **The npm distribution channel loses its artifact.** `CONTEXT.md` lists npm as
  a v0 channel, and npm's natural artifact was the CLI.
  `docs/reference/tauri-distribution-precedent.md` found no project shipping a
  Tauri desktop bundle on npm. Ticket 16 has to rethink or drop that channel.
- **Ticket 20 is constrained.** With Tauri IPC as the seam, `tauri-specta` is a
  live candidate for getting Rust types into the UI, and a generated OpenAPI
  client is not.
- **S12's "last sync" only advances while the app is open.** Nothing syncs when
  it is closed, and the Sources screen should not imply otherwise.
- **Adding a CLI or a daemon later is an added binary that links `core`.** That
  is the entire reason `core` is a separate crate rather than code inside
  `src-tauri`.
- **Single instance is load-bearing.** If two windows or two data profiles are
  ever wanted at once, the lock protocol this decision avoids has to be designed
  then.
- One behaviour is left to the build: whether closing the window quits the app.
  macOS keeps it alive by default, which would let a scan continue.

### What would reopen this

A user need for headless operation on a server, or a feature that consumes
background sync — price-increase alerts or notifications are the likely first
ones.
