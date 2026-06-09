# Windows desktop verification

This document is the evidence checklist for the Windows desktop target. Treat CI build/package verification and interactive WebView2 smoke as separate gates; do not mark interactive Windows desktop behavior as verified until a Windows machine or UI runner launches the app and completes the local smoke below.

## CI evidence

The GitHub Actions Dioxus matrix includes both Windows build and bundle jobs on `windows-latest`.
It runs on `push` to `main`, pull requests, and manual `workflow_dispatch` runs so Windows evidence can be collected without changing source code:

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
| Commit SHA | `2181eb925e1c642d2dd606dac2e21b9c4997642f` |
| GitHub Actions run URL | <https://github.com/v1cc0/GemEd/actions/runs/27216197199> |
| Matrix job | `Windows desktop build` |
| Job URL | <https://github.com/v1cc0/GemEd/actions/runs/27216197199/job/80358232720> |
| Artifact | `gemed-windows-desktop-dx-build` / artifact ID `7512505812` |
| Result | success |
| Notes | Windows runner `dx build --desktop --features desktop` completed successfully. Downloaded artifact contains `target_dx_gemed_debug/windows/app/gemed.exe` (`32872448` bytes), plus `rustc-version.txt`, `cargo-version.txt`, `dioxus-version.txt`, and a manifest. Tool evidence: `rustc 1.96.0 (ac68faa20 2026-05-25)`, host `x86_64-pc-windows-msvc`, `cargo 1.96.0`, `dioxus 0.8.0-alpha.0 (a82361e)`. |

Record the Windows providers-http build result here when it exists:

| Field | Value |
| --- | --- |
| Commit SHA | `2181eb925e1c642d2dd606dac2e21b9c4997642f` |
| GitHub Actions run URL | <https://github.com/v1cc0/GemEd/actions/runs/27216197199> |
| Matrix job | `Windows desktop providers-http build` |
| Job URL | <https://github.com/v1cc0/GemEd/actions/runs/27216197199/job/80358232747> |
| Artifact | `gemed-windows-desktop-providers-http-dx-build` / artifact ID `7512537555` |
| Result | success |
| Notes | Windows runner `dx build --desktop --features desktop,providers-http` completed successfully. Downloaded artifact contains `target_dx_gemed_debug/windows/app/gemed.exe` (`36916224` bytes), plus tool-version files and a manifest. Tool evidence: `rustc 1.96.0 (ac68faa20 2026-05-25)`, host `x86_64-pc-windows-msvc`, `cargo 1.96.0`, `dioxus 0.8.0-alpha.0 (a82361e)`. |

Record the Windows bundle result here when it exists:

| Field | Value |
| --- | --- |
| Commit SHA | `2181eb925e1c642d2dd606dac2e21b9c4997642f` |
| GitHub Actions run URL | <https://github.com/v1cc0/GemEd/actions/runs/27216197199> |
| Matrix job | `Windows desktop bundle` |
| Job URL | <https://github.com/v1cc0/GemEd/actions/runs/27216197199/job/80358232543> |
| Artifact | `gemed-windows-desktop-bundle-dx-build` / artifact ID `7512528548` |
| Installer | `target_dx_gemed_bundle/windows/msi/Gemed_0.1.0_x64.msi` (`9879552` bytes) |
| Result | success |
| Notes | Windows runner `dx bundle --desktop --features desktop,bundle --package-types msi` completed successfully. Downloaded artifact contains bundled `target_dx_gemed_bundle/windows/gemed.exe` (`32872448` bytes), `Gemed_0.1.0_x64.msi`, `Gemed.wxs`, `Gemed.wixobj`, `Gemed_0.1.0_x64.wixpdb`, staging `gemed.exe`, tool-version files, and bundle manifest. |

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

Windows CI build and MSI bundle evidence is now filled from a real `windows-latest` runner. Windows is still a foundation target rather than full interactive verification until a local Windows machine or Windows UI runner launches the app and completes the WebView2 smoke checklist above. Linux desktop builds, Linux `.deb` bundle, web build, Chromium web interaction smoke, and native Linux WebKitGTK Frame/GLB adapter self-smoke are verified; the Linux self-smoke does not prove Windows WebView2 behavior.
