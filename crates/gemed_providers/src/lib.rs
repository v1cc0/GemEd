use async_trait::async_trait;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use std::{fmt, sync::Arc};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ProviderId {
    Gemini,
    Google,
    OpenAi,
    Anthropic,
    Replicate,
    Fal,
    Kie,
    WaveSpeed,
    Mock,
    Custom(String),
}

impl ProviderId {
    pub fn from_wire(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "gemini" => Self::Gemini,
            "google" => Self::Google,
            "openai" => Self::OpenAi,
            "anthropic" => Self::Anthropic,
            "replicate" => Self::Replicate,
            "fal" | "fal.ai" => Self::Fal,
            "kie" | "kie.ai" => Self::Kie,
            "wavespeed" | "wave_speed" | "wave-speed" => Self::WaveSpeed,
            "mock" | "local-mock" => Self::Mock,
            other => Self::Custom(other.to_string()),
        }
    }

    pub fn as_wire(&self) -> &str {
        match self {
            Self::Gemini => "gemini",
            Self::Google => "google",
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
            Self::Replicate => "replicate",
            Self::Fal => "fal",
            Self::Kie => "kie",
            Self::WaveSpeed => "wavespeed",
            Self::Mock => "mock",
            Self::Custom(value) => value.as_str(),
        }
    }

    pub fn display_name(&self) -> String {
        match self {
            Self::Gemini | Self::Google => "Gemini".to_string(),
            Self::OpenAi => "OpenAI".to_string(),
            Self::Anthropic => "Anthropic".to_string(),
            Self::Replicate => "Replicate".to_string(),
            Self::Fal => "fal.ai".to_string(),
            Self::Kie => "Kie.ai".to_string(),
            Self::WaveSpeed => "WaveSpeed".to_string(),
            Self::Mock => "Mock".to_string(),
            Self::Custom(value) => value.clone(),
        }
    }
}

impl Serialize for ProviderId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_wire())
    }
}

impl<'de> Deserialize<'de> for ProviderId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(Self::from_wire(&value))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProviderCapability {
    Llm,
    Image,
    Video,
    Audio,
    Model3d,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum ProviderSecretSource {
    None,
    Environment { variable: String },
    DesktopKeychain { service: String, account: String },
    WebBackend { binding: String },
}

impl fmt::Debug for ProviderSecretSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => formatter.write_str("None"),
            Self::Environment { variable } => formatter
                .debug_struct("Environment")
                .field("variable", variable)
                .finish(),
            Self::DesktopKeychain { service, account } => formatter
                .debug_struct("DesktopKeychain")
                .field("service", service)
                .field("account", account)
                .finish(),
            Self::WebBackend { binding } => formatter
                .debug_struct("WebBackend")
                .field("binding", binding)
                .finish(),
        }
    }
}

impl ProviderSecretSource {
    pub fn public_label(&self) -> String {
        match self {
            Self::None => "no secret".to_string(),
            Self::Environment { variable } => format!("env:{variable}"),
            Self::DesktopKeychain { service, account } => {
                format!("desktop-keychain:{service}/{account}")
            }
            Self::WebBackend { binding } => format!("web-backend:{binding}"),
        }
    }

