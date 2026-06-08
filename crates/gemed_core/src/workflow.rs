use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use thiserror::Error;

pub const WORKFLOW_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowFile {
    pub version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directory_path: Option<String>,
    #[serde(default)]
    pub nodes: Vec<WorkflowNode>,
    #[serde(default)]
    pub edges: Vec<WorkflowEdge>,
    #[serde(default = "default_edge_style")]
    pub edge_style: EdgeStyle,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub groups: IndexMap<String, NodeGroup>,
}

impl WorkflowFile {
    pub fn blank() -> Self {
        Self {
            version: WORKFLOW_VERSION,
            id: None,
            name: "Untitled Workflow".to_string(),
            directory_path: None,
            nodes: Vec::new(),
            edges: Vec::new(),
            edge_style: EdgeStyle::Curved,
            groups: IndexMap::new(),
        }
    }

    pub fn example() -> Self {
        let mut workflow = Self {
            name: "GemEd Dioxus Starter".to_string(),
            nodes: vec![
                WorkflowNode::new(
                    "node_prompt",
                    NodeType::Prompt,
                    Position { x: 80.0, y: 120.0 },
                    serde_json::json!({
                        "label": "Prompt",
                        "text": "A polished product photo of a gemstone editor UI",
                        "status": "idle"
                    }),
                ),
                WorkflowNode::new(
                    "node_generate",
                    NodeType::NanoBanana,
                    Position { x: 440.0, y: 100.0 },
                    serde_json::json!({
                        "label": "Generate Image",
                        "status": "idle",
                        "provider": "gemini"
                    }),
                ),
                WorkflowNode::new(
                    "node_output",
                    NodeType::Output,
                    Position { x: 810.0, y: 130.0 },
                    serde_json::json!({
                        "label": "Output",
                        "status": "idle"
                    }),
                ),
            ],
            edges: vec![
                WorkflowEdge::new("edge_prompt_generate", "node_prompt", "node_generate"),
                WorkflowEdge::new("edge_generate_output", "node_generate", "node_output"),
            ],
            ..Self::blank()
        };
        workflow.groups = IndexMap::new();
        workflow
    }

    pub fn from_json_str(source: &str) -> Result<Self, WorkflowError> {
        let workflow: Self = serde_json::from_str(source)?;
        workflow.validate()?;
        Ok(workflow)
    }

    pub fn to_pretty_json(&self) -> Result<String, WorkflowError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn validate(&self) -> Result<(), WorkflowError> {
        if self.version != WORKFLOW_VERSION {
            return Err(WorkflowError::UnsupportedVersion(self.version));
        }

        let mut ids = HashSet::with_capacity(self.nodes.len());
        for node in &self.nodes {
            if node.id.trim().is_empty() {
                return Err(WorkflowError::EmptyNodeId);
            }
            if !ids.insert(node.id.as_str()) {
                return Err(WorkflowError::DuplicateNodeId(node.id.clone()));
            }
        }

        for edge in &self.edges {
            if edge.id.trim().is_empty() {
                return Err(WorkflowError::EmptyEdgeId);
            }
            if !ids.contains(edge.source.as_str()) {
                return Err(WorkflowError::MissingEdgeEndpoint {
                    edge_id: edge.id.clone(),
                    node_id: edge.source.clone(),
                    side: EdgeEndpoint::Source,
                });
            }
            if !ids.contains(edge.target.as_str()) {
                return Err(WorkflowError::MissingEdgeEndpoint {
                    edge_id: edge.id.clone(),
                    node_id: edge.target.clone(),
                    side: EdgeEndpoint::Target,
                });
            }
        }

        Ok(())
    }

