# Web/browser media interaction smoke

This smoke verifies the Dioxus web target with real Chromium clicks. It covers the browser-side adapters used by the current Frame Sample and GLB viewer flows without adding Playwright or npm metadata to the repo.

It does **not** replace Linux desktop WebView or Windows WebView2 evidence. Use it as the reproducible web/browser check before doing native desktop runner validation.

## Start the deterministic web server

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


## CI smoke

`.github/workflows/ci.yml` includes a `Web interaction smoke` job that runs the same browser-side contract in CI without committing Playwright metadata to the repo:

1. Installs Rust, the WASM target, and the pinned Dioxus CLI.
2. Creates an isolated Playwright project under the runner temp directory.
3. Starts the deterministic Dioxus web server on `127.0.0.1:4564` with hot patching and hot reload disabled.
4. Waits until both `/` and `/vendor/model-viewer/4.3.1/model-viewer.min.js` return `200 OK`.
5. Clicks runnable execution paths in Chromium: starter `Run Local`, mock `Run Providers`, Frame Sample `Capture`, and Media Sample GLB `Capture PNG`.
6. Uploads the Dioxus server log and any Playwright evidence as the `gemed-web-interaction-smoke` artifact.

This CI job is web/Chromium evidence only. Native Linux desktop WebView and Windows WebView2 click evidence still require native desktop runs.

## Run isolated Playwright smoke

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

## Current local evidence

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
