# GemEd

GemEd is being rewritten as a Rust + Dioxus visual workflow editor. The current runnable slice is a cross-platform Dioxus shell with a Rust workflow schema, validation, JSON import/export, and a read-only node/edge canvas.

## Current Status

Implemented now:

- Rust workspace with `gemed_core`, `gemed_executor`, and `gemed_app` crates
- Dioxus app targeting web and desktop from one codebase
- Workflow v1 schema for nodes, edges, groups, node statuses, and edge style
- Graph traversal, connected-input resolution, and topological execution ordering
- Local simple executor for prompt, array, prompt-constructor, output, gallery, annotation, and control nodes
- Built-in sample workflow
- JSON editor that loads and validates workflow JSON
- Read-only SVG/HTML canvas rendering nodes and edges
- Execution spine panel and Run Local action for pure-Rust workflow smoke runs
- Web and Linux desktop build verification
- Windows desktop foundation through Dioxus desktop feature and Windows GUI subsystem setting

Not implemented yet:

- Editable drag/connect canvas
- Provider/API execution engine beyond explicit local skips
- Media/video/3D nodes
- Native file dialogs and persistent project storage

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

## Notes

This repo includes `.cargo/config.toml` to prevent host-only linker flags from leaking into WASM builds. Without it, user/global Cargo configs that pass ELF linker options can make `rust-lld` fail for `wasm32-unknown-unknown`.