    pub fn node_type_counts(&self) -> IndexMap<NodeType, usize> {
        let mut counts = IndexMap::new();
        for node in &self.nodes {
            *counts.entry(node.node_type.clone()).or_insert(0) += 1;
        }
        counts
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowNode {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: NodeType,
    pub position: Position,
    #[serde(default)]
    pub data: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dragging: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    #[serde(flatten)]
    pub extra: IndexMap<String, Value>,
}

impl WorkflowNode {
    pub fn new(
        id: impl Into<String>,
        node_type: NodeType,
        position: Position,
        data: Value,
    ) -> Self {
        Self {
            id: id.into(),
            node_type,
            position,
            data,
            width: None,
            height: None,
            selected: None,
            dragging: None,
            group_id: None,
            extra: IndexMap::new(),
        }
    }

    pub fn display_label(&self) -> String {
        self.data
            .get("label")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| self.node_type.title().to_string())
    }

    pub fn status(&self) -> NodeStatus {
        self.data
            .get("status")
            .and_then(Value::as_str)
            .and_then(NodeStatus::from_wire)
            .unwrap_or(NodeStatus::Idle)
    }

    pub fn preview_text(&self) -> Option<String> {
        [
            "text",
            "prompt",
            "outputText",
            "defaultPrompt",
            "incomingText",
        ]
        .iter()
        .filter_map(|key| self.data.get(*key).and_then(Value::as_str))
        .find(|value| !value.trim().is_empty())
        .map(|value| truncate(value, 120))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_handle: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_handle: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge_type: Option<String>,
    #[serde(default)]
    pub data: WorkflowEdgeData,
    #[serde(flatten)]
    pub extra: IndexMap<String, Value>,
}

impl WorkflowEdge {
    pub fn new(
        id: impl Into<String>,
        source: impl Into<String>,
        target: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            source: source.into(),
            target: target.into(),
            source_handle: None,
            target_handle: None,
            edge_type: None,
            data: WorkflowEdgeData::default(),
            extra: IndexMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowEdgeData {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_pause: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_loop: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loop_count: Option<u32>,
    #[serde(flatten)]
    pub extra: IndexMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeGroup {
    pub id: String,
    pub name: String,
    pub color: GroupColor,
    pub position: Position,
    pub size: Size,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locked: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_nbp_input: Option<bool>,
    #[serde(flatten)]
    pub extra: IndexMap<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EdgeStyle {
    Angular,
    Curved,
}

fn default_edge_style() -> EdgeStyle {
    EdgeStyle::Curved
}

impl Default for EdgeStyle {
    fn default() -> Self {
        default_edge_style()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GroupColor {
    Neutral,
    Blue,
    Green,
    Purple,
    Orange,
    Red,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NodeType {
    ImageInput,
    AudioInput,
    VideoInput,
    Annotation,
    Prompt,
    Array,
    PromptConstructor,
    NanoBanana,
    GenerateVideo,
    #[serde(rename = "generate3d")]
    Generate3d,
    GenerateAudio,
    LlmGenerate,
    SplitGrid,
    Output,
    OutputGallery,
    ImageCompare,
    VideoStitch,
    EaseCurve,
    VideoTrim,
    VideoFrameGrab,
    Router,
    Switch,
    ConditionalSwitch,
    GlbViewer,
    #[serde(other)]
    Unknown,
}

impl NodeType {
    pub fn title(&self) -> &'static str {
        match self {
            Self::ImageInput => "Image Input",
            Self::AudioInput => "Audio Input",
            Self::VideoInput => "Video Input",
            Self::Annotation => "Annotation",
            Self::Prompt => "Prompt",
            Self::Array => "Array",
            Self::PromptConstructor => "Prompt Constructor",
            Self::NanoBanana => "Generate Image",
            Self::GenerateVideo => "Generate Video",
            Self::Generate3d => "Generate 3D",
            Self::GenerateAudio => "Generate Audio",
            Self::LlmGenerate => "LLM",
            Self::SplitGrid => "Split Grid",
            Self::Output => "Output",
            Self::OutputGallery => "Output Gallery",
            Self::ImageCompare => "Image Compare",
            Self::VideoStitch => "Video Stitch",
            Self::EaseCurve => "Ease Curve",
            Self::VideoTrim => "Video Trim",
            Self::VideoFrameGrab => "Video Frame Grab",
            Self::Router => "Router",
            Self::Switch => "Switch",
            Self::ConditionalSwitch => "Conditional Switch",
            Self::GlbViewer => "GLB Viewer",
            Self::Unknown => "Unknown",
        }
    }

    pub fn class_name(&self) -> &'static str {
        match self {
            Self::ImageInput | Self::AudioInput | Self::VideoInput | Self::Prompt | Self::Array => {
                "input"
            }
            Self::NanoBanana
            | Self::GenerateVideo
            | Self::Generate3d
            | Self::GenerateAudio
            | Self::LlmGenerate => "generation",
            Self::Router | Self::Switch | Self::ConditionalSwitch => "control",
            Self::Output | Self::OutputGallery => "output",
            Self::Unknown => "unknown",
            _ => "processing",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeStatus {
    Idle,
    Loading,
    Complete,
    Error,
    Skipped,
}

impl NodeStatus {
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "idle" => Some(Self::Idle),
            "loading" => Some(Self::Loading),
            "complete" => Some(Self::Complete),
            "error" => Some(Self::Error),
            "skipped" => Some(Self::Skipped),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Loading => "loading",
            Self::Complete => "complete",
            Self::Error => "error",
            Self::Skipped => "skipped",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeEndpoint {
    Source,
    Target,
}

impl EdgeEndpoint {
    pub fn label(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Target => "target",
        }
    }
}

#[derive(Debug, Error)]
pub enum WorkflowError {
    #[error("workflow JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported workflow version {0}")]
    UnsupportedVersion(u8),
    #[error("node id must not be empty")]
    EmptyNodeId,
    #[error("duplicate node id `{0}`")]
    DuplicateNodeId(String),
    #[error("edge id must not be empty")]
    EmptyEdgeId,
    #[error("edge `{edge_id}` references missing {side:?} node `{node_id}`")]
    MissingEdgeEndpoint {
        edge_id: String,
        node_id: String,
        side: EdgeEndpoint,
    },
}

pub fn parse_workflow_or_sample(source: &str) -> Result<WorkflowFile, WorkflowError> {
    WorkflowFile::from_json_str(source)
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut output = String::new();
    for (index, ch) in value.chars().enumerate() {
        if index >= max_chars {
            output.push('…');
            return output;
        }
        output.push(ch);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_workflow_is_valid() {
        let workflow = WorkflowFile::example();
        workflow.validate().expect("sample workflow validates");
        assert_eq!(workflow.nodes.len(), 3);
        assert_eq!(workflow.edges.len(), 2);
    }

    #[test]
    fn sample_workflow_round_trips() {
        let workflow = WorkflowFile::example();
        let json = workflow.to_pretty_json().expect("serialize workflow");
        let parsed = WorkflowFile::from_json_str(&json).expect("parse workflow");
        assert_eq!(parsed.name, workflow.name);
        assert_eq!(parsed.nodes.len(), workflow.nodes.len());
        assert_eq!(parsed.edges.len(), workflow.edges.len());
    }

    #[test]
    fn invalid_edge_is_rejected() {
        let json = r#"
        {
          "version": 1,
          "name": "bad",
          "nodes": [],
          "edges": [{ "id": "e1", "source": "a", "target": "b" }],
          "edgeStyle": "curved"
        }
        "#;

        let err = WorkflowFile::from_json_str(json).expect_err("missing endpoint rejected");
        assert!(matches!(err, WorkflowError::MissingEdgeEndpoint { .. }));
    }

    #[test]
    fn current_examples_directory_has_no_workflow_json_yet() {
        let examples_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("examples");
        let workflow_json_count = std::fs::read_dir(examples_dir)
            .ok()
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
            .count();
        assert_eq!(workflow_json_count, 0);
    }
}
