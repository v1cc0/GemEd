#[cfg(not(feature = "http"))]
fn main() {
    eprintln!(
        "GemEd live LLM smoke requires the `http` feature. Run with: \
         cargo run -p gemed_providers --features http --example live_llm_smoke -- <plan|all|gemini|openai|anthropic>"
    );
    std::process::exit(2);
}

#[cfg(feature = "http")]
fn main() {
    if let Err(err) = live::run() {
        eprintln!("GemEd live LLM smoke failed: {err}");
        std::process::exit(1);
    }
}

#[cfg(feature = "http")]
mod live {
    use gemed_providers::{
        AnthropicMessagesProvider, GeminiGenerateContentProvider, LlmProvider, LlmRequest,
        OpenAiResponsesProvider, ProviderId,
    };
    use serde_json::Value;

    const PROMPT_ENV: &str = "GEMED_LIVE_PROMPT";

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Target {
        Gemini,
        OpenAi,
        Anthropic,
    }

    struct Selection {
        targets: Vec<Target>,
        skip_missing_secrets: bool,
    }

    enum Command {
        Plan,
        Run(Selection),
    }

    pub fn run() -> Result<(), String> {
        let Command::Run(selection) = parse_command()? else {
            print_plan();
            return Ok(());
        };
        let prompt = env_trimmed(PROMPT_ENV).unwrap_or_else(|| {
            "Reply with one short sentence containing the words GemEd live smoke.".to_string()
        });
        let mut attempted = 0usize;
        let mut failures = Vec::new();

        for target in selection.targets {
            let Some(api_key) = env_trimmed(target.secret_env()) else {
                let message = format!(
                    "{} missing {}; export it before running this smoke.",
                    target.name(),
                    target.secret_env()
                );
                if selection.skip_missing_secrets {
                    eprintln!("skip: {message}");
                    continue;
                }
                return Err(message);
            };

            attempted += 1;
            if let Err(err) = run_target(target, api_key, &prompt) {
                failures.push(format!("{}: {err}", target.name()));
            }
        }

        if attempted == 0 {
            return Err(
                "no live provider secrets were configured; set at least one provider API-key env var"
                    .to_string(),
            );
        }
        if !failures.is_empty() {
            return Err(failures.join("; "));
        }
        Ok(())
    }

    fn parse_command() -> Result<Command, String> {
        let raw = std::env::args().nth(1).unwrap_or_else(|| "all".to_string());
        let raw = raw.trim().to_ascii_lowercase();
        if raw == "plan" || raw == "--plan" || raw == "dry-run" || raw == "--dry-run" {
            return Ok(Command::Plan);
        }
        if raw.is_empty() || raw == "all" {
            return Ok(Command::Run(Selection {
                targets: vec![Target::Gemini, Target::OpenAi, Target::Anthropic],
                skip_missing_secrets: true,
            }));
        }

        let mut targets = Vec::new();
        for part in raw
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
        {
            let target = match part {
                "gemini" | "google" => Target::Gemini,
                "openai" | "open-ai" => Target::OpenAi,
                "anthropic" | "claude" => Target::Anthropic,
                other => {
                    return Err(format!(
                        "unknown provider `{other}`; expected all, gemini, openai, or anthropic"
                    ));
                }
            };
            if !targets.contains(&target) {
                targets.push(target);
            }
        }

        if targets.is_empty() {
            return Err("no provider target selected".to_string());
        }
        Ok(Command::Run(Selection {
            targets,
            skip_missing_secrets: false,
        }))
    }

    fn print_plan() {
        println!("GemEd live LLM smoke plan (no network requests):");
        for target in [Target::Gemini, Target::OpenAi, Target::Anthropic] {
            let secret_state = if env_trimmed(target.secret_env()).is_some() {
                "present"
            } else {
                "missing"
            };
            let model_state = env_trimmed(target.model_env())
                .map(|model| format!("custom `{model}`"))
                .unwrap_or_else(|| format!("default `{}`", target.default_model()));
            let base_url_state = if env_trimmed(target.base_url_env()).is_some() {
                "custom"
            } else {
                "default"
            };
            println!(
                "- {}: secret {}={} · model {} · base URL {}",
                target.name(),
                target.secret_env(),
                secret_state,
                model_state,
                base_url_state
            );
        }
    }

