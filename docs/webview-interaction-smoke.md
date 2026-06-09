# WebView and browser media interaction smoke

This document records three adapter smokes:

- Web target: real Chromium clicks against `dx serve --web`, with Playwright kept outside the repo.
- Linux desktop target: a native Dioxus Desktop/WebKitGTK self-smoke triggered by `GEMED_DESKTOP_SELF_SMOKE=1`.
- Windows desktop target: an opt-in GitHub Actions Dioxus Desktop/WebView2 self-smoke triggered by `windows_webview_smoke=true`.

These checks cover the browser/WebView adapters used by the current Frame Sample and GLB viewer flows. The Windows CI self-smoke proves the real WebView2 adapter boundary for the current app; a physical Windows machine is still useful for final bundled-installer human click testing.

## Web target: start the deterministic server

Use the hot-patch-free command recorded in `INCIDENTS.md`:

```bash
rtk dx serve --web --no-default-features --features web --open false --port 4563 --addr 127.0.0.1 --watch false --hot-patch false --hot-reload false
```

Check the shell and vendored model-viewer asset:

```bash
rtk curl -I http://127.0.0.1:4563/
rtk curl -I http://127.0.0.1:4563/vendor/model-viewer/4.3.1/model-viewer.min.js
```

Both should return `HTTP/1.1 200 OK`.


## Web target: CI smoke

`.github/workflows/ci.yml` includes a `Web interaction smoke` job that runs the same browser-side contract in CI without committing Playwright metadata to the repo:

1. Installs Rust, the WASM target, and the pinned Dioxus CLI.
2. Creates an isolated Playwright project under the runner temp directory.
3. Starts the deterministic Dioxus web server on `127.0.0.1:4564` with hot patching and hot reload disabled.
4. Waits until both `/` and `/vendor/model-viewer/4.3.1/model-viewer.min.js` return `200 OK`.
5. Clicks runnable execution paths in Chromium: starter `Run Local`, mock `Run Providers`, Frame Sample `Capture`, and Media Sample GLB `Capture PNG`.
6. Uploads the Dioxus server log and any Playwright evidence as the `gemed-web-interaction-smoke` artifact.

This CI job is web/Chromium evidence only. Native Linux desktop WebView evidence is covered separately by the self-smoke below; Windows WebView2 evidence is covered by the opt-in Windows self-smoke documented in `docs/windows-desktop-verification.md`.

## Web target: run isolated Playwright smoke

Create the temporary harness outside the repo so the legacy Next.js `package.json` and lockfile stay untouched:

```bash
rtk mkdir -p /tmp/gemed-pw-smoke
cd /tmp/gemed-pw-smoke
rtk npm init -y
rtk npm install @playwright/test@1.60.0
rtk npx playwright install chromium
```

Write this spec as `/tmp/gemed-pw-smoke/capture-click.spec.ts`:

