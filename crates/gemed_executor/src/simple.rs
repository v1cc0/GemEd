use crate::array_parser::{ParseArrayOptions, SplitMode, parse_text_to_array};
use crate::graph::{
    ConnectedInputs, DynamicInputValue, GraphError, connected_inputs, execution_order,
};
use gemed_core::{NodeStatus, NodeType, WorkflowFile, WorkflowNode};
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
    let mut workflow = workflow.clone();
    let order = execution_order(&workflow)?;
    let mut report = SimpleExecutionReport::default();

    for node_id in order {
        let inputs = connected_inputs(&workflow, &node_id);
        let index = workflow
            .nodes
            .iter()
            .position(|node| node.id == node_id)
            .ok_or_else(|| SimpleExecutionError::MissingNode(node_id.clone()))?;
        let node_snapshot = workflow.nodes[index].clone();
        let outcome = execute_node(&node_snapshot, &inputs);
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

fn execute_node(node: &WorkflowNode, inputs: &ConnectedInputs) -> NodeOutcome {
    match node.node_type {
        NodeType::ImageInput | NodeType::AudioInput | NodeType::VideoInput => {
            NodeOutcome::complete("Input node is ready.", IndexMap::new())
        }
        NodeType::Prompt => execute_prompt(node, inputs),
        NodeType::Array => execute_array(node, inputs),
        NodeType::PromptConstructor => execute_prompt_constructor(node, inputs),
        NodeType::Annotation => execute_annotation(inputs),
        NodeType::Output => execute_output(inputs),
        NodeType::OutputGallery => execute_output_gallery(inputs),
        NodeType::Router | NodeType::Switch | NodeType::ConditionalSwitch => NodeOutcome::complete(
            "Control node evaluated as a pass-through/gate.",
            IndexMap::new(),
        ),
        NodeType::NanoBanana
        | NodeType::GenerateVideo
        | NodeType::Generate3d
        | NodeType::GenerateAudio
        | NodeType::LlmGenerate => NodeOutcome::skipped(
            "Provider execution is intentionally not wired in this local simple executor yet.",
        ),
        NodeType::SplitGrid
        | NodeType::ImageCompare
        | NodeType::VideoStitch
        | NodeType::EaseCurve
        | NodeType::VideoTrim
        | NodeType::VideoFrameGrab
        | NodeType::GlbViewer => NodeOutcome::skipped(
            "Advanced media execution is not wired in this local simple executor yet.",
        ),
        NodeType::Unknown => NodeOutcome::skipped("Unknown node type skipped."),
    }
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

fn execute_annotation(inputs: &ConnectedInputs) -> NodeOutcome {
    let mut updates = IndexMap::new();
    if let Some(image) = inputs.images.first() {
        updates.insert("sourceImage".to_string(), json!(image));
        updates.insert("outputImage".to_string(), json!(image));
    }
    NodeOutcome::complete("Annotation pass-through complete.", updates)
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

fn array_options_from_node(node: &WorkflowNode) -> ParseArrayOptions {
    ParseArrayOptions {
        split_mode: SplitMode::from_wire(string_field(&node.data, "splitMode").as_deref()),
        delimiter: string_field(&node.data, "delimiter"),
        regex_pattern: string_field(&node.data, "regexPattern"),
        trim_items: bool_field(&node.data, "trimItems").unwrap_or(true),
        remove_empty: bool_field(&node.data, "removeEmpty").unwrap_or(true),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use gemed_core::{Position, WorkflowEdge};

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
}
