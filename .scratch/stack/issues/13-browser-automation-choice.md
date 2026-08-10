# Browser automation choice

Type: grilling
Status: open
Blocked by: 03, 06, 12
ADR: docs/adr/0008-browser-automation.md

## Question

Which browser automation stack drives agentic cancellation, given it ships
behind an experimental flag?

Axes:

- Install weight. A bundled browser download for an experimental feature is
  hard to justify. Is it fetched on first use instead?
- Cancellation needs a logged-in session. Does the choice reuse the user's
  own browser profile, or does the user log in inside our browser?
- S09 requires live progress, pause, and hand-over to the user. Which
  option supports hand-over at all?
- What the failure path is when a site blocks automation, and how that
  becomes the "needs you" result.
- Whether the same stack is reused later for virtual-card swaps
  (`CONTEXT.md` v0.1) without being designed for it now.