```ts
import { test, expect } from '@playwright/test';

test('Run Local streams execution events into the Execution Spine', async ({ page }) => {
  test.setTimeout(60000);
  await page.goto('http://127.0.0.1:4563/');
  await page.getByRole('button', { name: 'Sample', exact: true }).click();
  await page.getByRole('button', { name: 'Run Local' }).click();
  await expect(page.getByText(/Local executor finished: 2 complete, 1 skipped, 0 errors/)).toBeVisible({ timeout: 45000 });
  await expect(page.getByText('Last run: 2 complete, 1 skipped, 0 errors')).toBeVisible();
  await expect(page.locator('.execution-log .event').filter({ hasText: 'loading' })).toHaveCount(3);
  await expect(page.locator('.execution-log .event').filter({ hasText: 'complete' })).toHaveCount(2);
  await expect(page.locator('.execution-log .event').filter({ hasText: 'skipped' })).toHaveCount(1);
});

test('Mock provider run streams provider execution events and outputs', async ({ page }) => {
  test.setTimeout(60000);
  await page.goto('http://127.0.0.1:4563/');
  await page.getByRole('button', { name: 'Provider Sample' }).click();
  await page.getByRole('button', { name: 'Mock Defaults' }).click();
  await page.getByRole('button', { name: 'Run Providers' }).click();
  await expect(page.getByText(/Provider run finished: 7 complete, 0 skipped, 0 errors/)).toBeVisible({ timeout: 45000 });
  await expect(page.getByText('Last run: 7 complete, 0 skipped, 0 errors')).toBeVisible();
  await expect(page.locator('.execution-log .event').filter({ hasText: 'loading' })).toHaveCount(7);
  await expect(page.locator('.execution-log .event').filter({ hasText: 'complete' })).toHaveCount(7);
  await expect(page.locator('article', { hasText: 'provider_gemini_output' }).getByText(/\[mock:gemini:gemini-3\.5-flash\]/)).toBeVisible();
  await expect(page.locator('article', { hasText: 'provider_openai_output' }).getByText(/\[mock:openai:gpt-5\.5\]/)).toBeVisible();
  await expect(page.locator('article', { hasText: 'provider_anthropic_output' }).getByText(/\[mock:anthropic:claude-sonnet-4-6\]/)).toBeVisible();
});

test('Frame Sample capture button emits a PNG in Chromium', async ({ page }) => {
  test.setTimeout(60000);
  await page.goto('http://127.0.0.1:4563/');
  await page.getByRole('button', { name: 'Frame Sample' }).click();
  await page.getByRole('button', { name: 'Run Local' }).click();
  await page.locator('article', { hasText: 'frame_grab' }).click({ position: { x: 80, y: 20 } });
  await page.getByRole('button', { name: 'Capture', exact: true }).click();
  await expect(page.getByText(/Captured frame for `frame_grab`/)).toBeVisible({ timeout: 45000 });
  await expect(page.locator('article', { hasText: 'frame_grab' }).getByText(/webview-video-canvas emitted PNG output/)).toBeVisible();
});

test('GLB Capture PNG button emits a PNG in Chromium', async ({ page }) => {
  test.setTimeout(90000);
  await page.goto('http://127.0.0.1:4563/');
  await page.getByRole('button', { name: 'Media Sample' }).click();
  await page.getByRole('button', { name: 'Run Local' }).click();
  await page.locator('article', { hasText: 'media_glb' }).click({ position: { x: 80, y: 20 } });
  await page.getByRole('button', { name: 'Capture PNG' }).click();
  await expect(page.getByText(/Captured GLB snapshot for `media_glb`/)).toBeVisible({ timeout: 75000 });
  await expect(page.locator('article', { hasText: 'media_glb' }).getByText(/webview-model-viewer emitted PNG snapshot/)).toBeVisible();
});
```

Run it:

```bash
rtk npx playwright test capture-click.spec.ts --reporter=line --browser=chromium
```

Expected result:

```text
4 passed
```

## Web target: current local evidence

On 2026-06-09, from this repo state:

- `rtk dx serve --web --no-default-features --features web --open false --port 4590 --addr 127.0.0.1 --watch false --hot-patch false --hot-reload false` built and served the app.
- `rtk curl -I http://127.0.0.1:4590/` returned `HTTP/1.1 200 OK`.
- `rtk curl -I http://127.0.0.1:4590/vendor/model-viewer/4.3.1/model-viewer.min.js` returned `HTTP/1.1 200 OK` with `content-type: text/javascript`.
- The isolated Playwright harness under `/tmp/gemed-pw-smoke` reused Chromium from the current local Playwright install.
- `rtk npx playwright test capture-click.spec.ts --reporter=line --browser=chromium` passed: `PASS (4) FAIL (0)`.

Validated flows:

1. `Sample` → `Run Local` renders Execution Spine loading/complete/skipped events and final summary.
2. `Provider Sample` → `Mock Defaults` → `Run Providers` renders provider progress events and mock provider output text.
3. `Frame Sample` → `Run Local` → select `frame_grab` → `Capture` produced a PNG and updated the node insight with `webview-video-canvas emitted PNG output`.
4. `Media Sample` → `Run Local` → select `media_glb` → `Capture PNG` produced a PNG snapshot and updated the node insight with `webview-model-viewer emitted PNG snapshot`.

