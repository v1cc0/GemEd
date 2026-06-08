# GemEd Dioxus Rewrite Plan

Date: 2026-06-09

## Goal

Rewrite the current Next.js/React visual AI workflow editor as a Rust Dioxus application that ships as:

- Web app (`wasm32-unknown-unknown` + optional Rust backend for provider calls)
- Linux desktop app
- Windows desktop app

The rewrite should preserve workflow JSON compatibility first. UI polish and advanced media behavior come after the workflow model, graph execution, and save/load contracts are stable.

## Current Repository Snapshot

The current app is effectively a TypeScript/Next.js codebase with a placeholder Rust crate.

- Frontend: `src/app`, `src/components`, `src/hooks`, `src/store`, `src/types`
- API/server routes: `src/app/api/**`
- Workflow execution: `src/store/execution/**`
- Workflow state: `src/store/workflowStore.ts`
- Provider integrations: `src/app/api/generate/providers/**`, `src/app/api/llm/route.ts`, `src/app/api/models/**`
- Media/persistence helpers: `src/utils/mediaStorage.ts`, `src/app/api/workflow*`, `src/app/api/save-generation`, `src/app/api/load-generation`
- Existing Rust files: `Cargo.toml`, `src/main.rs` only

Primary node types to preserve:

- Inputs: image, audio, video, prompt, array
- Generation: image (`nanoBanana`), video, 3D, audio, LLM
- Processing: annotation, split grid, video stitch, ease curve, trim, frame grab, image compare, GLB viewer
- Flow/control: router, switch, conditional switch
- Outputs: output, output gallery

## Non-Negotiables

1. **Do not do a blind line-by-line port.** Preserve behavior by freezing data contracts, then rewrite around a smaller Rust domain model.
2. **Workflow files remain compatible.** Existing `/examples/*.json` must load in the new app, or migrate through an explicit versioned upgrader.
3. **Platform differences are explicit.** Web cannot pretend to have native filesystem access; desktop cannot assume browser-only APIs exist.
4. **Rust core owns data and execution.** Dioxus components render state; they do not become the business logic dumping ground.
5. **Keep escape hatches narrow.** Use JS interop only for browser-native hard parts such as canvas/WebGL/video APIs, and isolate it behind traits.

## Target Architecture

```text
GemEd/
  Cargo.toml                 # Rust workspace
  Dioxus.toml                # dx build/bundle config
  crates/
    gemed_core/              # workflow schema, graph model, validation, traversal, migrations
    gemed_executor/          # node execution scheduler, cancellation, batching, progress events
    gemed_providers/         # Gemini/OpenAI/Anthropic/Replicate/fal/Kie/WaveSpeed clients
    gemed_media/             # image/audio/video transforms and media references
    gemed_storage/           # project save/load, external media store, platform storage traits
    gemed_app/               # Dioxus UI: shell, canvas, nodes, dialogs
  src/main.rs                # thin launcher selecting Dioxus web/desktop feature
  public/                    # static assets, css, icons
  examples/                  # existing workflow fixtures retained
```

### Crate Boundaries

- `gemed_core`
  - `Workflow`, `WorkflowNode`, `WorkflowEdge`, `NodeType`, `NodeData`
  - JSON schema versioning and migrations
  - graph validation, topological ordering, connected-input resolution
  - cost model and model metadata types

- `gemed_executor`
  - dispatcher replacing `executeNode.ts`
  - batch execution replacing `batchExecution.ts`
  - cancellation and progress stream
  - deterministic scheduler with configurable concurrency

- `gemed_providers`
  - provider traits: `ImageProvider`, `VideoProvider`, `AudioProvider`, `LlmProvider`, `ModelCatalog`
  - direct desktop HTTP clients
  - web backend/server-function clients when secrets or CORS require server-side calls

- `gemed_storage`
  - desktop filesystem implementation
  - web browser storage implementation: localStorage/IndexedDB/file picker/download fallback
  - media references instead of always embedding large base64 blobs

