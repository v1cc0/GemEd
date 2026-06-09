use gemed_core::{EdgeStyle, NodeType, WorkflowFile};
use serde_json::Value;
use std::path::{Path, PathBuf};

#[test]
fn representative_legacy_workflow_fixtures_import_and_roundtrip() {
    for fixture in workflow_fixture_paths() {
        let source = std::fs::read_to_string(&fixture)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", fixture.display()));
        let workflow = WorkflowFile::from_json_str(&source)
            .unwrap_or_else(|err| panic!("failed to parse {}: {err}", fixture.display()));

        assert!(
            !workflow.nodes.is_empty(),
            "{} should contain nodes",
            fixture.display()
        );
        assert!(
            !workflow.edges.is_empty(),
            "{} should contain edges",
            fixture.display()
        );

        let exported = workflow
            .to_pretty_json()
            .unwrap_or_else(|err| panic!("failed to export {}: {err}", fixture.display()));
        let reparsed = WorkflowFile::from_json_str(&exported).unwrap_or_else(|err| {
            panic!(
                "failed to reparse exported fixture {}: {err}",
                fixture.display()
            )
        });
        assert_eq!(
            reparsed,
            workflow,
            "{} roundtrip drifted",
            fixture.display()
        );
    }
}

#[test]
fn legacy_media_provider_fixture_preserves_react_flow_and_provider_contracts() {
    let workflow = load_fixture("legacy-media-provider.json");

    assert_eq!(workflow.id.as_deref(), Some("wf_legacy_media_provider"));
    assert_eq!(workflow.directory_path.as_deref(), Some("/legacy/project"));
    assert_eq!(workflow.edge_style, EdgeStyle::Curved);
    assert_eq!(workflow.nodes.len(), 5);
    assert_eq!(workflow.edges.len(), 4);
    assert_eq!(workflow.groups.len(), 1);

    let image_node = workflow
        .nodes
        .iter()
        .find(|node| node.id == "image-input-1")
        .expect("image node exists");
    assert_eq!(image_node.node_type, NodeType::ImageInput);
    assert_eq!(image_node.selected, Some(true));
    assert_eq!(image_node.group_id.as_deref(), Some("group-inputs"));
    assert_eq!(image_node.data["image"], "/sample-images/watch.jpg");
    assert!(image_node.extra.contains_key("reactFlowMeasured"));

    let provider_node = workflow
        .nodes
        .iter()
        .find(|node| node.id == "generate-1")
        .expect("provider node exists");
    assert_eq!(provider_node.node_type, NodeType::NanoBanana);
    assert_eq!(provider_node.data["provider"], "gemini");
    assert_eq!(provider_node.data["modelId"], "gemini-2.5-flash-image");

    let llm_node = workflow
        .nodes
        .iter()
        .find(|node| node.id == "llm-1")
        .expect("llm node exists");
    assert_eq!(llm_node.node_type, NodeType::LlmGenerate);
    assert_eq!(llm_node.data["selectedModel"]["provider"], "openai");

    let edge = workflow
        .edges
        .iter()
        .find(|edge| edge.id == "edge-image-generate")
        .expect("handle edge exists");
    assert_eq!(edge.source_handle.as_deref(), Some("image"));
    assert_eq!(edge.target_handle.as_deref(), Some("image"));
    assert_eq!(edge.edge_type.as_deref(), Some("editable"));
    assert_eq!(edge.data.has_pause, Some(true));
    assert_eq!(edge.data.extra["offsetX"], 12);

    let group = workflow.groups.get("group-inputs").expect("group exists");
    assert_eq!(group.color.to_string_for_test(), "blue");
    assert_eq!(group.is_nbp_input, Some(true));
    assert_eq!(group.extra["legacyCollapsed"], false);

    let root_extra = serde_json::to_value(&workflow)
        .expect("serialize workflow")
        .get("viewport")
        .cloned();
    assert!(
        root_extra.is_none(),
        "unknown root-level fields are intentionally not part of the Rust workflow contract"
    );
}

