# Tier 1 runtime and Tier 2 client

Type: grilling
Status: open
Blocked by: 02, 06
ADR: docs/adr/NNNN-intelligence-runtime.md (number assigned at resolution, in acceptance order)

## Question

What runs a local model (Tier 1), and what talks to a user's own cloud
endpoint (Tier 2)?

S13 promises: pick and download a local model, enter an endpoint and key
for OpenAI-compatible, Anthropic-compatible, OpenRouter, DeepSeek or a
custom provider, and test the connection.

Axes:

- Bundled runtime versus an external dependency the user installs. The
  second is smaller but breaks the promise of a working download.
- One client abstraction across all Tier 2 providers, or per-provider
  handling. Which library, if any.
- What "test connection" verifies.
- How a tier failure degrades. Tier 0 must keep working when a model is
  absent, unreachable or out of credit.
- Whether Tier 1 and Tier 2 share one interface inside the core.
