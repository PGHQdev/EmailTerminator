# Shipping a Tauri binary via npm and curl | sh

Type: research
Status: resolved
Blocked by: —
Output: docs/reference/tauri-distribution-precedent.md

## Question

How do comparable projects distribute a Tauri desktop app through a shell
installer and an npm package, without a signed and notarized macOS build?

Cover:

- Real projects that publish a Tauri or other native binary on npm, and the
  packaging pattern they use (optionalDependencies per platform, postinstall
  download, or a single fat package).
- What macOS Gatekeeper does to an unsigned `.app` installed by
  `curl | sh` versus downloaded in a browser, and whether the quarantine
  claim in `CONTEXT.md` holds.
- Update mechanisms available to an unsigned build, including the Tauri
  updater's signing requirements.
- What a Windows PowerShell one-liner install looks like, and SmartScreen
  behaviour for an unsigned executable.

## Answer

Findings: `docs/reference/tauri-distribution-precedent.md`.

**The quarantine-free claim in `CONTEXT.md` holds.** `com.apple.quarantine` is
set by the downloading application through Launch Services, not by the kernel
or the filesystem. Browsers opt in; `curl`, `npm` / `bun` install, and the
Tauri updater's in-process `tar` plus `rename` do not. No quarantine means no
Gatekeeper first-launch assessment and no dialog.

Two holes the claim does not cover:

- The GitHub Releases page is a browser download. `CONTEXT.md` names the
  GitHub repository as the canonical address, so the canonical address hands
  users the quarantined path. Ticket 16 settles what the Releases page offers.
- Homebrew Cask quarantines deliberately, with no opt-out since brew 6.0.14
  (July 2026).

When a bundle is quarantined, an unsigned build now costs the user System
Settings → Privacy & Security → Open Anyway plus a password. Sequoia removed
the Control-click shortcut.

Other findings:

- Tauri's updater signing is **minisign** — free, and unrelated to Apple. An
  Apple-unsigned build can self-update in full. `pubkey` is non-optional.
- Install to `~/Applications`, or the updater falls back to an AppleScript
  admin-password prompt.
- Windows SmartScreen keys off Mark-of-the-Web. Neither PowerShell's web
  cmdlets nor `curl.exe` writes it, and no major PowerShell installer calls
  `Unblock-File`. Microsoft has retracted the "EV certificate bypasses
  SmartScreen" claim that Tauri's own docs still repeat.
- npm precedent is unanimous: a thin wrapper package plus `optionalDependencies`
  pinned by `os` and `cpu` — `@tauri-apps/cli`, biome, turbo, rollup, esbuild,
  deno, bun. No known project ships a Tauri **desktop bundle** on npm.
- Unverified and flagged in the file: macOS App Management behaviour on
  unsigned self-update, current SmartScreen dialog text, and whether `.app`
  symlinks survive an npm tarball round trip.
