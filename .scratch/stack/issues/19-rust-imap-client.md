# Rust IMAP client crate

Type: grilling
Status: open
Blocked by: —
ADR: docs/adr/NNNN-imap-client.md

## Question

Which crate speaks IMAP?

Ticket 06 verified that the Rust ecosystem covers the features v0 needs, and
deliberately left the crate unchosen.

Candidates, with the facts gathered on 2026-08-10:

- `async-imap` 0.11.3 — session-level API, IDLE, `BODY.PEEK`, typed Gmail
  accessors, eight releases since Sept 2024, 578k downloads per 90 days,
  maintained by the Delta Chat group. Parses through `imap-proto`.
- `imap-codec` / `imap-types` 1.0.0 — a codec rather than a client. Reached 1.0
  on 2026-07-19. Pairs with `imap-next` 0.3.4 for connection handling.
  Correctness-first, and more code for us to write.
- `imap` (`jonhoo/rust-imap`) 2.4.1 stable, 3.0.0 in alpha since 2022, with a
  public "looking for maintainers" notice. Still 421k downloads per 90 days.

Axes:

- Sync or async, and whether that forces an async runtime into the core.
- What we must write ourselves in each case: reconnect, backoff, resync after
  disconnect, per-provider quirks.
- Gmail, iCloud, Fastmail and Outlook behaviour, since v0 targets app passwords
  across all four.
- CONDSTORE and QRESYNC for incremental resync: needed for v0, or later?
- What happens if the crate stalls. `imap-codec` at 1.0 is the fallback with the
  most work attached.
