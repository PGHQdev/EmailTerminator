# Agent-skill harness shape

Type: grilling
Status: open
Blocked by: —
ADR: docs/adr/NNNN-agent-skill-harness.md (number assigned at resolution, in acceptance order)

## Question

`CONTEXT.md` calls the product a "specialized, local-first agent harness"
with "domain-specific agent skills". What is a skill, concretely, in code?

Rewired 2026-08-10: no longer waits on 11. Ticket 02 established that Tier 1 and
Tier 2 both reduce to one OpenAI-compatible client, so this ticket is specified
against "a chat client exists" rather than against a chosen runtime. It now waits
on 13 instead, because what a skill is allowed to do includes browser access.

ADR 0003 settled one axis in advance: a skill reaches the browser only through
the core's CDP driver, never directly, and the browser is the user's own Chrome
via a `chrome.debugger` extension.

Axes:

- Is a skill a data file the core interprets, a plugin the core loads, or a
  prompt bundle handed to a model? The community must send skills as pull
  requests, so the format decides who can contribute.
- The stated order is deterministic parsers first, skills second, models
  third. What mechanism enforces that order at runtime?
- Reuse or build: existing agent frameworks in the chosen language versus a
  purpose-built loop. Name why the existing option does or does not fit.
- What a skill is allowed to do — network, filesystem, browser — and what
  enforces the limit.
- How a skill run produces the evidence link that S14 requires.
