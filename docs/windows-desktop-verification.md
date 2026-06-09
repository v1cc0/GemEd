# Windows desktop verification

This document is the evidence checklist for the Windows desktop target. Do not mark Windows desktop as verified until a real Windows runner or Windows machine produces the evidence below.

## CI evidence

The GitHub Actions Dioxus matrix includes both Windows build and bundle jobs on `windows-latest`:

```bash
dx build --desktop --features desktop
dx build --desktop --features desktop,providers-http
dx bundle --desktop --features desktop,bundle --package-types msi
```

Each Dioxus matrix job uploads an artifact named from its matrix target:

- `gemed-web-dx-build`
- `gemed-linux-desktop-dx-build`
- `gemed-linux-desktop-providers-http-dx-build`
- `gemed-linux-desktop-bundle-dx-build`
- `gemed-windows-desktop-dx-build`
- `gemed-windows-desktop-providers-http-dx-build`
- `gemed-windows-desktop-bundle-dx-build`

The artifact contains:

- `rustc-version.txt`
- `cargo-version.txt`
- `dioxus-version.txt`
- copied Dioxus build output if the runner produced one
- copied Dioxus bundle output for bundle jobs, including the generated installer if present
- a manifest of collected files, or `no-dioxus-output-found.txt` if the output path changed

Record the Windows build result here when it exists:

| Field | Value |
| --- | --- |
| Commit SHA | pending |
| GitHub Actions run URL | pending |
| Matrix job | `Windows desktop build` |
| Artifact | `gemed-windows-desktop-dx-build` |
| Result | pending |
| Notes | pending |

Record the Windows providers-http build result here when it exists:

| Field | Value |
| --- | --- |
| Commit SHA | pending |
| GitHub Actions run URL | pending |
| Matrix job | `Windows desktop providers-http build` |
| Artifact | `gemed-windows-desktop-providers-http-dx-build` |
| Result | pending |
| Notes | pending |

Record the Windows bundle result here when it exists:

| Field | Value |
| --- | --- |
| Commit SHA | pending |
| GitHub Actions run URL | pending |
| Matrix job | `Windows desktop bundle` |
| Artifact | `gemed-windows-desktop-bundle-dx-build` |
| Installer | pending `.msi` |
| Result | pending |
| Notes | pending |

## Local Windows smoke

Run from a Windows shell with Rust and Dioxus CLI installed:

```powershell
rustup toolchain install stable --profile minimal
rustup default stable
cargo install dioxus-cli --version 0.8.0-alpha.0 --locked

cargo test --workspace
cargo clippy --workspace --all-targets --features desktop -- -D warnings
dx build --desktop --features desktop
dx build --desktop --features desktop,providers-http
dx bundle --desktop --features desktop,bundle --package-types msi
```

Then launch the built app or `dx serve --desktop --features desktop` and verify:

1. The window opens without a console window in release/bundle mode.
2. `Provider Sample` + `Mock Defaults` + `Run Providers` produces mock Gemini/OpenAI/Anthropic text.
3. `Frame Sample` + `Run Local` records `frameGrabPlan`; selecting the frame grab node exposes the `Capture` action for renderable sources.
4. `Media Sample` + `Run Local` records `glbViewerPlan`; selecting the GLB Viewer exposes `Capture PNG` for renderable GLB sources, and successful capture routes a PNG snapshot through explicit image/snapshot handles.
5. `Transform Sample` + `Run Local` populates split-grid cells and generated child navigation still works.
6. Save/load workflow JSON works.

## Current status

Windows is still a foundation target until the CI or local Windows evidence tables above are filled. Linux desktop, web builds, and a local Linux `.deb` bundle are verified in the normal development loop; Windows requires the actual Windows runner because WebView2, path handling, installer tooling, and Dioxus desktop packaging are platform-specific.
