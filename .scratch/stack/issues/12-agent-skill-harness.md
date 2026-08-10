# Agent-skill harness shape

Type: grilling
Status: open
Blocked by: 06, 11
ADR: docs/adr/0007-agent-skill-harness.md

## Question

`CONTEXT.md` calls the product a "specialized, local-first agent harness"
with "domain-specific agent skills". What is a skill, concretely, in code?

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
