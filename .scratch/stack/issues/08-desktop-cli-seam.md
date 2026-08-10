# Desktop and CLI seam

Type: grilling
Status: open
Blocked by: 06
ADR: docs/adr/0003-desktop-cli-seam.md

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
