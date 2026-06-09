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
- Local simple executor for prompt, array, prompt-constructor, output, gallery, annotation, and control nodes
- Built-in starter and Media Sample workflows
- JSON editor that loads and validates workflow JSON
- SVG/HTML canvas rendering nodes and edges
- Basic Canvas MVP controls: select/multi-select/drag/nudge nodes, create groups from selection or Shift-drag box selection, grouped node backgrounds with lock/unlock, drag-to-move group headers, sidebar resize controls, direct canvas group resize handles, sidebar and direct wheel/blank-canvas pan/zoom/reset viewport controls, visual handle-to-handle connect, connect selected node to the next node, visual edge removal, undo, and redo
- Provider trait boundary with deterministic mock providers for LLM/image/video/audio/3D generation nodes
- Optional desktop LLM HTTP backends behind the `providers-http` feature: Gemini GenerateContent via `GEMINI_API_KEY`, OpenAI Responses via `OPENAI_API_KEY`, and Anthropic Messages via `ANTHROPIC_API_KEY`; normal builds keep network/provider calls out unless explicitly enabled
- Provider configuration boundary with explicit runtime modes (`mock`, `directDesktop`, `webBackend`, `disabled`) and secret sources; desktop env var names are modeled without storing secret values in app state
- Provider settings panel with platform/mock defaults, per-provider mode toggles, and provider-config save/load through desktop filesystem or web localStorage without raw API-key persistence
- Provider secret setup/status hints that tell users which desktop environment variable or web backend binding to configure without writing API keys into workflow/provider JSON
- Fake-transport coverage for the opt-in Gemini, OpenAI, and Anthropic LLM backends so request mapping, secret/header construction, response parsing, and transport errors are tested without real API calls
- Media capability profiles for image/audio/video/3D-capable nodes, with web/desktop readiness and adapter-gap summaries surfaced in the app sidebar
- Node-card media preview foundation that detects common inline/reference media fields, renders size-guarded image/audio/video previews, exposes Open/Download/Copy URI links, supports an in-app image/audio/video media overlay, and flags GLB/project-reference adapter gaps honestly
- Header action to load the built-in Media Sample and exercise the JSON import/export → node-card preview path without external provider calls
- Storage trait boundary with in-memory, browser localStorage, and desktop filesystem implementations
- Execution spine panel, Run Local action for pure-Rust workflow smoke runs, and Run Providers action for configured provider-trait smoke runs
- Save Slot / Load Slot actions backed by platform storage (`localStorage` on web, app data JSON files on desktop)
- Desktop-only native Open File / Save As actions for workflow JSON files
- Desktop-only project directory Open Project / Save Project actions using `gemed-project.json`, `workflow.json`, and `media/`, with known media fields saved through companion `*Ref` fields, generic data URL fallback externalization, stale manifest-tracked media cleanup, and media hydration on load
- Web and Linux desktop build verification
- Windows desktop foundation through Dioxus desktop feature and Windows GUI subsystem setting

Not implemented yet:

- Full editable canvas UX: polished drag/handle affordances
- Broader live Provider/API execution beyond the opt-in Gemini/OpenAI/Anthropic LLM desktop paths
- Provider secret entry/storage, OS keychain persistence, and web backend/server-function execution
- Real media/video/3D execution adapters beyond schema/mock capability modeling
- Media storage polish: richer node-specific editor/player affordances, GLB/WebGL previews, stronger clipboard fallbacks, and real media transform adapters

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

For release-style build artifacts:

```bash
dx build --web --no-default-features --features web
dx build --desktop --features desktop
dx build --desktop --features desktop,providers-http
```

On Windows, run the desktop build command from a Windows machine/runner.

## Validate

```bash
cargo fmt --all --check
cargo test --workspace
cargo test --workspace --features desktop,providers-http
cargo clippy --workspace --all-targets --features desktop -- -D warnings
cargo clippy --workspace --all-targets --no-default-features --features web -- -D warnings
cargo clippy --workspace --all-targets --features desktop,providers-http -- -D warnings
```

CI mirrors these gates and runs a Dioxus build matrix for web, Linux desktop, and Windows desktop in `.github/workflows/ci.yml`.

## Notes

This repo includes `.cargo/config.toml` to prevent host-only linker flags from leaking into WASM builds. Without it, user/global Cargo configs that pass ELF linker options can make `rust-lld` fail for `wasm32-unknown-unknown`.
