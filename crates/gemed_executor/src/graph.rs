use gemed_core::{NodeType, WorkflowEdge, WorkflowFile, WorkflowNode, is_node_in_locked_group};
use indexmap::IndexMap;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConnectedInputs {
    pub images: Vec<String>,
    pub videos: Vec<String>,
    pub audio: Vec<String>,
    pub model3d: Option<String>,
    pub text: Option<String>,
    pub text_items: Vec<String>,
    pub dynamic_inputs: IndexMap<String, DynamicInputValue>,
    pub ease_curve: Option<EaseCurveInput>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DynamicInputValue {
    Single(String),
    Many(Vec<String>),
}

impl DynamicInputValue {
    fn push(&mut self, value: String) {
        match self {
            Self::Single(existing) => {
                *self = Self::Many(vec![std::mem::take(existing), value]);
            }
            Self::Many(values) => values.push(value),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EaseCurveInput {
    pub bezier_handles: [f64; 4],
    pub easing_preset: Option<String>,
    pub output_duration: f64,
}

impl Eq for EaseCurveInput {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceOutputKind {
    Image,
    Text,
    Video,
    Audio,
    Model3d,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceOutput {
    pub kind: SourceOutputKind,
    pub value: String,
}

#[derive(Debug, Error)]
pub enum GraphError {
    #[error("workflow validation failed: {0}")]
    InvalidWorkflow(#[from] gemed_core::WorkflowError),
    #[error("workflow contains a cycle involving node `{0}`")]
    Cycle(String),
}

pub fn execution_order(workflow: &WorkflowFile) -> Result<Vec<String>, GraphError> {
    workflow.validate()?;

    let mut indegree: HashMap<&str, usize> = workflow
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), 0usize))
        .collect();
    let mut outgoing: HashMap<&str, Vec<&str>> = HashMap::new();

    for edge in workflow
        .edges
        .iter()
        .filter(|edge| !edge.data.is_loop.unwrap_or(false))
    {
        if let Some(target_degree) = indegree.get_mut(edge.target.as_str()) {
            *target_degree += 1;
        }
        outgoing
            .entry(edge.source.as_str())
            .or_default()
            .push(edge.target.as_str());
    }

    let mut ready: Vec<&str> = workflow
        .nodes
        .iter()
        .filter_map(|node| (indegree[node.id.as_str()] == 0).then_some(node.id.as_str()))
        .collect();
    ready.reverse();

    let mut order = Vec::with_capacity(workflow.nodes.len());
    while let Some(node_id) = ready.pop() {
        order.push(node_id.to_string());
        for target in outgoing.get(node_id).into_iter().flatten() {
            if let Some(degree) = indegree.get_mut(target) {
                *degree = degree.saturating_sub(1);
                if *degree == 0 {
                    ready.push(target);
                }
            }
        }
    }

    if order.len() != workflow.nodes.len() {
        let cyclic = indegree
            .into_iter()
            .find_map(|(id, degree)| (degree > 0).then_some(id.to_string()))
            .unwrap_or_else(|| "unknown".to_string());
        return Err(GraphError::Cycle(cyclic));
    }

    Ok(order)
}

pub fn connected_inputs(workflow: &WorkflowFile, node_id: &str) -> ConnectedInputs {
    let node_map: HashMap<&str, &WorkflowNode> = workflow
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    let incoming_by_target = incoming_edges_by_target(&workflow.edges);
    let locked_node_ids: HashSet<String> = workflow
        .nodes
        .iter()
        .filter(|node| is_node_in_locked_group(workflow, &node.id))
        .map(|node| node.id.clone())
        .collect();
    connected_inputs_inner(
        node_id,
        &node_map,
        &incoming_by_target,
        &locked_node_ids,
        &mut HashSet::new(),
        &mut HashMap::new(),
    )
}

fn connected_inputs_inner<'a>(
    node_id: &str,
    node_map: &HashMap<&'a str, &'a WorkflowNode>,
    incoming_by_target: &HashMap<&'a str, Vec<&'a WorkflowEdge>>,
    locked_node_ids: &HashSet<String>,
    visited: &mut HashSet<String>,
    passthrough_cache: &mut HashMap<String, ConnectedInputs>,
) -> ConnectedInputs {
    if !visited.insert(node_id.to_string()) {
        return ConnectedInputs::default();
    }

    let mut inputs = ConnectedInputs::default();
    let handle_to_schema_name = node_map
        .get(node_id)
        .map_or_else(IndexMap::new, |node| handle_schema_names(&node.data));

    for edge in incoming_by_target
        .get(node_id)
        .into_iter()
        .flatten()
        .copied()
    {
        if edge.data.is_loop.unwrap_or(false) {
            continue;
        }
        let Some(source) = node_map.get(edge.source.as_str()).copied() else {
            continue;
        };
        if locked_node_ids.contains(source.id.as_str()) {
            continue;
        }

        if source.node_type == NodeType::Array
            && bool_field(&source.data, "batchMode") == Some(true)
        {
            let items = string_array_field(&source.data, "outputItems");
            if !items.is_empty() {
                if inputs.text.is_none() {
                    inputs.text = items.first().cloned();
                }
                inputs.text_items.extend(items);
            }
            continue;
        }

        match source.node_type {
            NodeType::Router => {
                let router_inputs = passthrough_inputs(
                    source.id.as_str(),
                    node_map,
                    incoming_by_target,
                    locked_node_ids,
                    visited,
                    passthrough_cache,
                );
                route_passthrough(&mut inputs, &router_inputs, edge.source_handle.as_deref());
            }
            NodeType::Switch => {
                if switch_output_enabled(source, edge.source_handle.as_deref()) {
                    let switch_inputs = passthrough_inputs(
                        source.id.as_str(),
                        node_map,
                        incoming_by_target,
                        locked_node_ids,
                        visited,
                        passthrough_cache,
                    );
                    let edge_type = string_field(&source.data, "inputType");
                    route_passthrough(&mut inputs, &switch_inputs, edge_type.as_deref());
                }
            }
            NodeType::ConditionalSwitch => {
                // The legacy conditional switch is currently a gate/trigger; it does not pass
                // upstream data through in getConnectedInputsPure. Preserve that behavior here.
                if conditional_output_active(source, edge.source_handle.as_deref()) {
                    continue;
                }
            }
            _ => {
                if let Some(output) = source_output(source, edge.source_handle.as_deref(), edge) {
                    map_dynamic_input(
                        &mut inputs,
                        edge.target_handle.as_deref(),
                        &handle_to_schema_name,
                        &output.value,
                    );
                    route_source_output(&mut inputs, output, edge.target_handle.as_deref());
                }
            }
        }
    }

    if inputs.ease_curve.is_none()
        && let Some(ease_edge) =
            incoming_by_target
                .get(node_id)
                .into_iter()
                .flatten()
                .find(|edge| {
                    edge.target_handle.as_deref() == Some("easeCurve")
                        && !edge.data.is_loop.unwrap_or(false)
                })
        && let Some(source) = node_map.get(ease_edge.source.as_str()).copied()
        && source.node_type == NodeType::EaseCurve
        && !locked_node_ids.contains(source.id.as_str())
    {
        inputs.ease_curve = ease_curve_from_node(source);
    }

    visited.remove(node_id);
    inputs
}

fn passthrough_inputs<'a>(
    node_id: &str,
    node_map: &HashMap<&'a str, &'a WorkflowNode>,
    incoming_by_target: &HashMap<&'a str, Vec<&'a WorkflowEdge>>,
    locked_node_ids: &HashSet<String>,
    visited: &mut HashSet<String>,
    passthrough_cache: &mut HashMap<String, ConnectedInputs>,
) -> ConnectedInputs {
    if let Some(cached) = passthrough_cache.get(node_id) {
        return cached.clone();
    }
    let result = connected_inputs_inner(
        node_id,
        node_map,
        incoming_by_target,
        locked_node_ids,
        visited,
        passthrough_cache,
    );
    passthrough_cache.insert(node_id.to_string(), result.clone());
    result
}

fn incoming_edges_by_target(edges: &[WorkflowEdge]) -> HashMap<&str, Vec<&WorkflowEdge>> {
    let mut incoming: HashMap<&str, Vec<&WorkflowEdge>> = HashMap::new();
    for edge in edges {
        incoming.entry(edge.target.as_str()).or_default().push(edge);
    }
    incoming
}

fn source_output(
    source: &WorkflowNode,
    source_handle: Option<&str>,
    edge: &WorkflowEdge,
) -> Option<SourceOutput> {
    match source.node_type {
        NodeType::ImageInput => string_field(&source.data, "image").map(image_output),
        NodeType::VideoInput => string_field(&source.data, "video").map(video_output),
        NodeType::AudioInput => string_field(&source.data, "audioFile").map(audio_output),
        NodeType::Annotation => string_field(&source.data, "outputImage").map(image_output),
        NodeType::NanoBanana => string_field(&source.data, "outputImage").map(image_output),
        NodeType::Generate3d => string_field(&source.data, "output3dUrl").map(model3d_output),
        NodeType::GenerateVideo
        | NodeType::VideoStitch
        | NodeType::EaseCurve
        | NodeType::VideoTrim => string_field(&source.data, "outputVideo").map(video_output),
        NodeType::GenerateAudio => string_field(&source.data, "outputAudio").map(audio_output),
        NodeType::Prompt => string_field(&source.data, "prompt")
            .or_else(|| string_field(&source.data, "text"))
            .map(text_output),
        NodeType::Array => array_output(source, source_handle, edge).map(text_output),
        NodeType::PromptConstructor => string_field(&source.data, "outputText")
            .or_else(|| string_field(&source.data, "constructedPrompt"))
            .map(text_output),
        NodeType::LlmGenerate => string_field(&source.data, "outputText").map(text_output),
        NodeType::Output => string_field(&source.data, "text").map(text_output),
        NodeType::OutputGallery
        | NodeType::ImageCompare
        | NodeType::SplitGrid
        | NodeType::Router
        | NodeType::Switch
        | NodeType::ConditionalSwitch
        | NodeType::GlbViewer
        | NodeType::VideoFrameGrab
        | NodeType::Unknown => None,
    }
}

fn array_output(
    source: &WorkflowNode,
    source_handle: Option<&str>,
    edge: &WorkflowEdge,
) -> Option<String> {
    let items = string_array_field(&source.data, "outputItems");
    if items.is_empty() {
        return string_field(&source.data, "outputText");
    }

    if let Some(index) =
        number_field(&edge.data.extra_value(), "arrayItemIndex").and_then(non_negative_integer)
    {
        return items.get(index % items.len()).cloned();
    }

    if let Some(index) = source_handle
        .and_then(|handle| handle.strip_prefix("text-"))
        .and_then(|value| value.parse::<usize>().ok())
    {
        return items.get(index).cloned();
    }

    string_field(&source.data, "outputText").or_else(|| items.first().cloned())
}

fn image_output(value: String) -> SourceOutput {
    SourceOutput {
        kind: SourceOutputKind::Image,
        value,
    }
}

fn text_output(value: String) -> SourceOutput {
    SourceOutput {
        kind: SourceOutputKind::Text,
        value,
    }
}

fn video_output(value: String) -> SourceOutput {
    SourceOutput {
        kind: SourceOutputKind::Video,
        value,
    }
}

fn audio_output(value: String) -> SourceOutput {
    SourceOutput {
        kind: SourceOutputKind::Audio,
        value,
    }
}

fn model3d_output(value: String) -> SourceOutput {
    SourceOutput {
        kind: SourceOutputKind::Model3d,
        value,
    }
}

fn route_source_output(
    inputs: &mut ConnectedInputs,
    output: SourceOutput,
    target_handle: Option<&str>,
) {
    match output.kind {
        SourceOutputKind::Model3d => inputs.model3d = Some(output.value),
        SourceOutputKind::Video => inputs.videos.push(output.value),
        SourceOutputKind::Audio => inputs.audio.push(output.value),
        SourceOutputKind::Text if is_text_handle(target_handle) || target_handle.is_some() => {
            inputs.text = Some(output.value);
        }
        SourceOutputKind::Text => inputs.text = Some(output.value),
        SourceOutputKind::Image if is_image_handle(target_handle) || target_handle.is_none() => {
            inputs.images.push(output.value);
        }
        SourceOutputKind::Image => inputs.images.push(output.value),
    }
}

fn route_passthrough(
    inputs: &mut ConnectedInputs,
    passthrough: &ConnectedInputs,
    edge_type: Option<&str>,
) {
    match edge_type {
        Some("image") => inputs.images.extend(passthrough.images.clone()),
        Some("text") => inputs.text = passthrough.text.clone(),
        Some("video") => inputs.videos.extend(passthrough.videos.clone()),
        Some("audio") => inputs.audio.extend(passthrough.audio.clone()),
        Some("3d") => inputs.model3d = passthrough.model3d.clone(),
        Some("easeCurve") => inputs.ease_curve = passthrough.ease_curve.clone(),
        Some(handle) if is_text_handle(Some(handle)) => inputs.text = passthrough.text.clone(),
        Some(handle) if is_image_handle(Some(handle)) => {
            inputs.images.extend(passthrough.images.clone())
        }
        _ => {
            inputs.images.extend(passthrough.images.clone());
            if inputs.text.is_none() {
                inputs.text = passthrough.text.clone();
            }
            inputs.videos.extend(passthrough.videos.clone());
            inputs.audio.extend(passthrough.audio.clone());
            if inputs.model3d.is_none() {
                inputs.model3d = passthrough.model3d.clone();
            }
            if inputs.ease_curve.is_none() {
                inputs.ease_curve = passthrough.ease_curve.clone();
            }
        }
    }
}

fn map_dynamic_input(
    inputs: &mut ConnectedInputs,
    handle_id: Option<&str>,
    handle_to_schema_name: &IndexMap<String, String>,
    value: &str,
) {
    let Some(schema_name) = handle_id.and_then(|handle| handle_to_schema_name.get(handle)) else {
        return;
    };
    match inputs.dynamic_inputs.get_mut(schema_name) {
        Some(existing) => existing.push(value.to_string()),
        None => {
            inputs.dynamic_inputs.insert(
                schema_name.clone(),
                DynamicInputValue::Single(value.to_string()),
            );
        }
    }
}

fn handle_schema_names(data: &Value) -> IndexMap<String, String> {
    let mut handle_to_schema_name = IndexMap::new();
    let Some(schema) = data.get("inputSchema").and_then(Value::as_array) else {
        return handle_to_schema_name;
    };

    for schema_type in ["image", "text", "audio"] {
        let inputs: Vec<&Value> = schema
            .iter()
            .filter(|item| item.get("type").and_then(Value::as_str) == Some(schema_type))
            .collect();
        for (index, input) in inputs.into_iter().enumerate() {
            if let Some(name) = input.get("name").and_then(Value::as_str) {
                let prefix = match schema_type {
                    "audio" => "audio",
                    "text" => "text",
                    _ => "image",
                };
                handle_to_schema_name.insert(format!("{prefix}-{index}"), name.to_string());
                if index == 0 {
                    handle_to_schema_name.insert(prefix.to_string(), name.to_string());
                }
            }
        }
    }

    handle_to_schema_name
}

fn switch_output_enabled(source: &WorkflowNode, source_handle: Option<&str>) -> bool {
    let Some(switches) = source.data.get("switches").and_then(Value::as_array) else {
        return false;
    };
    switches.iter().any(|switch| {
        switch.get("id").and_then(Value::as_str) == source_handle
            && switch
                .get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false)
    })
}

fn conditional_output_active(source: &WorkflowNode, source_handle: Option<&str>) -> bool {
    if bool_field(&source.data, "evaluationPaused") == Some(true) {
        return true;
    }
    let Some(rules) = source.data.get("rules").and_then(Value::as_array) else {
        return false;
    };
    if let Some(handle) = source_handle {
        if handle == "default" {
            return !rules.iter().any(|rule| {
                rule.get("isMatched")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            });
        }
        return rules.iter().any(|rule| {
            rule.get("id").and_then(Value::as_str) == Some(handle)
                && rule
                    .get("isMatched")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
        });
    }
    false
}

fn ease_curve_from_node(node: &WorkflowNode) -> Option<EaseCurveInput> {
    let handles = node
        .data
        .get("bezierHandles")
        .and_then(Value::as_array)
        .and_then(|values| {
            (values.len() == 4).then(|| {
                [
                    values[0].as_f64().unwrap_or(0.25),
                    values[1].as_f64().unwrap_or(0.1),
                    values[2].as_f64().unwrap_or(0.25),
                    values[3].as_f64().unwrap_or(1.0),
                ]
            })
        })?;
    Some(EaseCurveInput {
        bezier_handles: handles,
        easing_preset: string_field(&node.data, "easingPreset"),
        output_duration: node
            .data
            .get("outputDuration")
            .and_then(Value::as_f64)
            .unwrap_or(0.0),
    })
}

fn is_image_handle(handle_id: Option<&str>) -> bool {
    handle_id.is_some_and(|handle| {
        handle == "image" || handle.starts_with("image-") || handle.contains("frame")
    })
}

fn is_text_handle(handle_id: Option<&str>) -> bool {
    handle_id.is_some_and(|handle| {
        handle == "text" || handle.starts_with("text-") || handle.contains("prompt")
    })
}

fn string_field(data: &Value, key: &str) -> Option<String> {
    data.get(key)
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
        .map(ToOwned::to_owned)
        .collect()
}

fn bool_field(data: &Value, key: &str) -> Option<bool> {
    data.get(key).and_then(Value::as_bool)
}

fn number_field(data: &Value, key: &str) -> Option<f64> {
    data.get(key).and_then(Value::as_f64)
}

fn non_negative_integer(value: f64) -> Option<usize> {
    (value >= 0.0 && value.fract() == 0.0).then_some(value as usize)
}

trait EdgeDataValue {
    fn extra_value(&self) -> Value;
}

impl EdgeDataValue for gemed_core::WorkflowEdgeData {
    fn extra_value(&self) -> Value {
        let mut map = serde_json::Map::new();
        for (key, value) in &self.extra {
            map.insert(key.clone(), value.clone());
        }
        Value::Object(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gemed_core::{Position, WorkflowEdge, WorkflowNode};

    #[test]
    fn orders_nodes_topologically() {
        let workflow = WorkflowFile {
            nodes: vec![
                WorkflowNode::new(
                    "a",
                    NodeType::Prompt,
                    Position { x: 0.0, y: 0.0 },
                    serde_json::json!({}),
                ),
                WorkflowNode::new(
                    "b",
                    NodeType::Output,
                    Position { x: 0.0, y: 0.0 },
                    serde_json::json!({}),
                ),
            ],
            edges: vec![WorkflowEdge::new("e", "a", "b")],
            ..WorkflowFile::blank()
        };
        assert_eq!(execution_order(&workflow).unwrap(), ["a", "b"]);
    }

    #[test]
    fn detects_cycles() {
        let workflow = WorkflowFile {
            nodes: vec![
                WorkflowNode::new(
                    "a",
                    NodeType::Prompt,
                    Position { x: 0.0, y: 0.0 },
                    serde_json::json!({}),
                ),
                WorkflowNode::new(
                    "b",
                    NodeType::Output,
                    Position { x: 0.0, y: 0.0 },
                    serde_json::json!({}),
                ),
            ],
            edges: vec![
                WorkflowEdge::new("e1", "a", "b"),
                WorkflowEdge::new("e2", "b", "a"),
            ],
            ..WorkflowFile::blank()
        };
        assert!(matches!(
            execution_order(&workflow),
            Err(GraphError::Cycle(_))
        ));
    }

    #[test]
    fn resolves_prompt_text_input() {
        let workflow = WorkflowFile {
            nodes: vec![
                WorkflowNode::new(
                    "p",
                    NodeType::Prompt,
                    Position { x: 0.0, y: 0.0 },
                    serde_json::json!({"prompt":"hello"}),
                ),
                WorkflowNode::new(
                    "o",
                    NodeType::Output,
                    Position { x: 0.0, y: 0.0 },
                    serde_json::json!({}),
                ),
            ],
            edges: vec![WorkflowEdge::new("e", "p", "o")],
            ..WorkflowFile::blank()
        };
        assert_eq!(
            connected_inputs(&workflow, "o").text.as_deref(),
            Some("hello")
        );
    }

    #[test]
    fn resolves_array_batch_items() {
        let workflow = WorkflowFile {
            nodes: vec![
                WorkflowNode::new(
                    "arr",
                    NodeType::Array,
                    Position { x: 0.0, y: 0.0 },
                    serde_json::json!({"batchMode":true,"outputItems":["a","b"]}),
                ),
                WorkflowNode::new(
                    "llm",
                    NodeType::LlmGenerate,
                    Position { x: 0.0, y: 0.0 },
                    serde_json::json!({}),
                ),
            ],
            edges: vec![WorkflowEdge::new("e", "arr", "llm")],
            ..WorkflowFile::blank()
        };
        let inputs = connected_inputs(&workflow, "llm");
        assert_eq!(inputs.text.as_deref(), Some("a"));
        assert_eq!(inputs.text_items, ["a", "b"]);
    }
}
