# GemEd

GemEd is being rewritten as a Rust + Dioxus visual workflow editor. The current runnable slice is a cross-platform Dioxus shell with a Rust workflow schema, validation, JSON import/export, storage, mock provider execution, and a basic editable node/edge canvas.

## Current Status

Implemented now:

- Rust workspace with `gemed_core`, `gemed_executor`, `gemed_providers`, `gemed_storage`, and `gemed_app` crates
- Dioxus app targeting web and desktop from one codebase
- Workflow v1 schema for nodes, edges, groups, node statuses, and edge style
- Typed node-handle metadata in `gemed_core` for canvas connection UI
- Graph traversal, connected-input resolution, and topological execution ordering
- Local simple executor for prompt, array, prompt-constructor, output, gallery, annotation, and control nodes
- Built-in sample workflow
- JSON editor that loads and validates workflow JSON
- SVG/HTML canvas rendering nodes and edges
- Basic Canvas MVP controls: select/multi-select/drag/nudge nodes, create groups from selection, grouped node backgrounds with lock/unlock, sidebar resize controls, direct canvas group resize handles, sidebar and direct wheel/blank-canvas pan/zoom/reset viewport controls, visual handle-to-handle connect, connect selected node to the next node, visual edge removal, undo, and redo
- Provider trait boundary with deterministic mock providers for LLM/image/video/audio/3D generation nodes
- Storage trait boundary with in-memory, browser localStorage, and desktop filesystem implementations
- Execution spine panel, Run Local action for pure-Rust workflow smoke runs, and Run Mocks action for provider-trait smoke runs without secrets
- Save Slot / Load Slot actions backed by platform storage (`localStorage` on web, app data JSON files on desktop)
- Web and Linux desktop build verification
- Windows desktop foundation through Dioxus desktop feature and Windows GUI subsystem setting

Not implemented yet:

- Full editable canvas UX: drag-to-create group selection boxes, group move handles, and polished drag/handle affordances
- Live Provider/API execution beyond mock providers
- Media/video/3D nodes
- Native file dialogs, explicit project picker/save-as flows, and external media storage

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

For release-style build artifacts:

```bash
dx build --web --no-default-features --features web
dx build --desktop --features desktop
```

On Windows, run the desktop build command from a Windows machine/runner.

## Validate

```bash
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets --features desktop -- -D warnings
cargo clippy --workspace --all-targets --no-default-features --features web -- -D warnings
```

CI mirrors these gates and runs a Dioxus build matrix for web, Linux desktop, and Windows desktop in `.github/workflows/ci.yml`.

## Notes

This repo includes `.cargo/config.toml` to prevent host-only linker flags from leaking into WASM builds. Without it, user/global Cargo configs that pass ELF linker options can make `rust-lld` fail for `wasm32-unknown-unknown`.
