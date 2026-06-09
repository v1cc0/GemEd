use gemed_core::{NodeType, WorkflowFile, WorkflowNode};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MediaKind {
    Image,
    Audio,
    Video,
    Model3d,
}

impl MediaKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Audio => "audio",
            Self::Video => "video",
            Self::Model3d => "3D",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PlatformTarget {
    Web,
    Desktop,
}

impl PlatformTarget {
    pub fn label(self) -> &'static str {
        match self {
            Self::Web => "web",
            Self::Desktop => "desktop",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MediaSupportLevel {
    Ready,
    PreviewOnly,
    AdapterRequired,
    NotApplicable,
}

impl MediaSupportLevel {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::PreviewOnly => "preview",
            Self::AdapterRequired => "adapter",
            Self::NotApplicable => "n/a",
        }
    }

    pub fn needs_adapter(self) -> bool {
        matches!(self, Self::AdapterRequired)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaNodeProfile {
    pub node_type: NodeType,
    pub label: String,
    pub media_kinds: Vec<MediaKind>,
    pub web: MediaSupportLevel,
    pub desktop: MediaSupportLevel,
    pub notes: String,
}

impl MediaNodeProfile {
    pub fn needs_adapter(&self) -> bool {
        self.web.needs_adapter() || self.desktop.needs_adapter()
    }

    pub fn kind_labels(&self) -> String {
        if self.media_kinds.is_empty() {
            return "none".to_string();
        }
        self.media_kinds
            .iter()
            .map(|kind| kind.label())
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn platform_label(&self) -> String {
        format!(
            "web: {}, desktop: {}",
            self.web.label(),
            self.desktop.label()
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowMediaSummary {
    pub media_node_count: usize,
    pub adapter_required_count: usize,
    pub ready_count: usize,
    pub preview_only_count: usize,
    pub profiles: Vec<MediaNodeProfile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaPreview {
    pub kind: MediaKind,
    pub label: String,
    pub uri: String,
    pub source_field: String,
}

impl MediaPreview {
    pub fn is_renderable_uri(&self) -> bool {
        let uri = self.uri.trim();
        uri.starts_with("data:")
            || uri.starts_with("blob:")
            || uri.starts_with("http://")
            || uri.starts_with("https://")
            || uri.starts_with('/')
            || uri.starts_with("./")
            || uri.starts_with("../")
    }

    pub fn uri_hint(&self) -> String {
        let uri = self.uri.trim();
        if let Some(mime) = uri.strip_prefix("data:").and_then(|value| {
            value
                .split_once(';')
                .map(|(mime, _)| mime)
                .or_else(|| value.split_once(',').map(|(mime, _)| mime))
        }) {
            return format!("inline {mime}");
        }
        if uri.starts_with("gemed-media://") {
            return "project media reference".to_string();
        }
        truncate_middle(uri, 72)
    }
}

impl WorkflowMediaSummary {
    pub fn sentence(&self) -> String {
        if self.media_node_count == 0 {
            return "Media: no media-specific nodes in this workflow.".to_string();
        }
        format!(
            "Media: {} media node(s), {} ready, {} preview-only, {} need adapter work.",
            self.media_node_count,
            self.ready_count,
            self.preview_only_count,
            self.adapter_required_count
        )
    }
}

pub fn workflow_media_summary(workflow: &WorkflowFile) -> WorkflowMediaSummary {
    let mut profiles = workflow
        .nodes
        .iter()
        .filter_map(|node| media_profile_for_node_type(&node.node_type))
        .collect::<Vec<_>>();
    profiles.sort_by(|left, right| left.label.cmp(&right.label));
    let media_node_count = profiles.len();
    let adapter_required_count = profiles
        .iter()
        .filter(|profile| profile.needs_adapter())
        .count();
    let ready_count = profiles
        .iter()
        .filter(|profile| {
            profile.web == MediaSupportLevel::Ready && profile.desktop == MediaSupportLevel::Ready
        })
        .count();
    let preview_only_count = profiles
        .iter()
        .filter(|profile| {
            profile.web == MediaSupportLevel::PreviewOnly
                || profile.desktop == MediaSupportLevel::PreviewOnly
        })
        .count();

    WorkflowMediaSummary {
        media_node_count,
        adapter_required_count,
        ready_count,
        preview_only_count,
        profiles,
    }
}

pub fn media_previews_for_node(node: &WorkflowNode) -> Vec<MediaPreview> {
    let content_hint = content_type_hint(&node.data);
    let mut previews = Vec::new();

    for spec in SINGLE_MEDIA_FIELDS {
        let inline = string_field(&node.data, spec.field);
        let reference = spec
            .ref_field
            .and_then(|ref_field| string_field(&node.data, ref_field).map(|uri| (ref_field, uri)));
        let Some((source_field, uri)) = inline.map(|uri| (spec.field, uri)).or(reference) else {
            continue;
        };
        push_preview(
            &mut previews,
            spec,
            source_field.to_string(),
            uri,
            content_hint,
        );
    }

    for spec in ARRAY_MEDIA_FIELDS {
        let inline_values = string_array_slots(&node.data, spec.field);
        let reference_values = spec
            .ref_field
            .map(|ref_field| string_array_slots(&node.data, ref_field))
            .unwrap_or_default();
        let max_len = inline_values.len().max(reference_values.len());
        for index in 0..max_len {
            let inline = inline_values.get(index).cloned().flatten();
            let reference = reference_values.get(index).cloned().flatten();
            let Some((source_field, uri)) = inline
                .map(|uri| (spec.field, uri))
                .or_else(|| reference.map(|uri| (spec.ref_field.unwrap_or(spec.field), uri)))
            else {
                continue;
            };
            let label = format!("{} {}", spec.label, index + 1);
            push_preview_with_label(
                &mut previews,
                spec,
                label,
                format!("{source_field}[{index}]"),
                uri,
                content_hint,
            );
        }
    }

    previews
}

pub fn media_profile_for_node_type(node_type: &NodeType) -> Option<MediaNodeProfile> {
    let profile = match node_type {
        NodeType::ImageInput => profile(
            node_type,
            vec![MediaKind::Image],
            MediaSupportLevel::Ready,
            MediaSupportLevel::Ready,
            "Image data can be referenced or embedded; project bundles externalize data URLs on desktop.",
        ),
        NodeType::AudioInput => profile(
            node_type,
            vec![MediaKind::Audio],
            MediaSupportLevel::PreviewOnly,
            MediaSupportLevel::Ready,
            "Audio input metadata/storage is modeled; richer waveform preview remains UI work.",
        ),
        NodeType::VideoInput => profile(
            node_type,
            vec![MediaKind::Video],
            MediaSupportLevel::PreviewOnly,
            MediaSupportLevel::Ready,
            "Video input metadata/storage is modeled; browser/video adapter work remains.",
        ),
        NodeType::Annotation => profile(
            node_type,
            vec![MediaKind::Image],
            MediaSupportLevel::PreviewOnly,
            MediaSupportLevel::PreviewOnly,
            "Current executor passes image references through; drawing tools need a canvas adapter.",
        ),
        NodeType::NanoBanana => profile(
            node_type,
            vec![MediaKind::Image],
            MediaSupportLevel::Ready,
            MediaSupportLevel::Ready,
            "Image generation works through provider traits and mocks; live image providers remain future work.",
        ),
        NodeType::GenerateVideo => profile(
            node_type,
            vec![MediaKind::Video, MediaKind::Image],
            MediaSupportLevel::AdapterRequired,
            MediaSupportLevel::AdapterRequired,
            "Video generation requires polling/provider adapters and platform preview handling.",
        ),
        NodeType::Generate3d => profile(
            node_type,
            vec![MediaKind::Model3d, MediaKind::Image],
            MediaSupportLevel::AdapterRequired,
            MediaSupportLevel::AdapterRequired,
            "3D generation requires model fetch/storage plus GLB preview adapters.",
        ),
        NodeType::GenerateAudio => profile(
            node_type,
            vec![MediaKind::Audio],
            MediaSupportLevel::AdapterRequired,
            MediaSupportLevel::AdapterRequired,
            "Audio generation requires live provider adapters and playback/storage polish.",
        ),
        NodeType::SplitGrid => profile(
            node_type,
            vec![MediaKind::Image],
            MediaSupportLevel::AdapterRequired,
            MediaSupportLevel::AdapterRequired,
            "Split-grid image processing needs deterministic transform implementation.",
        ),
        NodeType::ImageCompare => profile(
            node_type,
            vec![MediaKind::Image],
            MediaSupportLevel::PreviewOnly,
            MediaSupportLevel::PreviewOnly,
            "Image compare metadata is executable; richer visual comparison remains UI work.",
        ),
        NodeType::VideoStitch => profile(
            node_type,
            vec![MediaKind::Video],
            MediaSupportLevel::AdapterRequired,
            MediaSupportLevel::AdapterRequired,
            "Video stitching needs browser/native media processing adapters.",
        ),
        NodeType::EaseCurve => profile(
            node_type,
            vec![MediaKind::Video],
            MediaSupportLevel::AdapterRequired,
            MediaSupportLevel::AdapterRequired,
            "Ease curve needs media timeline transform support.",
        ),
        NodeType::VideoTrim => profile(
            node_type,
            vec![MediaKind::Video],
            MediaSupportLevel::AdapterRequired,
            MediaSupportLevel::AdapterRequired,
            "Video trim needs browser/native media processing adapters.",
        ),
        NodeType::VideoFrameGrab => profile(
            node_type,
            vec![MediaKind::Video, MediaKind::Image],
            MediaSupportLevel::AdapterRequired,
            MediaSupportLevel::AdapterRequired,
            "Frame grab needs video decode and image capture adapters.",
        ),
        NodeType::GlbViewer => profile(
            node_type,
            vec![MediaKind::Model3d],
            MediaSupportLevel::AdapterRequired,
            MediaSupportLevel::AdapterRequired,
            "GLB viewer needs WebGL/WebView-compatible preview adapter.",
        ),
        NodeType::Output | NodeType::OutputGallery => profile(
            node_type,
            vec![
                MediaKind::Image,
                MediaKind::Audio,
                MediaKind::Video,
                MediaKind::Model3d,
            ],
            MediaSupportLevel::PreviewOnly,
            MediaSupportLevel::PreviewOnly,
            "Output nodes collect media references; rich gallery/player previews remain UI work.",
        ),
        _ => return None,
    };
    Some(profile)
}

fn profile(
    node_type: &NodeType,
    media_kinds: Vec<MediaKind>,
    web: MediaSupportLevel,
    desktop: MediaSupportLevel,
    notes: &str,
) -> MediaNodeProfile {
    MediaNodeProfile {
        node_type: node_type.clone(),
        label: node_type.title().to_string(),
        media_kinds,
        web,
        desktop,
        notes: notes.to_string(),
    }
}

#[derive(Debug, Clone, Copy)]
struct MediaFieldSpec {
    field: &'static str,
    ref_field: Option<&'static str>,
    kind: MediaKind,
    label: &'static str,
}

const SINGLE_MEDIA_FIELDS: &[MediaFieldSpec] = &[
    MediaFieldSpec {
        field: "image",
        ref_field: Some("imageRef"),
        kind: MediaKind::Image,
        label: "Image",
    },
    MediaFieldSpec {
        field: "audio",
        ref_field: None,
        kind: MediaKind::Audio,
        label: "Audio",
    },
    MediaFieldSpec {
        field: "audioFile",
        ref_field: Some("audioFileRef"),
        kind: MediaKind::Audio,
        label: "Audio file",
    },
    MediaFieldSpec {
        field: "video",
        ref_field: Some("videoRef"),
        kind: MediaKind::Video,
        label: "Video",
    },
    MediaFieldSpec {
        field: "sourceImage",
        ref_field: Some("sourceImageRef"),
        kind: MediaKind::Image,
        label: "Source image",
    },
    MediaFieldSpec {
        field: "outputImage",
        ref_field: Some("outputImageRef"),
        kind: MediaKind::Image,
        label: "Output image",
    },
    MediaFieldSpec {
        field: "outputVideo",
        ref_field: Some("outputVideoRef"),
        kind: MediaKind::Video,
        label: "Output video",
    },
    MediaFieldSpec {
        field: "outputAudio",
        ref_field: Some("outputAudioRef"),
        kind: MediaKind::Audio,
        label: "Output audio",
    },
    MediaFieldSpec {
        field: "imageA",
        ref_field: Some("imageARef"),
        kind: MediaKind::Image,
        label: "Image A",
    },
    MediaFieldSpec {
        field: "imageB",
        ref_field: Some("imageBRef"),
        kind: MediaKind::Image,
        label: "Image B",
    },
    MediaFieldSpec {
        field: "capturedImage",
        ref_field: Some("capturedImageRef"),
        kind: MediaKind::Image,
        label: "Captured image",
    },
    MediaFieldSpec {
        field: "thumbnail",
        ref_field: None,
        kind: MediaKind::Image,
        label: "Thumbnail",
    },
    MediaFieldSpec {
        field: "output3dUrl",
        ref_field: None,
        kind: MediaKind::Model3d,
        label: "3D model",
    },
    MediaFieldSpec {
        field: "glbUrl",
        ref_field: None,
        kind: MediaKind::Model3d,
        label: "GLB",
    },
    MediaFieldSpec {
        field: "model3d",
        ref_field: None,
        kind: MediaKind::Model3d,
        label: "3D model",
    },
];

const ARRAY_MEDIA_FIELDS: &[MediaFieldSpec] = &[
    MediaFieldSpec {
        field: "inputImages",
        ref_field: Some("inputImageRefs"),
        kind: MediaKind::Image,
        label: "Input image",
    },
    MediaFieldSpec {
        field: "images",
        ref_field: Some("imageRefs"),
        kind: MediaKind::Image,
        label: "Gallery image",
    },
    MediaFieldSpec {
        field: "videos",
        ref_field: Some("videoRefs"),
        kind: MediaKind::Video,
        label: "Gallery video",
    },
];

fn push_preview(
    previews: &mut Vec<MediaPreview>,
    spec: &MediaFieldSpec,
    source_field: String,
    uri: String,
    content_hint: Option<MediaKind>,
) {
    push_preview_with_label(
        previews,
        spec,
        spec.label.to_string(),
        source_field,
        uri,
        content_hint,
    );
}

fn push_preview_with_label(
    previews: &mut Vec<MediaPreview>,
    spec: &MediaFieldSpec,
    label: String,
    source_field: String,
    uri: String,
    content_hint: Option<MediaKind>,
) {
    let uri = uri.trim().to_string();
    if uri.is_empty() {
        return;
    }
    previews.push(MediaPreview {
        kind: detect_media_kind(spec.kind, &uri, content_hint),
        label,
        uri,
        source_field,
    });
}

fn content_type_hint(data: &Value) -> Option<MediaKind> {
    match string_field(data, "contentType")?
        .to_ascii_lowercase()
        .as_str()
    {
        "image" => Some(MediaKind::Image),
        "audio" => Some(MediaKind::Audio),
        "video" => Some(MediaKind::Video),
        "3d" | "model3d" | "model-3d" => Some(MediaKind::Model3d),
        _ => None,
    }
}

fn detect_media_kind(default: MediaKind, uri: &str, content_hint: Option<MediaKind>) -> MediaKind {
    let lower = uri.to_ascii_lowercase();
    if lower.starts_with("data:image/") {
        return MediaKind::Image;
    }
    if lower.starts_with("data:audio/") {
        return MediaKind::Audio;
    }
    if lower.starts_with("data:video/") {
        return MediaKind::Video;
    }
    if lower.starts_with("data:model/") || lower.starts_with("data:application/gltf") {
        return MediaKind::Model3d;
    }

    let path = lower
        .split(['?', '#'])
        .next()
        .unwrap_or(lower.as_str())
        .trim_end_matches('/');
    if [".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg"]
        .iter()
        .any(|extension| path.ends_with(extension))
    {
        return MediaKind::Image;
    }
    if [".mp3", ".wav", ".ogg", ".flac", ".m4a"]
        .iter()
        .any(|extension| path.ends_with(extension))
    {
        return MediaKind::Audio;
    }
    if [".mp4", ".webm", ".mov", ".m4v"]
        .iter()
        .any(|extension| path.ends_with(extension))
    {
        return MediaKind::Video;
    }
    if [".glb", ".gltf"]
        .iter()
        .any(|extension| path.ends_with(extension))
    {
        return MediaKind::Model3d;
    }

    content_hint.unwrap_or(default)
}

fn string_field(data: &Value, field: &str) -> Option<String> {
    data.get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn string_array_slots(data: &Value, field: &str) -> Vec<Option<String>> {
    data.get(field)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    item.as_str()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(ToOwned::to_owned)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn truncate_middle(value: &str, max_chars: usize) -> String {
    let char_count = value.chars().count();
    if char_count <= max_chars {
        return value.to_string();
    }
    let head_len = max_chars.saturating_sub(1) * 2 / 3;
    let tail_len = max_chars.saturating_sub(1).saturating_sub(head_len);
    let head = value.chars().take(head_len).collect::<String>();
    let tail = value
        .chars()
        .rev()
        .take(tail_len)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("{head}…{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use gemed_core::{Position, WorkflowNode};
    use serde_json::json;

    #[test]
    fn generation_video_requires_adapters() {
        let profile = media_profile_for_node_type(&NodeType::GenerateVideo).unwrap();

        assert_eq!(
            profile.media_kinds,
            vec![MediaKind::Video, MediaKind::Image]
        );
        assert!(profile.needs_adapter());
        assert_eq!(profile.platform_label(), "web: adapter, desktop: adapter");
    }

    #[test]
    fn prompt_is_not_media_profile() {
        assert_eq!(media_profile_for_node_type(&NodeType::Prompt), None);
    }

    #[test]
    fn workflow_summary_counts_media_nodes() {
        let workflow = WorkflowFile {
            name: "media summary".to_string(),
            nodes: vec![
                WorkflowNode::new(
                    "prompt",
                    NodeType::Prompt,
                    Position { x: 0.0, y: 0.0 },
                    json!({}),
                ),
                WorkflowNode::new(
                    "image",
                    NodeType::ImageInput,
                    Position { x: 0.0, y: 0.0 },
                    json!({}),
                ),
                WorkflowNode::new(
                    "video",
                    NodeType::VideoTrim,
                    Position { x: 0.0, y: 0.0 },
                    json!({}),
                ),
            ],
            ..WorkflowFile::blank()
        };

        let summary = workflow_media_summary(&workflow);

        assert_eq!(summary.media_node_count, 2);
        assert_eq!(summary.ready_count, 1);
        assert_eq!(summary.adapter_required_count, 1);
        assert!(summary.sentence().contains("2 media node"));
    }

    #[test]
    fn media_preview_prefers_inline_image_over_reference() {
        let node = WorkflowNode::new(
            "image",
            NodeType::ImageInput,
            Position { x: 0.0, y: 0.0 },
            json!({
                "image": "data:image/png;base64,aGVsbG8=",
                "imageRef": "gemed-media://media/image.png"
            }),
        );

        let previews = media_previews_for_node(&node);

        assert_eq!(previews.len(), 1);
        assert_eq!(previews[0].kind, MediaKind::Image);
        assert_eq!(previews[0].source_field, "image");
        assert_eq!(previews[0].uri, "data:image/png;base64,aGVsbG8=");
    }

    #[test]
    fn media_preview_uses_reference_when_inline_media_was_externalized() {
        let node = WorkflowNode::new(
            "gallery",
            NodeType::OutputGallery,
            Position { x: 0.0, y: 0.0 },
            json!({
                "images": ["", "data:image/webp;base64,aGVsbG8="],
                "imageRefs": ["gemed-media://media/first.png"]
            }),
        );

        let previews = media_previews_for_node(&node);

        assert_eq!(previews.len(), 2);
        assert_eq!(previews[0].uri, "gemed-media://media/first.png");
        assert_eq!(previews[0].source_field, "imageRefs[0]");
        assert!(!previews[0].is_renderable_uri());
        assert_eq!(previews[1].uri, "data:image/webp;base64,aGVsbG8=");
        assert_eq!(previews[1].source_field, "images[1]");
        assert!(previews[1].is_renderable_uri());
    }

    #[test]
    fn media_preview_detects_legacy_output_video_in_image_field() {
        let node = WorkflowNode::new(
            "output",
            NodeType::Output,
            Position { x: 0.0, y: 0.0 },
            json!({
                "image": "https://example.invalid/render.mp4?download=1",
                "contentType": "video"
            }),
        );

        let previews = media_previews_for_node(&node);

        assert_eq!(previews.len(), 1);
        assert_eq!(previews[0].kind, MediaKind::Video);
    }

    #[test]
    fn media_preview_reports_3d_model_urls_without_claiming_viewer_adapter() {
        let node = WorkflowNode::new(
            "model",
            NodeType::Generate3d,
            Position { x: 0.0, y: 0.0 },
            json!({
                "output3dUrl": "https://example.invalid/model.glb"
            }),
        );

        let previews = media_previews_for_node(&node);

        assert_eq!(previews.len(), 1);
        assert_eq!(previews[0].kind, MediaKind::Model3d);
        assert_eq!(previews[0].label, "3D model");
    }
}
