# Provider live smoke verification

This repo keeps provider secrets out of workflow JSON and provider settings. Live LLM calls are an explicit desktop/http opt-in, while normal web/desktop builds and CI stay offline.

## Offline app smoke

Use this first because it is deterministic and needs no credentials:

1. Launch the app with desktop features.
2. Click `Provider Sample`.
3. In `Provider Settings`, click `Mock Defaults`.
4. Click `Run Providers`.
5. Verify the Gemini, OpenAI, and Anthropic output nodes receive mock text.

## Live provider fixture

The provider crate includes a small live smoke example that calls the same Rust HTTP backends used by the Dioxus desktop app:

```bash
cargo run -p gemed_providers --features http --example live_llm_smoke -- plan
```

`plan` performs no network requests; it only reports whether the expected secret env vars are present and whether model/base URL overrides are configured.

```bash
cargo run -p gemed_providers --features http --example live_llm_smoke -- all
```

`all` skips providers whose secret env var is absent and fails if none are configured. To require one provider, pass `gemini`, `openai`, or `anthropic`:

```bash
GEMINI_API_KEY=... cargo run -p gemed_providers --features http --example live_llm_smoke -- gemini
OPENAI_API_KEY=... cargo run -p gemed_providers --features http --example live_llm_smoke -- openai
ANTHROPIC_API_KEY=... cargo run -p gemed_providers --features http --example live_llm_smoke -- anthropic
```

Optional override env vars:

| Provider | Secret env | Model override | Base URL override |
| --- | --- | --- | --- |
| Gemini | `GEMINI_API_KEY` | `GEMED_LIVE_GEMINI_MODEL` | `GEMED_LIVE_GEMINI_BASE_URL` |
| OpenAI | `OPENAI_API_KEY` | `GEMED_LIVE_OPENAI_MODEL` | `GEMED_LIVE_OPENAI_BASE_URL` |
| Anthropic | `ANTHROPIC_API_KEY` | `GEMED_LIVE_ANTHROPIC_MODEL` | `GEMED_LIVE_ANTHROPIC_BASE_URL` |

Set `GEMED_LIVE_PROMPT` to override the default short prompt. The fixture prints provider name, model, response length, and a compact response preview; it does not print API keys.

## Live desktop app smoke

```bash
GEMINI_API_KEY=... OPENAI_API_KEY=... ANTHROPIC_API_KEY=... \
  dx serve --desktop --features desktop,providers-http
```

Then:

1. Click `Provider Sample`.
2. In `Provider Settings`, set the desired providers to `Env`.
3. Optionally edit model/base URL fields if your account or proxy uses different model names/endpoints.
4. Click `Run Providers`.
5. Verify provider output nodes receive non-empty text and provider node cards show `__providerUsed`/`__modelUsed` metadata in JSON.

If a live call fails, first verify the standalone fixture above. It has a narrower surface area than the Dioxus UI and makes provider/backend failures easier to isolate.
