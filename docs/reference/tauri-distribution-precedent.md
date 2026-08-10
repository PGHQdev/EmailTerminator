# Tauri distribution without an Apple signature — research summary (August 2026)

Primary sources only: Apple platform and developer documentation, Tauri
documentation and plugin source, npm/pnpm/Bun documentation, Microsoft Learn,
and the published package metadata or install scripts of named projects.
Informs the v0 distribution decision in `CONTEXT.md`.

## macOS: what sets `com.apple.quarantine`

Quarantine is not a filesystem property and not applied by the kernel. The
**downloading application** applies it, by choice, through Launch Services.

- An app quarantines a file by writing `kLSItemQuarantineProperties` with
  `LSSetItemAttribute`, or by declaring `LSFileQuarantineEnabled` in its
  `Info.plist`, after which "all files *created* by the application process
  will be automatically quarantined". Source:
  [Launch Services Release Notes](https://developer.apple.com/library/archive/releasenotes/Carbon/RN-LaunchServices/index.html),
  [`LSFileQuarantineEnabled`](https://developer.apple.com/documentation/bundleresources/information-property-list/lsfilequarantineenabled).
- Gatekeeper's approval prompt is scoped to that state: "Gatekeeper also
  requests user approval before opening **downloaded** software for the first
  time." Source:
  [Gatekeeper and runtime protection in macOS](https://support.apple.com/guide/security/gatekeeper-and-runtime-protection-sec5599b66df/web).
- Quarantine also propagates: "Gatekeeper also tracks the provenance of files
  written by downloaded software" (same page). A quarantined process taints
  what it writes. A shell run from Terminal is not quarantined.
- Malware scanning is independent of quarantine: "all software in macOS is
  checked for known malicious content the first time it's opened, regardless
  of how it arrived on the Mac" (same page). XProtect runs either way.
- On Apple silicon a binary must carry at least an ad-hoc signature to execute
  at all: "A Mac with Apple silicon doesn't permit native arm64 code to execute
  unless a valid signature is attached." Source:
  [Rosetta 2 on a Mac with Apple silicon](https://support.apple.com/guide/security/rosetta-2-on-a-mac-with-apple-silicon-secebb113be1/web).
  Rust on `aarch64-apple-darwin` ad-hoc signs at link time, so "unsigned" here
  means "no Developer ID", not "no signature".

### Install path versus outcome

| Install path | Quarantine applied | Gatekeeper first-launch prompt | What the user does |
|---|---|---|---|
| Browser download of `.dmg` / `.zip` | Yes — Safari/Chrome/Firefox all opt in via Launch Services | Yes | System Settings → Privacy & Security → Open Anyway → password |
| `curl \| sh` from Terminal (curl + `tar`/`unzip`/`ditto`) | No — curl sets no quarantine property | No | Nothing; app opens |
| `npm install` / `bun install` (Node/Bun HTTP client) | No | No | Nothing |
| Tauri updater replacing the bundle in place | No — plain `tar` extract plus `rename` in-process | No | Nothing |
| `brew install --cask` | **Yes, deliberately** | Yes | Same as browser download |
| AirDrop, Messages, mail attachment | Yes | Yes | Same as browser download |

Homebrew Cask is the trap. `Library/Homebrew/extend/os/mac/cask/quarantine.rb`
builds a Launch Services dictionary with
`quarantine_agent_name = "Homebrew Cask"` and
`quarantine_type = quarantine_type_web_download`, then sets it on the download.
Source: [quarantine.rb](https://github.com/Homebrew/brew/blob/master/Library/Homebrew/extend/os/mac/cask/quarantine.rb),
[cask/quarantine.rb](https://github.com/Homebrew/brew/blob/master/Library/Homebrew/cask/quarantine.rb).
The `--no-quarantine` escape hatch was deprecated in Homebrew 4.6.19
([PR #20929](https://github.com/Homebrew/brew/pull/20929), Oct 2025) and its
leftover code removed in 6.0.14
([PR #23363](https://github.com/Homebrew/brew/pull/23363), Jul 2026).
Verified locally against Homebrew 6.0.15: `brew install --help` no longer
lists a quarantine flag.

### What the user sees for a quarantined unsigned `.app`

Apple lists the alerts on
[Safely open apps on your Mac](https://support.apple.com/en-us/102445):

- Developer ID but not notarized → "Apple cannot check the app for malicious
  software".
- No Developer ID → "the app developer cannot be verified".
- Ad-hoc signature that Gatekeeper cannot evaluate → "If macOS detects that
  software has been modified or damaged, your Mac notifies you that the app
  can't be opened." This is the "is damaged and can't be opened" message that
  Tauri users report; the recorded fix is `xattr -cr /Applications/App.app`.
  Source: [tauri-apps/tauri#5778](https://github.com/tauri-apps/tauri/issues/5778).

The bypass got harder. From macOS Sequoia: "users will no longer be able to
Control-click to override Gatekeeper when opening software that isn't signed
correctly or notarized. They'll need to visit System Settings > Privacy &
Security". Source:
[Updates to runtime protection in macOS Sequoia](https://developer.apple.com/news/?id=saqachfa).
The full override is now: System Settings → Privacy & Security → Security →
Open Anyway → admin password, and the button "is available for about an hour
after you try to open the app". Source:
[Open an app by overriding security settings](https://support.apple.com/guide/mac-help/open-an-app-by-overriding-security-settings-mh40617/mac).
Apple frames it as dangerous: "Overriding security settings to open an app is
the most common way that a Mac gets infected with malware."

### Verdict on the `CONTEXT.md` claim

**The claim holds, with two holes that must be closed by process, not by code.**

Correct: a `curl | sh` installer, an `npm install`, and the Tauri updater all
write the bundle with tools that never call Launch Services, so no
`com.apple.quarantine` is set, so Gatekeeper never runs its first-launch
assessment and the user sees nothing. Tauri's own documentation scopes the
failure to the browser path: signing prevents "a warning that your application
is broken and can not be started, **when downloaded from the browser**".
Source: [Tauri — macOS code signing](https://v2.tauri.app/distribute/sign/macos/).

Hole 1 — GitHub Releases. An open-source project has a Releases page. Any user
who downloads the `.dmg` there in a browser gets the full quarantine
experience. "Never a browser download" is a claim about our installer, not
about our repository.

Hole 2 — Homebrew. A community cask, or our own tap, re-introduces quarantine
with no opt-out as of Homebrew 6.0.14. Do not ship a cask while unsigned.

## npm precedent for shipping native binaries

Three patterns exist. All package metadata below was read from the live
registry on 2026-08-10 with `npm view` and `npm pack`.

| Project | Pattern | Platform packages | Install script |
|---|---|---|---|
| [`@tauri-apps/cli`](https://www.npmjs.com/package/@tauri-apps/cli) 2.11.4 | optionalDependencies | 11, e.g. `@tauri-apps/cli-darwin-arm64` (`os:["darwin"]`, `cpu:["arm64"]`) | none |
| [`@biomejs/biome`](https://www.npmjs.com/package/@biomejs/biome) 2.5.7 | optionalDependencies | 8, `@biomejs/cli-*`; runtime `require.resolve` + `spawnSync` in `bin/biome` | none |
| [`turbo`](https://www.npmjs.com/package/turbo) 2.10.9 | optionalDependencies | 6, `@turbo/*` (renamed from unscoped `turbo-*`, frozen at 2.8.17) | none |
| [`rollup`](https://www.npmjs.com/package/rollup) 4.62.4 | optionalDependencies | 27, `@rollup/rollup-*` | none |
| [`esbuild`](https://www.npmjs.com/package/esbuild) 0.28.2 | optionalDependencies + fallback | 26, `@esbuild/*` | `postinstall` retries via `npm install`, then a direct registry tarball GET |
| [`@swc/core`](https://www.npmjs.com/package/@swc/core) 1.15.47 | optionalDependencies + fallback | 12 | `postinstall` verifies the `.node` loads, else installs `@swc/wasm` |
| [`deno`](https://www.npmjs.com/package/deno) 2.9.5 | optionalDependencies | 6, `@deno/*` | `postinstall` only hardlinks the resolved binary; no network |
| [`bun`](https://www.npmjs.com/package/bun) 1.3.14 | optionalDependencies + fallback | 16, `@oven/bun-*` | `postinstall` verifies with `--version`, else `npm install`, else registry tarball |
| [`electron`](https://www.npmjs.com/package/electron) 43.3.0 | download at first use | none | no `scripts`; `index.js` lazily runs `install.js` → `@electron/get` → unzip `Electron.app` into `dist/`. `postinstall` was dropped in 42.0.0 |
| [`cypress`](https://www.npmjs.com/package/cypress) 15.20.0 | postinstall download | none | `postinstall: node dist/index.js --exec install` into a user cache dir |

npm's own semantics, from
[package.json docs](https://docs.npmjs.com/cli/v10/configuring-npm/package-json#optionaldependencies):
"build failures do not cause installation to fail", `os` is matched against
`process.platform`, `cpu` against `process.arch`. The docs do **not** state the
mismatch consequence; the code does. A non-optional package with a mismatched
`os`/`cpu` throws `EBADPLATFORM`; an optional one is marked inert and skipped.
Sources:
[npm-install-checks](https://github.com/npm/npm-install-checks/blob/main/lib/index.js),
[build-ideal-tree.js](https://github.com/npm/cli/blob/latest/workspaces/arborist/lib/arborist/build-ideal-tree.js).

Two traps in the optionalDependencies pattern:

- **Lockfiles.** [npm/cli#4828](https://github.com/npm/cli/issues/4828): an
  arm64 Mac can write a `package-lock.json` that omits the other platforms, and
  CI then installs nothing. Rollup ships the workaround as a runtime error
  string: "npm has a bug related to optional dependencies … Please try `npm i`
  again after removing both package-lock.json and node_modules directory"
  ([native.js](https://unpkg.com/rollup@4.62.4/dist/native.js)).
- **Install scripts are no longer reliable.** Bun "does not execute arbitrary
  lifecycle scripts by default" and runs them only for an allow list or
  `trustedDependencies`
  ([Bun docs](https://bun.com/docs/install/lifecycle)). pnpm 10 requires
  explicit approval through `allowBuilds` / `onlyBuiltDependencies`, with
  `dangerouslyAllowAllBuilds` as the opt-out
  ([pnpm docs](https://pnpm.io/settings/build)). A postinstall-download design
  is broken by default for two of the three major clients.

### Does anyone ship a Tauri desktop app on npm?

Effectively no. A sweep of all 584 npm packages keyworded `tauri`, plus the
named apps (Spacedrive, Yaak, Hoppscotch Desktop, Clash Verge Rev, Pot,
GitButler, Cap, Jan, Wealthfolio, Screenpipe), found every one shipping through
GitHub Releases, its own site, a brew cask, winget, Snap or the AUR. Screenpipe
puts only its CLI on npm. **No package in the sweep ships a `.app`, `.dmg`,
`.msi`, `.AppImage` or `.deb` as bytes inside an npm tarball.**

A hobby-scale tail does exist, and it uses exactly the design we are
considering. [`francois`](https://www.npmjs.com/package/francois) 0.18.5 (a
Tauri app, 38 KB npm tarball, no binary inside) has a `postinstall` that reads
a baked `manifest.json`, downloads `francois-darwin-universal.tar.gz` from its
GitHub release, verifies a sha256 digest, unpacks with system `tar`, and moves
`Francois.app` into `~/Applications`. Its header comment states the rationale
in full:

> Windows SmartScreen and macOS Gatekeeper key off the Mark-of-the-Web /
> com.apple.quarantine attribute that a *browser* attaches at download time. A
> binary fetched by npm never carries one, so the same unsigned build that gets
> blocked when downloaded as a .dmg or .exe launches clean from here — no
> code-signing certificate involved.

It still runs `xattr -dr com.apple.quarantine` on the vendor directory, marked
"belt and braces". [`@seg4lt/flowstate`](https://www.npmjs.com/package/@seg4lt/flowstate)
does the same with `hdiutil attach` + `cp -R` + `xattr -cr`. These are
low-download packages; treat them as corroboration of the reasoning, not as
proof the pattern scales.

### Packaging a Rust binary versus a Node/Bun program

| | Rust core (Tauri) | Node/Bun core |
|---|---|---|
| Build artifact | `cargo`/`tauri build` per target triple | [`bun build --compile --target=…`](https://bun.com/docs/bundler/executables) or [Node SEA](https://nodejs.org/api/single-executable-applications.html) |
| Cross-compile | Per-target CI runners; NSIS cross-builds, WiX and `.app` do not | Bun cross-compiles from any host to 12 targets; Node SEA cross-compiles only with `useCodeCache` and `useSnapshot` off |
| npm shape | Thin wrapper + `optionalDependencies`, one package per triple, `os`/`cpu` pinned — the `@tauri-apps/cli` shape | Identical; the executable is just a different producer |
| Stability | Stable | Node SEA is stability **1.1, active development**; macOS CI covers arm64 only, x64 "is not currently supported" |
| macOS signing | Rust ad-hoc signs arm64 at link time | Node SEA documents `codesign --remove-signature` then `codesign --sign -`; Bun requires `com.apple.security.cs.allow-jit` in entitlements or its JIT will not run |
| Deprecated option | — | [`pkg`](https://github.com/vercel/pkg): "deprecated with `5.8.1` as the last release" |

The npm packaging shape is the same either way, so the language choice does not
constrain distribution. The signing detail differs: a Bun-compiled binary needs
a JIT entitlement the day we do sign, and Node SEA is not yet stable enough to
be the only path to a shipped binary.

## Update mechanisms for an unsigned build

The Tauri updater's signature requirement is **Tauri's own**, unrelated to
Apple. It uses minisign; the key is generated by
`tauri signer generate -- -w ~/.tauri/myapp.key`, the private half is passed at
build time as `TAURI_SIGNING_PRIVATE_KEY`, and "Tauri's updater needs a
signature to verify that the update is from a trusted source. This cannot be
disabled." Source: [Tauri updater plugin](https://v2.tauri.app/plugin/updater/).
`pubkey` is a non-optional `String` in the plugin config, so a build without
one fails to deserialize. Source:
[config.rs](https://github.com/tauri-apps/plugins-workspace/blob/v2/plugins/updater/src/config.rs).

Cost: zero. An unsigned-by-Apple build can use the Tauri updater in full.

How the macOS update lands, read from
[updater.rs](https://github.com/tauri-apps/plugins-workspace/blob/v2/plugins/updater/src/updater.rs):

1. Download the `[AppName]_[version]_[arch].app.tar.gz` produced by
   `tauri-bundler` (`bundle.createUpdaterArtifacts`).
2. `verify_signature` with `minisign_verify` against the configured `pubkey`.
3. Extract with the `tar` crate into a temp dir, `rename` the current bundle to
   a backup, `rename` the new bundle into place.
4. If the `rename` fails with `PermissionDenied`, fall back to an AppleScript
   `do shell script "rm -rf ... && mv -f ..." with administrator privileges`.

Consequence: install to a user-writable location so step 4 never fires. An
installer that places the bundle in `~/Applications`, or in `/Applications`
where the user owns the directory, updates silently.

| Update option | Works unsigned | Cost | Note |
|---|---|---|---|
| Tauri updater plugin | Yes | Free | minisign key only; static JSON endpoint on emailterminator.com or a GitHub release asset |
| Re-run the shell installer | Yes | Free | Idempotent; the fallback when the updater is unavailable |
| `npm update` / `bun update` | Yes | Free | Only for the npm-installed copy |
| Sparkle | Yes | Free | EdDSA appcast, its own signing scheme; more surface than the Tauri updater buys |
| Homebrew cask upgrade | Yes | Free | Re-quarantines — rejected while unsigned |
| Mac App Store | No | $99/yr + review | Out of scope |

## Windows: PowerShell one-liner and SmartScreen

SmartScreen's app-reputation check in the shell is gated on Mark of the Web.
Microsoft: "When a file is downloaded and tagged with a Mark-of-the-Web (MotW),
SmartScreen in Windows Shell scans it when opened." Source:
[SmartScreen deprecation notice](https://support.microsoft.com/en-us/topic/smartscreen-deprecation-in-internet-explorer-and-ie-mode-in-windows-11-7ac03f87-84e8-414e-a3c5-991b02ebc081).
Consistently, the feature "doesn't protect against malicious files on internal
locations or network shares". Source:
[Microsoft Defender SmartScreen](https://learn.microsoft.com/en-us/windows/security/operating-system-security/virus-and-threat-protection/microsoft-defender-smartscreen/).

MOTW is the `Zone.Identifier` alternate data stream, written by the
downloading application — browsers and mail clients, via
[`IAttachmentExecute`](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-iattachmentexecute).
`Unblock-File` "removes the Zone.Identifier alternate data stream, which has a
value of 3 to indicate that it was downloaded from the internet". Source:
[Unblock-File](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.utility/unblock-file).

Neither `Invoke-WebRequest`/`Invoke-RestMethod` nor bundled `curl.exe` writes
it. Evidence is source-level, not prose: `Zone.Identifier` appears nowhere in
PowerShell's web-cmdlet implementation, and curl declined to add it —
"It's not set by curl, and I'd expect it to break use-cases if set."
Source: [curl#22345](https://github.com/curl/curl/issues/22345).
`Expand-Archive` and `tar.exe` likewise do not propagate MOTW to extracted
members, unlike Explorer's own zip extraction.

Real installers, all read directly, none of which calls `Unblock-File` because
none of them ever creates MOTW:

| Installer | Fetch | Extract | Target |
|---|---|---|---|
| [Bun](https://bun.sh/install.ps1) | `curl.exe`, falls back to `Invoke-RestMethod -OutFile` | `Expand-Archive` | `~\.bun\bin` |
| [Scoop](https://raw.githubusercontent.com/scoopinstaller/install/master/install.ps1) | `WebClient.DownloadFile` | `Expand-Archive` | `~\scoop` |
| [Chocolatey](https://community.chocolatey.org/install.ps1) | `WebClient.DownloadFile` | `Expand-Archive` | `%PROGRAMDATA%\chocolatey` |
| [Deno](https://deno.land/install.ps1) | `curl.exe -Lo` | `tar.exe xf` | `~\.deno\bin` |
| rustup | browser download of `rustup-init.exe` from [win.rustup.rs](https://win.rustup.rs) | — | — |

rustup is the counter-example that proves the rule: its Windows story is a
browser `.exe` download, which is exactly the path that gets MOTW and the
SmartScreen prompt.

For a Tauri app the shape is: fetch the NSIS `-setup.exe` with `curl.exe` or
`Invoke-WebRequest`, then run it with `/S`. Tauri's NSIS installer defaults to
a per-user install requiring no administrator privileges, and the updater's own
`nsis_args` confirm `/S` (silent) and `/P` (passive) are supported. Source:
[Tauri — Windows installer](https://v2.tauri.app/distribute/windows-installer/),
[config.rs](https://github.com/tauri-apps/plugins-workspace/blob/v2/plugins/updater/src/config.rs).

### Signing economics on Windows

Tauri's page states an EV certificate "will receive an immediate reputation
with Microsoft SmartScreen and won't show any warnings". Source:
[Tauri — Windows code signing](https://v2.tauri.app/distribute/sign/windows/).
**Microsoft contradicts this.** Its current developer page says: "EV
certificates no longer bypass SmartScreen. Years ago, signing files with an
Extended Validation (EV) code signing certificate would result in positive
SmartScreen reputation by default, but this behavior no longer exists …
Paying a premium for EV solely to avoid SmartScreen warnings is no longer
justified." Source:
[SmartScreen and app reputation](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation).
Tauri is repeating Microsoft's retracted
[2012 position](https://learn.microsoft.com/en-us/archive/blogs/ie/microsoft-smartscreen-extended-validation-ev-code-signing-certificates).

The same Microsoft page gives the unsigned outcome verbatim: "Warning —
'Windows protected your PC'; User must choose 'Run anyway' before the app can
run." A self-signed certificate has "same behavior as no signature". OV and EV
both still show "app flagged as unrecognized until reputation accumulates",
with the publisher name displayed and reputation attached to the certificate
across releases; unsigned files "must build reputation anew with every update".
Microsoft's recommended cheap path is
[Azure Trusted / Artifact Signing](https://learn.microsoft.com/en-us/azure/trusted-signing/)
at roughly $10/month with no hardware token.

## Could not verify

- The literal Windows 11 SmartScreen dialog body text and button labels on a
  current Microsoft page. The "More info" → "Run anyway" two-step is confirmed
  only by a [2013 archived MSDN post](https://learn.microsoft.com/en-us/archive/blogs/vsnetsetup/windows-smartscreen-prevented-an-unrecognized-app-from-running-running-this-app-might-put-your-pc-at-risk);
  the title string and the "Run anyway" action are confirmed by the 2026 page.
- Any Microsoft prose statement that PowerShell or `curl.exe` do not set MOTW.
  Established from source trees and the curl maintainers, not from Microsoft.
- Whether SmartScreen app reputation has any non-MOTW trigger on default
  consumer Windows other than
  [Smart App Control](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation),
  which Microsoft says "signature checks apply to all executable files, not
  just those downloaded from the Internet" and which is off by default on
  upgraded machines.
- Whether macOS App Management (TCC) prompts when an unsigned Tauri app
  replaces its own bundle in `/Applications` during a self-update. Apple has no
  page stating the rule for self-replacement. Homebrew carries an
  `app_management_permissions_granted?` check for the third-party case, which
  suggests the protection is real but does not settle the self-update case.
  Test on hardware before relying on `/Applications`.
- Whether an unmodified `.app` bundle survives an npm tarball round trip.
  `npm publish` silently drops symbolic links
  ([npm/cli#6746](https://github.com/npm/cli/issues/6746), still open), and
  macOS bundles may contain them. Ship a `.app.tar.gz` inside the package and
  expand it, rather than the bundle tree itself.

## Decision consequence (v0)

The `CONTEXT.md` distribution line stands. Ship unsigned, through two channels
that never set quarantine or Mark-of-the-Web:

1. **Shell installer** from emailterminator.com. macOS: `curl` the
   `.app.tar.gz`, `tar -xz`, move the bundle to `~/Applications`, symlink the
   CLI into `~/.local/bin`. Windows: `curl.exe`/`Invoke-WebRequest` the NSIS
   `-setup.exe`, run it with `/S`; Tauri's per-user default needs no
   administrator rights. Both follow the Bun and Deno shape, and neither needs
   `Unblock-File` or `xattr` because neither ever creates the attribute.
2. **npm package** — a thin wrapper whose `postinstall` downloads the platform
   archive from the GitHub release, verifies a baked sha256, unpacks with
   system `tar`, and places the bundle. Ship a `.app.tar.gz` inside the
   release, never the bundle tree in the npm tarball, because `npm publish`
   drops symlinks. Because Bun and pnpm 10 gate install scripts, the wrapper
   must also work when its `postinstall` never runs: download lazily on first
   `bin` invocation, the way `electron` 42+ does.

Consequences that follow, and must be honoured:

- **No Homebrew cask while unsigned.** Homebrew quarantines every cask and, as
  of 6.0.14, has no opt-out. A cask would hand every brew user the "damaged"
  dialog.
- **The GitHub Releases page is a browser download.** It will produce the
  Gatekeeper and SmartScreen warnings. Put the `xattr -dr com.apple.quarantine`
  and "More info → Run anyway" instructions in the release notes and the README,
  and point the front page at the installer instead.
- **Install to a user-writable location.** `~/Applications` on macOS, per-user
  NSIS on Windows. This keeps the Tauri updater on its silent `rename` path and
  off its AppleScript admin-password fallback.
- **Adopt the Tauri updater now.** Its signing requirement is a free minisign
  key, not an Apple certificate. Generate the key before the first release and
  back it up; losing it strands every installed copy.
- **Windows signing is worth ~$120/yr, macOS ~$99/yr, and neither is urgent.**
  Microsoft has retracted the "EV bypasses SmartScreen" claim that Tauri's docs
  still repeat, so the cheap Azure Artifact Signing certificate is the only
  sensible Windows purchase, and it buys reputation accrual rather than an
  immediate clean launch. Both stay in Deferred until sponsorship money exists.
