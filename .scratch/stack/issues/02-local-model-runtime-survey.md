# Tier 1 local model runtimes

Type: research
Status: resolved
Blocked by: —
Output: docs/reference/local-model-runtime-survey.md

## Question

What are the options for running a Gemma / Qwen / Ministral class model on
the user's machine from a Tauri desktop app, and what does each cost us?

Candidates to cover: an external Ollama dependency, embedded llama.cpp as a
sidecar binary, mistral.rs or candle in-process, node-llama-cpp, and any
Apple-native path (MLX, Foundation Models).

For each report: added bundle size, macOS Metal and Linux CPU support,
whether the user must install anything separately, how model download and
storage is handled, license, and how it is invoked from both a Tauri app
and a headless CLI.

Constraint from `CONTEXT.md`: Tier 1 is optional. Nothing here may become a
hard install requirement for Tier 0.

## Answer

Findings: `docs/reference/local-model-runtime-survey.md`.

- The cheapest Tier 1 is to **detect an OpenAI-compatible endpoint the user
  already runs** — Ollama on `127.0.0.1:11434/v1`, `llama serve`, LM Studio.
  Zero bundle bytes, and it reuses the same client as Tier 2. This collapses
  part of ticket 11 before it starts.
- Fallback is a llama.cpp sidecar fetched on enable: 23.0 MB on macOS arm64,
  ~20.0 MB on Linux x64 (release b10333), MIT, Metal shaders embedded, and
  `-hf` handles the model download into `LLAMA_CACHE`.
- mistral.rs does the same job at 141.6 MB on macOS, and its crates.io release
  trails its GitHub release. MLX is Apple-silicon-only and depends on Python
  plus transformers. candle is a framework, not a runtime.
- node-llama-cpp only makes sense if the core is already Bun: the Tauri webview
  cannot load an N-API addon, so it needs a Bun sidecar (23.6 MB) on top of the
  native packages (12.7 MB macOS, 28.0 MB Linux).
- Apple Foundation Models ships for free and is reachable from Rust through
  Apple's own C ABI, but its 4,096-token context fits short classification
  only, not receipt extraction.
- Ollama's own install is heavy (180 MB dmg, 1.42 GB Linux tarball) and it now
  ships cloud models. If we support it, set `OLLAMA_NO_CLOUD=1`.
- **Weights matter more than runtimes.** Every `google/gemma-3-*` repo is
  gated, so unattended download fails; Gemma 4 is ungated under Apache-2.0.
  Ministral 8B Instruct 2410 is research-only under MRL-0.1 and **cannot
  ship**; Ministral 3 (Apache-2.0) replaces it. Default recommendation is the
  Qwen 3.5 ladder (533 MB / 1.28 GB / 2.74 GB).
- Unverified and marked as such in the file: in-process link size for
  `llama-cpp-2` and candle, the mistral.rs cache path, Foundation Models
  behaviour in an unsigned app, and WebGPU support in Tauri's webviews.

Conflict to carry: `CONTEXT.md` names "Gemma / Qwen / Ministral class" as the
Tier 1 examples. Two of those three names are wrong as written. Ticket 11
settles the default model and the wording is corrected with it.
