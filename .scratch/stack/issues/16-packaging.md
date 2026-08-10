# Packaging and release artifacts

Type: grilling
Status: open
Blocked by: 04, 06, 08
ADR: docs/adr/0011-packaging.md

## Question

What artifacts does a release produce, and how does each reach a user?

`CONTEXT.md` names a shell installer, a PowerShell one-liner, npm, and no
browser download of the `.app`. Unsigned at launch, macOS first.

Axes:

- One binary containing app and CLI, or separate artifacts.
- The npm package shape for the chosen core language (04 reports the
  precedent).
- Where the installer script is hosted and how it is versioned.
- What the unsigned first-run experience actually looks like on macOS, and
  what the app tells the user about it.
- Update mechanism, or none, and what "none" implies.
