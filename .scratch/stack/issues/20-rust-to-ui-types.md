# Rust domain types in the SvelteKit UI

Type: grilling
Status: open
Blocked by: —
ADR: docs/adr/NNNN-type-sharing.md

## Question

How does a `Subscription` defined in Rust reach the 18 screens that render it?

Ticket 06 chose Rust for the core and left this open, because the answer depends
on the shape of the seam that ticket 08 decides.

ADR 0002 settled the seam as Tauri IPC in-process, which rules out a generated
OpenAPI client and makes `tauri-specta` a live candidate.

Candidates:

- `ts-rs` — derive macro emits `.ts` type files at test time.
- `specta` plus `tauri-specta` — emits types and typed Tauri command bindings.
- An OpenAPI or JSON Schema document generated from the Rust types, with a
  TypeScript client generated from it. Fits a localhost HTTP seam.
- Hand-written TypeScript types, checked by tests.

Axes:

- Which candidates survive ticket 08's seam choice. `tauri-specta` assumes Tauri
  IPC; a generated OpenAPI client assumes HTTP.
- Whether generation runs in CI and fails the build on drift, or is committed
  and reviewed.
- How the CLI's output types stay consistent with the UI's.
- Enum and date representation across the boundary, since spend history and
  billing cadence cross it constantly.
