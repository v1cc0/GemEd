# Windows desktop verification

This document is the evidence checklist for the Windows desktop target. Treat CI build/package verification, the automated WebView2 adapter self-smoke, and final physical-machine bundled-app click testing as separate gates.

## CI evidence

The GitHub Actions Dioxus matrix includes both Windows build and bundle jobs on `windows-latest`.
It runs on `push` to `main`, pull requests, and manual `workflow_dispatch` runs so Windows build/package evidence can be collected without changing source code:

```bash
dx build --desktop --features desktop
dx build --desktop --features desktop,providers-http
dx bundle --desktop --features desktop,bundle --package-types msi
```

The workflow also has an opt-in manual `windows_webview_smoke` input. When that input is true, the `Windows desktop WebView2 self-smoke` job runs:

```powershell
$env:GEMED_DESKTOP_SELF_SMOKE = "1"
cargo run --features desktop
```

That self-smoke launches the real Dioxus Desktop app on the Windows runner, exercises the Frame Sample video/canvas capture adapter and Media Sample GLB model-viewer capture adapter through WebView2, checks for the `[gemed-desktop-self-smoke] PASS` marker, and uploads `gemed-windows-webview2-self-smoke`. Keep this separate from the normal matrix because GUI/WebView runtime behavior can be runner-dependent.

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


Record the Windows WebView2 self-smoke result here when it exists:

| Field | Value |
| --- | --- |
| Commit SHA | `c0ded944d693f1f9889340f0c63c1f3a380a4f69` |
| GitHub Actions run URL | <https://github.com/v1cc0/GemEd/actions/runs/27221485574> |
| Job | `Windows desktop WebView2 self-smoke` |
| Job URL | <https://github.com/v1cc0/GemEd/actions/runs/27221485574/job/80377184715> |
| Artifact | `gemed-windows-webview2-self-smoke` |
| Result | success |
| Notes | Manual `workflow_dispatch` run with `windows_webview_smoke=true` launched the real Dioxus Desktop app through WebView2 and completed `GEMED_DESKTOP_SELF_SMOKE=1 cargo run --features desktop`. Artifact log contains `[gemed-desktop-self-smoke] PASS Frame Sample capture PASS 16×16, routed 1; Media Sample GLB capture PASS 640×480, routed 1.` Tool evidence: `rustc 1.96.0 (ac68faa20 2026-05-25)`, host `x86_64-pc-windows-msvc`, `cargo 1.96.0`. |

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

Windows CI build, `providers-http` build, MSI bundle, and automated WebView2 Frame/GLB adapter self-smoke evidence are now filled from real `windows-latest` runners. Windows remains a foundation target rather than fully polished release target until a physical Windows machine verifies the bundled installer launch and human click checklist above. Linux desktop builds, Linux `.deb` bundle, web build, Chromium web interaction smoke, native Linux WebKitGTK Frame/GLB adapter self-smoke, and Windows WebView2 Frame/GLB adapter self-smoke are verified.