    pub fn is_configured_with<F>(&self, resolver: &F) -> bool
    where
        F: Fn(&str) -> Option<String>,
    {
        match self {
            Self::None => false,
            Self::Environment { variable } => resolver(variable)
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false),
            Self::DesktopKeychain { .. } | Self::WebBackend { .. } => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProviderRuntimeMode {
    Mock,
    Disabled,
    DirectDesktop,
    WebBackend,
}

impl ProviderRuntimeMode {
    pub fn requires_local_secret(self) -> bool {
        matches!(self, Self::DirectDesktop)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    pub id: ProviderId,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub runtime_mode: ProviderRuntimeMode,
    pub secret_source: ProviderSecretSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_model: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<ProviderCapability>,
}

impl ProviderConfig {
    pub fn mock(id: ProviderId) -> Self {
        Self {
            id: id.clone(),
            enabled: true,
            runtime_mode: ProviderRuntimeMode::Mock,
            secret_source: ProviderSecretSource::None,
            base_url: None,
            default_model: None,
            capabilities: default_capabilities(&id),
        }
    }

    pub fn direct_desktop_env(
        id: ProviderId,
        variable: impl Into<String>,
        default_model: Option<String>,
    ) -> Self {
        Self {
            id: id.clone(),
            enabled: true,
            runtime_mode: ProviderRuntimeMode::DirectDesktop,
            secret_source: ProviderSecretSource::Environment {
                variable: variable.into(),
            },
            base_url: None,
            default_model,
            capabilities: default_capabilities(&id),
        }
    }

    pub fn web_backend(
        id: ProviderId,
        binding: impl Into<String>,
        base_url: Option<String>,
    ) -> Self {
        Self {
            id: id.clone(),
            enabled: true,
            runtime_mode: ProviderRuntimeMode::WebBackend,
            secret_source: ProviderSecretSource::WebBackend {
                binding: binding.into(),
            },
            base_url,
            default_model: None,
            capabilities: default_capabilities(&id),
        }
    }

    pub fn disabled(id: ProviderId) -> Self {
        Self {
            id: id.clone(),
            enabled: false,
            runtime_mode: ProviderRuntimeMode::Disabled,
            secret_source: ProviderSecretSource::None,
            base_url: None,
            default_model: None,
            capabilities: default_capabilities(&id),
        }
    }

    pub fn is_available_with<F>(&self, resolver: &F) -> bool
    where
        F: Fn(&str) -> Option<String>,
    {
        if !self.enabled || self.runtime_mode == ProviderRuntimeMode::Disabled {
            return false;
        }

        match self.runtime_mode {
            ProviderRuntimeMode::Mock | ProviderRuntimeMode::WebBackend => true,
            ProviderRuntimeMode::Disabled => false,
            ProviderRuntimeMode::DirectDesktop => {
                self.direct_desktop_secret_available_with(resolver)
            }
        }
    }

    pub fn missing_required_secret_with<F>(&self, resolver: &F) -> bool
    where
        F: Fn(&str) -> Option<String>,
    {
        self.enabled
            && self.runtime_mode.requires_local_secret()
            && !self.direct_desktop_secret_available_with(resolver)
    }

    fn direct_desktop_secret_available_with<F>(&self, resolver: &F) -> bool
    where
        F: Fn(&str) -> Option<String>,
    {
        match &self.secret_source {
            ProviderSecretSource::Environment { .. }
            | ProviderSecretSource::DesktopKeychain { .. } => {
                self.secret_source.is_configured_with(resolver)
            }
            ProviderSecretSource::None | ProviderSecretSource::WebBackend { .. } => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfigSet {
    #[serde(default)]
    pub providers: Vec<ProviderConfig>,
}

impl ProviderConfigSet {
    pub fn new(providers: Vec<ProviderConfig>) -> Self {
        Self { providers }
    }

    pub fn mock_all() -> Self {
        Self::new(mock_provider_ids().map(ProviderConfig::mock).collect())
    }

    pub fn desktop_env_defaults() -> Self {
        Self::new(vec![
            ProviderConfig::direct_desktop_env(ProviderId::Gemini, "GEMINI_API_KEY", None),
            ProviderConfig::direct_desktop_env(ProviderId::Google, "GOOGLE_API_KEY", None),
            ProviderConfig::direct_desktop_env(ProviderId::OpenAi, "OPENAI_API_KEY", None),
            ProviderConfig::direct_desktop_env(ProviderId::Anthropic, "ANTHROPIC_API_KEY", None),
            ProviderConfig::direct_desktop_env(ProviderId::Replicate, "REPLICATE_API_TOKEN", None),
            ProviderConfig::direct_desktop_env(ProviderId::Fal, "FAL_KEY", None),
            ProviderConfig::direct_desktop_env(ProviderId::Kie, "KIE_API_KEY", None),
            ProviderConfig::direct_desktop_env(ProviderId::WaveSpeed, "WAVESPEED_API_KEY", None),
        ])
    }

    pub fn web_backend_defaults() -> Self {
        Self::new(vec![
            ProviderConfig::web_backend(ProviderId::Gemini, "GEMINI_API_KEY", None),
            ProviderConfig::web_backend(ProviderId::Google, "GOOGLE_API_KEY", None),
            ProviderConfig::web_backend(ProviderId::OpenAi, "OPENAI_API_KEY", None),
            ProviderConfig::web_backend(ProviderId::Anthropic, "ANTHROPIC_API_KEY", None),
            ProviderConfig::web_backend(ProviderId::Replicate, "REPLICATE_API_TOKEN", None),
            ProviderConfig::web_backend(ProviderId::Fal, "FAL_KEY", None),
            ProviderConfig::web_backend(ProviderId::Kie, "KIE_API_KEY", None),
            ProviderConfig::web_backend(ProviderId::WaveSpeed, "WAVESPEED_API_KEY", None),
        ])
    }

    pub fn get(&self, id: &ProviderId) -> Option<&ProviderConfig> {
        self.providers.iter().find(|config| config.id == *id)
    }

    pub fn available_provider_ids_with<F>(&self, resolver: &F) -> Vec<ProviderId>
    where
        F: Fn(&str) -> Option<String>,
    {
        self.providers
            .iter()
            .filter(|config| config.is_available_with(resolver))
            .map(|config| config.id.clone())
            .collect()
    }

    pub fn missing_secret_provider_ids_with<F>(&self, resolver: &F) -> Vec<ProviderId>
    where
        F: Fn(&str) -> Option<String>,
    {
        self.providers
            .iter()
            .filter(|config| config.missing_required_secret_with(resolver))
            .map(|config| config.id.clone())
            .collect()
    }

    pub fn summary_with<F>(&self, resolver: F) -> ProviderConfigSummary
    where
        F: Fn(&str) -> Option<String>,
    {
        let mut summary = ProviderConfigSummary {
            total: self.providers.len(),
            ..ProviderConfigSummary::default()
        };

        for config in &self.providers {
            if !config.enabled || config.runtime_mode == ProviderRuntimeMode::Disabled {
                summary.disabled += 1;
                continue;
            }

            summary.enabled += 1;
            if config.is_available_with(&resolver) {
                summary.available += 1;
            }
            if config.missing_required_secret_with(&resolver) {
                summary.missing_local_secrets += 1;
            }

            match config.runtime_mode {
                ProviderRuntimeMode::Mock => summary.mock += 1,
                ProviderRuntimeMode::DirectDesktop => summary.direct_desktop += 1,
                ProviderRuntimeMode::WebBackend => summary.web_backend += 1,
                ProviderRuntimeMode::Disabled => summary.disabled += 1,
            }
        }

        summary
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProviderConfigSummary {
    pub total: usize,
    pub enabled: usize,
    pub available: usize,
    pub mock: usize,
    pub direct_desktop: usize,
    pub web_backend: usize,
    pub disabled: usize,
    pub missing_local_secrets: usize,
}

impl ProviderConfigSummary {
    pub fn sentence(self) -> String {
        format!(
            "Providers: {} configured, {} enabled, {} available ({} mock, {} direct desktop, {} web backend), {} missing local secrets.",
            self.total,
            self.enabled,
            self.available,
            self.mock,
            self.direct_desktop,
            self.web_backend,
            self.missing_local_secrets
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModel {
    pub provider: ProviderId,
    pub model_id: String,
    pub display_name: String,
    pub capabilities: Vec<ProviderCapability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pricing: Option<Value>,
}

fn default_true() -> bool {
    true
}

fn mock_provider_ids() -> impl Iterator<Item = ProviderId> {
    [
        ProviderId::Mock,
        ProviderId::Gemini,
        ProviderId::Google,
        ProviderId::OpenAi,
        ProviderId::Anthropic,
        ProviderId::Replicate,
        ProviderId::Fal,
        ProviderId::Kie,
        ProviderId::WaveSpeed,
    ]
    .into_iter()
}

fn default_capabilities(id: &ProviderId) -> Vec<ProviderCapability> {
    match id {
        ProviderId::Gemini | ProviderId::Google | ProviderId::Mock => vec![
            ProviderCapability::Llm,
            ProviderCapability::Image,
            ProviderCapability::Video,
            ProviderCapability::Audio,
            ProviderCapability::Model3d,
        ],
        ProviderId::OpenAi | ProviderId::Anthropic => vec![ProviderCapability::Llm],
        ProviderId::Replicate | ProviderId::Fal | ProviderId::Kie | ProviderId::WaveSpeed => vec![
            ProviderCapability::Image,
            ProviderCapability::Video,
            ProviderCapability::Audio,
            ProviderCapability::Model3d,
        ],
        ProviderId::Custom(_) => Vec::new(),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmRequest {
    pub provider: ProviderId,
    pub model: String,
    pub prompt: String,
    #[serde(default)]
    pub input_images: Vec<String>,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub parameters: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmResponse {
    pub text: String,
    pub provider: ProviderId,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageRequest {
    pub provider: ProviderId,
    pub model: String,
    pub prompt: String,
    #[serde(default)]
    pub input_images: Vec<String>,
    #[serde(default)]
    pub parameters: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageResponse {
    pub image: String,
    pub provider: ProviderId,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoRequest {
    pub provider: ProviderId,
    pub model: String,
    pub prompt: String,
    #[serde(default)]
    pub input_images: Vec<String>,
    #[serde(default)]
    pub parameters: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoResponse {
    pub video: String,
    pub provider: ProviderId,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioRequest {
    pub provider: ProviderId,
    pub model: String,
    pub prompt: String,
    #[serde(default)]
    pub parameters: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioResponse {
    pub audio: String,
    pub provider: ProviderId,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Model3dRequest {
    pub provider: ProviderId,
    pub model: String,
    pub prompt: String,
    #[serde(default)]
    pub input_images: Vec<String>,
    #[serde(default)]
    pub parameters: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Model3dResponse {
    pub model_url: String,
    pub provider: ProviderId,
    pub model: String,
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("provider `{0}` does not implement capability `{1}`")]
    UnsupportedCapability(String, &'static str),
    #[error("provider request is invalid: {0}")]
    InvalidRequest(String),
    #[error("provider secret is missing: {0}")]
    MissingSecret(String),
    #[error("provider request failed: {0}")]
    RequestFailed(String),
}

#[async_trait(?Send)]
pub trait ModelCatalog {
    async fn list_models(&self) -> Result<Vec<ProviderModel>, ProviderError>;
}

#[async_trait(?Send)]
pub trait LlmProvider {
    async fn generate_text(&self, request: LlmRequest) -> Result<LlmResponse, ProviderError>;
}

#[async_trait(?Send)]
pub trait ImageProvider {
    async fn generate_image(&self, request: ImageRequest) -> Result<ImageResponse, ProviderError>;
}

#[async_trait(?Send)]
pub trait VideoProvider {
    async fn generate_video(&self, request: VideoRequest) -> Result<VideoResponse, ProviderError>;
}

#[async_trait(?Send)]
pub trait AudioProvider {
    async fn generate_audio(&self, request: AudioRequest) -> Result<AudioResponse, ProviderError>;
}

#[async_trait(?Send)]
pub trait Model3dProvider {
    async fn generate_model3d(
        &self,
        request: Model3dRequest,
    ) -> Result<Model3dResponse, ProviderError>;
}

#[async_trait(?Send)]
pub trait ProviderBackend:
    ModelCatalog + LlmProvider + ImageProvider + VideoProvider + AudioProvider + Model3dProvider
{
    fn id(&self) -> ProviderId;
}

pub type SharedProviderBackend = Arc<dyn ProviderBackend>;

#[derive(Clone, Default)]
pub struct ProviderRegistry {
    providers: Vec<SharedProviderBackend>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_provider(provider: impl ProviderBackend + 'static) -> Self {
        let mut registry = Self::new();
        registry.register(provider);
        registry
    }

    pub fn mock_all() -> Self {
        let mut registry = Self::new();
        for id in mock_provider_ids() {
            registry.register(MockProvider::new(id));
        }
        registry
    }

    pub fn mock_from_config(config: &ProviderConfigSet) -> Self {
        let mut registry = Self::new();
        for provider in &config.providers {
            if provider.enabled && provider.runtime_mode == ProviderRuntimeMode::Mock {
                registry.register(MockProvider::new(provider.id.clone()));
            }
        }
        registry
    }

    pub fn register(&mut self, provider: impl ProviderBackend + 'static) {
        self.providers.push(Arc::new(provider));
    }

    pub fn get(&self, id: &ProviderId) -> Option<SharedProviderBackend> {
        self.providers
            .iter()
            .find(|provider| provider.id() == *id)
            .cloned()
    }

    pub fn contains(&self, id: &ProviderId) -> bool {
        self.get(id).is_some()
    }

    pub fn len(&self) -> usize {
        self.providers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
}

#[derive(Clone)]
pub struct MockProvider {
    id: ProviderId,
}

impl MockProvider {
    pub fn new(id: ProviderId) -> Self {
        Self { id }
    }
}

impl Default for MockProvider {
    fn default() -> Self {
        Self {
            id: ProviderId::Mock,
        }
    }
}

impl ProviderBackend for MockProvider {
    fn id(&self) -> ProviderId {
        self.id.clone()
    }
}

#[async_trait(?Send)]
impl ModelCatalog for MockProvider {
    async fn list_models(&self) -> Result<Vec<ProviderModel>, ProviderError> {
        Ok(vec![
            ProviderModel {
                provider: self.id.clone(),
                model_id: "mock-llm".to_string(),
                display_name: "Mock LLM".to_string(),
                capabilities: vec![ProviderCapability::Llm],
                pricing: None,
            },
            ProviderModel {
                provider: self.id.clone(),
                model_id: "mock-image".to_string(),
                display_name: "Mock Image".to_string(),
                capabilities: vec![ProviderCapability::Image],
                pricing: None,
            },
            ProviderModel {
                provider: self.id.clone(),
                model_id: "mock-video".to_string(),
                display_name: "Mock Video".to_string(),
                capabilities: vec![ProviderCapability::Video],
                pricing: None,
            },
            ProviderModel {
                provider: self.id.clone(),
                model_id: "mock-audio".to_string(),
                display_name: "Mock Audio".to_string(),
                capabilities: vec![ProviderCapability::Audio],
                pricing: None,
            },
            ProviderModel {
                provider: self.id.clone(),
                model_id: "mock-3d".to_string(),
                display_name: "Mock 3D".to_string(),
                capabilities: vec![ProviderCapability::Model3d],
                pricing: None,
            },
        ])
    }
}

#[async_trait(?Send)]
impl LlmProvider for MockProvider {
    async fn generate_text(&self, request: LlmRequest) -> Result<LlmResponse, ProviderError> {
        validate_prompt(&request.prompt)?;
        Ok(LlmResponse {
            text: format!(
                "[mock:{}:{}] {}",
                self.id.as_wire(),
                request.model,
                request.prompt
            ),
            provider: self.id.clone(),
            model: request.model,
        })
    }
}

#[async_trait(?Send)]
impl ImageProvider for MockProvider {
    async fn generate_image(&self, request: ImageRequest) -> Result<ImageResponse, ProviderError> {
        validate_prompt(&request.prompt)?;
        Ok(ImageResponse {
            image: format!(
                "mock://image/{}/{}?prompt={}",
                self.id.as_wire(),
                sanitize_path_segment(&request.model),
                percent_encode(&request.prompt)
            ),
            provider: self.id.clone(),
            model: request.model,
        })
    }
}

#[async_trait(?Send)]
impl VideoProvider for MockProvider {
    async fn generate_video(&self, request: VideoRequest) -> Result<VideoResponse, ProviderError> {
        validate_prompt(&request.prompt)?;
        Ok(VideoResponse {
            video: format!(
                "mock://video/{}/{}?prompt={}",
                self.id.as_wire(),
                sanitize_path_segment(&request.model),
                percent_encode(&request.prompt)
            ),
            provider: self.id.clone(),
            model: request.model,
        })
    }
}

#[async_trait(?Send)]
impl AudioProvider for MockProvider {
    async fn generate_audio(&self, request: AudioRequest) -> Result<AudioResponse, ProviderError> {
        validate_prompt(&request.prompt)?;
        Ok(AudioResponse {
            audio: format!(
                "mock://audio/{}/{}?prompt={}",
                self.id.as_wire(),
                sanitize_path_segment(&request.model),
                percent_encode(&request.prompt)
            ),
            provider: self.id.clone(),
            model: request.model,
        })
    }
}

#[async_trait(?Send)]
impl Model3dProvider for MockProvider {
    async fn generate_model3d(
        &self,
        request: Model3dRequest,
    ) -> Result<Model3dResponse, ProviderError> {
        validate_prompt(&request.prompt)?;
        Ok(Model3dResponse {
            model_url: format!(
                "mock://3d/{}/{}?prompt={}",
                self.id.as_wire(),
                sanitize_path_segment(&request.model),
                percent_encode(&request.prompt)
            ),
            provider: self.id.clone(),
            model: request.model,
        })
    }
}

#[cfg(feature = "http")]
#[derive(Clone, Debug)]
pub struct OpenAiResponsesProvider {
    api_key: String,
    endpoint: String,
    default_model: String,
}

#[cfg(feature = "http")]
impl OpenAiResponsesProvider {
    pub const DEFAULT_ENDPOINT: &'static str = "https://api.openai.com/v1/responses";
    pub const DEFAULT_MODEL: &'static str = "gpt-5.5";

    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            endpoint: Self::DEFAULT_ENDPOINT.to_string(),
            default_model: Self::DEFAULT_MODEL.to_string(),
        }
    }

    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = normalize_openai_responses_endpoint(&endpoint.into());
        self
    }

    pub fn with_default_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = model.into();
        self
    }

    pub fn from_config_with_secret<F>(
        config: &ProviderConfig,
        resolver: &F,
    ) -> Result<Option<Self>, ProviderError>
    where
        F: Fn(&str) -> Option<String>,
    {
        if !config.enabled
            || config.id != ProviderId::OpenAi
            || config.runtime_mode != ProviderRuntimeMode::DirectDesktop
        {
            return Ok(None);
        }

        let api_key = match &config.secret_source {
            ProviderSecretSource::Environment { variable } => resolver(variable)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    ProviderError::MissingSecret(format!(
                        "environment variable `{variable}` is not set"
                    ))
                })?,
            ProviderSecretSource::DesktopKeychain { service, account } => {
                return Err(ProviderError::MissingSecret(format!(
                    "desktop keychain `{service}/{account}` resolution is not implemented yet"
                )));
            }
            ProviderSecretSource::None | ProviderSecretSource::WebBackend { .. } => {
                return Err(ProviderError::MissingSecret(
                    "OpenAI direct desktop provider requires an environment secret source"
                        .to_string(),
                ));
            }
        };

        let mut provider = Self::new(api_key);
        if let Some(base_url) = config.base_url.as_deref() {
            provider = provider.with_endpoint(base_url);
        }
        if let Some(model) = config.default_model.as_deref() {
            provider = provider.with_default_model(model);
        }
        Ok(Some(provider))
    }
}

#[cfg(feature = "http")]
impl ProviderBackend for OpenAiResponsesProvider {
    fn id(&self) -> ProviderId {
        ProviderId::OpenAi
    }
}

#[cfg(feature = "http")]
#[async_trait(?Send)]
impl ModelCatalog for OpenAiResponsesProvider {
    async fn list_models(&self) -> Result<Vec<ProviderModel>, ProviderError> {
        Ok(vec![ProviderModel {
            provider: ProviderId::OpenAi,
            model_id: self.default_model.clone(),
            display_name: self.default_model.clone(),
            capabilities: vec![ProviderCapability::Llm],
            pricing: None,
        }])
    }
}

#[cfg(feature = "http")]
#[async_trait(?Send)]
impl LlmProvider for OpenAiResponsesProvider {
    async fn generate_text(&self, request: LlmRequest) -> Result<LlmResponse, ProviderError> {
        validate_prompt(&request.prompt)?;
        let body = openai_responses_request_body(&request);
        let authorization = format!("Bearer {}", self.api_key);
        let mut response: Value = ureq::post(&self.endpoint)
            .header("Authorization", authorization)
            .header("Accept", "application/json")
            .send_json(&body)
            .map_err(openai_transport_error)?
            .body_mut()
            .read_json()
            .map_err(openai_transport_error)?;
        let text = extract_openai_response_text(&mut response).ok_or_else(|| {
            ProviderError::RequestFailed("OpenAI response did not contain output text".to_string())
        })?;
        Ok(LlmResponse {
            text,
            provider: ProviderId::OpenAi,
            model: request.model,
        })
    }
}

#[cfg(feature = "http")]
#[async_trait(?Send)]
impl ImageProvider for OpenAiResponsesProvider {
    async fn generate_image(&self, _request: ImageRequest) -> Result<ImageResponse, ProviderError> {
        Err(ProviderError::UnsupportedCapability(
            self.id().display_name(),
            "image",
        ))
    }
}

#[cfg(feature = "http")]
#[async_trait(?Send)]
impl VideoProvider for OpenAiResponsesProvider {
    async fn generate_video(&self, _request: VideoRequest) -> Result<VideoResponse, ProviderError> {
        Err(ProviderError::UnsupportedCapability(
            self.id().display_name(),
            "video",
        ))
    }
}

#[cfg(feature = "http")]
#[async_trait(?Send)]
impl AudioProvider for OpenAiResponsesProvider {
    async fn generate_audio(&self, _request: AudioRequest) -> Result<AudioResponse, ProviderError> {
        Err(ProviderError::UnsupportedCapability(
            self.id().display_name(),
            "audio",
        ))
    }
}

#[cfg(feature = "http")]
#[async_trait(?Send)]
impl Model3dProvider for OpenAiResponsesProvider {
    async fn generate_model3d(
        &self,
        _request: Model3dRequest,
    ) -> Result<Model3dResponse, ProviderError> {
        Err(ProviderError::UnsupportedCapability(
            self.id().display_name(),
            "3D",
        ))
    }
}

#[cfg(feature = "http")]
fn normalize_openai_responses_endpoint(value: &str) -> String {
    let trimmed = value.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return OpenAiResponsesProvider::DEFAULT_ENDPOINT.to_string();
    }
    if trimmed.ends_with("/responses") {
        trimmed.to_string()
    } else {
        format!("{trimmed}/responses")
    }
}

#[cfg(feature = "http")]
fn openai_responses_request_body(request: &LlmRequest) -> Value {
    let model = if request.model.trim().is_empty() {
        OpenAiResponsesProvider::DEFAULT_MODEL
    } else {
        request.model.as_str()
    };
    let mut body = serde_json::Map::new();
    body.insert("model".to_string(), json_value(model));
    body.insert("input".to_string(), json_value(&request.prompt));
    if let Some(max_tokens) = request.max_tokens {
        body.insert(
            "max_output_tokens".to_string(),
            serde_json::json!(max_tokens),
        );
    }
    if let Value::Object(parameters) = &request.parameters {
        for (key, value) in parameters {
            body.entry(key.clone()).or_insert_with(|| value.clone());
        }
    }
    Value::Object(body)
}

#[cfg(feature = "http")]
fn json_value(value: &str) -> Value {
    Value::String(value.to_string())
}

#[cfg(feature = "http")]
fn openai_transport_error(error: ureq::Error) -> ProviderError {
    match error {
        ureq::Error::StatusCode(status) => {
            ProviderError::RequestFailed(format!("OpenAI API returned HTTP {status}"))
        }
        other => ProviderError::RequestFailed(format!("OpenAI API request failed: {other}")),
    }
}

#[cfg(feature = "http")]
fn extract_openai_response_text(value: &mut Value) -> Option<String> {
    if let Some(text) = value
        .get("output_text")
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
    {
        return Some(text.to_string());
    }

    let mut parts = Vec::new();
    let output = value.get_mut("output")?.as_array_mut()?;
    for item in output {
        let Some(content) = item.get_mut("content").and_then(Value::as_array_mut) else {
            continue;
        };
        for content_item in content {
            let Some(text) = content_item
                .get("text")
                .and_then(Value::as_str)
                .filter(|text| !text.is_empty())
            else {
                continue;
            };
            let item_type = content_item
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if item_type.is_empty() || item_type == "output_text" || item_type == "text" {
                parts.push(text.to_string());
            }
        }
    }

    (!parts.is_empty()).then(|| parts.join("\n"))
}

fn validate_prompt(prompt: &str) -> Result<(), ProviderError> {
    if prompt.trim().is_empty() {
        return Err(ProviderError::InvalidRequest(
            "prompt must not be empty".to_string(),
        ));
    }
    Ok(())
}

fn sanitize_path_segment(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => ch,
            _ => '-',
        })
        .collect()
}

fn percent_encode(value: &str) -> String {
    let mut output = String::new();
    for byte in value.bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                output.push(byte as char);
            }
            b' ' => output.push('+'),
            _ => output.push_str(&format!("%{byte:02X}")),
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_llm_returns_deterministic_text() {
        let provider = MockProvider::default();
        let response = futures::executor::block_on(provider.generate_text(LlmRequest {
            provider: ProviderId::Mock,
            model: "mock-llm".to_string(),
            prompt: "hello".to_string(),
            input_images: Vec::new(),
            temperature: Some(0.2),
            max_tokens: Some(32),
            parameters: Value::Null,
        }))
        .expect("mock text generation succeeds");

        assert_eq!(response.text, "[mock:mock:mock-llm] hello");
    }

    #[test]
    fn mock_image_rejects_empty_prompt() {
        let provider = MockProvider::default();
        let err = futures::executor::block_on(provider.generate_image(ImageRequest {
            provider: ProviderId::Mock,
            model: "mock-image".to_string(),
            prompt: "  ".to_string(),
            input_images: Vec::new(),
            parameters: Value::Null,
        }))
        .expect_err("empty prompt rejected");

        assert!(matches!(err, ProviderError::InvalidRequest(_)));
    }

    #[test]
    fn mock_config_needs_no_secret() {
        let configs = ProviderConfigSet::mock_all();
        let summary = configs.summary_with(|_| None::<String>);

        assert_eq!(summary.total, 9);
        assert_eq!(summary.enabled, 9);
        assert_eq!(summary.available, 9);
        assert_eq!(summary.mock, 9);
        assert_eq!(summary.missing_local_secrets, 0);
        assert!(
            configs
                .missing_secret_provider_ids_with(&|_| None::<String>)
                .is_empty()
        );
    }

    #[test]
    fn mock_registry_from_config_registers_only_mock_enabled_providers() {
        let configs = ProviderConfigSet::new(vec![
            ProviderConfig::mock(ProviderId::Mock),
            ProviderConfig::mock(ProviderId::OpenAi),
            ProviderConfig::disabled(ProviderId::Anthropic),
            ProviderConfig::direct_desktop_env(ProviderId::Gemini, "GEMINI_API_KEY", None),
        ]);

        let registry = ProviderRegistry::mock_from_config(&configs);

        assert!(registry.contains(&ProviderId::Mock));
        assert!(registry.contains(&ProviderId::OpenAi));
        assert!(!registry.contains(&ProviderId::Anthropic));
        assert!(!registry.contains(&ProviderId::Gemini));
    }

    #[test]
    fn desktop_env_defaults_use_expected_secret_names() {
        let configs = ProviderConfigSet::desktop_env_defaults();

        let expected = [
            (ProviderId::Gemini, "GEMINI_API_KEY"),
            (ProviderId::Google, "GOOGLE_API_KEY"),
            (ProviderId::OpenAi, "OPENAI_API_KEY"),
            (ProviderId::Anthropic, "ANTHROPIC_API_KEY"),
            (ProviderId::Replicate, "REPLICATE_API_TOKEN"),
            (ProviderId::Fal, "FAL_KEY"),
            (ProviderId::Kie, "KIE_API_KEY"),
            (ProviderId::WaveSpeed, "WAVESPEED_API_KEY"),
        ];

        for (id, variable) in expected {
            let config = configs.get(&id).expect("provider config exists");
            assert_eq!(config.runtime_mode, ProviderRuntimeMode::DirectDesktop);
            assert_eq!(
                config.secret_source,
                ProviderSecretSource::Environment {
                    variable: variable.to_string()
                }
            );
        }
    }

    #[test]
    fn direct_desktop_reports_missing_and_available_secrets() {
        let configs = ProviderConfigSet::desktop_env_defaults();

        let missing = configs.missing_secret_provider_ids_with(&|_| None::<String>);
        assert_eq!(missing.len(), 8);

        let available = configs.available_provider_ids_with(&|name| match name {
            "OPENAI_API_KEY" | "ANTHROPIC_API_KEY" => Some("present".to_string()),
            _ => None,
        });
        assert_eq!(available, vec![ProviderId::OpenAi, ProviderId::Anthropic]);

        let summary = configs.summary_with(|name| match name {
            "OPENAI_API_KEY" | "ANTHROPIC_API_KEY" => Some("present".to_string()),
            _ => None,
        });
        assert_eq!(summary.available, 2);
        assert_eq!(summary.missing_local_secrets, 6);
    }

    #[test]
    fn disabled_and_web_backend_do_not_require_local_secret() {
        let configs = ProviderConfigSet::new(vec![
            ProviderConfig::disabled(ProviderId::OpenAi),
            ProviderConfig {
                id: ProviderId::Custom("private-proxy".to_string()),
                enabled: true,
                runtime_mode: ProviderRuntimeMode::WebBackend,
                secret_source: ProviderSecretSource::WebBackend {
                    binding: "PROVIDER_API_KEY".to_string(),
                },
                base_url: Some("https://provider-backend.local".to_string()),
                default_model: None,
                capabilities: vec![ProviderCapability::Image],
            },
        ]);

        let summary = configs.summary_with(|_| None::<String>);
        assert_eq!(summary.disabled, 1);
        assert_eq!(summary.web_backend, 1);
        assert_eq!(summary.available, 1);
        assert_eq!(summary.missing_local_secrets, 0);
    }

    #[test]
    fn config_debug_and_summary_do_not_expose_secret_values() {
        let secret = ProviderSecretSource::Environment {
            variable: "OPENAI_API_KEY".to_string(),
        };
        assert!(!format!("{secret:?}").contains("sk-live-secret"));

        let configs = ProviderConfigSet::desktop_env_defaults();
        let summary = configs
            .summary_with(|name| (name == "OPENAI_API_KEY").then(|| "sk-live-secret".to_string()));
        let sentence = summary.sentence();
        assert!(!sentence.contains("sk-live-secret"));
        assert!(sentence.contains("missing local secrets"));
    }

    #[cfg(feature = "http")]
    #[test]
    fn openai_responses_body_maps_llm_request() {
        let body = openai_responses_request_body(&LlmRequest {
            provider: ProviderId::OpenAi,
            model: "gpt-test".to_string(),
            prompt: "hello".to_string(),
            input_images: Vec::new(),
            temperature: Some(0.2),
            max_tokens: Some(64),
            parameters: serde_json::json!({
                "text": {
                    "verbosity": "low"
                }
            }),
        });

        assert_eq!(body["model"], "gpt-test");
        assert_eq!(body["input"], "hello");
        assert_eq!(body["max_output_tokens"], 64);
        assert_eq!(body["text"]["verbosity"], "low");
    }

    #[cfg(feature = "http")]
    #[test]
    fn openai_response_text_extracts_flat_and_nested_shapes() {
        let mut flat = serde_json::json!({
            "output_text": "flat response"
        });
        assert_eq!(
            extract_openai_response_text(&mut flat),
            Some("flat response".to_string())
        );

        let mut nested = serde_json::json!({
            "output": [
                {
                    "content": [
                        {
                            "type": "output_text",
                            "text": "nested response"
                        }
                    ]
                }
            ]
        });
        assert_eq!(
            extract_openai_response_text(&mut nested),
            Some("nested response".to_string())
        );
    }

    #[cfg(feature = "http")]
    #[test]
    fn openai_provider_from_config_resolves_env_secret_without_leaking_it() {
        let config = ProviderConfig::direct_desktop_env(
            ProviderId::OpenAi,
            "OPENAI_API_KEY",
            Some("gpt-test".to_string()),
        );

        let provider = OpenAiResponsesProvider::from_config_with_secret(&config, &|name| {
            (name == "OPENAI_API_KEY").then(|| "sk-test-secret".to_string())
        })
        .expect("config resolves")
        .expect("provider created");

        assert_eq!(provider.default_model, "gpt-test");
        assert_eq!(
            OpenAiResponsesProvider::from_config_with_secret(&config, &|_| None::<String>)
                .expect_err("missing secret rejected")
                .to_string(),
            "provider secret is missing: environment variable `OPENAI_API_KEY` is not set"
        );
    }
}
