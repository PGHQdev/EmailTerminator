# Packaging and release artifacts

Type: grilling
Status: open
Blocked by: 04, 06, 08
ADR: docs/adr/NNNN-packaging.md (number assigned at resolution, in acceptance order)

## Question

What artifacts does a release produce, and how does each reach a user?

`CONTEXT.md` names a shell installer, a PowerShell one-liner, npm, and no
browser download of the `.app`. Unsigned at launch, macOS first.

ADR 0002 removed the CLI from v0, which was npm's natural artifact, and
`docs/reference/tauri-distribution-precedent.md` found no project shipping a
Tauri desktop bundle on npm. Whether npm remains a channel at all is now part of
this ticket.

Axes:

- One binary containing app and CLI, or separate artifacts.
- The npm package shape for the chosen core language (04 reports the
  precedent).
- Where the installer script is hosted and how it is versioned.
- What the unsigned first-run experience actually looks like on macOS, and
  what the app tells the user about it.
- Update mechanism, or none, and what "none" implies.
- The Chrome extension is a second release artifact (ADR 0003), with its own
  version, its own store listing, and a sideloadable build. How it is versioned
  against the app, and what the app installs — the native-messaging manifest and
  a stable wrapper path that survives updates — belongs here.