- `gemed_media`
  - pure image operations where feasible
  - platform adapters for canvas/video APIs or sidecar tools
  - keep advanced video features behind capabilities so web/desktop degrade honestly

- `gemed_app`
  - Dioxus components and state signals
  - no provider-specific or filesystem-specific logic except calling service traits

## Platform Strategy

### Web

- Build to WASM using Dioxus web.
- For public/static deployments, avoid storing provider secrets in shipped assets.
- Use one of two modes:
  1. **Local BYOK browser mode:** only providers that work from browser CORS and user-local API keys.
  2. **Full web mode:** Dioxus fullstack/Axum backend exposes provider calls, model listing, logs, and secure secret handling.
- Replace native file path workflows with import/export/download and browser storage.

### Linux Desktop

- Dioxus desktop runs Rust natively and renders through the system WebView.
- Implement direct filesystem, native dialogs, provider HTTP calls, local logs, and optional sidecar binaries.
- Package on Linux CI/native Linux build runner; declare WebKitGTK/xdotool and related package dependencies in release docs/bundles.

### Windows Desktop

- Build and bundle on a Windows runner.
- Use `#![cfg_attr(feature = "bundle", windows_subsystem = "windows")]` for release bundles to avoid a console window.
- Account for WebView2 availability and installer mode.
- Use Windows-native file dialogs and app data directories.

## Migration Phases

### Phase 0 — Baseline Freeze

Deliverables:

- Add schema snapshots for every existing example workflow.
- Export representative saved workflows with embedded and externalized media references.
- Record current API route contract examples: request, response, errors.
- Run existing tests and keep their output as a baseline.

Done when:

- We can tell whether a new Rust implementation is compatible without eyeballing UI behavior.

### Phase 1 — Workspace Scaffold

Deliverables:

- Convert root `Cargo.toml` into a workspace.
- Add `Dioxus.toml` with web and bundle metadata.
- Add `src/main.rs` as a thin `dioxus::launch(App)` launcher.
- Add platform features: `web`, `desktop`, `bundle`.
- Add CI jobs:
  - `cargo fmt --check`
  - `cargo clippy --all-targets --all-features`
  - `cargo test --workspace`
  - web build
  - Linux desktop bundle
  - Windows desktop bundle

Done when:

- Empty GemEd Dioxus shell builds for web and desktop.

### Phase 2 — Port Data Model and Pure Logic

Port first:

- `src/types/nodes.ts`
- `src/types/workflow.ts`
- `src/types/providers.ts`
- `src/utils/pathValidation.ts`
- `src/utils/urlValidation.ts`
- `src/utils/arrayParser.ts`
- `src/utils/costCalculator.ts`
- `src/utils/spatialLayout.ts`
- `src/lib/quickstart/validation.ts`

Rust dependencies likely needed:

- `serde`, `serde_json`
- `schemars` or equivalent schema generation
- `thiserror`
- `uuid`
- `indexmap`
- `petgraph` or a small custom DAG traversal if graph logic stays simple

Done when:

- All existing example workflow JSON files deserialize, validate, serialize, and round-trip with stable output or explicit migrations.

### Phase 3 — Dioxus UI Shell

Deliverables:

- App frame: header, side panel, canvas area, settings dialogs, toast/log panel.
- Central app state using Dioxus signals/context.
- Import/export workflow JSON.
- Read-only rendering of nodes and edges.

Done when:

- Existing workflows can be opened and visually inspected in web and desktop builds.

### Phase 4 — Canvas MVP

Rewrite React Flow behavior as GemEd-owned canvas logic:

- Pan/zoom
- Drag nodes
- Select/multi-select
- Connect/disconnect handles
- Edge labels/options: pause, loop count, edge style
- Group rectangles and group lock behavior
- Undo/redo stack

Implementation preference:

- HTML/SVG canvas first for simplicity and portability.
- Move to `<canvas>`/WebGL only if SVG performance is proven insufficient.

Done when:

- A user can create, edit, connect, save, reload, undo, and redo a workflow without execution.

### Phase 5 — Execution Engine

Deliverables:

