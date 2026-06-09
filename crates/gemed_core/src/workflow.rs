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
        workflow.nodes[0].group_id = Some("group_starter".to_string());
        workflow.nodes[1].group_id = Some("group_starter".to_string());
        workflow.groups = IndexMap::from([(
            "group_starter".to_string(),
            NodeGroup {
                id: "group_starter".to_string(),
                name: "Starter Group".to_string(),
                color: GroupColor::Blue,
                position: Position { x: 50.0, y: 70.0 },
                size: Size {
                    width: 660.0,
                    height: 230.0,
                },
                locked: None,
                is_nbp_input: None,
                extra: IndexMap::new(),
            },
        )]);
        workflow
    }

    pub fn media_preview_example() -> Self {
        Self {
            name: "GemEd Media Preview Sample".to_string(),
            nodes: vec![
                WorkflowNode::new(
                    "media_image",
                    NodeType::ImageInput,
                    Position { x: 80.0, y: 120.0 },
                    serde_json::json!({
                        "label": "Inline SVG Image",
                        "status": "complete",
                        "image": SAMPLE_SVG_IMAGE_DATA_URL,
                        "filename": "gemed-preview.svg",
                        "dimensions": { "width": 160, "height": 100 }
                    }),
                ),
                WorkflowNode::new(
                    "media_audio",
                    NodeType::AudioInput,
                    Position { x: 80.0, y: 330.0 },
                    serde_json::json!({
                        "label": "Inline WAV Audio",
                        "status": "complete",
                        "audioFile": SAMPLE_WAV_AUDIO_DATA_URL,
                        "filename": "gemed-preview.wav",
                        "duration": 0.05,
                        "format": "audio/wav"
                    }),
                ),
                WorkflowNode::new(
                    "media_video",
                    NodeType::VideoInput,
                    Position { x: 430.0, y: 120.0 },
                    serde_json::json!({
                        "label": "Inline MP4 Video",
                        "status": "complete",
                        "video": SAMPLE_MP4_VIDEO_DATA_URL,
                        "filename": "gemed-preview.mp4",
                        "duration": 0.12,
                        "dimensions": { "width": 16, "height": 16 },
                        "format": "video/mp4"
                    }),
                ),
                WorkflowNode::new(
                    "media_gallery",
                    NodeType::OutputGallery,
                    Position { x: 780.0, y: 150.0 },
                    serde_json::json!({
                        "label": "Gallery Preview",
                        "status": "complete",
                        "images": ["", SAMPLE_SVG_IMAGE_DATA_URL],
                        "imageRefs": ["gemed-media://media/external-preview.png"],
                        "videos": [SAMPLE_MP4_VIDEO_DATA_URL]
                    }),
                ),
                WorkflowNode::new(
                    "media_glb",
                    NodeType::GlbViewer,
                    Position { x: 780.0, y: 410.0 },
                    serde_json::json!({
                        "label": "Project GLB Reference",
                        "status": "idle",
                        "glbUrl": "gemed-media://media/demo-model.glb",
                        "filename": "demo-model.glb"
                    }),
                ),
            ],
            edges: vec![
                WorkflowEdge::with_handles(
                    "edge_image_gallery",
                    "media_image",
                    "media_gallery",
                    "image",
                    "image",
                ),
                WorkflowEdge::with_handles(
                    "edge_video_gallery",
                    "media_video",
                    "media_gallery",
                    "video",
                    "video",
                ),
                WorkflowEdge::with_handles(
                    "edge_audio_gallery",
                    "media_audio",
                    "media_gallery",
                    "audio",
                    "audio",
                ),
            ],
            ..Self::blank()
        }
    }

    pub fn video_frame_grab_example() -> Self {
        Self {
            name: "GemEd Video Frame Grab Sample".to_string(),
            nodes: vec![
                WorkflowNode::new(
                    "frame_video",
                    NodeType::VideoInput,
                    Position { x: 80.0, y: 140.0 },
                    serde_json::json!({
                        "label": "Inline MP4 Source",
                        "status": "complete",
                        "video": SAMPLE_MP4_VIDEO_DATA_URL,
                        "filename": "gemed-preview.mp4",
                        "duration": 0.12,
                        "dimensions": { "width": 16, "height": 16 },
                        "format": "video/mp4"
                    }),
                ),
                WorkflowNode::new(
                    "frame_grab",
                    NodeType::VideoFrameGrab,
                    Position { x: 430.0, y: 120.0 },
                    serde_json::json!({
                        "label": "Plan First Frame",
                        "status": "idle",
                        "framePosition": "first",
                        "sourceVideo": null,
                        "sourceVideoRef": null,
                        "outputImage": null,
                        "outputImageRef": null,
                        "frameGrabPlan": null
                    }),
                ),
                WorkflowNode::new(
                    "frame_output",
                    NodeType::Output,
                    Position { x: 780.0, y: 150.0 },
                    serde_json::json!({
                        "label": "Frame Output",
                        "status": "idle",
                        "contentType": "image",
                        "image": null
                    }),
                ),
            ],
            edges: vec![
                WorkflowEdge::with_handles(
                    "edge_frame_video_grab",
                    "frame_video",
                    "frame_grab",
                    "video",
                    "video",
                ),
                WorkflowEdge::with_handles(
                    "edge_frame_grab_output",
                    "frame_grab",
                    "frame_output",
                    "image",
                    "image",
                ),
            ],
            ..Self::blank()
        }
    }

    pub fn llm_provider_example() -> Self {
        Self {
            name: "GemEd LLM Provider Sample".to_string(),
            nodes: vec![
                WorkflowNode::new(
                    "provider_prompt",
                    NodeType::Prompt,
                    Position { x: 80.0, y: 220.0 },
                    serde_json::json!({
                        "label": "Shared Prompt",
                        "status": "idle",
                        "prompt": "Write one concise sentence about a portable visual AI workflow editor."
                    }),
                ),
                WorkflowNode::new(
                    "provider_gemini",
                    NodeType::LlmGenerate,
                    Position { x: 430.0, y: 60.0 },
                    serde_json::json!({
                        "label": "Gemini LLM",
                        "status": "idle",
                        "provider": "gemini",
                        "model": "gemini-3.5-flash",
                        "inputPrompt": null,
                        "inputImages": [],
                        "outputText": null,
                        "temperature": 0.2,
                        "maxTokens": 128,
                        "parameters": {}
                    }),
                ),
                WorkflowNode::new(
                    "provider_openai",
                    NodeType::LlmGenerate,
                    Position { x: 430.0, y: 240.0 },
                    serde_json::json!({
                        "label": "OpenAI LLM",
                        "status": "idle",
                        "provider": "openai",
                        "model": "gpt-5.5",
                        "inputPrompt": null,
                        "inputImages": [],
                        "outputText": null,
                        "temperature": 0.2,
                        "maxTokens": 128,
                        "parameters": {}
                    }),
                ),
                WorkflowNode::new(
                    "provider_anthropic",
                    NodeType::LlmGenerate,
                    Position { x: 430.0, y: 420.0 },
                    serde_json::json!({
                        "label": "Anthropic LLM",
                        "status": "idle",
                        "provider": "anthropic",
                        "model": "claude-sonnet-4-6",
                        "inputPrompt": null,
                        "inputImages": [],
                        "outputText": null,
                        "temperature": 0.2,
                        "maxTokens": 128,
                        "parameters": {}
                    }),
                ),
                WorkflowNode::new(
                    "provider_gemini_output",
                    NodeType::Output,
                    Position { x: 800.0, y: 70.0 },
                    serde_json::json!({
                        "label": "Gemini Text Output",
                        "status": "idle"
                    }),
                ),
                WorkflowNode::new(
                    "provider_openai_output",
                    NodeType::Output,
                    Position { x: 800.0, y: 250.0 },
                    serde_json::json!({
                        "label": "OpenAI Text Output",
                        "status": "idle"
                    }),
                ),
                WorkflowNode::new(
                    "provider_anthropic_output",
                    NodeType::Output,
                    Position { x: 800.0, y: 430.0 },
                    serde_json::json!({
                        "label": "Anthropic Text Output",
                        "status": "idle"
                    }),
                ),
            ],
            edges: vec![
                WorkflowEdge::with_handles(
                    "edge_provider_prompt_gemini",
                    "provider_prompt",
                    "provider_gemini",
                    "text",
                    "prompt",
                ),
                WorkflowEdge::with_handles(
                    "edge_provider_prompt_openai",
                    "provider_prompt",
                    "provider_openai",
                    "text",
                    "prompt",
                ),
                WorkflowEdge::with_handles(
                    "edge_provider_prompt_anthropic",
                    "provider_prompt",
                    "provider_anthropic",
                    "text",
                    "prompt",
                ),
                WorkflowEdge::with_handles(
                    "edge_provider_gemini_output",
                    "provider_gemini",
                    "provider_gemini_output",
                    "text",
                    "text",
                ),
                WorkflowEdge::with_handles(
                    "edge_provider_openai_output",
                    "provider_openai",
                    "provider_openai_output",
                    "text",
                    "text",
                ),
                WorkflowEdge::with_handles(
                    "edge_provider_anthropic_output",
                    "provider_anthropic",
                    "provider_anthropic_output",
                    "text",
                    "text",
                ),
            ],
            ..Self::blank()
        }
    }

    pub fn media_transform_example() -> Self {
        Self {
            name: "GemEd Media Transform Sample".to_string(),
            nodes: vec![
                WorkflowNode::new(
                    "transform_image",
                    NodeType::ImageInput,
                    Position { x: 80.0, y: 120.0 },
                    serde_json::json!({
                        "label": "2x2 Inline PNG",
                        "status": "complete",
                        "image": SAMPLE_GRID_PNG_DATA_URL,
                        "filename": "gemed-grid.png",
                        "dimensions": { "width": 2, "height": 2 }
                    }),
                ),
                WorkflowNode::new(
                    "transform_split",
                    NodeType::SplitGrid,
                    Position { x: 430.0, y: 100.0 },
                    serde_json::json!({
                        "label": "Rust Split Grid",
                        "status": "idle",
                        "sourceImage": null,
                        "targetCount": 4,
                        "gridRows": 2,
                        "gridCols": 2,
                        "isConfigured": true,
                        "defaultPrompt": "Describe this grid cell"
                    }),
                ),
                WorkflowNode::new(
                    "transform_gallery",
                    NodeType::OutputGallery,
                    Position { x: 780.0, y: 120.0 },
                    serde_json::json!({
                        "label": "Split Output Gallery",
                        "status": "idle",
                        "images": []
                    }),
                ),
                WorkflowNode::new(
                    "transform_compare",
                    NodeType::ImageCompare,
                    Position { x: 780.0, y: 390.0 },
                    serde_json::json!({
                        "label": "Compare First Two Cells",
                        "status": "idle",
                        "imageA": null,
                        "imageB": null
                    }),
                ),
            ],
            edges: vec![
                WorkflowEdge::with_handles(
                    "edge_transform_input_split",
                    "transform_image",
                    "transform_split",
                    "image",
                    "image",
                ),
                WorkflowEdge::with_handles(
                    "edge_transform_split_gallery_0",
                    "transform_split",
                    "transform_gallery",
                    "image-0",
                    "image",
                ),
                WorkflowEdge::with_handles(
                    "edge_transform_split_gallery_1",
                    "transform_split",
                    "transform_gallery",
                    "image-1",
                    "image",
                ),
                WorkflowEdge::with_handles(
                    "edge_transform_split_gallery_2",
                    "transform_split",
                    "transform_gallery",
                    "image-2",
                    "image",
                ),
                WorkflowEdge::with_handles(
                    "edge_transform_split_gallery_3",
                    "transform_split",
                    "transform_gallery",
                    "image-3",
                    "image",
                ),
                WorkflowEdge::with_handles(
                    "edge_transform_split_compare_0",
                    "transform_split",
                    "transform_compare",
                    "image-0",
                    "image-0",
                ),
                WorkflowEdge::with_handles(
                    "edge_transform_split_compare_1",
                    "transform_split",
                    "transform_compare",
                    "image-1",
                    "image-1",
                ),
            ],
            ..Self::blank()
        }
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

#[derive(Debug, Clone, PartialEq, Serialize)]
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

impl<'de> Deserialize<'de> for WorkflowEdge {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct WireWorkflowEdge {
            id: String,
            source: String,
            target: String,
            #[serde(default)]
            source_handle: Option<String>,
            #[serde(default)]
            target_handle: Option<String>,
            #[serde(default)]
            edge_type: Option<String>,
            #[serde(default, rename = "type")]
            react_flow_type: Option<String>,
            #[serde(default)]
            data: WorkflowEdgeData,
            #[serde(flatten)]
            extra: IndexMap<String, Value>,
        }

        let wire = WireWorkflowEdge::deserialize(deserializer)?;
        Ok(Self {
            id: wire.id,
            source: wire.source,
            target: wire.target,
            source_handle: wire.source_handle,
            target_handle: wire.target_handle,
            edge_type: wire.edge_type.or(wire.react_flow_type),
            data: wire.data,
            extra: wire.extra,
        })
    }
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

    pub fn with_handles(
        id: impl Into<String>,
        source: impl Into<String>,
        target: impl Into<String>,
        source_handle: impl Into<String>,
        target_handle: impl Into<String>,
    ) -> Self {
        Self {
            source_handle: Some(source_handle.into()),
            target_handle: Some(target_handle.into()),
            ..Self::new(id, source, target)
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

const SAMPLE_GRID_PNG_DATA_URL: &str = concat!(
    "data:image/png;base64,",
    "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAFElEQVR4nGP4z8DwHwyBNBAw/AcAR8oI+ItOQ4UAAAAASUVORK5CYII=",
);

const SAMPLE_SVG_IMAGE_DATA_URL: &str = concat!(
    "data:image/svg+xml;base64,",
    "PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAxNjAg",
    "MTAwIj48cmVjdCB3aWR0aD0iMTYwIiBoZWlnaHQ9IjEwMCIgZmlsbD0iIzBmMTcyYSIvPjxwYXRo",
    "IGQ9Ik04MCAxMiAxMzIgNDIgMTEyIDg4SDQ4TDI4IDQyWiIgZmlsbD0iIzYwYTVmYSIvPjxwYXRo",
    "IGQ9Ik04MCAxMiAxMDIgNDJINThaIiBmaWxsPSIjYmZkYmZlIi8+PHBhdGggZD0iTTU4IDQyaDQ0",
    "TDgwIDg4WiIgZmlsbD0iIzI1NjNlYiIvPjx0ZXh0IHg9IjgwIiB5PSI5NiIgdGV4dC1hbmNob3I9",
    "Im1pZGRsZSIgZm9udC1mYW1pbHk9InNhbnMtc2VyaWYiIGZvbnQtc2l6ZT0iMTAiIGZpbGw9IiNl",
    "NWVjZmYiPkdlbUVkPC90ZXh0Pjwvc3ZnPg==",
);

const SAMPLE_WAV_AUDIO_DATA_URL: &str = concat!(
    "data:audio/wav;base64,",
    "UklGRrQBAABXQVZFZm10IBAAAAABAAEAQB8AAEAfAAABAAgAZGF0YZABAACAgICAgICAgICAgICA",
    "gICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICA",
    "gICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICA",
    "gICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICA",
    "gICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICA",
    "gICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICA",
    "gICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICA",
    "gICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICA",
);

const SAMPLE_MP4_VIDEO_DATA_URL: &str = concat!(
    "data:video/mp4;base64,",
    "AAAAIGZ0eXBpc29tAAACAGlzb21pc28yYXZjMW1wNDEAAAMVbW9vdgAAAGxtdmhkAAAAAAAAAAAA",
    "AAAAAAAD6AAAACgAAQAAAQAAAAAAAAAAAAAAAAEAAAAAAAAAAAAAAAAAAAABAAAAAAAAAAAAAAAA",
    "AABAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAgAAAj90cmFrAAAAXHRraGQAAAADAAAA",
    "AAAAAAAAAAABAAAAAAAAACgAAAAAAAAAAAAAAAAAAAAAAAEAAAAAAAAAAAAAAAAAAAABAAAAAAAA",
    "AAAAAAAAAABAAAAAABAAAAAQAAAAAAAkZWR0cwAAABxlbHN0AAAAAAAAAAEAAAAoAAAAAAABAAAA",
    "AAG3bWRpYQAAACBtZGhkAAAAAAAAAAAAAAAAAAAyAAAAAgBVxAAAAAAALWhkbHIAAAAAAAAAAHZp",
    "ZGUAAAAAAAAAAAAAAABWaWRlb0hhbmRsZXIAAAABYm1pbmYAAAAUdm1oZAAAAAEAAAAAAAAAAAAA",
    "ACRkaW5mAAAAHGRyZWYAAAAAAAAAAQAAAAx1cmwgAAAAAQAAASJzdGJsAAAAvnN0c2QAAAAAAAAA",
    "AQAAAK5hdmMxAAAAAAAAAAEAAAAAAAAAAAAAAAAAAAAAABAAEABIAAAASAAAAAAAAAABFUxhdmM2",
    "Mi4yOC4xMDEgbGlieDI2NAAAAAAAAAAAAAAAGP//AAAANGF2Y0MBZAAK/+EAF2dkAAqs2V7ARAAA",
    "AwAEAAADAMg8SJZYAQAGaOvjyyLA/fj4AAAAABBwYXNwAAAAAQAAAAEAAAAUYnRydAAAAAAAAino",
    "AAAAAAAAABhzdHRzAAAAAAAAAAEAAAABAAACAAAAABxzdHNjAAAAAAAAAAEAAAABAAAAAQAAAAEA",
    "AAAUc3RzegAAAAAAAALFAAAAAQAAABRzdGNvAAAAAAAAAAEAAANFAAAAYnVkdGEAAABabWV0YQAA",
    "AAAAAAAhaGRscgAAAAAAAAAAbWRpcmFwcGwAAAAAAAAAAAAAAAAtaWxzdAAAACWpdG9vAAAAHWRh",
    "dGEAAAABAAAAAExhdmY2Mi4xMi4xMDEAAAAIZnJlZQAAAs1tZGF0AAACrgYF//+q3EXpvebZSLeW",
    "LNgg2SPu73gyNjQgLSBjb3JlIDE2NSByMzIyMiBiMzU2MDVhIC0gSC4yNjQvTVBFRy00IEFWQyBj",
    "b2RlYyAtIENvcHlsZWZ0IDIwMDMtMjAyNSAtIGh0dHA6Ly93d3cudmlkZW9sYW4ub3JnL3gyNjQu",
    "aHRtbCAtIG9wdGlvbnM6IGNhYmFjPTEgcmVmPTMgZGVibG9jaz0xOjA6MCBhbmFseXNlPTB4Mzow",
    "eDExMyBtZT1oZXggc3VibWU9NyBwc3k9MSBwc3lfcmQ9MS4wMDowLjAwIG1peGVkX3JlZj0xIG1l",
    "X3JhbmdlPTE2IGNocm9tYV9tZT0xIHRyZWxsaXM9MSA4eDhkY3Q9MSBjcW09MCBkZWFkem9uZT0y",
    "MSwxMSBmYXN0X3Bza2lwPTEgY2hyb21hX3FwX29mZnNldD0tMiB0aHJlYWRzPTEgbG9va2FoZWFk",
    "X3RocmVhZHM9MSBzbGljZWRfdGhyZWFkcz0wIG5yPTAgZGVjaW1hdGU9MSBpbnRlcmxhY2VkPTAg",
    "Ymx1cmF5X2NvbXBhdD0wIGNvbnN0cmFpbmVkX2ludHJhPTAgYmZyYW1lcz0zIGJfcHlyYW1pZD0y",
    "IGJfYWRhcHQ9MSBiX2JpYXM9MCBkaXJlY3Q9MSB3ZWlnaHRiPTEgb3Blbl9nb3A9MCB3ZWlnaHRw",
    "PTIga2V5aW50PTI1MCBrZXlpbnRfbWluPTI1IHNjZW5lY3V0PTQwIGludHJhX3JlZnJlc2g9MCBy",
    "Y19sb29rYWhlYWQ9NDAgcmM9Y3JmIG1idHJlZT0xIGNyZj0yMy4wIHFjb21wPTAuNjAgcXBtaW49",
    "MCBxcG1heD02OSBxcHN0ZXA9NCBpcF9yYXRpbz0xLjQwIGFxPTE6MS4wMACAAAAAD2WIhAAr//72",
    "c3wKa22xgQ==",
);

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
    fn media_preview_sample_workflow_is_valid_and_roundtrips() {
        let workflow = WorkflowFile::media_preview_example();
        workflow.validate().expect("media sample validates");
        assert_eq!(workflow.nodes.len(), 5);
        assert_eq!(workflow.edges.len(), 3);
        assert!(
            workflow
                .nodes
                .iter()
                .any(|node| node.data.get("image").is_some_and(Value::is_string))
        );
        assert!(
            workflow
                .nodes
                .iter()
                .any(|node| node.data.get("audioFile").is_some_and(Value::is_string))
        );
        assert!(
            workflow
                .nodes
                .iter()
                .any(|node| node.data.get("video").is_some_and(Value::is_string))
        );

        let json = workflow.to_pretty_json().expect("serialize media sample");
        let parsed = WorkflowFile::from_json_str(&json).expect("parse media sample");
        assert_eq!(parsed, workflow);
    }

    #[test]
    fn video_frame_grab_sample_workflow_is_valid_and_roundtrips() {
        let workflow = WorkflowFile::video_frame_grab_example();
        workflow
            .validate()
            .expect("video frame grab sample validates");
        assert_eq!(workflow.nodes.len(), 3);
        assert_eq!(workflow.edges.len(), 2);
        assert!(
            workflow
                .nodes
                .iter()
                .any(|node| node.node_type == NodeType::VideoFrameGrab)
        );
        assert!(
            workflow
                .edges
                .iter()
                .any(|edge| edge.target_handle.as_deref() == Some("video"))
        );

        let json = workflow
            .to_pretty_json()
            .expect("serialize video frame grab sample");
        let parsed = WorkflowFile::from_json_str(&json).expect("parse video frame grab sample");
        assert_eq!(parsed, workflow);
    }

    #[test]
    fn llm_provider_sample_workflow_is_valid_and_roundtrips() {
        let workflow = WorkflowFile::llm_provider_example();
        workflow.validate().expect("provider sample validates");
        assert_eq!(workflow.nodes.len(), 7);
        assert_eq!(workflow.edges.len(), 6);
        for provider in ["gemini", "openai", "anthropic"] {
            assert!(workflow.nodes.iter().any(|node| {
                node.node_type == NodeType::LlmGenerate
                    && node.data.get("provider").and_then(Value::as_str) == Some(provider)
            }));
        }

        let json = workflow
            .to_pretty_json()
            .expect("serialize provider sample");
        let parsed = WorkflowFile::from_json_str(&json).expect("parse provider sample");
        assert_eq!(parsed, workflow);
    }

    #[test]
    fn media_transform_sample_workflow_is_valid_and_roundtrips() {
        let workflow = WorkflowFile::media_transform_example();
        workflow
            .validate()
            .expect("media transform sample validates");
        assert_eq!(workflow.nodes.len(), 4);
        assert_eq!(workflow.edges.len(), 7);
        assert!(
            workflow
                .nodes
                .iter()
                .any(|node| node.node_type == NodeType::SplitGrid)
        );
        assert!(
            workflow
                .edges
                .iter()
                .any(|edge| edge.source_handle.as_deref() == Some("image-0"))
        );

        let json = workflow
            .to_pretty_json()
            .expect("serialize media transform sample");
        let parsed = WorkflowFile::from_json_str(&json).expect("parse media transform sample");
        assert_eq!(parsed, workflow);
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
    fn react_flow_edge_type_alias_imports_as_edge_type() {
        let json = r#"
        {
          "version": 1,
          "name": "edge type alias",
          "nodes": [
            { "id": "a", "type": "prompt", "position": { "x": 0, "y": 0 }, "data": {} },
            { "id": "b", "type": "output", "position": { "x": 200, "y": 0 }, "data": {} }
          ],
          "edges": [
            { "id": "e1", "source": "a", "target": "b", "type": "editable", "data": {} }
          ]
        }
        "#;

        let workflow = WorkflowFile::from_json_str(json).expect("workflow parses");
        assert_eq!(workflow.edges[0].edge_type.as_deref(), Some("editable"));
        let exported = workflow.to_pretty_json().expect("workflow exports");
        assert!(exported.contains("\"edgeType\": \"editable\""));
        assert!(!exported.contains("\"type\": \"editable\""));
    }

    #[test]
    fn examples_directory_workflow_json_files_import_when_present() {
        let examples_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("examples");
        for entry in std::fs::read_dir(examples_dir)
            .ok()
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
        {
            let source = std::fs::read_to_string(entry.path()).expect("read example workflow JSON");
            WorkflowFile::from_json_str(&source).unwrap_or_else(|err| {
                panic!(
                    "example workflow JSON `{}` failed to import: {err}",
                    entry.path().display()
                )
            });
        }
    }
}
