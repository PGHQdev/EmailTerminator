# Local model runtimes for Tier 1 — research summary (August 2026)

Compiled from primary sources: project repositories, official documentation,
registry APIs, and release assets. Informs the optional Tier 1 (local model)
decision.

Constraint from `CONTEXT.md`: Tier 1 is optional. Nothing below may become a
hard install requirement for Tier 0.

Sizes marked **measured** were obtained by downloading the named official
release asset and reading file sizes. All other sizes come from the cited page
or registry field.

## Four shapes of the answer

| Shape | What we ship | What the user installs |
|---|---|---|
| Delegate | An HTTP client, ~0 bytes | A whole runtime (Ollama, LM Studio, llama.cpp) |
| Sidecar | An inference binary in the app bundle | Nothing |
| In-process | An inference engine linked into our core binary | Nothing |
| OS-native | Nothing | Nothing (the OS ships the model) |

Tier 3 (BYOK cloud) already forces us to write an OpenAI-compatible client.
Every delegate and sidecar option below reuses that one client. That is the
cheapest seam in the whole design.

## Comparison

| Option | Added to our bundle | macOS Metal | Linux CPU | Separate user install | Model download and storage | License |
|---|---|---|---|---|---|---|
| [Ollama](https://github.com/ollama/ollama) | 0 | Yes | Yes | Yes — 180 MB `.dmg`, 1.42 GB Linux tarball | `ollama pull`, `~/.ollama/models` | MIT |
| [llama.cpp sidecar](https://github.com/ggml-org/llama.cpp) | 23.0 MB macOS arm64, 20.0 MB Linux x64 (measured, minimal set) | Yes | Yes | No | `-hf user/repo:quant` → `LLAMA_CACHE` | MIT |
| [llama-cpp-2](https://crates.io/crates/llama-cpp-2) in-process | Same engine, statically linked; not separately measured | Yes (auto on macOS aarch64) | Yes | No | We implement it | MIT OR Apache-2.0 |
| [mistral.rs](https://github.com/EricLBuehler/mistral.rs) | 141.6 MB macOS arm64, 99.2 MB Linux x64 (measured, single binary) | Yes | Yes | No | Auto-pull from Hugging Face | MIT |
| [candle](https://github.com/huggingface/candle) in-process | Not measured | Yes (`metal` feature) | Yes | No | [`hf-hub`](https://crates.io/crates/hf-hub) crate | MIT OR Apache-2.0 |
| [node-llama-cpp](https://node-llama-cpp.withcat.ai/) | 12.7 MB macOS + 28.0 MB Linux native, plus 37.5 MB JS package, plus a Node/Bun runtime | Yes | Yes | No | `pull` CLI / `resolveModelFile` | MIT |
| [MLX](https://github.com/ml-explore/mlx) | 40.8–56.5 MB `mlx-metal` wheel, plus a Python runtime | Apple silicon only | CPU wheel exists (10.4 MB) | Python and its stack | `~/.cache/huggingface/hub` | MIT |
| [Foundation Models](https://developer.apple.com/documentation/foundationmodels) | ~0 (C shim or the `fm` CLI) | n/a — OS-managed | No | No | OS-managed | Apple acceptable-use requirements |
| [LM Studio](https://lmstudio.ai/) | 0 | Yes | Yes | Yes | `lms get` | Proprietary EULA |
| [llamafile](https://github.com/Mozilla-Ocho/llamafile) | 42.3 MB thin, 350.8 MB full | Yes | Yes | No | User supplies the GGUF | Apache-2.0 (+ MIT for llama.cpp changes) |

## How a sidecar reaches the app

Tauri v2 bundles external binaries through `bundle.externalBin` in
`tauri.conf.json`. Each file carries a target-triple suffix, for example
`my-sidecar-aarch64-apple-darwin`. Execution needs an explicit capability:
`shell:allow-execute` with `"sidecar": true`. Rust calls
`app.shell().sidecar("my-sidecar")`; JavaScript calls
`Command.sidecar('binaries/my-sidecar')`.
Source: [Tauri sidecar guide](https://v2.tauri.app/develop/sidecar/).

The same binary is also the headless CLI path. A CLI process spawns it with
`std::process::Command` or `Bun.spawn` and talks to it over `127.0.0.1`. One
sidecar therefore serves both forms with no second implementation.

## Ollama — external dependency

- License MIT, per the [repository](https://github.com/ollama/ollama).
- macOS needs "MacOS Sonoma (v14) or newer" and "Apple M series (CPU and GPU
  support) or x86 (CPU only)"
  ([macOS docs](https://docs.ollama.com/macos)).
- "Ollama supports GPU acceleration on Apple devices via the Metal API"
  ([hardware support](https://docs.ollama.com/gpu)).
- Install weight on the user: `Ollama.dmg` is 180,349,804 bytes and
  `ollama-linux-amd64.tar.zst` is 1,420,686,963 bytes (arm64: 1,541,445,919
  bytes), measured from the `Content-Length` of
  [ollama.com/download/Ollama.dmg](https://ollama.com/download/Ollama.dmg) and
  [ollama.com/download/ollama-linux-amd64.tar.zst](https://ollama.com/download/ollama-linux-amd64.tar.zst),
  which redirect to release `v0.32.6`. The macOS app is Electron; the uninstall
  instructions list `com.electron.ollama` paths
  ([macOS docs](https://docs.ollama.com/macos)).
- Linux install needs root: the [Linux docs](https://docs.ollama.com/linux)
  create an `ollama` user and a systemd unit via `sudo`.
- Models live in `~/.ollama/models` (macOS) and
  `/usr/share/ollama/.ollama/models` (Linux), overridable with
  `OLLAMA_MODELS` ([FAQ](https://docs.ollama.com/faq)).
- Server binds `127.0.0.1:11434`. It exposes an OpenAI-compatible API at
  `http://localhost:11434/v1/` with `/chat/completions`, `/completions`,
  `/models`, `/embeddings`, and `/responses`
  ([OpenAI compatibility](https://docs.ollama.com/api/openai-compatibility)).
- Privacy caveat: Ollama now ships cloud models that run on Ollama servers and
  need an ollama.com account ([Cloud](https://docs.ollama.com/cloud)). The FAQ
  states "Ollama runs locally. We don't see your prompts or data when you run
  locally," and documents a local-only switch: `disable_ollama_cloud` in
  `~/.ollama/server.json`, or `OLLAMA_NO_CLOUD=1`
  ([FAQ](https://docs.ollama.com/faq)). If we recommend Ollama, we should
  recommend that switch with it.
- Tauri invocation: an HTTP POST to `/v1/chat/completions`. No sidecar, no
  shell permission, no plugin.
- CLI invocation: identical HTTP call.
- Rust core: any HTTP client. TypeScript/Bun core: `fetch`, or the official
  [`ollama` npm package](https://www.npmjs.com/package/ollama) (v0.6.3, MIT,
  128,259 bytes unpacked). Both sides are equally cheap.

## llama.cpp as a sidecar binary

- License MIT ([repository](https://github.com/ggml-org/llama.cpp)).
- "Apple silicon is a first-class citizen - optimized via ARM NEON, Accelerate
  and Metal frameworks" ([README](https://github.com/ggml-org/llama.cpp)).
- Release [b10333](https://github.com/ggml-org/llama.cpp/releases) (9 Aug 2026)
  ships `llama-b10333-bin-macos-arm64.tar.gz` at 11,015,270 bytes and
  `llama-b10333-bin-ubuntu-x64.tar.gz` at 16,507,165 bytes.
- Measured from those assets:

  | Set | macOS arm64 | Linux x64 |
  |---|---|---|
  | Everything in the archive | 27,348,246 B | 41,571,150 B |
  | Minimal server set (`llama`, `llama-server`, `libllama-server-impl`, `libllama`, `libllama-common`, `libggml-base`, one `libggml-cpu`, `libggml-blas`/`libggml-metal`) | 23,032,592 B | 20,043,576 B |

  The Linux build ships 14 `libggml-cpu-*.so` micro-architecture variants
  totalling 16,558,288 bytes. Shipping one variant saves ~15 MB and costs CPU
  portability. This trade-off is ours to make at bundle time.
- Metal shaders are compiled into the binary: `GGML_METAL_EMBED_LIBRARY`
  defaults to the value of `GGML_METAL`
  ([ggml/CMakeLists.txt](https://github.com/ggml-org/llama.cpp/blob/master/ggml/CMakeLists.txt)),
  so there is no loose `.metallib` to place next to the executable.
- Model download is built in: `-hf, -hfr, --hf-repo <user>/<model>[:quant]`,
  where "quant is optional, case-insensitive, default to Q4_K_M"
  ([server README](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md)).
  Files land in the cache directory: `$LLAMA_CACHE` if set, otherwise
  `~/Library/Caches/llama.cpp` on macOS and `$XDG_CACHE_HOME`/`~/.cache/llama.cpp`
  on Linux
  ([common/common.cpp](https://github.com/ggml-org/llama.cpp/blob/master/common/common.cpp)).
  `--models-dir` points the server at our own directory instead
  ([server README](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md)).
- The server exposes `POST /v1/chat/completions` as an OpenAI-compatible
  endpoint, and a router mode that starts with no model and swaps models on
  demand
  ([server README](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md)).
- Tauri invocation: `externalBin` plus `shell:allow-execute`, spawn
  `llama serve --port <free port>`, then HTTP.
- CLI invocation: spawn the same binary, or reuse a server the user already
  runs. The project now publishes its own one-line installer,
  `curl -LsSf https://llama.app/install.sh | sh` ([llama.app](https://llama.app)),
  which matches our own distribution idiom.
- Rust core and TypeScript/Bun core are equally viable. Process spawn plus HTTP
  exists in both. Tauri's JS `Command.sidecar` covers the TS side without any
  Rust code beyond the capability entry.

## llama.cpp in-process from Rust (llama-cpp-2)

- [`llama-cpp-2`](https://crates.io/crates/llama-cpp-2) v0.1.154, updated
  5 Aug 2026, 1,001,611 total downloads, license MIT OR Apache-2.0,
  repository [utilityai/llama-cpp-rs](https://github.com/utilityai/llama-cpp-rs).
- The manifest enables Metal automatically on Apple silicon: a
  `cfg(all(target_os = "macos", target_arch = "aarch64"))` dependency block
  turns on the `metal` feature
  ([llama-cpp-2/Cargo.toml](https://github.com/utilityai/llama-cpp-rs/blob/main/llama-cpp-2/Cargo.toml)).
- `llama-cpp-sys-2` vendors the llama.cpp sources and drives CMake from
  `build.rs`
  ([build.rs](https://github.com/utilityai/llama-cpp-rs/blob/main/llama-cpp-sys-2/build.rs)).
  A C++ toolchain and CMake are needed on **our** build machine, never on the
  user's.
- The README states the project "does not follow semver meaningfully"
  ([README](https://github.com/utilityai/llama-cpp-rs)). Pin an exact version.
- Tauri invocation: a direct Rust function call inside a `#[tauri::command]`.
  No sidecar, no shell permission, no port, no orphan process.
- CLI invocation: the same crate in the same workspace.
- Rust core: strong fit. TypeScript/Bun core: unusable. There are no Node
  bindings for this crate, so a TS core would fall back to the sidecar.

## mistral.rs

- License MIT
  ([LICENSE](https://github.com/EricLBuehler/mistral.rs/blob/master/LICENSE)).
- Prebuilt binaries from the install script cover "Metal on Apple Silicon;
  per-GPU CUDA or CPU on Linux; CPU on Windows"
  ([README](https://github.com/EricLBuehler/mistral.rs)).
- Release v0.9.0 (7 Jul 2026). `mistralrs-metal-aarch64-apple-darwin.tar.gz` is
  46,537,255 bytes and unpacks to a single 141,557,680-byte binary (measured).
  `mistralrs-cpu-x86_64-unknown-linux-gnu.tar.gz` is 33,499,414 bytes and
  unpacks to 99,151,728 bytes (measured). That is roughly six times the
  llama.cpp footprint on macOS for the same job.
- Three entry points: `mistralrs serve` (HTTP with a web UI), the `mistralrs`
  Rust crate embedded in-process, and a Python SDK
  ([README](https://github.com/EricLBuehler/mistral.rs)).
- Formats: GGUF 2–8 bit, Hugging Face safetensors, in-situ quantization of any
  Hugging Face model, plus GPTQ/AWQ/HQQ/FP8/BNB
  ([README](https://github.com/EricLBuehler/mistral.rs)).
- Models auto-resolve from Hugging Face: `mistralrs run -m Qwen/Qwen3-4B`
  ([README](https://github.com/EricLBuehler/mistral.rs)).
- Release cadence risk: GitHub is at v0.9.0 while
  [crates.io `mistralrs`](https://crates.io/crates/mistralrs) is at 0.8.1, last
  updated 2 Apr 2026. The in-process Rust path therefore trails the binary path
  by a release.
- Tauri invocation: in-process Rust call, or sidecar plus HTTP.
- CLI invocation: same crate, or the same binary.
- Rust core: viable, and the only option here with a first-class in-process
  Rust API plus a real server. TypeScript/Bun core: sidecar only; there are no
  Node bindings.

## candle

- Licenses MIT and Apache-2.0;
  [`candle-core`](https://crates.io/crates/candle-core) 0.11.0, updated
  26 Jun 2026, 6,838,108 total downloads.
- Backends: "Optimized CPU backend with optional MKL support for x86 and
  Accelerate for macs", CUDA, and WASM
  ([README](https://github.com/huggingface/candle)). Metal exists as a
  feature flag rather than a README bullet: `metal = ["dep:objc2-metal",
  "dep:objc2-foundation", "dep:candle-metal-kernels", ...]`
  ([candle-core/Cargo.toml](https://github.com/huggingface/candle/blob/main/candle-core/Cargo.toml)).
- Architecture coverage is current: `gemma3.rs`, `gemma4`, `qwen3.rs`,
  `quantized_gemma3.rs`, `quantized_qwen3.rs` all exist under
  [candle-transformers/src/models](https://github.com/huggingface/candle/tree/main/candle-transformers/src/models).
- candle is a tensor framework, not an inference server. There is no OpenAI
  endpoint, no chat-template handling, no KV-cache server, no model router. We
  would write and maintain the sampling loop, the prompt template, the
  tokenizer wiring, and the model-download logic (through
  [`hf-hub`](https://crates.io/crates/hf-hub) 1.0.0) ourselves, per
  architecture, forever.
- Tauri invocation: in-process Rust call. CLI invocation: same crate.
- Rust core: viable but expensive. TypeScript/Bun core: not applicable.

## node-llama-cpp

- v3.19.1, license MIT, `engines.node >= 20.0.0`
  ([npm registry](https://registry.npmjs.org/node-llama-cpp/latest)).
- "node-llama-cpp comes with pre-built binaries for macOS, Linux and Windows.
  If binaries are not available for your platform, it'll fallback to download a
  release of llama.cpp and build it from source with cmake"
  ([guide](https://node-llama-cpp.withcat.ai/guide/)). Metal is "Enabled by
  default on Macs with Apple Silicon"; CUDA "Used by default when support is
  detected" (same page).
- Sizes: the main package is 37,487,356 bytes unpacked across 920 files;
  [`@node-llama-cpp/mac-arm64-metal`](https://registry.npmjs.org/@node-llama-cpp/mac-arm64-metal/latest)
  is 12,743,468 bytes; [`@node-llama-cpp/linux-x64`](https://registry.npmjs.org/@node-llama-cpp/linux-x64/latest)
  is 28,002,361 bytes.
- Model download: `node-llama-cpp pull --dir ./models <model-url>` or
  `resolveModelFile("hf:user/model:quant", modelsDirectory)`, with the models
  directory chosen by us
  ([downloading models](https://node-llama-cpp.withcat.ai/guide/downloading-models)).
- The homepage states it "Works in Node.js, Bun, and Electron"
  ([node-llama-cpp.withcat.ai](https://node-llama-cpp.withcat.ai/)).
- Structural cost inside Tauri: the Tauri webview is a browser context. It
  cannot load a native N-API addon. A Tauri app therefore needs a Node or Bun
  **sidecar process** to host node-llama-cpp, which adds the runtime itself:
  `bun-darwin-aarch64.zip` is 23,586,433 bytes and `bun-linux-x64.zip` is
  35,969,274 bytes at
  [bun-v1.3.14](https://github.com/oven-sh/bun/releases) (13 May 2026).
  `bun build --compile` can embed `.node` files, and the Bun docs concede
  "Overall though, Bun's binary is still way too big and we need to make it
  smaller" ([Bun executables](https://bun.com/docs/bundler/executables)).
- Tauri invocation: sidecar process plus IPC or HTTP.
- CLI invocation: direct import, no sidecar, if the CLI is already a Bun
  program.
- Rust core: pointless — it would mean shipping a JS runtime to reach a C++
  library that Rust can link directly. TypeScript/Bun core: this is the natural
  choice, but only if the core is already a Bun process for other reasons.

## Options the ticket did not name

**LM Studio.** Its `llmster` daemon is "the core of the LM Studio desktop app,
packaged to be server-native, without reliance on the GUI", installed with
`curl -fsSL https://lmstudio.ai/install.sh | bash`
([headless docs](https://lmstudio.ai/docs/app/api/headless)). The `lms` CLI is
MIT ([lmstudio-ai/lms](https://github.com/lmstudio-ai/lms)) and
[`@lmstudio/sdk`](https://www.npmjs.com/package/@lmstudio/sdk) is Apache-2.0,
but the app itself is proprietary: the terms grant "a non-exclusive,
non-transferable license to use the Software solely for Your personal and / or
internal business purposes" and forbid sublicensing or redistribution
([terms](https://lmstudio.ai/terms)). Usable as a detected, user-owned
OpenAI-compatible endpoint. Never bundleable.

**llamafile.** Release 0.10.5 (3 Aug 2026), Apache-2.0 with MIT for the
llama.cpp changes ([repository](https://github.com/Mozilla-Ocho/llamafile)).
Assets: `llamafile-0.10.5` at 350,768,862 bytes and `llamafile-0.10.5-thin` at
42,328,074 bytes
([release](https://github.com/Mozilla-Ocho/llamafile/releases/tag/0.10.5)). The
single-file cross-OS property solves a problem we do not have, because Tauri
already bundles per-triple binaries. The exact difference between the full and
thin builds is unverified.

**In-webview WASM inference.** Dropped without deep investigation. A 2–5 GB
weight file inside a WKWebView or WebKitGTK process is the wrong place for it,
and WebGPU availability in those embedded webviews is unverified.

## Apple-native path

### Foundation Models framework

The one option that costs nothing in bundle size and nothing in user install.

- Available from macOS 26.0, iOS 26.0, iPadOS 26.0, visionOS 26.0
  ([framework docs](https://developer.apple.com/documentation/foundationmodels)).
  Apple silicon only, Apple Intelligence must be on, and the assets need "7 GB
  of storage on device" ([Apple Intelligence requirements](https://support.apple.com/en-us/121115)).
  Not available for devices bought in mainland China (same page).
- The app downloads nothing. The model is "provided by the OS at no download
  cost to the app"
  ([adding intelligent app features](https://developer.apple.com/documentation/foundationmodels/adding-intelligent-app-features-with-generative-models)).
  It is roughly 3B parameters with 2-bit QAT decoder weights
  ([Apple ML research](https://machinelearning.apple.com/research/apple-foundation-models-2025-updates)).
- **The context window is 4,096 tokens.** "A single token corresponds to three
  or four characters in languages like English"
  ([generating content](https://developer.apple.com/documentation/FoundationModels/generating-content-and-performing-tasks-with-foundation-models)).
  A single long email thread fills it. This caps the framework at short,
  single-message classification, and rules it out for messy multi-receipt
  extraction.
- Graceful degradation is built in: `SystemLanguageModel.Availability` returns
  `.available` or `.unavailable` with `.appleIntelligenceNotEnabled`,
  `.deviceNotEligible`, or `.modelNotReady`
  ([SystemLanguageModel](https://developer.apple.com/documentation/foundationmodels/systemlanguagemodel)).
  That maps cleanly onto an optional tier.
- Guardrails have two settings, `.default` and
  `.permissiveContentTransformations`, and no off switch
  ([Guardrails](https://developer.apple.com/documentation/foundationmodels/systemlanguagemodel/guardrails)).
  The [acceptable use requirements](https://developer.apple.com/apple-intelligence/acceptable-use-requirements-for-the-foundation-models-framework/)
  forbid circumventing them, and are referenced from the
  [Apple Developer Program License Agreement](https://developer.apple.com/support/terms/apple-developer-program-license-agreement/).
  Refusals are a documented error case
  ([LanguageModelError](https://developer.apple.com/documentation/foundationmodels/languagemodelerror)).
- Sessions are single-request: calling a session again before the previous
  request finishes "causes a runtime error"
  ([generating content](https://developer.apple.com/documentation/FoundationModels/generating-content-and-performing-tasks-with-foundation-models)).
  Batch classification needs our own queue.
- Reaching it from a non-Swift process, in order of cost:
  1. **C ABI.** Apple ships `foundation-models-c` inside the Python SDK source
     tree — a SwiftPM package producing a dynamic `FoundationModels` library
     with a plain C header, plus a C example target
     ([apple/python-apple-fm-sdk](https://github.com/apple/python-apple-fm-sdk),
     Apache-2.0). Rust can FFI straight into it.
  2. **`fm` CLI.** Apple's WWDC26 session describes `fm respond`, `fm chat`,
     and `fm schema` with `--instructions`, `--schema file.json`, and JSON
     output piped to `jq`, "pre-installed with macOS 27"
     ([WWDC26 session 334](https://developer.apple.com/videos/play/wwdc2026/334/)).
     Zero build dependency, but macOS 27 and later only.
  3. **Python SDK.** [`apple-fm-sdk`](https://pypi.org/project/apple-fm-sdk/)
     0.2.1 (29 Jun 2026), Apache-2.0, ships an sdist only, so `pip install`
     compiles Swift and needs Xcode on the end user's machine.
- No entitlement is documented for base on-device use. The two published
  entitlements cover optional features:
  [`foundation-model-adapter`](https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.developer.foundation-model-adapter)
  and
  [`private-cloud-compute`](https://developer.apple.com/documentation/BundleResources/Entitlements/com.apple.developer.private-cloud-compute).
- Linux: nonexistent, and permanently so. The platform list is Apple operating
  systems only
  ([framework docs](https://developer.apple.com/documentation/foundationmodels)).
- Tauri invocation: a Rust FFI call into the C dylib, wrapped in a
  `#[tauri::command]`, gated on the availability check. CLI invocation: the same
  FFI, or `fm` on macOS 27.
- Rust core: workable through the C ABI. TypeScript/Bun core: only by spawning
  `fm`, which narrows support to macOS 27 and later.

### MLX

- MIT throughout: [mlx](https://github.com/ml-explore/mlx/blob/main/LICENSE),
  [mlx-lm](https://github.com/ml-explore/mlx-lm/blob/main/LICENSE),
  [mlx-swift](https://github.com/ml-explore/mlx-swift/blob/main/LICENSE),
  [mlx-c](https://github.com/ml-explore/mlx-c/blob/main/LICENSE). Copyright
  Apple Inc.
- macOS support is Apple silicon only, macOS 14.0 or newer, with a native arm64
  Python ([install docs](https://ml-explore.github.io/mlx/build/html/install.html)).
  Every macOS wheel on [PyPI](https://pypi.org/project/mlx/) is `arm64`. Intel
  Macs are excluded, so MLX cannot be the only macOS path.
- Linux is now supported: `pip install mlx[cpu]` and `pip install mlx[cuda]`
  ([README](https://github.com/ml-explore/mlx)). CUDA needs SM >= 7.5, driver
  >= 550.54.14, and glibc >= 2.35
  ([install docs](https://ml-explore.github.io/mlx/build/html/install.html)).
- Real footprint: the `mlx` wheel is a shim at 558,799 bytes; the backend
  [`mlx-metal`](https://pypi.org/project/mlx-metal/) 0.32.0 is 40,824,649 bytes
  for `macosx_14_0_arm64` and 56,511,379 bytes for `macosx_26_0_arm64`.
  [`mlx-cpu`](https://pypi.org/project/mlx-cpu/) for Linux x86_64 is 10,370,672
  bytes. The Swift path is heavier: `Cmlx.xcframework.zip` in
  [mlx-swift 0.31.6](https://github.com/ml-explore/mlx-swift/releases) is
  202,476,839 bytes.
- The real cost is Python. [`mlx-lm`](https://pypi.org/project/mlx-lm/) 0.31.3
  is a 408,890-byte wheel that requires `mlx`, `numpy`, `transformers>=5.0.0`,
  `sentencepiece`, `protobuf`, `pyyaml`, and `jinja2`. Bundling that stack into
  a Tauri app is a larger problem than any inference binary in this survey.
- Models are safetensors from the
  [mlx-community](https://huggingface.co/mlx-community) org, downloaded on first
  use into `~/.cache/huggingface/hub`, overridable with `HF_HOME` or
  `HF_HUB_CACHE`
  ([SERVER.md](https://github.com/ml-explore/mlx-lm/blob/main/mlx_lm/SERVER.md),
  [HF cache guide](https://huggingface.co/docs/huggingface_hub/en/guides/manage-cache)).
- Server mode: `mlx_lm.server` listens on localhost port 8080 and is "intended
  to be similar to the OpenAI chat API", with an explicit warning that it "is
  not recommended for production as it only implements basic security checks"
  ([SERVER.md](https://github.com/ml-explore/mlx-lm/blob/main/mlx_lm/SERVER.md)).
- Rust binding status is poor. `oxideai/mlx-rs` now redirects to
  [oxiglade/mlx-rs](https://github.com/oxiglade/mlx-rs); crates.io
  [`mlx-rs`](https://crates.io/crates/mlx-rs) is at 0.25.3, published
  16 Dec 2025, while mlx itself is at
  [v0.32.0](https://github.com/ml-explore/mlx/releases) (7 Jul 2026). The
  binding is an unofficial project several months behind.
- [`mlx-c`](https://github.com/ml-explore/mlx-c) is the official C API and "can
  be used standalone or as a bridge to bind other languages to MLX". It exposes
  arrays and operations, not a tokenizer or a chat loop, so an in-process Rust
  path would mean rebuilding what mlx-lm already does in Python.
- Node and Bun have no official package; npm carries third-party wrappers only.
- Tauri invocation: spawn `mlx_lm.server` as a sidecar and use HTTP. CLI
  invocation: the same. Both require a Python environment we would have to ship
  or find.
- Rust core: sidecar only, in practice. TypeScript/Bun core: sidecar only.

## Model weights: sizes, licenses, and gating

Runtime choice is only half the cost. The weights are the larger download and
the harder license question.

Current generation:

| Model | 4-bit file | Size | Context | License | HF gated |
|---|---|---|---|---|---|
| [Gemma 4 E2B](https://huggingface.co/google/gemma-4-E2B-it-qat-q4_0-gguf/tree/main) | `gemma-4-E2B_q4_0-it.gguf` | 3,349,516,256 B | 128K | [Apache-2.0](https://ai.google.dev/gemma/docs/gemma_4_license) | No |
| [Gemma 4 E4B](https://huggingface.co/google/gemma-4-E4B-it-qat-q4_0-gguf/tree/main) | `gemma-4-E4B_q4_0-it.gguf` | 5,154,941,280 B | [128K](https://huggingface.co/google/gemma-4-E4B-it) | [Apache-2.0](https://ai.google.dev/gemma/docs/gemma_4_license) | No |
| [Qwen3.5 0.8B](https://huggingface.co/unsloth/Qwen3.5-0.8B-GGUF/tree/main) | Q4_K_M | 532,517,120 B | — | [Apache-2.0](https://huggingface.co/Qwen/Qwen3.5-4B/blob/main/LICENSE) | No |
| [Qwen3.5 2B](https://huggingface.co/unsloth/Qwen3.5-2B-GGUF/tree/main) | Q4_K_M | 1,280,835,840 B | — | Apache-2.0 | No |
| [Qwen3.5 4B](https://huggingface.co/unsloth/Qwen3.5-4B-GGUF/tree/main) | Q4_K_M | 2,740,937,888 B | [262,144](https://huggingface.co/Qwen/Qwen3.5-4B) | Apache-2.0 | No |
| [Ministral 3 3B](https://huggingface.co/mistralai/Ministral-3-3B-Instruct-2512-GGUF/tree/main) | Q4_K_M | 2,147,023,008 B | [256K](https://ollama.com/library/ministral-3) | [Apache-2.0](https://huggingface.co/api/models/mistralai/Ministral-3-8B-Instruct-2512) | No |
| [Ministral 3 8B](https://huggingface.co/mistralai/Ministral-3-8B-Instruct-2512-GGUF/tree/main) | Q4_K_M | 5,198,911,904 B | 256K | Apache-2.0 | No |

Previous generation, and the licensing traps:

| Model | 4-bit file | Size | License | HF gated |
|---|---|---|---|---|
| [Gemma 3 270M](https://huggingface.co/unsloth/gemma-3-270m-it-GGUF/tree/main) | Q4_K_M | 253,115,424 B | [Gemma Terms of Use](https://ai.google.dev/gemma/terms) | Google repos: **yes** (`manual`); mirrors: no |
| [Gemma 3 1B](https://huggingface.co/google/gemma-3-1b-it-qat-q4_0-gguf/tree/main) | `q4_0` QAT | 1,003,541,152 B | Gemma Terms of Use | **Yes** |
| [Gemma 3 4B](https://huggingface.co/unsloth/gemma-3-4b-it-GGUF/tree/main) | Q4_K_M | 2,489,894,016 B | Gemma Terms of Use | Google repos: **yes**; unsloth mirror: no |
| [Qwen3 0.6B](https://huggingface.co/unsloth/Qwen3-0.6B-GGUF/tree/main) | Q4_K_M | 396,705,472 B | [Apache-2.0](https://huggingface.co/Qwen/Qwen3-4B-Instruct-2507/blob/main/LICENSE) | No |
| [Qwen3 4B Instruct 2507](https://huggingface.co/unsloth/Qwen3-4B-Instruct-2507-GGUF/tree/main) | Q4_K_M | 2,497,281,120 B | Apache-2.0 | No |
| [Ministral 8B Instruct 2410](https://huggingface.co/bartowski/Ministral-8B-Instruct-2410-GGUF/tree/main) | Q4_K_M | 4,911,500,096 B | [Mistral Research License](https://mistral.ai/licenses/MRL-0.1.md) — research only | No, but unusable |
| [Llama 3.2 3B](https://huggingface.co/bartowski/Llama-3.2-3B-Instruct-GGUF/tree/main) | Q4_K_M | 2,019,377,696 B | [Llama 3.2 Community License](https://www.llama.com/llama3_2/use-policy) | **Yes** (`manual`) |
| [Phi-4-mini](https://huggingface.co/bartowski/microsoft_Phi-4-mini-instruct-GGUF/tree/main) | Q4_K_M | 2,491,874,688 B | [MIT](https://huggingface.co/microsoft/Phi-4-mini-instruct/resolve/main/LICENSE) | No |
| [SmolLM3 3B](https://huggingface.co/ggml-org/SmolLM3-3B-GGUF/tree/main) | Q4_K_M | 1,915,305,312 B | [Apache-2.0](https://huggingface.co/api/models/HuggingFaceTB/SmolLM3-3B) | No |

Three findings drive the model choice:

1. **Gating decides whether we can download silently.** Every `google/gemma-3-*`
   repository reports `"gated": "manual"`, including the official GGUF ones
   ([example](https://huggingface.co/api/models/google/gemma-3-4b-it-qat-q4_0-gguf)).
   So does `meta-llama/Llama-3.2-3B-Instruct`
   ([API](https://huggingface.co/api/models/meta-llama/Llama-3.2-3B-Instruct)).
   A gated repository needs a Hugging Face account and per-repository approval,
   which no unattended download can satisfy. Gemma 4, Qwen 3.5, and Ministral 3
   are ungated at the vendor org.
2. **Gemma 4 changed license to Apache-2.0**
   ([license](https://ai.google.dev/gemma/docs/gemma_4_license)), replacing the
   [Gemma Terms of Use](https://ai.google.dev/gemma/terms) that governs Gemma 3.
   That removes the downstream use-restriction problem for an MIT product.
3. **Ministral 8B Instruct 2410 is research-only.** MRL-0.1 §3.2: "You shall
   only use the Mistral Models, Derivatives … and Outputs for Research
   Purposes" ([MRL-0.1](https://mistral.ai/licenses/MRL-0.1.md)). The ticket
   named it; it cannot ship. Ministral 3 (Apache-2.0) is its replacement.

Ollama default tags for reference: `gemma4` 9.6 GB / 128K
([library](https://ollama.com/library/gemma4)), `qwen3.5` 6.6 GB / 256K with
`0.8b` at 1.0 GB and `4b` at 3.4 GB
([library](https://ollama.com/library/qwen3.5)), `ministral-3` 6.0 GB / 256K
([library](https://ollama.com/library/ministral-3)).

## Rust core versus TypeScript/Bun core

| Option | Rust core | TypeScript/Bun core |
|---|---|---|
| Ollama | HTTP client | HTTP client |
| llama.cpp sidecar | Spawn + HTTP | Spawn + HTTP (`Command.sidecar` from JS) |
| llama-cpp-2 | Native, best fit | Not available |
| mistral.rs | Native crate or sidecar | Sidecar only |
| candle | Native, high build cost | Not available |
| node-llama-cpp | Needs a JS runtime we would not otherwise ship | Natural fit |
| MLX | Sidecar plus HTTP; `mlx-rs` is stale | Sidecar plus HTTP |
| Foundation Models | C ABI through FFI | Spawn `fm`, macOS 27+ only |
| LM Studio / llamafile | Spawn or HTTP | Spawn or HTTP |

The decisive observation: **the sidecar plus OpenAI-compatible-HTTP design is
language-neutral.** It costs the same in Rust and in Bun, it is identical in the
Tauri app and in the headless CLI, and it is the same code path Tier 3 already
needs. Choosing it means the Tier 1 research does not block the core-language
decision. Choosing `llama-cpp-2` or `candle` forces Rust; choosing
node-llama-cpp forces a JS runtime.

## What could not be verified

- Bundle-size delta of linking `llama-cpp-2` or `candle` into a Tauri binary.
  Both compile from source, so the number depends on our feature flags. No
  primary source publishes it and no build was run here.
- The mistral.rs Hugging Face cache location. The README shows auto-download
  but names no path.
- The difference between `llamafile` and `llamafile-thin` assets. The release
  notes reference a PR that documents it; the text was not found in the README.
- WebGPU availability in the WKWebView and WebKitGTK webviews that Tauri uses.
- Whether `ollama-rs` (0.3.6) carries a license the project can rely on. Only
  its version, date, and repository were checked. Plain HTTP avoids the
  question entirely.
- Q4_K_M GGUF for Gemma 4. Google publishes `q4_0` QAT builds only.
- Ollama's `gemma4` tag reports 9.6 GB against a 5.15 GB `q4_0` file. The
  quantization behind that tag is not stated on the library page.
- Context-window figures for Qwen3.5 0.8B and 2B, and MLX 4-bit builds for
  Ministral 3.
- Whether Foundation Models works from an unsigned or ad-hoc-signed app
  distributed outside the App Store. Apple publishes no explicit statement. The
  circumstantial evidence is that Apple's own `fm` CLI and the `fm-c-example`
  SwiftPM executable call the same model with no entitlement. A short test on a
  macOS 26 machine settles it, and that test should happen before this option
  is planned in.
- The built size of the `foundation-models-c` dylib. No machine with macOS 26
  was available.
- The on-disk size of the Foundation Models weights alone. Apple publishes 7 GB
  for all Apple Intelligence assets together.
- The numeric threshold behind the Foundation Models `rateLimited` error.
- Whether any XPC or daemon interface to Foundation Models exists. Third-party
  npm wrappers claim one; no Apple source documents it.

## Decision consequence (Tier 1)

Ranked for this product, under the rule that Tier 0 must stay free of any model
dependency.

1. **Detect and use an existing OpenAI-compatible local endpoint.** Ollama at
   `127.0.0.1:11434/v1`, `llama serve`, and LM Studio all speak the same
   protocol as Tier 3. We ship one client, zero bytes, and no install step.
   A probe that finds nothing leaves Tier 0 untouched. This is the whole of
   Tier 1 for v0.
2. **A llama.cpp sidecar, fetched on demand when the user enables Tier 1.**
   23.0 MB on macOS arm64 and about 20.0 MB on Linux x64, MIT, Metal and CPU
   both covered, `-hf` handles model download and caching, and the same binary
   serves the desktop app and the CLI. Fetching it at enable-time rather than
   bundling it keeps the Tier 0 installer honest. This is the fallback for
   users with no runtime of their own.
3. **`llama-cpp-2` in-process.** Take this only if the core language lands on
   Rust and a single self-contained binary becomes a goal. It removes the port,
   the spawn, and the orphan-process handling. It also forces Rust and pins us
   to a crate that disclaims semver.
4. **Apple Foundation Models, as an opportunistic extra on macOS 26 and later.**
   Free in bundle size and free in user install, with a real availability API
   for graceful degradation. The 4,096-token context confines it to short
   classification prompts, so it supplements the sidecar and never replaces it.
   Settle the unsigned-app question before planning it in.
5. **mistral.rs.** Same license and similar capability, at 141.6 MB on macOS
   against llama.cpp's 23.0 MB, with the published crate a release behind the
   published binary. No advantage here to pay that with.
6. **MLX.** Apple silicon only, no Intel Mac path, and the usable inference
   layer is Python with `transformers`. The Rust binding is months stale.
7. **candle.** A tensor framework, not a runtime. Choosing it means owning a
   sampling loop and a chat template per architecture, forever, for no gain
   over llama.cpp.
8. **LM Studio.** Detect it, never bundle it. Its terms forbid redistribution.
9. **llamafile.** Solves single-file portability, which Tauri's per-triple
   bundling already solves.

On weights: default to **Qwen 3.5**. It is Apache-2.0, ungated at the vendor
org, and offers a 0.8B / 2B / 4B ladder from 533 MB to 2.74 GB, so the download
can match the machine. **Gemma 4** is the alternate, now Apache-2.0 and ungated,
starting at 3.35 GB for E2B. Avoid Gemma 3: every Google repository is gated, so
an unattended download fails and we would depend on third-party mirrors.
**Ministral 8B Instruct 2410, named in the ticket, cannot ship** — its license
restricts use to research.

Nothing above changes the Tier 0 path, and nothing above forces the core
language. Option 1 and option 2 are identical work in Rust and in Bun, so this
research does not block that decision.
