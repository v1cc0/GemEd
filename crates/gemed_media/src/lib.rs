use gemed_core::{NodeType, WorkflowFile, WorkflowNode};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const DEFAULT_INLINE_PREVIEW_LIMIT_BYTES: usize = 512 * 1024;

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
    pub fn should_inline_preview(&self) -> bool {
        self.is_renderable_uri() && !self.is_large_inline()
    }

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

    pub fn is_large_inline(&self) -> bool {
        self.inline_byte_len()
            .is_some_and(|bytes| bytes > DEFAULT_INLINE_PREVIEW_LIMIT_BYTES)
    }

    pub fn inline_mime(&self) -> Option<String> {
        data_url_parts(&self.uri).map(|parts| parts.mime.to_string())
    }

    pub fn inline_byte_len(&self) -> Option<usize> {
        let parts = data_url_parts(&self.uri)?;
        if parts.is_base64 {
            Some(base64_decoded_len(parts.payload))
        } else {
            Some(parts.payload.len())
        }
    }

    pub fn size_hint(&self) -> Option<String> {
        self.inline_byte_len().map(human_bytes)
    }

    pub fn download_filename(&self) -> String {
        let base = sanitize_filename(&self.label)
            .or_else(|| sanitize_filename(&self.source_field))
            .unwrap_or_else(|| "gemed-media".to_string());
        let extension = data_url_parts(&self.uri)
            .and_then(|parts| extension_for_mime(parts.mime))
            .or_else(|| extension_from_uri(&self.uri))
            .unwrap_or_else(|| self.kind.default_extension());
        format!("{base}.{extension}")
    }

    pub fn uri_hint(&self) -> String {
        let uri = self.uri.trim();
        if let Some(parts) = data_url_parts(uri) {
            return match self.size_hint() {
                Some(size) => format!("inline {} · {size}", parts.mime),
                None => format!("inline {}", parts.mime),
            };
        }
        if uri.starts_with("gemed-media://") {
            return "project media reference".to_string();
        }
        truncate_middle(uri, 72)
    }
}

impl MediaKind {
    fn default_extension(self) -> &'static str {
        match self {
            Self::Image => "png",
            Self::Audio => "wav",
            Self::Video => "mp4",
            Self::Model3d => "glb",
        }
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
    let node_label = explicit_node_label(&node.data).unwrap_or_default();
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
            PreviewSource {
                label: label_for_single_preview(spec, &node_label),
                source_field: source_field.to_string(),
            },
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
                PreviewSource {
                    label,
                    source_field: format!("{source_field}[{index}]"),
                },
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
    source: PreviewSource,
    uri: String,
    content_hint: Option<MediaKind>,
) {
    push_preview_with_label(previews, spec, source, uri, content_hint);
}

fn push_preview_with_label(
    previews: &mut Vec<MediaPreview>,
    spec: &MediaFieldSpec,
    source: PreviewSource,
    uri: String,
    content_hint: Option<MediaKind>,
) {
    let uri = uri.trim().to_string();
    if uri.is_empty() {
        return;
    }
    previews.push(MediaPreview {
        kind: detect_media_kind(spec.kind, &uri, content_hint),
        label: source.label,
        uri,
        source_field: source.source_field,
    });
}

struct PreviewSource {
    label: String,
    source_field: String,
}

fn label_for_single_preview(spec: &MediaFieldSpec, node_label: &str) -> String {
    let node_label = node_label.trim();
    if !node_label.is_empty() && node_label != spec.label {
        node_label.to_string()
    } else {
        spec.label.to_string()
    }
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

fn explicit_node_label(data: &Value) -> Option<String> {
    string_field(data, "label")
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

struct DataUrlParts<'a> {
    mime: &'a str,
    payload: &'a str,
    is_base64: bool,
}

fn data_url_parts(value: &str) -> Option<DataUrlParts<'_>> {
    let value = value.trim().strip_prefix("data:")?;
    let (metadata, payload) = value.split_once(',')?;
    let mut parts = metadata.split(';');
    let mime = parts
        .next()
        .filter(|mime| !mime.trim().is_empty())
        .unwrap_or("text/plain")
        .trim();
    let is_base64 = parts.any(|part| part.eq_ignore_ascii_case("base64"));
    Some(DataUrlParts {
        mime,
        payload,
        is_base64,
    })
}

fn base64_decoded_len(value: &str) -> usize {
    let encoded_len = value
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .count();
    let padding = value
        .trim_end()
        .bytes()
        .rev()
        .take_while(|byte| *byte == b'=')
        .count()
        .min(2);
    (encoded_len.saturating_mul(3) / 4usize).saturating_sub(padding)
}

fn human_bytes(bytes: usize) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    if bytes >= 1024 * 1024 {
        format!("{:.1} MiB", bytes as f64 / MIB)
    } else if bytes >= 1024 {
        format!("{:.1} KiB", bytes as f64 / KIB)
    } else {
        format!("{bytes} B")
    }
}

