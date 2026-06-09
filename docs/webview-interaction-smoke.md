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
2 passed
```

## Current local evidence

On 2026-06-09, from this repo state:

- `rtk dx serve --web --no-default-features --features web --open false --port 4563 --addr 127.0.0.1 --watch false --hot-patch false --hot-reload false` built and served the app.
- `rtk curl -I http://127.0.0.1:4563/` returned `HTTP/1.1 200 OK`.
- `rtk curl -I http://127.0.0.1:4563/vendor/model-viewer/4.3.1/model-viewer.min.js` returned `HTTP/1.1 200 OK` with `content-type: text/javascript`.
- `rtk npx --yes playwright@1.60.0 install chromium` installed Chromium v1223 / Chrome for Testing `148.0.7778.96`.
- `rtk npx playwright test capture-click.spec.ts --reporter=line --browser=chromium` passed: `PASS (2) FAIL (0)` / `2 passed (5.2s)`.

Validated flows:

1. `Frame Sample` → `Run Local` → select `frame_grab` → `Capture` produced a PNG and updated the node insight with `webview-video-canvas emitted PNG output`.
2. `Media Sample` → `Run Local` → select `media_glb` → `Capture PNG` produced a PNG snapshot and updated the node insight with `webview-model-viewer emitted PNG snapshot`.

Stop the Dioxus server by explicit PID after the smoke:

```bash
rtk pgrep -af "dx serve.*4563"
rtk kill <pid>
```
