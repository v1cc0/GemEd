use crate::{NodeType, WorkflowNode};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeHandle {
    pub id: String,
    pub label: String,
}

impl NodeHandle {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

pub fn source_handle_options(node: &WorkflowNode) -> Vec<NodeHandle> {
    match node.node_type {
        NodeType::ImageInput
        | NodeType::NanoBanana
        | NodeType::Annotation
        | NodeType::ImageCompare
        | NodeType::VideoFrameGrab => vec![handle("image", "image")],
        NodeType::VideoInput
        | NodeType::GenerateVideo
        | NodeType::VideoStitch
        | NodeType::EaseCurve
        | NodeType::VideoTrim => vec![handle("video", "video")],
        NodeType::AudioInput | NodeType::GenerateAudio => vec![handle("audio", "audio")],
        NodeType::Generate3d | NodeType::GlbViewer => vec![handle("3d", "3D")],
        NodeType::Array => array_source_handles(node),
        NodeType::Switch => switch_source_handles(node),
        NodeType::ConditionalSwitch => conditional_switch_source_handles(node),
        NodeType::Router => vec![
            handle("text", "text"),
            handle("image", "image"),
            handle("video", "video"),
            handle("audio", "audio"),
            handle("3d", "3D"),
        ],
        NodeType::Prompt
        | NodeType::PromptConstructor
        | NodeType::LlmGenerate
        | NodeType::Output => {
            vec![handle("text", "text")]
        }
        NodeType::SplitGrid => split_grid_source_handles(node),
        NodeType::OutputGallery | NodeType::Unknown => vec![handle("text", "text")],
    }
}

pub fn target_handle_options(node: &WorkflowNode) -> Vec<NodeHandle> {
    match node.node_type {
        NodeType::NanoBanana | NodeType::GenerateVideo | NodeType::Generate3d => {
            let mut handles = vec![handle("prompt", "prompt"), handle("image", "image")];
            handles.extend(schema_handle_options(node));
            dedupe_handles(handles)
        }
        NodeType::GenerateAudio | NodeType::LlmGenerate | NodeType::PromptConstructor => {
            let mut handles = vec![handle("prompt", "prompt"), handle("text", "text")];
            handles.extend(schema_handle_options(node));
            dedupe_handles(handles)
        }
        NodeType::Output => vec![
            handle("text", "text"),
            handle("image", "image"),
            handle("video", "video"),
            handle("audio", "audio"),
            handle("3d", "3D"),
        ],
        NodeType::OutputGallery => vec![handle("image", "image"), handle("video", "video")],
        NodeType::ImageCompare => vec![handle("image-0", "image A"), handle("image-1", "image B")],
        NodeType::VideoStitch => vec![handle("video-0", "video 1"), handle("video-1", "video 2")],
        NodeType::EaseCurve => vec![handle("video", "video"), handle("easeCurve", "ease curve")],
        NodeType::VideoTrim | NodeType::VideoFrameGrab => vec![handle("video", "video")],
        NodeType::Switch | NodeType::Router => vec![
            handle("text", "text"),
            handle("image", "image"),
            handle("video", "video"),
            handle("audio", "audio"),
            handle("3d", "3D"),
        ],
        NodeType::ConditionalSwitch | NodeType::Array => vec![handle("text", "text")],
        NodeType::Annotation => vec![handle("image", "image")],
        NodeType::Prompt
        | NodeType::ImageInput
        | NodeType::AudioInput
        | NodeType::VideoInput
        | NodeType::SplitGrid
        | NodeType::GlbViewer
        | NodeType::Unknown => vec![handle("text", "text")],
    }
}

pub fn selected_handle_or_first(handles: &[NodeHandle], selected: &str) -> String {
    handles
        .iter()
        .find(|handle| handle.id == selected)
        .or_else(|| handles.first())
        .map(|handle| handle.id.clone())
        .unwrap_or_default()
}

fn split_grid_source_handles(node: &WorkflowNode) -> Vec<NodeHandle> {
    let image_count = node
        .data
        .get("images")
        .and_then(Value::as_array)
        .map_or(0, Vec::len)
        .max(
            node.data
                .get("targetCount")
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .unwrap_or(0),
        );
    let mut handles = vec![handle("image", "first image")];
    for index in 0..image_count.min(12) {
        handles.push(handle(
            format!("image-{index}"),
            format!("image {}", index + 1),
        ));
    }
    dedupe_handles(handles)
}

fn array_source_handles(node: &WorkflowNode) -> Vec<NodeHandle> {
    let mut handles = vec![handle("text", "text / selected")];
    let items = node
        .data
        .get("outputItems")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    for index in 0..items.min(8) {
        handles.push(handle(format!("text-{index}"), format!("item {index}")));
    }
    handles
}

fn switch_source_handles(node: &WorkflowNode) -> Vec<NodeHandle> {
    node.data
        .get("switches")
        .and_then(Value::as_array)
        .map(|switches| {
            switches
                .iter()
                .filter_map(|switch| {
                    let id = switch.get("id").and_then(Value::as_str)?;
                    let label = switch.get("name").and_then(Value::as_str).unwrap_or(id);
                    Some(handle(id, label))
                })
                .collect()
        })
        .unwrap_or_else(|| vec![handle("text", "text")])
}

fn conditional_switch_source_handles(node: &WorkflowNode) -> Vec<NodeHandle> {
    let mut handles = node
        .data
        .get("rules")
        .and_then(Value::as_array)
        .map(|rules| {
            rules
                .iter()
                .filter_map(|rule| {
                    let id = rule.get("id").and_then(Value::as_str)?;
                    let label = rule.get("label").and_then(Value::as_str).unwrap_or(id);
                    Some(handle(id, label))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    handles.push(handle("default", "default"));
    handles
}

fn schema_handle_options(node: &WorkflowNode) -> Vec<NodeHandle> {
    let mut options = Vec::new();
    let Some(schema) = node.data.get("inputSchema").and_then(Value::as_array) else {
        return options;
    };

    let mut image_index = 0;
    let mut text_index = 0;
    let mut audio_index = 0;
    for input in schema {
        let Some(name) = input.get("name").and_then(Value::as_str) else {
            continue;
        };
        match input.get("type").and_then(Value::as_str) {
            Some("image") => {
                options.push(handle(format!("image-{image_index}"), name));
                image_index += 1;
            }
            Some("text") => {
                options.push(handle(format!("text-{text_index}"), name));
                text_index += 1;
            }
            Some("audio") => {
                options.push(handle(format!("audio-{audio_index}"), name));
                audio_index += 1;
            }
            _ => {}
        }
    }
    options
}

fn dedupe_handles(handles: Vec<NodeHandle>) -> Vec<NodeHandle> {
    let mut deduped = Vec::new();
    for handle in handles {
        if !deduped
            .iter()
            .any(|existing: &NodeHandle| existing.id == handle.id)
        {
            deduped.push(handle);
        }
    }
    deduped
}

fn handle(id: impl Into<String>, label: impl Into<String>) -> NodeHandle {
    NodeHandle::new(id, label)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Position, WorkflowNode};
    use serde_json::json;

    #[test]
    fn array_source_handles_include_item_indices() {
        let node = WorkflowNode::new(
            "array",
            NodeType::Array,
            Position { x: 0.0, y: 0.0 },
            json!({"outputItems":["a","b"]}),
        );

        let handles = source_handle_options(&node);
        let ids: Vec<&str> = handles.iter().map(|handle| handle.id.as_str()).collect();
        assert_eq!(ids, vec!["text", "text-0", "text-1"]);
    }

    #[test]
    fn split_grid_source_handles_include_generated_image_cells() {
        let node = WorkflowNode::new(
            "split",
            NodeType::SplitGrid,
            Position { x: 0.0, y: 0.0 },
            json!({"targetCount":3}),
        );

        let handles = source_handle_options(&node);
        let ids: Vec<&str> = handles.iter().map(|handle| handle.id.as_str()).collect();
        assert_eq!(ids, vec!["image", "image-0", "image-1", "image-2"]);
    }

    #[test]
    fn dynamic_schema_target_handles_are_deduped_with_defaults() {
        let node = WorkflowNode::new(
            "gen",
            NodeType::NanoBanana,
            Position { x: 0.0, y: 0.0 },
            json!({
                "inputSchema": [
                    {"name":"primary image", "type":"image", "required":true, "label":"Image"},
                    {"name":"copy prompt", "type":"text", "required":false, "label":"Prompt"},
                    {"name":"voice", "type":"audio", "required":false, "label":"Audio"}
                ]
            }),
        );

        let handles = target_handle_options(&node);
        let ids: Vec<&str> = handles.iter().map(|handle| handle.id.as_str()).collect();
        assert_eq!(ids, vec!["prompt", "image", "image-0", "text-0", "audio-0"]);
    }

    #[test]
    fn conditional_switch_outputs_include_rules_and_default() {
        let node = WorkflowNode::new(
            "cond",
            NodeType::ConditionalSwitch,
            Position { x: 0.0, y: 0.0 },
            json!({
                "rules": [
                    {"id":"rule-1", "label":"Matched", "value":"yes", "mode":"contains"}
                ]
            }),
        );

        let handles = source_handle_options(&node);
        let ids: Vec<&str> = handles.iter().map(|handle| handle.id.as_str()).collect();
        assert_eq!(ids, vec!["rule-1", "default"]);
    }

    #[test]
    fn selected_handle_falls_back_to_first_valid_handle() {
        let handles = vec![handle("text", "text"), handle("image", "image")];
        assert_eq!(selected_handle_or_first(&handles, "missing"), "text");
        assert_eq!(selected_handle_or_first(&handles, "image"), "image");
    }
}