fn sanitize_filename(value: &str) -> Option<String> {
    let mut output = String::new();
    let mut previous_separator = false;
    for ch in value.trim().chars() {
        let next = if ch.is_ascii_alphanumeric() {
            Some(ch.to_ascii_lowercase())
        } else if matches!(ch, '-' | '_') {
            Some(ch)
        } else {
            Some('-')
        };

        if let Some(ch) = next {
            if ch == '-' || ch == '_' {
                if previous_separator {
                    continue;
                }
                previous_separator = true;
            } else {
                previous_separator = false;
            }
            output.push(ch);
        }
    }

    let output = output.trim_matches(['-', '_']).to_string();
    (!output.is_empty()).then_some(output)
}

fn extension_for_mime(mime: &str) -> Option<&'static str> {
    match mime.to_ascii_lowercase().as_str() {
        "image/svg+xml" => Some("svg"),
        "image/png" => Some("png"),
        "image/jpeg" => Some("jpg"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        "audio/wav" | "audio/x-wav" => Some("wav"),
        "audio/mpeg" | "audio/mp3" => Some("mp3"),
        "audio/ogg" => Some("ogg"),
        "video/mp4" => Some("mp4"),
        "video/webm" => Some("webm"),
        "model/gltf-binary" => Some("glb"),
        "model/gltf+json" | "application/gltf+json" => Some("gltf"),
        _ => None,
    }
}

fn extension_from_uri(uri: &str) -> Option<&str> {
    let path = uri
        .split(['?', '#'])
        .next()
        .unwrap_or(uri)
        .trim_end_matches('/');
    let extension = path.rsplit_once('.')?.1;
    (!extension.is_empty()
        && extension.len() <= 8
        && extension.bytes().all(|byte| byte.is_ascii_alphanumeric()))
    .then_some(extension)
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
        assert_eq!(previews[0].inline_mime().as_deref(), Some("image/png"));
        assert_eq!(previews[0].inline_byte_len(), Some(5));
        assert_eq!(previews[0].size_hint().as_deref(), Some("5 B"));
        assert_eq!(previews[0].download_filename(), "image.png");
        assert!(previews[0].should_inline_preview());
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
        assert_eq!(previews[0].download_filename(), "3d-model.glb");
    }

    #[test]
    fn media_preview_marks_large_inline_payloads_as_non_inline_previews() {
        let payload = "A".repeat(DEFAULT_INLINE_PREVIEW_LIMIT_BYTES * 4 / 3 + 8);
        let node = WorkflowNode::new(
            "large",
            NodeType::ImageInput,
            Position { x: 0.0, y: 0.0 },
            json!({
                "label": "Large Inline Image",
                "image": format!("data:image/png;base64,{payload}")
            }),
        );

        let previews = media_previews_for_node(&node);

        assert_eq!(previews.len(), 1);
        assert!(previews[0].is_renderable_uri());
        assert!(previews[0].is_large_inline());
        assert!(!previews[0].should_inline_preview());
        assert_eq!(previews[0].download_filename(), "large-inline-image.png");
    }

    #[test]
    fn media_preview_download_filename_uses_url_extension_when_available() {
        let node = WorkflowNode::new(
            "video",
            NodeType::VideoInput,
            Position { x: 0.0, y: 0.0 },
            json!({
                "label": "Rendered Clip",
                "video": "https://example.invalid/media/rendered.webm?token=1"
            }),
        );

        let previews = media_previews_for_node(&node);

        assert_eq!(previews.len(), 1);
        assert_eq!(previews[0].download_filename(), "rendered-clip.webm");
        assert_eq!(
            previews[0].uri_hint(),
            "https://example.invalid/media/rendered.webm?token=1"
        );
    }

    #[test]
    fn built_in_media_sample_exercises_renderable_and_project_reference_previews() {
        let workflow = WorkflowFile::media_preview_example();
        let previews = workflow
            .nodes
            .iter()
            .flat_map(media_previews_for_node)
            .collect::<Vec<_>>();

        assert_eq!(previews.len(), 7);
        assert_eq!(
            previews
                .iter()
                .filter(|preview| preview.is_renderable_uri())
                .count(),
            5
        );
        assert_eq!(
            previews
                .iter()
                .filter(|preview| preview.uri.starts_with("gemed-media://"))
                .count(),
            2
        );
        assert!(
            previews
                .iter()
                .any(|preview| preview.kind == MediaKind::Audio)
        );
        assert!(
            previews
                .iter()
                .any(|preview| preview.kind == MediaKind::Video)
        );
        assert!(
            previews
                .iter()
                .any(|preview| preview.kind == MediaKind::Model3d)
        );
    }
}
