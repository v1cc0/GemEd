use crate::array_parser::{ParseArrayOptions, SplitMode, parse_text_to_array};
use crate::graph::{
    ConnectedInputs, DynamicInputValue, GraphError, connected_inputs, execution_order,
};
use gemed_core::{NodeStatus, NodeType, WorkflowFile, WorkflowNode, is_node_in_locked_group};
use gemed_media::{
    InlineImageCompareResult, compare_inline_images, inspect_inline_image, split_inline_image_grid,
};
use gemed_providers::{
    AudioRequest, ImageRequest, LlmRequest, Model3dRequest, ProviderError, ProviderId,
    ProviderRegistry, VideoRequest,
};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SimpleExecutionReport {
    pub events: Vec<NodeExecutionEvent>,
}

impl SimpleExecutionReport {
    pub fn executed_count(&self) -> usize {
        self.events
            .iter()
            .filter(|event| event.status == NodeStatusWire::Complete)
            .count()
    }

    pub fn skipped_count(&self) -> usize {
        self.events
            .iter()
            .filter(|event| event.status == NodeStatusWire::Skipped)
            .count()
    }

    pub fn error_count(&self) -> usize {
        self.events
            .iter()
            .filter(|event| event.status == NodeStatusWire::Error)
            .count()
    }

