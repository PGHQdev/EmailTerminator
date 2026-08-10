# Core language: Rust core or TypeScript/Bun core

Type: grilling
Status: open
Blocked by: 01
ADR: docs/adr/0001-core-language.md

## Question

Where does the logic live — a Rust core with Tauri as a thin window, or a
TypeScript/Bun core with Tauri's Rust side reduced to a shell?

This is the root of the map. Storage, credentials, the desktop/CLI seam,
the model runtimes, the agent harness, packaging, CI and the test stack all
inherit from it.

Axes to settle:

- Which ecosystem actually supplies the mail libraries (answered by 01).
- The headless CLI and localhost mode ship from the same core
  (`CONTEXT.md`). Which language makes that one binary, and which makes it
  two artifacts?
- npm distribution is a stated v0 channel. A Node/Bun core publishes
  natively; a Rust core needs a binary wrapper.
- Scan performance over a multi-gigabyte Takeout mbox.
- Contributor reach — the project's value is community recipes and
  playbooks, so who can send a pull request matters.
- A split core (Rust for ingestion and parsing, TypeScript for the agent
  layer) is a live third option. Judge it against the cost of two
  toolchains.

The answer names one core language, states what the other side is allowed
to contain, and gives the reason a contributor could not have guessed.