Stop the Dioxus server by explicit PID after the smoke:

```bash
rtk pgrep -af "dx serve.*4563"
rtk kill <pid>
```


## Linux desktop target: WebKitGTK self-smoke

For native Linux desktop WebView validation, run the app binary with the explicit self-smoke environment variable:

```bash
rtk timeout 180s env GEMED_DESKTOP_SELF_SMOKE=1 cargo run --features desktop
```

The normal app does not run this path. With `GEMED_DESKTOP_SELF_SMOKE=1`, the Dioxus Desktop app launches through the real system WebKitGTK WebView, then performs the same adapter boundary work that the UI buttons perform:

1. Builds `Frame Sample`, runs the Rust local executor to record `frameGrabPlan`, evaluates the WebView video/canvas capture script, verifies a PNG data URL, and routes it to the downstream output node.
2. Builds `Media Sample`, runs the Rust local executor to record `glbViewerPlan`, evaluates the WebView `model-viewer` snapshot script with the vendored model-viewer module fallback chain, and verifies a PNG data URL.
3. Prints a `PASS` or `FAIL` line and exits with the matching process status.

Expected decisive output:

```text
[gemed-desktop-self-smoke] START env=GEMED_DESKTOP_SELF_SMOKE=1 target=desktop-webview
[gemed-desktop-self-smoke] PASS Frame Sample capture PASS 16×16, routed 1; Media Sample GLB capture PASS 640×480, routed 1.
```

This is native Linux WebKitGTK evidence, not Windows WebView2 evidence. It also intentionally bypasses external Wayland/X11 click automation: on native Wayland compositors, `xdotool` may not see WebKitGTK windows at all, while the self-smoke still exercises the real WebView JavaScript/runtime boundary.

### Current Linux desktop evidence

On 2026-06-09, from this repo state:

- Environment: Wayland/Niri session with `DISPLAY=:0`, `WAYLAND_DISPLAY=wayland-1`, `XDG_SESSION_TYPE=wayland`.
- `rtk dx serve --desktop --features desktop` launched a native `gemed-*` app process and WebKitGTK `WebKitNetworkProcess` / `WebKitWebProcess`; Niri reported a real `GemEd` window with app ID `gemed-5175b724`.
- `xdotool search --name GemEd` did not see the native Wayland window, so external click automation was not reliable evidence on this host.
- `rtk timeout 180s env GEMED_DESKTOP_SELF_SMOKE=1 cargo run --features desktop` exited `0` with:
  - `Frame Sample capture PASS 16×16, routed 1`
  - `Media Sample GLB capture PASS 640×480, routed 1`


## Windows desktop target: WebView2 self-smoke

For native Windows desktop WebView validation, trigger the manual GitHub Actions workflow with the opt-in input:

```bash
rtk gh workflow run 292131199 --ref main -f windows_webview_smoke=true
```

The job runs on `windows-latest`:

```powershell
$env:GEMED_DESKTOP_SELF_SMOKE = "1"
cargo run --features desktop
```

Expected decisive output:

```text
[gemed-desktop-self-smoke] START env=GEMED_DESKTOP_SELF_SMOKE=1 target=desktop-webview
[gemed-desktop-self-smoke] PASS Frame Sample capture PASS 16×16, routed 1; Media Sample GLB capture PASS 640×480, routed 1.
```

### Current Windows desktop evidence

On 2026-06-09, workflow run <https://github.com/v1cc0/GemEd/actions/runs/27221485574> at commit `c0ded944d693f1f9889340f0c63c1f3a380a4f69` completed job `Windows desktop WebView2 self-smoke` successfully:

- Job URL: <https://github.com/v1cc0/GemEd/actions/runs/27221485574/job/80377184715>
- Artifact: `gemed-windows-webview2-self-smoke`
- Tool evidence: `rustc 1.96.0 (ac68faa20 2026-05-25)`, host `x86_64-pc-windows-msvc`, `cargo 1.96.0`
- PASS marker:
  - `Frame Sample capture PASS 16×16, routed 1`
  - `Media Sample GLB capture PASS 640×480, routed 1`