    fn run_target(target: Target, api_key: String, prompt: &str) -> Result<(), String> {
        let model =
            env_trimmed(target.model_env()).unwrap_or_else(|| target.default_model().into());
        let request = LlmRequest {
            provider: target.provider_id(),
            model: model.clone(),
            prompt: prompt.to_string(),
            input_images: Vec::new(),
            temperature: Some(0.0),
            max_tokens: Some(48),
            parameters: Value::Null,
        };

        let response = match target {
            Target::Gemini => {
                let mut provider =
                    GeminiGenerateContentProvider::new(api_key).with_default_model(model.clone());
                if let Some(base_url) = env_trimmed(target.base_url_env()) {
                    provider = provider.with_endpoint_base(base_url);
                }
                futures::executor::block_on(provider.generate_text(request))
            }
            Target::OpenAi => {
                let mut provider =
                    OpenAiResponsesProvider::new(api_key).with_default_model(model.clone());
                if let Some(base_url) = env_trimmed(target.base_url_env()) {
                    provider = provider.with_endpoint(base_url);
                }
                futures::executor::block_on(provider.generate_text(request))
            }
            Target::Anthropic => {
                let mut provider =
                    AnthropicMessagesProvider::new(api_key).with_default_model(model.clone());
                if let Some(base_url) = env_trimmed(target.base_url_env()) {
                    provider = provider.with_endpoint(base_url);
                }
                futures::executor::block_on(provider.generate_text(request))
            }
        }
        .map_err(|err| err.to_string())?;

        let text = response.text.trim();
        if text.is_empty() {
            return Err("provider returned an empty text response".to_string());
        }

        println!(
            "{} OK: model={} chars={} text=\"{}\"",
            target.name(),
            response.model,
            text.chars().count(),
            compact_text(text)
        );
        Ok(())
    }

    fn env_trimmed(name: &str) -> Option<String> {
        std::env::var(name)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    }

    fn compact_text(text: &str) -> String {
        let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let mut chars = compact.chars();
        let preview = chars.by_ref().take(180).collect::<String>();
        if chars.next().is_some() {
            format!("{preview}…")
        } else {
            preview
        }
    }

    impl Target {
        fn name(self) -> &'static str {
            match self {
                Self::Gemini => "Gemini",
                Self::OpenAi => "OpenAI",
                Self::Anthropic => "Anthropic",
            }
        }

        fn provider_id(self) -> ProviderId {
            match self {
                Self::Gemini => ProviderId::Gemini,
                Self::OpenAi => ProviderId::OpenAi,
                Self::Anthropic => ProviderId::Anthropic,
            }
        }

        fn secret_env(self) -> &'static str {
            match self {
                Self::Gemini => "GEMINI_API_KEY",
                Self::OpenAi => "OPENAI_API_KEY",
                Self::Anthropic => "ANTHROPIC_API_KEY",
            }
        }

        fn model_env(self) -> &'static str {
            match self {
                Self::Gemini => "GEMED_LIVE_GEMINI_MODEL",
                Self::OpenAi => "GEMED_LIVE_OPENAI_MODEL",
                Self::Anthropic => "GEMED_LIVE_ANTHROPIC_MODEL",
            }
        }

        fn base_url_env(self) -> &'static str {
            match self {
                Self::Gemini => "GEMED_LIVE_GEMINI_BASE_URL",
                Self::OpenAi => "GEMED_LIVE_OPENAI_BASE_URL",
                Self::Anthropic => "GEMED_LIVE_ANTHROPIC_BASE_URL",
            }
        }

        fn default_model(self) -> &'static str {
            match self {
                Self::Gemini => GeminiGenerateContentProvider::DEFAULT_MODEL,
                Self::OpenAi => OpenAiResponsesProvider::DEFAULT_MODEL,
                Self::Anthropic => AnthropicMessagesProvider::DEFAULT_MODEL,
            }
        }
    }
}
