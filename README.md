# GemEd

GemEd is being rewritten as a Rust + Dioxus visual workflow editor. The current runnable slice is a cross-platform Dioxus shell with a Rust workflow schema, validation, JSON import/export, storage, mock provider execution, and a basic editable node/edge canvas.

## Current Status

Implemented now:

- Rust workspace with `gemed_core`, `gemed_executor`, `gemed_media`, `gemed_providers`, `gemed_storage`, and `gemed_app` crates
- Dioxus app targeting web and desktop from one codebase
- Workflow v1 schema for nodes, edges, groups, node statuses, and edge style
- Representative legacy and media-preview workflow JSON fixtures with import/roundtrip coverage in `gemed_core`
- Typed node-handle metadata in `gemed_core` for canvas connection UI
- Graph traversal, connected-input resolution, and topological execution ordering
- Local simple executor for prompt, array, prompt-constructor, output, gallery, annotation, inline image compare metrics, inline split-grid image cells, and control nodes
- Built-in starter, Media Sample, Transform Sample, Frame Sample, LLM Provider Sample, and multimodal Provider Media Sample workflows
- JSON editor that loads and validates workflow JSON
- SVG/HTML canvas rendering nodes and edges
- Basic Canvas MVP controls: select/multi-select/drag/nudge nodes, create groups from selection or Shift-drag box selection, generate Split Grid child node sets, grouped node backgrounds with lock/unlock, drag-to-move group headers, sidebar resize controls, direct canvas group resize handles, sidebar and direct wheel/blank-canvas pan/zoom/reset viewport controls, visual handle-to-handle connect, connect selected node to the next node, visual edge removal, undo, and redo
- Provider trait boundary with deterministic mock providers for LLM/image/video/audio/3D generation nodes, including an offline LLM Provider Sample that exercises Gemini/OpenAI/Anthropic routing and a Provider Media Sample that exercises image/video/audio/3D mock generation without real API calls
- Optional desktop LLM HTTP backends behind the `providers-http` feature: Gemini GenerateContent via `GEMINI_API_KEY`, OpenAI Responses via `OPENAI_API_KEY`, and Anthropic Messages via `ANTHROPIC_API_KEY`; normal builds keep network/provider calls out unless explicitly enabled
- Provider configuration boundary with explicit runtime modes (`mock`, `directDesktop`, `webBackend`, `disabled`) and secret sources; desktop env var names are modeled without storing secret values in app state
- Provider settings panel with platform/mock defaults, per-provider mode toggles, editable default model/base URL fields, and provider-config save/load through desktop filesystem or web localStorage without raw API-key persistence
- Provider secret setup/status hints that tell users which desktop environment variable or web backend binding to configure without writing API keys into workflow/provider JSON
- Fake-transport coverage for the opt-in Gemini, OpenAI, and Anthropic LLM backends so request mapping, secret/header construction, response parsing, and transport errors are tested without real API calls
- Media capability profiles for image/audio/video/3D-capable nodes, with web/desktop readiness and adapter-gap summaries surfaced in the app sidebar; inline image compare metrics and split-grid PNG/JPEG/WebP transforms now have first Rust adapters, video frame-grab records source/seek metadata and exposes an opt-in WebView video/canvas capture action for renderable sources, GLB viewer records metadata and provides local-first WebView model-viewer preview plus opt-in PNG snapshot capture for renderable GLB URIs without fake Run Local capture, split-grid can generate legacy child ImageInput/Prompt/Generate sets, and desktop project load hydrates saved image/model refs back into transformable data URLs
- Node-card media preview foundation that detects common inline/reference media fields, renders size-guarded image/audio/video/GLB previews, exposes Open/Download/Copy URI links, supports an in-app image/audio/video/GLB media overlay, and flags project-reference hydration/capture gaps honestly
- Header action to load the built-in Media Sample and exercise the JSON import/export → node-card preview path without external provider calls
- Storage trait boundary with in-memory, browser localStorage, and desktop filesystem implementations
- Execution spine panel, Run Local action for pure-Rust workflow smoke runs, Run Providers action for configured provider-trait smoke runs, and streamed per-node loading → terminal execution events for progress/status visibility while runs are active
- Executor cancellation-control foundation with backward-compatible APIs, header `Cancel Run` UI wiring for active local/provider runs, and tests for pre-run cancellation plus downstream skip behavior after a provider-triggered cancel
- Save Slot / Load Slot actions backed by platform storage (`localStorage` on web, app data JSON files on desktop)
- Desktop-only native Open File / Save As actions for workflow JSON files
- Desktop-only project directory Open Project / Save Project actions using `gemed-project.json`, `workflow.json`, and `media/`, with known media fields saved through companion `*Ref` fields, generic data URL fallback externalization, stale manifest-tracked media cleanup, ref-preserving media hydration on load, and split-grid rerun coverage after project roundtrip
- Web, Linux desktop, and Linux `.deb` bundle verification
- Rust release-smoke coverage for opening a built-in example, creating/connecting a workflow, save/load through storage, running a no-provider workflow, and running the mock provider sample
- Real Chromium web interaction smoke for Frame Sample `Capture`, local-first GLB preview, and GLB `Capture PNG`; see `docs/webview-interaction-smoke.md`
- Windows desktop foundation through Dioxus desktop feature, Windows GUI subsystem setting, app/bundle icon configuration, CI build/bundle matrix, and the evidence checklist in `docs/windows-desktop-verification.md`

