use gemed_core::{NodeType, WorkflowFile};
use serde::{Deserialize, Serialize};

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
}