    pub fn summary(&self) -> String {
        format!(
            "{} complete, {} skipped, {} errors",
            self.executed_count(),
            self.skipped_count(),
            self.error_count()
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeExecutionEvent {
    pub node_id: String,
    pub node_type: String,
    pub status: NodeStatusWire,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NodeStatusWire {
    Idle,
    Loading,
    Complete,
    Error,
    Skipped,
}

impl NodeStatusWire {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Loading => "loading",
            Self::Complete => "complete",
            Self::Error => "error",
            Self::Skipped => "skipped",
        }
    }
}

impl From<NodeStatus> for NodeStatusWire {
    fn from(value: NodeStatus) -> Self {
        match value {
            NodeStatus::Idle => Self::Idle,
            NodeStatus::Loading => Self::Loading,
            NodeStatus::Complete => Self::Complete,
            NodeStatus::Error => Self::Error,
            NodeStatus::Skipped => Self::Skipped,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SimpleExecutionResult {
    pub workflow: WorkflowFile,
    pub report: SimpleExecutionReport,
}

#[derive(Debug, Error)]
pub enum SimpleExecutionError {
    #[error(transparent)]
    Graph(#[from] GraphError),
    #[error("node `{0}` disappeared during execution")]
    MissingNode(String),
}

pub fn execute_simple_workflow(
    workflow: &WorkflowFile,
) -> Result<SimpleExecutionResult, SimpleExecutionError> {
    futures::executor::block_on(execute_simple_workflow_async(workflow))
}

pub async fn execute_simple_workflow_async(
    workflow: &WorkflowFile,
) -> Result<SimpleExecutionResult, SimpleExecutionError> {
    execute_workflow_inner(workflow, &ProviderRegistry::new()).await
}

pub fn execute_workflow_with_providers(
    workflow: &WorkflowFile,
    providers: &ProviderRegistry,
) -> Result<SimpleExecutionResult, SimpleExecutionError> {
    futures::executor::block_on(execute_workflow_with_providers_async(workflow, providers))
}

pub async fn execute_workflow_with_providers_async(
    workflow: &WorkflowFile,
    providers: &ProviderRegistry,
) -> Result<SimpleExecutionResult, SimpleExecutionError> {
    execute_workflow_inner(workflow, providers).await
}

async fn execute_workflow_inner(
    workflow: &WorkflowFile,
    providers: &ProviderRegistry,
) -> Result<SimpleExecutionResult, SimpleExecutionError> {
    let mut workflow = workflow.clone();
    let order = execution_order(&workflow)?;
    let mut report = SimpleExecutionReport::default();

    for node_id in order {
        let index = workflow
            .nodes
            .iter()
            .position(|node| node.id == node_id)
            .ok_or_else(|| SimpleExecutionError::MissingNode(node_id.clone()))?;
        if is_node_in_locked_group(&workflow, &node_id) {
            let node_type = workflow.nodes[index].node_type.title().to_string();
            set_status(&mut workflow.nodes[index], NodeStatusWire::Skipped);
            set_data_field(
                &mut workflow.nodes[index],
                "error",
                json!("Node skipped because its group is locked."),
            );
            report.events.push(NodeExecutionEvent {
                node_id,
                node_type,
                status: NodeStatusWire::Skipped,
                message: "Node skipped because its group is locked.".to_string(),
            });
            continue;
        }
        let inputs = connected_inputs(&workflow, &node_id);
        let node_snapshot = workflow.nodes[index].clone();
        let outcome = execute_node(&node_snapshot, &inputs, providers).await;
        apply_updates(&mut workflow.nodes[index], outcome.updates);
        set_status(&mut workflow.nodes[index], outcome.status);
        if let Some(error) = outcome.error.as_deref() {
            set_data_field(&mut workflow.nodes[index], "error", json!(error));
        } else {
            set_data_field(&mut workflow.nodes[index], "error", Value::Null);
        }
        report.events.push(NodeExecutionEvent {
            node_id,
            node_type: node_snapshot.node_type.title().to_string(),
            status: outcome.status,
            message: outcome.message,
        });
    }

    Ok(SimpleExecutionResult { workflow, report })
}

#[derive(Debug, Clone)]
struct NodeOutcome {
    status: NodeStatusWire,
    message: String,
    error: Option<String>,
    updates: IndexMap<String, Value>,
}

impl NodeOutcome {
    fn complete(message: impl Into<String>, updates: IndexMap<String, Value>) -> Self {
        Self {
            status: NodeStatusWire::Complete,
            message: message.into(),
            error: None,
            updates,
        }
    }

    fn skipped(message: impl Into<String>) -> Self {
        let message = message.into();
        Self {
            status: NodeStatusWire::Skipped,
            error: Some(message.clone()),
            message,
            updates: IndexMap::new(),
        }
    }

    fn error(message: impl Into<String>, updates: IndexMap<String, Value>) -> Self {
        let message = message.into();
        Self {
            status: NodeStatusWire::Error,
            error: Some(message.clone()),
            message,
            updates,
        }
    }
}

async fn execute_node(
    node: &WorkflowNode,
    inputs: &ConnectedInputs,
    providers: &ProviderRegistry,
) -> NodeOutcome {
    match node.node_type {
        NodeType::ImageInput | NodeType::AudioInput | NodeType::VideoInput => {
            NodeOutcome::complete("Input node is ready.", IndexMap::new())
        }
        NodeType::Prompt => execute_prompt(node, inputs),
        NodeType::Array => execute_array(node, inputs),
        NodeType::PromptConstructor => execute_prompt_constructor(node, inputs),
        NodeType::Annotation => execute_annotation(node, inputs),
        NodeType::Output => execute_output(inputs),
        NodeType::OutputGallery => execute_output_gallery(inputs),
        NodeType::SplitGrid => execute_split_grid(node, inputs),
        NodeType::ImageCompare => execute_image_compare(node, inputs),
        NodeType::Router | NodeType::Switch | NodeType::ConditionalSwitch => NodeOutcome::complete(
            "Control node evaluated as a pass-through/gate.",
            IndexMap::new(),
        ),
        NodeType::NanoBanana => execute_image_generation(node, inputs, providers).await,
        NodeType::GenerateVideo => execute_video_generation(node, inputs, providers).await,
        NodeType::Generate3d => execute_3d_generation(node, inputs, providers).await,
        NodeType::GenerateAudio => execute_audio_generation(node, inputs, providers).await,
        NodeType::LlmGenerate => execute_llm_generation(node, inputs, providers).await,
        NodeType::VideoStitch
        | NodeType::EaseCurve
        | NodeType::VideoTrim
        | NodeType::VideoFrameGrab
        | NodeType::GlbViewer => NodeOutcome::skipped(
            "Advanced media execution is not wired in this local simple executor yet.",
        ),
        NodeType::Unknown => NodeOutcome::skipped("Unknown node type skipped."),
    }
}

async fn execute_llm_generation(
    node: &WorkflowNode,
    inputs: &ConnectedInputs,
    providers: &ProviderRegistry,
) -> NodeOutcome {
    let provider_id = llm_provider_id(node);
    let Some(provider) = providers.get(&provider_id) else {
        return provider_skipped(provider_id, "LLM");
    };
    let prompt = prompt_for_provider_node(node, inputs);
    let model = model_for_node(node, "mock-llm");
    let request = LlmRequest {
        provider: provider_id,
        model,
        prompt,
        input_images: input_images_for_node(node, inputs),
        temperature: number_field(&node.data, "temperature"),
        max_tokens: integer_field(&node.data, "maxTokens")
            .and_then(|value| u32::try_from(value).ok()),
        parameters: parameters_for_node(node),
    };

    match provider.generate_text(request).await {
        Ok(response) => {
            let mut updates = provider_common_updates(response.provider, response.model);
            updates.insert(
                "inputPrompt".to_string(),
                json!(prompt_for_provider_node(node, inputs)),
            );
            updates.insert("outputText".to_string(), json!(response.text));
            NodeOutcome::complete("LLM provider generated text.", updates)
        }
        Err(err) => provider_error("LLM provider failed", err),
    }
}

async fn execute_image_generation(
    node: &WorkflowNode,
    inputs: &ConnectedInputs,
    providers: &ProviderRegistry,
) -> NodeOutcome {
    let provider_id = selected_provider_id(node, ProviderId::Gemini);
    let Some(provider) = providers.get(&provider_id) else {
        return provider_skipped(provider_id, "image");
    };
    let prompt = prompt_for_provider_node(node, inputs);
    let model = model_for_node(node, "mock-image");
    let input_images = input_images_for_node(node, inputs);
    let request = ImageRequest {
        provider: provider_id,
        model,
        prompt: prompt.clone(),
        input_images: input_images.clone(),
        parameters: parameters_for_node(node),
    };

    match provider.generate_image(request).await {
        Ok(response) => {
            let mut updates = provider_common_updates(response.provider, response.model);
            updates.insert("inputPrompt".to_string(), json!(prompt));
            updates.insert("inputImages".to_string(), json!(input_images));
            updates.insert("outputImage".to_string(), json!(response.image));
            NodeOutcome::complete("Image provider generated an image reference.", updates)
        }
        Err(err) => provider_error("Image provider failed", err),
    }
}

async fn execute_video_generation(
    node: &WorkflowNode,
    inputs: &ConnectedInputs,
    providers: &ProviderRegistry,
) -> NodeOutcome {
    let provider_id = selected_provider_id(node, ProviderId::Replicate);
    let Some(provider) = providers.get(&provider_id) else {
        return provider_skipped(provider_id, "video");
    };
    let prompt = prompt_for_provider_node(node, inputs);
    let model = model_for_node(node, "mock-video");
    let input_images = input_images_for_node(node, inputs);
    let request = VideoRequest {
        provider: provider_id,
        model,
        prompt: prompt.clone(),
        input_images: input_images.clone(),
        parameters: parameters_for_node(node),
    };

    match provider.generate_video(request).await {
        Ok(response) => {
            let mut updates = provider_common_updates(response.provider, response.model);
            updates.insert("inputPrompt".to_string(), json!(prompt));
            updates.insert("inputImages".to_string(), json!(input_images));
            updates.insert("outputVideo".to_string(), json!(response.video));
            NodeOutcome::complete("Video provider generated a video reference.", updates)
        }
        Err(err) => provider_error("Video provider failed", err),
    }
}

async fn execute_audio_generation(
    node: &WorkflowNode,
    inputs: &ConnectedInputs,
    providers: &ProviderRegistry,
) -> NodeOutcome {
    let provider_id = selected_provider_id(node, ProviderId::Replicate);
    let Some(provider) = providers.get(&provider_id) else {
        return provider_skipped(provider_id, "audio");
    };
    let prompt = prompt_for_provider_node(node, inputs);
    let model = model_for_node(node, "mock-audio");
    let request = AudioRequest {
        provider: provider_id,
        model,
        prompt: prompt.clone(),
        parameters: parameters_for_node(node),
    };

    match provider.generate_audio(request).await {
        Ok(response) => {
            let mut updates = provider_common_updates(response.provider, response.model);
            updates.insert("inputPrompt".to_string(), json!(prompt));
            updates.insert("outputAudio".to_string(), json!(response.audio));
            NodeOutcome::complete("Audio provider generated an audio reference.", updates)
        }
        Err(err) => provider_error("Audio provider failed", err),
    }
}

async fn execute_3d_generation(
    node: &WorkflowNode,
    inputs: &ConnectedInputs,
    providers: &ProviderRegistry,
) -> NodeOutcome {
    let provider_id = selected_provider_id(node, ProviderId::Replicate);
    let Some(provider) = providers.get(&provider_id) else {
        return provider_skipped(provider_id, "3D");
    };
    let prompt = prompt_for_provider_node(node, inputs);
    let model = model_for_node(node, "mock-3d");
    let input_images = input_images_for_node(node, inputs);
    let request = Model3dRequest {
        provider: provider_id,
        model,
        prompt: prompt.clone(),
        input_images: input_images.clone(),
        parameters: parameters_for_node(node),
    };

    match provider.generate_model3d(request).await {
        Ok(response) => {
            let mut updates = provider_common_updates(response.provider, response.model);
            updates.insert("inputPrompt".to_string(), json!(prompt));
            updates.insert("inputImages".to_string(), json!(input_images));
            updates.insert("output3dUrl".to_string(), json!(response.model_url));
            NodeOutcome::complete("3D provider generated a model reference.", updates)
        }
        Err(err) => provider_error("3D provider failed", err),
    }
}

fn provider_skipped(provider_id: ProviderId, capability: &str) -> NodeOutcome {
    NodeOutcome::skipped(format!(
        "{} provider `{}` is not registered. Local execution keeps secrets/platform calls out unless a provider backend is supplied.",
        capability,
        provider_id.display_name()
    ))
}

fn provider_error(prefix: &str, err: ProviderError) -> NodeOutcome {
    NodeOutcome::error(format!("{prefix}: {err}"), IndexMap::new())
}

fn provider_common_updates(provider: ProviderId, model: String) -> IndexMap<String, Value> {
    let mut updates = IndexMap::new();
    updates.insert("__providerUsed".to_string(), json!(provider.as_wire()));
    updates.insert("__modelUsed".to_string(), json!(model));
    updates
}

fn execute_prompt(node: &WorkflowNode, inputs: &ConnectedInputs) -> NodeOutcome {
    let prompt = inputs
        .text
        .clone()
        .or_else(|| string_field(&node.data, "prompt"))
        .or_else(|| string_field(&node.data, "text"))
        .unwrap_or_default();
    let mut updates = IndexMap::new();
    updates.insert("prompt".to_string(), json!(prompt));
    NodeOutcome::complete("Prompt text resolved.", updates)
}

fn execute_array(node: &WorkflowNode, inputs: &ConnectedInputs) -> NodeOutcome {
    let input_text = inputs
        .text
        .clone()
        .or_else(|| string_field(&node.data, "inputText"))
        .or_else(|| string_field(&node.data, "text"))
        .or_else(|| string_field(&node.data, "prompt"))
        .unwrap_or_default();
    let options = array_options_from_node(node);
    let parsed = parse_text_to_array(Some(&input_text), &options);
    let output_text = serde_json::to_string(&parsed.items).unwrap_or_else(|_| "[]".to_string());

    let mut updates = IndexMap::new();
    updates.insert("inputText".to_string(), json!(input_text));
    updates.insert("outputItems".to_string(), json!(parsed.items));
    updates.insert("outputText".to_string(), json!(output_text));

    if let Some(error) = parsed.error {
        NodeOutcome::error(error, updates)
    } else {
        NodeOutcome::complete("Array items parsed.", updates)
    }
}

fn execute_prompt_constructor(node: &WorkflowNode, inputs: &ConnectedInputs) -> NodeOutcome {
    let template = string_field(&node.data, "template")
        .or_else(|| string_field(&node.data, "prompt"))
        .unwrap_or_else(|| "@text".to_string());
    let mut output = template;

    if let Some(text) = inputs.text.as_deref() {
        output = output.replace("@text", text);
    }
    for (name, value) in &inputs.dynamic_inputs {
        let replacement = match value {
            DynamicInputValue::Single(value) => value.clone(),
            DynamicInputValue::Many(values) => values.join("\n"),
        };
        output = output.replace(&format!("@{name}"), &replacement);
    }

    let mut updates = IndexMap::new();
    updates.insert("outputText".to_string(), json!(output));
    NodeOutcome::complete("Prompt template constructed.", updates)
}

fn execute_annotation(node: &WorkflowNode, inputs: &ConnectedInputs) -> NodeOutcome {
    let mut updates = IndexMap::new();
    let source_image = inputs
        .images
        .first()
        .cloned()
        .or_else(|| string_field(&node.data, "sourceImage"));
    if let Some(image) = source_image {
        updates.insert("sourceImage".to_string(), json!(image));
        updates.insert("sourceImageRef".to_string(), Value::Null);
        let previous_output = string_field(&node.data, "outputImage");
        let previous_source = string_field(&node.data, "sourceImage");
        if previous_output.is_none() || previous_output == previous_source {
            updates.insert("outputImage".to_string(), json!(image));
            updates.insert("outputImageRef".to_string(), Value::Null);
        }
    }
    NodeOutcome::complete("Annotation pass-through complete.", updates)
}

fn execute_image_compare(node: &WorkflowNode, inputs: &ConnectedInputs) -> NodeOutcome {
    let image_a = inputs
        .images
        .first()
        .cloned()
        .or_else(|| string_field(&node.data, "imageA"));
    let image_b = inputs
        .images
        .get(1)
        .cloned()
        .or_else(|| string_field(&node.data, "imageB"));
    let mut updates = IndexMap::new();
    updates.insert(
        "imageA".to_string(),
        optional_string_value(image_a.as_deref()),
    );
    updates.insert(
        "imageB".to_string(),
        optional_string_value(image_b.as_deref()),
    );
    updates.insert(
        "outputImage".to_string(),
        optional_string_value(image_a.as_deref()),
    );
    if let Some(image) = image_a.as_deref()
        && let Ok(metadata) = inspect_inline_image(image)
    {
        updates.insert("imageAMetadata".to_string(), json!(metadata));
    }
    if let Some(image) = image_b.as_deref()
        && let Ok(metadata) = inspect_inline_image(image)
    {
        updates.insert("imageBMetadata".to_string(), json!(metadata));
    }

    let mut message = "Image compare metadata resolved.".to_string();
    if let (Some(left), Some(right)) = (image_a.as_deref(), image_b.as_deref()) {
        match compare_inline_images(left, right) {
            Ok(comparison) => {
                message = image_compare_summary(&comparison);
                updates.insert("comparison".to_string(), json!(comparison));
                updates.insert("outputText".to_string(), json!(message.clone()));
                updates.insert(
                    "__mediaAdapter".to_string(),
                    json!("rust-inline-image-compare"),
                );
            }
            Err(err) => {
                updates.insert("comparison".to_string(), Value::Null);
                updates.insert("comparisonError".to_string(), json!(err.to_string()));
                updates.insert(
                    "__mediaAdapter".to_string(),
                    json!("rust-inline-image-compare-unavailable"),
                );
                message = "Image compare pass-through complete; inline metric adapter unavailable."
                    .to_string();
            }
        }
    }

    NodeOutcome::complete(message, updates)
}

fn execute_split_grid(node: &WorkflowNode, inputs: &ConnectedInputs) -> NodeOutcome {
    let source_image = inputs
        .images
        .first()
        .cloned()
        .or_else(|| string_field(&node.data, "sourceImage"));
    let Some(source_image) = source_image else {
        return NodeOutcome::error(
            "Split grid requires a connected source image.".to_string(),
            IndexMap::new(),
        );
    };

    let rows = grid_dimension(node, "gridRows", 2);
    let cols = grid_dimension(node, "gridCols", 2);
    let target_count =
        integer_field(&node.data, "targetCount").and_then(|value| usize::try_from(value).ok());

    match split_inline_image_grid(&source_image, rows, cols, target_count) {
        Ok(result) => {
            let mut updates = IndexMap::new();
            updates.insert("sourceImage".to_string(), json!(source_image));
            updates.insert("sourceImageRef".to_string(), Value::Null);
            updates.insert("gridRows".to_string(), json!(result.rows));
            updates.insert("gridCols".to_string(), json!(result.cols));
            updates.insert("targetCount".to_string(), json!(result.images.len()));
            updates.insert("images".to_string(), json!(result.images));
            updates.insert("cells".to_string(), json!(result.cells));
            updates.insert("isConfigured".to_string(), json!(true));
            updates.insert(
                "__mediaAdapter".to_string(),
                json!("rust-inline-image-grid"),
            );
            NodeOutcome::complete("Split grid produced inline image cells.", updates)
        }
        Err(err) => NodeOutcome::error(format!("Split grid failed: {err}"), IndexMap::new()),
    }
}

fn execute_output(inputs: &ConnectedInputs) -> NodeOutcome {
    let mut updates = IndexMap::new();
    if let Some(text) = inputs.text.as_deref() {
        updates.insert("text".to_string(), json!(text));
    }
    if let Some(image) = inputs.images.first() {
        updates.insert("image".to_string(), json!(image));
    }
    if let Some(video) = inputs.videos.first() {
        updates.insert("video".to_string(), json!(video));
    }
    if let Some(audio) = inputs.audio.first() {
        updates.insert("audio".to_string(), json!(audio));
    }
    if let Some(model3d) = inputs.model3d.as_deref() {
        updates.insert("model3d".to_string(), json!(model3d));
    }
    NodeOutcome::complete("Output node collected upstream values.", updates)
}

fn execute_output_gallery(inputs: &ConnectedInputs) -> NodeOutcome {
    let mut updates = IndexMap::new();
    updates.insert("images".to_string(), json!(inputs.images));
    NodeOutcome::complete("Output gallery collected upstream images.", updates)
}

fn image_compare_summary(comparison: &InlineImageCompareResult) -> String {
    if !comparison.dimensions_match {
        return format!(
            "Image sizes differ: {}×{} vs {}×{}.",
            comparison.image_a.width,
            comparison.image_a.height,
            comparison.image_b.width,
            comparison.image_b.height
        );
    }
    if comparison.exact_match {
        return format!(
            "Images are an exact pixel match at {}×{}.",
            comparison.image_a.width, comparison.image_a.height
        );
    }
    let Some(difference) = comparison.difference.as_ref() else {
        return "Image comparison completed without pixel metrics.".to_string();
    };
    format!(
        "Images share {}×{} dimensions; {} of {} pixels changed ({:.2}%), MAE {:.2}, max delta {}.",
        comparison.image_a.width,
        comparison.image_a.height,
        difference.changed_pixels,
        difference.pixel_count,
        difference.changed_pixel_ratio * 100.0,
        difference.mean_absolute_error,
        difference.max_channel_delta
    )
}

fn llm_provider_id(node: &WorkflowNode) -> ProviderId {
    string_field(&node.data, "provider")
        .map(|value| match value.as_str() {
            "google" => ProviderId::Gemini,
            other => ProviderId::from_wire(other),
        })
        .unwrap_or(ProviderId::Gemini)
}

fn selected_provider_id(node: &WorkflowNode, default: ProviderId) -> ProviderId {
    nested_string_field(&node.data, "selectedModel", "provider")
        .or_else(|| string_field(&node.data, "provider"))
        .map(|value| ProviderId::from_wire(&value))
        .unwrap_or(default)
}

fn model_for_node(node: &WorkflowNode, fallback: &str) -> String {
    nested_string_field(&node.data, "selectedModel", "modelId")
        .or_else(|| string_field(&node.data, "model"))
        .unwrap_or_else(|| fallback.to_string())
}

fn prompt_for_provider_node(node: &WorkflowNode, inputs: &ConnectedInputs) -> String {
    inputs
        .text
        .clone()
        .or_else(|| string_field(&node.data, "inputPrompt"))
        .or_else(|| string_field(&node.data, "prompt"))
        .or_else(|| string_field(&node.data, "text"))
        .unwrap_or_default()
}

fn input_images_for_node(node: &WorkflowNode, inputs: &ConnectedInputs) -> Vec<String> {
    if !inputs.images.is_empty() {
        return inputs.images.clone();
    }
    string_array_field(&node.data, "inputImages")
}

fn parameters_for_node(node: &WorkflowNode) -> Value {
    node.data
        .get("parameters")
        .cloned()
        .unwrap_or_else(|| json!({}))
}

fn array_options_from_node(node: &WorkflowNode) -> ParseArrayOptions {
    ParseArrayOptions {
        split_mode: SplitMode::from_wire(string_field(&node.data, "splitMode").as_deref()),
        delimiter: string_field(&node.data, "delimiter"),
        regex_pattern: string_field(&node.data, "regexPattern"),
        trim_items: bool_field(&node.data, "trimItems").unwrap_or(true),
        remove_empty: bool_field(&node.data, "removeEmpty").unwrap_or(true),
    }
}

fn grid_dimension(node: &WorkflowNode, key: &str, fallback: u32) -> u32 {
    integer_field(&node.data, key)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)
        .unwrap_or(fallback)
}

fn optional_string_value(value: Option<&str>) -> Value {
    value.map_or(Value::Null, |value| json!(value))
}

fn apply_updates(node: &mut WorkflowNode, updates: IndexMap<String, Value>) {
    for (key, value) in updates {
        set_data_field(node, &key, value);
    }
}

fn set_status(node: &mut WorkflowNode, status: NodeStatusWire) {
    set_data_field(node, "status", json!(status.as_str()));
}

fn set_data_field(node: &mut WorkflowNode, key: &str, value: Value) {
    if !node.data.is_object() {
        node.data = json!({});
    }
    if let Some(map) = node.data.as_object_mut() {
        map.insert(key.to_string(), value);
    }
}

fn string_field(data: &Value, key: &str) -> Option<String> {
    data.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn bool_field(data: &Value, key: &str) -> Option<bool> {
    data.get(key).and_then(Value::as_bool)
}

fn number_field(data: &Value, key: &str) -> Option<f64> {
    data.get(key).and_then(Value::as_f64)
}

fn integer_field(data: &Value, key: &str) -> Option<u64> {
    data.get(key).and_then(Value::as_u64)
}

fn nested_string_field(data: &Value, object_key: &str, field_key: &str) -> Option<String> {
    data.get(object_key)
        .and_then(|object| object.get(field_key))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn string_array_field(data: &Value, key: &str) -> Vec<String> {
    data.get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gemed_core::{GroupColor, NodeGroup, Position, Size, WorkflowEdge};
    use indexmap::IndexMap;

    #[test]
    fn executes_prompt_array_output_flow() {
        let workflow = WorkflowFile {
            name: "simple".to_string(),
            nodes: vec![
                WorkflowNode::new(
                    "prompt",
                    NodeType::Prompt,
                    Position { x: 0.0, y: 0.0 },
                    json!({"text":"one\ntwo"}),
                ),
                WorkflowNode::new(
                    "array",
                    NodeType::Array,
                    Position { x: 0.0, y: 0.0 },
                    json!({"splitMode":"newline"}),
                ),
                WorkflowNode::new(
                    "output",
                    NodeType::Output,
                    Position { x: 0.0, y: 0.0 },
                    json!({}),
                ),
            ],
            edges: vec![
                WorkflowEdge::new("e1", "prompt", "array"),
                WorkflowEdge::new("e2", "array", "output"),
            ],
            ..WorkflowFile::blank()
        };

        let result = execute_simple_workflow(&workflow).expect("executes");
        let output = result
            .workflow
            .nodes
            .iter()
            .find(|node| node.id == "output")
            .unwrap();
        assert_eq!(
            output.data.get("text").and_then(Value::as_str),
            Some("[\"one\",\"two\"]")
        );
        assert_eq!(result.report.error_count(), 0);
    }

    #[test]
    fn provider_nodes_are_explicitly_skipped() {
        let workflow = WorkflowFile::example();
        let result = execute_simple_workflow(&workflow).expect("executes");
        assert!(result.report.skipped_count() >= 1);
    }

    #[test]
    fn locked_group_nodes_are_skipped_and_do_not_feed_downstream_inputs() {
        let mut prompt = WorkflowNode::new(
            "prompt",
            NodeType::Prompt,
            Position { x: 0.0, y: 0.0 },
            json!({"prompt":"locked text"}),
        );
        prompt.group_id = Some("locked-group".to_string());
        let workflow = WorkflowFile {
            name: "locked".to_string(),
            nodes: vec![
                prompt,
                WorkflowNode::new(
                    "output",
                    NodeType::Output,
                    Position { x: 100.0, y: 0.0 },
                    json!({}),
                ),
            ],
            edges: vec![WorkflowEdge::new("e1", "prompt", "output")],
            groups: IndexMap::from([(
                "locked-group".to_string(),
                NodeGroup {
                    id: "locked-group".to_string(),
                    name: "Locked".to_string(),
                    color: GroupColor::Neutral,
                    position: Position { x: 0.0, y: 0.0 },
                    size: Size {
                        width: 260.0,
                        height: 180.0,
                    },
                    locked: Some(true),
                    is_nbp_input: None,
                    extra: IndexMap::new(),
                },
            )]),
            ..WorkflowFile::blank()
        };

        let result = execute_simple_workflow(&workflow).expect("executes");
        let prompt = result
            .workflow
            .nodes
            .iter()
            .find(|node| node.id == "prompt")
            .unwrap();
        let output = result
            .workflow
            .nodes
            .iter()
            .find(|node| node.id == "output")
            .unwrap();

        assert_eq!(
            prompt.data.get("status").and_then(Value::as_str),
            Some("skipped")
        );
        assert_eq!(output.data.get("text").and_then(Value::as_str), None);
        assert_eq!(result.report.skipped_count(), 1);
    }

    #[test]
    fn mock_llm_provider_executes_text_generation() {
        let workflow = WorkflowFile {
            name: "mock llm".to_string(),
            nodes: vec![
                WorkflowNode::new(
                    "prompt",
                    NodeType::Prompt,
                    Position { x: 0.0, y: 0.0 },
                    json!({"prompt":"describe gemed"}),
                ),
                WorkflowNode::new(
                    "llm",
                    NodeType::LlmGenerate,
                    Position { x: 0.0, y: 0.0 },
                    json!({"provider":"mock","model":"mock-llm"}),
                ),
                WorkflowNode::new(
                    "output",
                    NodeType::Output,
                    Position { x: 0.0, y: 0.0 },
                    json!({}),
                ),
            ],
            edges: vec![
                WorkflowEdge::new("e1", "prompt", "llm"),
                WorkflowEdge::new("e2", "llm", "output"),
            ],
            ..WorkflowFile::blank()
        };

        let providers = ProviderRegistry::mock_all();
        let result =
            execute_workflow_with_providers(&workflow, &providers).expect("mock provider executes");
        let output = result
            .workflow
            .nodes
            .iter()
            .find(|node| node.id == "output")
            .unwrap();
        assert_eq!(
            output.data.get("text").and_then(Value::as_str),
            Some("[mock:mock:mock-llm] describe gemed")
        );
        assert_eq!(result.report.skipped_count(), 0);
    }

    #[test]
    fn provider_sample_runs_all_llm_routes_with_mock_registry() {
        let workflow = WorkflowFile::llm_provider_example();
        let providers = ProviderRegistry::mock_all();

        let result =
            execute_workflow_with_providers(&workflow, &providers).expect("mock providers execute");

        assert_eq!(result.report.error_count(), 0);
        assert_eq!(result.report.skipped_count(), 0);
        for (node_id, provider, model) in [
            ("provider_gemini_output", "gemini", "gemini-3.5-flash"),
            ("provider_openai_output", "openai", "gpt-5.5"),
            (
                "provider_anthropic_output",
                "anthropic",
                "claude-sonnet-4-6",
            ),
        ] {
            let output = result
                .workflow
                .nodes
                .iter()
                .find(|node| node.id == node_id)
                .expect("provider output node exists");
            let text = output
                .data
                .get("text")
                .and_then(Value::as_str)
                .expect("provider output text is routed");
            assert!(
                text.starts_with(&format!("[mock:{provider}:{model}]")),
                "unexpected output for {node_id}: {text}"
            );
        }
    }

    #[test]
    fn mock_image_provider_executes_generation_node() {
        let workflow = WorkflowFile {
            name: "mock image".to_string(),
            nodes: vec![
                WorkflowNode::new(
                    "prompt",
                    NodeType::Prompt,
                    Position { x: 0.0, y: 0.0 },
                    json!({"prompt":"blue gem"}),
                ),
                WorkflowNode::new(
                    "image",
                    NodeType::NanoBanana,
                    Position { x: 0.0, y: 0.0 },
                    json!({
                        "selectedModel": {
                            "provider": "mock",
                            "modelId": "mock-image",
                            "displayName": "Mock Image"
                        }
                    }),
                ),
                WorkflowNode::new(
                    "output",
                    NodeType::Output,
                    Position { x: 0.0, y: 0.0 },
                    json!({}),
                ),
            ],
            edges: vec![
                WorkflowEdge::new("e1", "prompt", "image"),
                WorkflowEdge::new("e2", "image", "output"),
            ],
            ..WorkflowFile::blank()
        };

        let providers = ProviderRegistry::mock_all();
        let result =
            execute_workflow_with_providers(&workflow, &providers).expect("mock provider executes");
        let output = result
            .workflow
            .nodes
            .iter()
            .find(|node| node.id == "output")
            .unwrap();
        assert!(
            output
                .data
                .get("image")
                .and_then(Value::as_str)
                .is_some_and(|value| value.starts_with("mock://image/mock/mock-image"))
        );
        assert_eq!(result.report.skipped_count(), 0);
    }

    #[test]
    fn image_compare_resolves_connected_images_and_can_feed_output() {
        let image_a = "data:image/png;base64,a";
        let image_b = "data:image/png;base64,b";
        let workflow = WorkflowFile {
            name: "compare".to_string(),
            nodes: vec![
                WorkflowNode::new(
                    "a",
                    NodeType::ImageInput,
                    Position { x: 0.0, y: 0.0 },
                    json!({"image": image_a}),
                ),
                WorkflowNode::new(
                    "b",
                    NodeType::ImageInput,
                    Position { x: 0.0, y: 0.0 },
                    json!({"image": image_b}),
                ),
                WorkflowNode::new(
                    "cmp",
                    NodeType::ImageCompare,
                    Position { x: 0.0, y: 0.0 },
                    json!({}),
                ),
                WorkflowNode::new(
                    "output",
                    NodeType::Output,
                    Position { x: 0.0, y: 0.0 },
                    json!({}),
                ),
            ],
            edges: vec![
                WorkflowEdge::new("e1", "a", "cmp"),
                WorkflowEdge::new("e2", "b", "cmp"),
                WorkflowEdge::new("e3", "cmp", "output"),
            ],
            ..WorkflowFile::blank()
        };

        let result = execute_simple_workflow(&workflow).expect("executes compare");
        let compare = result
            .workflow
            .nodes
            .iter()
            .find(|node| node.id == "cmp")
            .unwrap();
        let output = result
            .workflow
            .nodes
            .iter()
            .find(|node| node.id == "output")
            .unwrap();

        assert_eq!(
            compare.data.get("imageA").and_then(Value::as_str),
            Some(image_a)
        );
        assert_eq!(
            compare.data.get("imageB").and_then(Value::as_str),
            Some(image_b)
        );
        assert_eq!(
            compare.data.get("outputImage").and_then(Value::as_str),
            Some(image_a)
        );
        assert_eq!(
            output.data.get("image").and_then(Value::as_str),
            Some(image_a)
        );
        assert_eq!(result.report.skipped_count(), 0);
    }

    #[test]
    fn image_compare_computes_inline_image_metrics_when_decodable() {
        let red = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";
        let blue = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNgYPj/HwADAgH/5ncLrgAAAABJRU5ErkJggg==";
        let workflow = WorkflowFile {
            name: "compare metrics".to_string(),
            nodes: vec![
                WorkflowNode::new(
                    "a",
                    NodeType::ImageInput,
                    Position { x: 0.0, y: 0.0 },
                    json!({"image": red}),
                ),
                WorkflowNode::new(
                    "b",
                    NodeType::ImageInput,
                    Position { x: 0.0, y: 0.0 },
                    json!({"image": blue}),
                ),
                WorkflowNode::new(
                    "cmp",
                    NodeType::ImageCompare,
                    Position { x: 0.0, y: 0.0 },
                    json!({}),
                ),
            ],
            edges: vec![
                WorkflowEdge::with_handles("e1", "a", "cmp", "image", "image-0"),
                WorkflowEdge::with_handles("e2", "b", "cmp", "image", "image-1"),
            ],
            ..WorkflowFile::blank()
        };

        let result = execute_simple_workflow(&workflow).expect("executes compare metrics");
        let compare = result
            .workflow
            .nodes
            .iter()
            .find(|node| node.id == "cmp")
            .unwrap();
        let comparison = compare
            .data
            .get("comparison")
            .expect("comparison metrics are stored");

        assert_eq!(
            compare.data.get("__mediaAdapter").and_then(Value::as_str),
            Some("rust-inline-image-compare")
        );
        assert_eq!(
            comparison.get("dimensionsMatch").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            comparison.get("exactMatch").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            comparison
                .get("difference")
                .and_then(|difference| difference.get("changedPixels"))
                .and_then(Value::as_u64),
            Some(1)
        );
        assert!(
            compare
                .data
                .get("outputText")
                .and_then(Value::as_str)
                .is_some_and(|message| message.contains("1 of 1 pixels changed"))
        );
        assert_eq!(result.report.error_count(), 0);
    }

    #[test]
    fn split_grid_splits_inline_image_and_routes_selected_cell() {
        let source = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAFElEQVR4nGP4z8DwHwyBNBAw/AcAR8oI+ItOQ4UAAAAASUVORK5CYII=";
        let workflow = WorkflowFile {
            name: "split".to_string(),
            nodes: vec![
                WorkflowNode::new(
                    "input",
                    NodeType::ImageInput,
                    Position { x: 0.0, y: 0.0 },
                    json!({"image": source}),
                ),
                WorkflowNode::new(
                    "split",
                    NodeType::SplitGrid,
                    Position { x: 0.0, y: 0.0 },
                    json!({"gridRows":2,"gridCols":2,"targetCount":3}),
                ),
                WorkflowNode::new(
                    "output",
                    NodeType::Output,
                    Position { x: 0.0, y: 0.0 },
                    json!({}),
                ),
            ],
            edges: vec![
                WorkflowEdge::new("e1", "input", "split"),
                WorkflowEdge {
                    source_handle: Some("image-1".to_string()),
                    target_handle: Some("image".to_string()),
                    ..WorkflowEdge::new("e2", "split", "output")
                },
            ],
            ..WorkflowFile::blank()
        };

        let result = execute_simple_workflow(&workflow).expect("executes split grid");
        let split = result
            .workflow
            .nodes
            .iter()
            .find(|node| node.id == "split")
            .unwrap();
        let output = result
            .workflow
            .nodes
            .iter()
            .find(|node| node.id == "output")
            .unwrap();
        let images = split
            .data
            .get("images")
            .and_then(Value::as_array)
            .expect("split images are stored");

        assert_eq!(images.len(), 3);
        assert!(images.iter().all(|image| {
            image
                .as_str()
                .unwrap()
                .starts_with("data:image/png;base64,")
        }));
        assert_eq!(
            split.data.get("__mediaAdapter").and_then(Value::as_str),
            Some("rust-inline-image-grid")
        );
        assert_eq!(output.data.get("image"), images.get(1));
        assert_eq!(result.report.error_count(), 0);
        assert_eq!(result.report.skipped_count(), 0);
    }

    #[test]
    fn split_grid_reports_non_inline_project_reference_gap() {
        let workflow = WorkflowFile {
            name: "split ref".to_string(),
            nodes: vec![WorkflowNode::new(
                "split",
                NodeType::SplitGrid,
                Position { x: 0.0, y: 0.0 },
                json!({
                    "sourceImage": "gemed-media://media/input.png",
                    "gridRows": 2,
                    "gridCols": 2
                }),
            )],
            ..WorkflowFile::blank()
        };

        let result = execute_simple_workflow(&workflow).expect("execution records node error");
        let split = result
            .workflow
            .nodes
            .iter()
            .find(|node| node.id == "split")
            .unwrap();

        assert_eq!(
            split.data.get("status").and_then(Value::as_str),
            Some("error")
        );
        assert!(
            split
                .data
                .get("error")
                .and_then(Value::as_str)
                .is_some_and(|message| message.contains("only inline image data URLs"))
        );
        assert_eq!(result.report.error_count(), 1);
    }
}
