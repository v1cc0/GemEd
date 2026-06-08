use async_trait::async_trait;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use std::sync::Arc;
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
        for id in [
            ProviderId::Mock,
            ProviderId::Gemini,
            ProviderId::Google,
            ProviderId::OpenAi,
            ProviderId::Anthropic,
            ProviderId::Replicate,
            ProviderId::Fal,
            ProviderId::Kie,
            ProviderId::WaveSpeed,
        ] {
            registry.register(MockProvider::new(id));
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
}