#[test]
fn legacy_control_fixture_preserves_control_edges_and_unknown_node_payload() {
    let workflow = load_fixture("legacy-control-routing.json");

    assert_eq!(workflow.edge_style, EdgeStyle::Angular);
    assert_eq!(workflow.nodes.len(), 6);
    assert_eq!(workflow.edges.len(), 5);

    let conditional = workflow
        .nodes
        .iter()
        .find(|node| node.id == "switch-1")
        .expect("conditional switch exists");
    assert_eq!(conditional.node_type, NodeType::ConditionalSwitch);
    assert_eq!(conditional.data["rules"][0]["id"], "rule-ruby");
    assert_eq!(conditional.data["rules"][0]["isMatched"], true);

    let unknown = workflow
        .nodes
        .iter()
        .find(|node| node.id == "unknown-legacy-node")
        .expect("unknown legacy node exists");
    assert_eq!(unknown.node_type, NodeType::Unknown);
    assert_eq!(unknown.data["pluginPayload"]["kind"], "third-party");
    assert_eq!(unknown.extra["deletable"], false);

    let matched_edge = workflow
        .edges
        .iter()
        .find(|edge| edge.id == "edge-switch-compare")
        .expect("conditional edge exists");
    assert_eq!(
        matched_edge.source_handle.as_deref(),
        Some("matched:rule-ruby")
    );
    assert_eq!(matched_edge.data.has_pause, Some(true));
}

#[test]
fn media_preview_sample_fixture_covers_renderable_and_project_media_contracts() {
    let workflow = load_fixture("media-preview-sample.json");

    assert_eq!(workflow.id.as_deref(), Some("wf_media_preview_sample"));
    assert_eq!(workflow.nodes.len(), 5);
    assert_eq!(workflow.edges.len(), 3);

    let image = workflow
        .nodes
        .iter()
        .find(|node| node.id == "fixture-image")
        .expect("image fixture exists");
    assert_eq!(image.node_type, NodeType::ImageInput);
    assert!(
        image
            .data
            .get("image")
            .and_then(Value::as_str)
            .is_some_and(|value| value.starts_with("data:image/svg+xml;base64,"))
    );

    let audio = workflow
        .nodes
        .iter()
        .find(|node| node.id == "fixture-audio")
        .expect("audio fixture exists");
    assert_eq!(audio.node_type, NodeType::AudioInput);
    assert!(
        audio
            .data
            .get("audioFile")
            .and_then(Value::as_str)
            .is_some_and(|value| value.starts_with("data:audio/wav;base64,"))
    );

    let video = workflow
        .nodes
        .iter()
        .find(|node| node.id == "fixture-video")
        .expect("video fixture exists");
    assert_eq!(video.node_type, NodeType::VideoInput);
    assert_eq!(
        video.data.get("video").and_then(Value::as_str),
        Some("https://example.invalid/media-preview.mp4")
    );

    let gallery = workflow
        .nodes
        .iter()
        .find(|node| node.id == "fixture-gallery")
        .expect("gallery fixture exists");
    assert_eq!(
        gallery.data["imageRefs"][0],
        "gemed-media://media/external-preview.png"
    );

    let glb = workflow
        .nodes
        .iter()
        .find(|node| node.id == "fixture-glb")
        .expect("glb fixture exists");
    assert_eq!(
        glb.data.get("glbUrl").and_then(Value::as_str),
        Some("gemed-media://media/fixture-model.glb")
    );
}

#[test]
fn fixture_manifest_covers_expected_legacy_shapes() {
    let fixtures = workflow_fixture_paths();
    let names = fixtures
        .iter()
        .filter_map(|path| path.file_name().and_then(|name| name.to_str()))
        .collect::<Vec<_>>();

    assert!(names.contains(&"legacy-media-provider.json"));
    assert!(names.contains(&"legacy-control-routing.json"));
    assert!(names.contains(&"media-preview-sample.json"));
}

fn workflow_fixture_paths() -> Vec<PathBuf> {
    let fixture_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("workflows");
    let mut paths = std::fs::read_dir(&fixture_dir)
        .unwrap_or_else(|err| {
            panic!(
                "failed to read fixture dir {}: {err}",
                fixture_dir.display()
            )
        })
        .map(|entry| entry.expect("fixture dir entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn load_fixture(name: &str) -> WorkflowFile {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("workflows")
        .join(name);
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
    WorkflowFile::from_json_str(&source)
        .unwrap_or_else(|err| panic!("failed to parse {}: {err}", path.display()))
}

trait GroupColorTestExt {
    fn to_string_for_test(self) -> &'static str;
}

impl GroupColorTestExt for gemed_core::GroupColor {
    fn to_string_for_test(self) -> &'static str {
        match self {
            gemed_core::GroupColor::Neutral => "neutral",
            gemed_core::GroupColor::Blue => "blue",
            gemed_core::GroupColor::Green => "green",
            gemed_core::GroupColor::Purple => "purple",
            gemed_core::GroupColor::Orange => "orange",
            gemed_core::GroupColor::Red => "red",
        }
    }
}