Not implemented yet:

- Full editable canvas UX: polished drag/handle affordances
- Broader live Provider/API execution beyond the opt-in Gemini/OpenAI/Anthropic LLM desktop paths
- Provider secret entry/storage, OS keychain persistence, and web backend/server-function execution
- Broader video/audio/3D execution adapters beyond schema/mock/planning capability modeling; split-grid transforms hydrated inline image data URLs; video frame-grab and GLB snapshot capture still require renderable WebView/browser sources plus real browser/WebView interaction verification; unresolved/missing project refs remain non-fatal preview/storage gaps
- Media storage polish: richer node-specific editor/player affordances, native desktop WebView validation for local-first GLB model-viewer bundling, stronger clipboard fallbacks, and broader media transform adapters

See `docs/dioxus-rewrite-plan.md` for the full migration plan.

## Prerequisites

- Rust 1.96+
- `wasm32-unknown-unknown` target for web builds
- Dioxus CLI matching the app dependency:

```bash
cargo install dioxus-cli --version 0.8.0-alpha.0 --locked
rustup target add wasm32-unknown-unknown
```

Linux desktop builds need WebKitGTK development packages installed by the host distribution.

## Run

### Web

```bash
dx serve --web --no-default-features --features web --open false
```

### Desktop on Linux/Windows

```bash
dx serve --desktop --features desktop
```

To opt into the experimental desktop LLM HTTP backends:

```bash
GEMINI_API_KEY=... OPENAI_API_KEY=... ANTHROPIC_API_KEY=... dx serve --desktop --features desktop,providers-http
```

In the app, set Gemini, OpenAI, or Anthropic to the platform/env mode in Provider Settings and run an LLM node with provider `gemini`, `openai`, or `anthropic`. Gemini defaults to GenerateContent with `gemini-3.5-flash`; Anthropic defaults to Messages API with `claude-sonnet-4-6` when the workflow does not provide a concrete model. The regular desktop and web commands do not include live provider HTTP clients.

For deterministic offline provider verification, click `Provider Sample`, click `Mock Defaults` in Provider Settings, then click `Run Providers`. The three output nodes should receive mock text for `gemini`, `openai`, and `anthropic`. To verify non-LLM provider traits, click `Provider Media`, keep mock defaults, then click `Run Providers`; image/video/audio/3D output nodes should receive `mock://` media references. For opt-in live desktop LLM verification, launch with `--features desktop,providers-http`, set the same providers to `Env`, export the matching environment variables, optionally edit default model/base URL fields, and run the same sample. Provider config persists model/base URL labels only; raw API keys stay in the process environment.

For a narrower opt-in live provider check outside the UI, use `docs/provider-live-smoke.md`. The smoke fixture calls the same Rust Gemini/OpenAI/Anthropic LLM HTTP backends and reports only provider/model/response previews, never secret values.

For browser media interaction validation, use `docs/webview-interaction-smoke.md`. The current smoke clicks `Frame Sample` capture and `Media Sample` GLB `Capture PNG` in real Chromium while keeping Playwright outside the repo.

For release-style build artifacts:

```bash
dx build --web --no-default-features --features web
dx build --desktop --features desktop
dx build --desktop --features desktop,providers-http
dx bundle --desktop --features desktop,bundle --package-types deb
```

On Windows, run the desktop build/bundle commands from a Windows machine/runner:

```powershell
dx build --desktop --features desktop
dx build --desktop --features desktop,providers-http
dx bundle --desktop --features desktop,bundle --package-types msi
```

## Validate

```bash
cargo fmt --all --check
cargo test --workspace
cargo test --workspace --features desktop,providers-http
cargo clippy --workspace --all-targets --features desktop -- -D warnings
cargo clippy --workspace --all-targets --no-default-features --features web -- -D warnings
cargo clippy --workspace --all-targets --features desktop,providers-http -- -D warnings
```

CI mirrors these gates, runs the web interaction smoke in Chromium, and runs a Dioxus matrix for web build, Linux/Windows desktop builds including `providers-http`, and Linux/Windows desktop bundle evidence in `.github/workflows/ci.yml`.

## Notes

This repo includes `.cargo/config.toml` to prevent host-only linker flags from leaking into WASM builds. Without it, user/global Cargo configs that pass ELF linker options can make `rust-lld` fail for `wasm32-unknown-unknown`.
