# Test stack and corpus

Type: grilling
Status: open
Blocked by: 05, 06
ADR: docs/adr/0010-test-stack.md

## Question

What tests each layer, and what mail do those tests run against?

Axes:

- Test runner for the core, for the SvelteKit UI, and for the two together.
- Corpus approach from 05: a checked-in public archive, a synthetic
  generator, recorded fixtures, or a mix.
- How parser regressions are caught — the parser will keep improving after
  release, so a growing fixture set is likely.
- How IMAP and provider endpoints are faked, since tests may not reach a
  real mailbox.
- What CI runs on every pull request versus on a release only.