- Rust execution context equivalent to `NodeExecutionContext`.
- Dispatcher equivalent to `executeNode.ts`.
- Batch execution equivalent to `batchExecution.ts`.
- Scheduler with dependency ordering, parallelism limit, pause/stop/cancel.
- Status propagation: idle/loading/complete/error/skipped.
- Output routing for router/switch/conditional switch.

Start with simple nodes:

- prompt
- array
- prompt constructor
- output/gallery
- image compare metadata
- router/switch/conditional switch

Then add generation nodes behind provider traits.

Done when:

- A non-provider workflow executes fully in Rust with deterministic tests.

### Phase 6 — Provider Integrations

Port providers in this order:

1. LLM text generation: Gemini/OpenAI/Anthropic-compatible paths
2. Image generation: Gemini, Replicate, fal, Kie, WaveSpeed
3. Video generation/polling
4. Audio generation
5. 3D generation/model fetches

Rules:

- Use typed request/response structs; avoid untyped JSON blobs except at provider boundary.
- Keep retry/polling/cancellation common.
- Keep provider API keys in target-specific secure-ish storage:
  - desktop: OS app config/keyring if practical, otherwise app config file with clear warning
  - web backend: server environment
  - browser-only mode: user-local storage only if user opts in

Done when:

- Current provider route test fixtures pass against Rust clients/mocks.

### Phase 7 — Media and Advanced Nodes

Port in increasing difficulty:

1. Image reference/externalized media storage
2. Annotation drawing
3. Split grid
4. Audio/video input preview
5. Video trim/frame grab/stitch
6. GLB viewer and capture

Strategy:

- Use Rust for data transforms and metadata.
- Use web APIs through isolated adapters for canvas/video/WebGL behavior.
- For desktop, prefer WebView-compatible implementations for UI preview, but native Rust/sidecar implementation for heavy processing if needed.

Done when:

- Media workflows can run on at least desktop Linux/Windows and web capability gaps are documented in-app.

### Phase 8 — Packaging and Release

Deliverables:

- `dx bundle --release` artifacts for Linux and Windows.
- Web build artifact under `dist/` or configured deploy dir.
- Icons for PNG/ICO and bundle metadata.
- Release smoke tests:
  - launch app
  - open example workflow
  - create and save workflow
  - run simple no-provider workflow
  - run one mocked provider workflow

Done when:

- A clean machine/runner can build all requested targets from documented commands.

### Phase 9 — Cutover

Deliverables:

- Update README and quick start commands from npm/Next.js to Rust/Dioxus.
- Keep old Next.js implementation in a temporary `legacy-next/` branch or archive tag, not mixed into the active source tree.
- Delete dead TS/Next files only after parity gates pass.

Done when:

- Default branch contains the Dioxus app as the active app and no dead Next.js build path remains.

## Compatibility Gates

Do not delete the old app until these pass:

- Existing workflow JSON loads.
- Existing examples round-trip.
- Core node types can be created and connected.
- Save/load works in web and desktop with platform-appropriate behavior.
- Execution works for non-provider workflows.
- At least one LLM provider and one image provider work behind mocks and live opt-in tests.
- Linux and Windows desktop bundles launch.

## Major Risks

- React Flow replacement is the largest UI risk. Own the graph model first; canvas rendering comes second.
- Browser vs desktop API mismatch will bite if hidden. Use platform capability traits from day one.
- Provider secrets on web require a backend story. Do not fake security by embedding secrets in WASM/static assets.
- Video/3D behavior currently depends on JS/browser libraries. Isolate these as adapters before attempting pure Rust rewrites.
- Cross-compiling desktop WebView apps is less reliable than native build runners. Plan CI with Linux and Windows runners.

## First Implementation Slice

The first real coding slice should be deliberately small:

1. Scaffold Dioxus workspace.
2. Define Rust `Workflow`, `NodeType`, `WorkflowNode`, `WorkflowEdge`.
3. Load `/examples/*.json` into `gemed_core` tests.
4. Render a read-only workflow in Dioxus web and desktop.
5. Add JSON import/export.

This gives a working spine. Everything else hangs off that spine.
