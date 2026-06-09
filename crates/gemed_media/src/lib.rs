use base64::{Engine as _, engine::general_purpose};
use gemed_core::{NodeType, WorkflowFile, WorkflowNode};
use image::{DynamicImage, GenericImageView, ImageFormat};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub const DEFAULT_INLINE_PREVIEW_LIMIT_BYTES: usize = 512 * 1024;

#[derive(Debug, Error)]
pub enum MediaTransformError {
    #[error(
        "unsupported media URI `{0}`; only inline image data URLs are transformable in the current local adapter"
    )]
    UnsupportedUri(String),
    #[error("unsupported inline image media type `{0}`")]
    UnsupportedMime(String),
    #[error(
        "unsupported video URI `{0}`; current frame-grab adapter boundary accepts inline video data URLs, blob/http(s) URLs, project media refs, or app-relative/static paths"
    )]
    UnsupportedVideoUri(String),
    #[error("unsupported inline video media type `{0}`")]
    UnsupportedVideoMime(String),
    #[error(
        "unsupported 3D model URI `{0}`; current GLB viewer adapter boundary accepts inline GLB/GLTF data URLs, blob/http(s) URLs, project media refs, or app-relative/static paths"
    )]
    UnsupportedModel3dUri(String),
    #[error("unsupported inline 3D model media type `{0}`")]
    UnsupportedModel3dMime(String),
    #[error("invalid inline GLB payload: {0}")]
    InvalidInlineGlb(String),
    #[error("invalid data URL")]
    InvalidDataUrl,
    #[error("base64 image payload is invalid: {0}")]
    InvalidBase64(#[from] base64::DecodeError),
    #[error("image decode/encode failed: {0}")]
    Image(#[from] image::ImageError),
    #[error("grid dimensions must be greater than zero")]
    InvalidGrid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridCellRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SplitGridResult {
    pub rows: u32,
    pub cols: u32,
    pub cells: Vec<GridCellRect>,
    pub images: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InlineImageMetadata {
    pub mime: String,
    pub width: u32,
    pub height: u32,
    pub byte_length: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InlineImageDifference {
    pub pixel_count: u64,
    pub changed_pixels: u64,
    pub changed_pixel_ratio: f64,
    pub mean_absolute_error: f64,
    pub max_channel_delta: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InlineImageCompareResult {
    pub image_a: InlineImageMetadata,
    pub image_b: InlineImageMetadata,
    pub dimensions_match: bool,
    pub exact_match: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub difference: Option<InlineImageDifference>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VideoFramePosition {
    First,
    Last,
}

impl VideoFramePosition {
    pub fn from_wire(value: &str) -> Option<Self> {
        match value.trim() {
            "first" => Some(Self::First),
            "last" => Some(Self::Last),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::First => "first",
            Self::Last => "last",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MediaUriKind {
    InlineData,
    Blob,
    Http,
    ProjectReference,
    StaticPath,
    RelativePath,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoUriMetadata {
    pub uri_kind: MediaUriKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_length: Option<usize>,
    pub renderable_in_webview: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoFrameGrabPlan {
    pub source: VideoUriMetadata,
    pub frame_position: VideoFramePosition,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_seek_seconds: Option<f64>,
    pub seek_requires_duration: bool,
    pub output_mime: String,
    pub requires_decode_adapter: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Model3dUriMetadata {
    pub uri_kind: MediaUriKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_length: Option<usize>,
    pub renderable_in_webview: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlbMetadata {
    pub version: u32,
    pub declared_byte_length: usize,
    pub json_chunk_byte_length: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generator: Option<String>,
    pub scene_count: usize,
    pub node_count: usize,
    pub mesh_count: usize,
    pub material_count: usize,
    pub animation_count: usize,
    pub image_count: usize,
    pub buffer_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlbViewerPlan {
    pub source: Model3dUriMetadata,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<GlbMetadata>,
    pub viewer_adapter: String,
    pub requires_webgl_adapter: bool,
    pub can_open_uri_directly: bool,
    pub capture_output_mime: String,
    pub requires_capture_adapter: bool,
}

pub fn passthrough_inline_image_data_url(value: &str) -> Result<String, MediaTransformError> {
    let (mime, bytes) = decode_inline_image_data_url(value)?;
    encode_image_data_url(&bytes, &mime)
}

pub fn inspect_inline_image(value: &str) -> Result<InlineImageMetadata, MediaTransformError> {
    let decoded = decode_inline_image(value)?;
    Ok(decoded.metadata())
}

pub fn compare_inline_images(
    image_a: &str,
    image_b: &str,
) -> Result<InlineImageCompareResult, MediaTransformError> {
    let image_a = decode_inline_image(image_a)?;
    let image_b = decode_inline_image(image_b)?;
    let dimensions_match = image_a.image.dimensions() == image_b.image.dimensions();
    let difference = dimensions_match.then(|| image_difference(&image_a.image, &image_b.image));
    let exact_match = difference
        .as_ref()
        .is_some_and(|difference| difference.changed_pixels == 0);

    Ok(InlineImageCompareResult {
        image_a: image_a.metadata(),
        image_b: image_b.metadata(),
        dimensions_match,
        exact_match,
        difference,
    })
}

pub fn split_inline_image_grid(
    value: &str,
    rows: u32,
    cols: u32,
    target_count: Option<usize>,
) -> Result<SplitGridResult, MediaTransformError> {
    if rows == 0 || cols == 0 {
        return Err(MediaTransformError::InvalidGrid);
    }
    let (_mime, bytes) = decode_inline_image_data_url(value)?;
    let image = image::load_from_memory(&bytes)?;
    let cells = grid_cells(image.width(), image.height(), rows, cols);
    let limit = target_count.unwrap_or(cells.len()).min(cells.len());
    let mut images = Vec::with_capacity(limit);

    for cell in cells.iter().take(limit) {
        let cropped = crop_image_cell(&image, *cell);
        images.push(encode_png_data_url(&cropped)?);
    }

    Ok(SplitGridResult {
        rows,
        cols,
        cells,
        images,
    })
}

pub fn plan_video_frame_grab(
    value: &str,
    frame_position: VideoFramePosition,
    known_duration_seconds: Option<f64>,
) -> Result<VideoFrameGrabPlan, MediaTransformError> {
    let source = inspect_video_uri(value)?;
    let requested_seek_seconds = match frame_position {
        VideoFramePosition::First => Some(0.001),
        VideoFramePosition::Last => known_duration_seconds
            .filter(|duration| duration.is_finite() && *duration > 0.0)
            .map(|duration| (duration - 0.1).max(0.0)),
    };
    Ok(VideoFrameGrabPlan {
        source,
        frame_position,
        requested_seek_seconds,
        seek_requires_duration: frame_position == VideoFramePosition::Last
            && requested_seek_seconds.is_none(),
        output_mime: "image/png".to_string(),
        requires_decode_adapter: true,
    })
}

pub fn plan_glb_viewer(
    value: &str,
    filename: Option<&str>,
) -> Result<GlbViewerPlan, MediaTransformError> {
    let source = inspect_model3d_uri(value)?;
    let metadata = inspect_inline_glb_metadata(value)?;
    let can_open_uri_directly = source.renderable_in_webview;
    Ok(GlbViewerPlan {
        source,
        filename: filename
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        metadata,
        viewer_adapter: "webgl-glb-viewer".to_string(),
        requires_webgl_adapter: true,
        can_open_uri_directly,
        capture_output_mime: "image/png".to_string(),
        requires_capture_adapter: true,
    })
}

fn inspect_video_uri(value: &str) -> Result<VideoUriMetadata, MediaTransformError> {
    let uri = value.trim();
    if uri.is_empty() {
        return Err(MediaTransformError::UnsupportedVideoUri(
            "<empty>".to_string(),
        ));
    }

    if let Some(parts) = data_url_parts(uri) {
        let mime = parts.mime.trim().to_ascii_lowercase();
        if !mime.starts_with("video/") {
            return Err(MediaTransformError::UnsupportedVideoMime(
                parts.mime.to_string(),
            ));
        }
        let byte_length = if parts.is_base64 {
            base64_decoded_len(parts.payload)
        } else {
            parts.payload.len()
        };
        return Ok(VideoUriMetadata {
            uri_kind: MediaUriKind::InlineData,
            mime: Some(mime),
            byte_length: Some(byte_length),
            renderable_in_webview: true,
        });
    }

    if uri.starts_with("blob:") {
        return Ok(VideoUriMetadata {
            uri_kind: MediaUriKind::Blob,
            mime: video_mime_from_uri(uri),
            byte_length: None,
            renderable_in_webview: true,
        });
    }
    if uri.starts_with("http://") || uri.starts_with("https://") {
        return Ok(VideoUriMetadata {
            uri_kind: MediaUriKind::Http,
            mime: video_mime_from_uri(uri),
            byte_length: None,
            renderable_in_webview: true,
        });
    }
    if uri.starts_with("gemed-media://") {
        return Ok(VideoUriMetadata {
            uri_kind: MediaUriKind::ProjectReference,
            mime: video_mime_from_uri(uri),
            byte_length: None,
            renderable_in_webview: false,
        });
    }
    if uri.starts_with('/') {
        return Ok(VideoUriMetadata {
            uri_kind: MediaUriKind::StaticPath,
            mime: video_mime_from_uri(uri),
            byte_length: None,
            renderable_in_webview: true,
        });
    }
    if uri.starts_with("./") || uri.starts_with("../") {
        return Ok(VideoUriMetadata {
            uri_kind: MediaUriKind::RelativePath,
            mime: video_mime_from_uri(uri),
            byte_length: None,
            renderable_in_webview: true,
        });
    }

    Err(MediaTransformError::UnsupportedVideoUri(truncate_middle(
        uri, 72,
    )))
}

fn inspect_model3d_uri(value: &str) -> Result<Model3dUriMetadata, MediaTransformError> {
    let uri = value.trim();
    if uri.is_empty() {
        return Err(MediaTransformError::UnsupportedModel3dUri(
            "<empty>".to_string(),
        ));
    }

    if let Some(parts) = data_url_parts(uri) {
        let mime = parts.mime.trim().to_ascii_lowercase();
        if !is_model3d_mime(&mime) {
            return Err(MediaTransformError::UnsupportedModel3dMime(
                parts.mime.to_string(),
            ));
        }
        let byte_length = if parts.is_base64 {
            base64_decoded_len(parts.payload)
        } else {
            parts.payload.len()
        };
        return Ok(Model3dUriMetadata {
            uri_kind: MediaUriKind::InlineData,
            mime: Some(mime),
            byte_length: Some(byte_length),
            renderable_in_webview: true,
        });
    }

    if uri.starts_with("blob:") {
        return Ok(Model3dUriMetadata {
            uri_kind: MediaUriKind::Blob,
            mime: model3d_mime_from_uri(uri),
            byte_length: None,
            renderable_in_webview: true,
        });
    }
    if uri.starts_with("http://") || uri.starts_with("https://") {
        return Ok(Model3dUriMetadata {
            uri_kind: MediaUriKind::Http,
            mime: model3d_mime_from_uri(uri),
            byte_length: None,
            renderable_in_webview: true,
        });
    }
    if uri.starts_with("gemed-media://") {
        return Ok(Model3dUriMetadata {
            uri_kind: MediaUriKind::ProjectReference,
            mime: model3d_mime_from_uri(uri),
            byte_length: None,
            renderable_in_webview: false,
        });
    }
    if uri.starts_with('/') {
        return Ok(Model3dUriMetadata {
            uri_kind: MediaUriKind::StaticPath,
            mime: model3d_mime_from_uri(uri),
            byte_length: None,
            renderable_in_webview: true,
        });
    }
    if uri.starts_with("./") || uri.starts_with("../") {
        return Ok(Model3dUriMetadata {
            uri_kind: MediaUriKind::RelativePath,
            mime: model3d_mime_from_uri(uri),
            byte_length: None,
            renderable_in_webview: true,
        });
    }

    Err(MediaTransformError::UnsupportedModel3dUri(truncate_middle(
        uri, 72,
    )))
}

fn inspect_inline_glb_metadata(value: &str) -> Result<Option<GlbMetadata>, MediaTransformError> {
    let Some(parts) = data_url_parts(value) else {
        return Ok(None);
    };
    let mime = parts.mime.trim().to_ascii_lowercase();
    if mime != "model/gltf-binary" {
        return Ok(None);
    }
    if !parts.is_base64 {
        return Err(MediaTransformError::InvalidInlineGlb(
            "binary GLB data URLs must be base64 encoded".to_string(),
        ));
    }

    let bytes = decode_base64_data_url_payload(parts.payload)
        .map_err(|err| MediaTransformError::InvalidInlineGlb(err.to_string()))?;
    parse_glb_metadata(&bytes).map(Some)
}

fn decode_base64_data_url_payload(value: &str) -> Result<Vec<u8>, base64::DecodeError> {
    let payload = value
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect::<Vec<_>>();
    general_purpose::STANDARD.decode(payload)
}

fn parse_glb_metadata(bytes: &[u8]) -> Result<GlbMetadata, MediaTransformError> {
    const GLB_HEADER_LEN: usize = 12;
    const GLB_CHUNK_HEADER_LEN: usize = 8;
    const GLB_V2: u32 = 2;

    if bytes.len() < GLB_HEADER_LEN + GLB_CHUNK_HEADER_LEN {
        return Err(invalid_inline_glb("file is shorter than the GLB header"));
    }
    if bytes.get(0..4) != Some(b"glTF") {
        return Err(invalid_inline_glb("missing glTF magic header"));
    }

    let version = read_glb_u32(bytes, 4)?;
    if version != GLB_V2 {
        return Err(invalid_inline_glb(format!(
            "unsupported GLB version {version}; expected 2"
        )));
    }

    let declared_byte_length = read_glb_u32(bytes, 8)? as usize;
    if declared_byte_length != bytes.len() {
        return Err(invalid_inline_glb(format!(
            "declared length {declared_byte_length} does not match decoded length {}",
            bytes.len()
        )));
    }

    let json_chunk_byte_length = read_glb_u32(bytes, GLB_HEADER_LEN)? as usize;
    if json_chunk_byte_length == 0 || !json_chunk_byte_length.is_multiple_of(4) {
        return Err(invalid_inline_glb(
            "JSON chunk length must be a non-zero 4-byte multiple",
        ));
    }
    let chunk_type_offset = GLB_HEADER_LEN + 4;
    if bytes.get(chunk_type_offset..chunk_type_offset + 4) != Some(b"JSON") {
        return Err(invalid_inline_glb("first GLB chunk is not JSON"));
    }

    let json_start = GLB_HEADER_LEN + GLB_CHUNK_HEADER_LEN;
    let json_end = json_start
        .checked_add(json_chunk_byte_length)
        .ok_or_else(|| invalid_inline_glb("JSON chunk length overflows"))?;
    let json_bytes = bytes
        .get(json_start..json_end)
        .ok_or_else(|| invalid_inline_glb("JSON chunk exceeds declared GLB length"))?;
    let json: Value = serde_json::from_slice(json_bytes)
        .map_err(|err| invalid_inline_glb(format!("JSON chunk is invalid: {err}")))?;

    Ok(GlbMetadata {
        version,
        declared_byte_length,
        json_chunk_byte_length,
        asset_version: gltf_asset_string(&json, "version"),
        generator: gltf_asset_string(&json, "generator"),
        scene_count: gltf_array_count(&json, "scenes"),
        node_count: gltf_array_count(&json, "nodes"),
        mesh_count: gltf_array_count(&json, "meshes"),
        material_count: gltf_array_count(&json, "materials"),
        animation_count: gltf_array_count(&json, "animations"),
        image_count: gltf_array_count(&json, "images"),
        buffer_count: gltf_array_count(&json, "buffers"),
    })
}

fn read_glb_u32(bytes: &[u8], offset: usize) -> Result<u32, MediaTransformError> {
    let slice = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| invalid_inline_glb("unexpected end of GLB header"))?;
    Ok(u32::from_le_bytes(
        slice.try_into().expect("slice length checked"),
    ))
}

fn gltf_asset_string(json: &Value, field: &str) -> Option<String> {
    json.get("asset")
        .and_then(Value::as_object)
        .and_then(|asset| asset.get(field))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn gltf_array_count(json: &Value, field: &str) -> usize {
    json.get(field)
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0)
}

fn invalid_inline_glb(message: impl Into<String>) -> MediaTransformError {
    MediaTransformError::InvalidInlineGlb(message.into())
}

struct DecodedInlineImage {
    mime: String,
    bytes: Vec<u8>,
    image: DynamicImage,
}

impl DecodedInlineImage {
    fn metadata(&self) -> InlineImageMetadata {
        InlineImageMetadata {
            mime: self.mime.clone(),
            width: self.image.width(),
            height: self.image.height(),
            byte_length: self.bytes.len(),
        }
    }
}

fn decode_inline_image(value: &str) -> Result<DecodedInlineImage, MediaTransformError> {
    let (mime, bytes) = decode_inline_image_data_url(value)?;
    let image = image::load_from_memory(&bytes)?;
    Ok(DecodedInlineImage { mime, bytes, image })
}

fn decode_inline_image_data_url(value: &str) -> Result<(String, Vec<u8>), MediaTransformError> {
    let uri = value.trim();
    if !uri.starts_with("data:") {
        return Err(MediaTransformError::UnsupportedUri(truncate_middle(
            uri, 72,
        )));
    }
    let parts = data_url_parts(uri).ok_or(MediaTransformError::InvalidDataUrl)?;
    let mime = parts.mime.to_ascii_lowercase();
    if !mime.starts_with("image/") {
        return Err(MediaTransformError::UnsupportedMime(parts.mime.to_string()));
    }
    if !matches!(
        mime.as_str(),
        "image/png" | "image/jpeg" | "image/jpg" | "image/webp"
    ) {
        return Err(MediaTransformError::UnsupportedMime(parts.mime.to_string()));
    }

    let bytes = if parts.is_base64 {
        general_purpose::STANDARD.decode(parts.payload.as_bytes())?
    } else {
        parts.payload.as_bytes().to_vec()
    };
    Ok((mime, bytes))
}

fn encode_image_data_url(bytes: &[u8], source_mime: &str) -> Result<String, MediaTransformError> {
    let image = image::load_from_memory(bytes)?;
    if source_mime == "image/jpeg" || source_mime == "image/jpg" {
        return encode_jpeg_data_url(&image);
    }
    if source_mime == "image/webp" {
        return encode_webp_data_url(&image);
    }
    encode_png_data_url(&image)
}

fn encode_png_data_url(image: &DynamicImage) -> Result<String, MediaTransformError> {
    let mut cursor = std::io::Cursor::new(Vec::new());
    image.write_to(&mut cursor, ImageFormat::Png)?;
    Ok(format!(
        "data:image/png;base64,{}",
        general_purpose::STANDARD.encode(cursor.into_inner())
    ))
}

fn encode_jpeg_data_url(image: &DynamicImage) -> Result<String, MediaTransformError> {
    let mut cursor = std::io::Cursor::new(Vec::new());
    image.write_to(&mut cursor, ImageFormat::Jpeg)?;
    Ok(format!(
        "data:image/jpeg;base64,{}",
        general_purpose::STANDARD.encode(cursor.into_inner())
    ))
}

fn encode_webp_data_url(image: &DynamicImage) -> Result<String, MediaTransformError> {
    let mut cursor = std::io::Cursor::new(Vec::new());
    image.write_to(&mut cursor, ImageFormat::WebP)?;
    Ok(format!(
        "data:image/webp;base64,{}",
        general_purpose::STANDARD.encode(cursor.into_inner())
    ))
}

fn grid_cells(width: u32, height: u32, rows: u32, cols: u32) -> Vec<GridCellRect> {
    let mut cells = Vec::with_capacity(rows.saturating_mul(cols) as usize);
    for row in 0..rows {
        let y0 = row.saturating_mul(height) / rows;
        let y1 = (row + 1).saturating_mul(height) / rows;
        for col in 0..cols {
            let x0 = col.saturating_mul(width) / cols;
            let x1 = (col + 1).saturating_mul(width) / cols;
            cells.push(GridCellRect {
                x: x0,
                y: y0,
                width: x1.saturating_sub(x0),
                height: y1.saturating_sub(y0),
            });
        }
    }
    cells
}

fn crop_image_cell(image: &DynamicImage, cell: GridCellRect) -> DynamicImage {
    image.crop_imm(cell.x, cell.y, cell.width, cell.height)
}

fn image_difference(image_a: &DynamicImage, image_b: &DynamicImage) -> InlineImageDifference {
    let image_a = image_a.to_rgba8();
    let image_b = image_b.to_rgba8();
    let pixel_count = u64::from(image_a.width()) * u64::from(image_a.height());
    let mut total_delta = 0u64;
    let mut changed_pixels = 0u64;
    let mut max_channel_delta = 0u8;

    for (pixel_a, pixel_b) in image_a.pixels().zip(image_b.pixels()) {
        let mut pixel_changed = false;
        for (channel_a, channel_b) in pixel_a.0.iter().zip(pixel_b.0.iter()) {
            let delta = channel_a.abs_diff(*channel_b);
            total_delta += u64::from(delta);
            max_channel_delta = max_channel_delta.max(delta);
            if delta > 0 {
                pixel_changed = true;
            }
        }
        if pixel_changed {
            changed_pixels += 1;
        }
    }

    let channel_count = pixel_count.saturating_mul(4).max(1);
    InlineImageDifference {
        pixel_count,
        changed_pixels,
        changed_pixel_ratio: changed_pixels as f64 / pixel_count.max(1) as f64,
        mean_absolute_error: total_delta as f64 / channel_count as f64,
        max_channel_delta,
    }
}

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
            MediaSupportLevel::Ready,
            MediaSupportLevel::Ready,
            "Local executor preserves annotation pass-through image references; drawing tools still need a canvas adapter.",
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
            MediaSupportLevel::Ready,
            MediaSupportLevel::Ready,
            "Local executor can split inline PNG/JPEG/WebP data URLs into deterministic PNG grid cells; project refs need storage hydration before transform.",
        ),
        NodeType::ImageCompare => profile(
            node_type,
            vec![MediaKind::Image],
            MediaSupportLevel::Ready,
            MediaSupportLevel::Ready,
            "Local executor resolves comparison images and computes inline PNG/JPEG/WebP pixel metrics when both inputs are data URLs; richer slider visualization remains UI work.",
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
            "Local executor now plans frame-grab source/seek metadata; actual video decode and PNG capture still need browser/native adapters.",
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
        field: "sourceVideo",
        ref_field: Some("sourceVideoRef"),
        kind: MediaKind::Video,
        label: "Source video",
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

fn video_mime_from_uri(uri: &str) -> Option<String> {
    let extension = extension_from_uri(uri)?.to_ascii_lowercase();
    let mime = match extension.as_str() {
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        _ => return None,
    };
    Some(mime.to_string())
}

fn model3d_mime_from_uri(uri: &str) -> Option<String> {
    let extension = extension_from_uri(uri)?.to_ascii_lowercase();
    let mime = match extension.as_str() {
        "glb" => "model/gltf-binary",
        "gltf" => "model/gltf+json",
        _ => return None,
    };
    Some(mime.to_string())
}

fn is_model3d_mime(mime: &str) -> bool {
    matches!(
        mime.trim().to_ascii_lowercase().as_str(),
        "model/gltf-binary" | "model/gltf+json" | "application/gltf+json"
    )
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
    fn video_frame_grab_profile_mentions_planning_boundary_without_ready_claim() {
        let profile = media_profile_for_node_type(&NodeType::VideoFrameGrab).unwrap();

        assert_eq!(
            profile.media_kinds,
            vec![MediaKind::Video, MediaKind::Image]
        );
        assert!(profile.needs_adapter());
        assert!(
            profile
                .notes
                .contains("plans frame-grab source/seek metadata")
        );
        assert!(profile.notes.contains("still need browser/native adapters"));
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
    fn video_frame_grab_plan_accepts_inline_video_without_decoding() {
        let plan = plan_video_frame_grab(
            "data:video/mp4;base64,AAAA",
            VideoFramePosition::First,
            None,
        )
        .expect("inline video URI can be planned");

        assert_eq!(plan.source.uri_kind, MediaUriKind::InlineData);
        assert_eq!(plan.source.mime.as_deref(), Some("video/mp4"));
        assert_eq!(plan.source.byte_length, Some(3));
        assert!(plan.source.renderable_in_webview);
        assert_eq!(plan.requested_seek_seconds, Some(0.001));
        assert!(!plan.seek_requires_duration);
        assert_eq!(plan.output_mime, "image/png");
        assert!(plan.requires_decode_adapter);
    }

    #[test]
    fn video_frame_grab_plan_records_last_frame_duration_gap() {
        let unresolved = plan_video_frame_grab(
            "gemed-media://media/clip.webm",
            VideoFramePosition::Last,
            None,
        )
        .expect("project video ref can be planned");
        let resolved = plan_video_frame_grab(
            "https://example.invalid/clip.mp4?download=1",
            VideoFramePosition::Last,
            Some(3.0),
        )
        .expect("http video URI can be planned with duration");

        assert_eq!(unresolved.source.uri_kind, MediaUriKind::ProjectReference);
        assert_eq!(unresolved.source.mime.as_deref(), Some("video/webm"));
        assert!(!unresolved.source.renderable_in_webview);
        assert_eq!(unresolved.requested_seek_seconds, None);
        assert!(unresolved.seek_requires_duration);
        assert_eq!(resolved.source.uri_kind, MediaUriKind::Http);
        assert_eq!(resolved.source.mime.as_deref(), Some("video/mp4"));
        assert_eq!(resolved.requested_seek_seconds, Some(2.9));
        assert!(!resolved.seek_requires_duration);
    }

    #[test]
    fn video_frame_grab_plan_rejects_non_video_data_urls() {
        let err = plan_video_frame_grab(
            "data:image/png;base64,AAAA",
            VideoFramePosition::First,
            None,
        )
        .expect_err("image data URL is not a video frame source");

        assert!(
            err.to_string()
                .contains("unsupported inline video media type")
        );
    }

    #[test]
    fn glb_viewer_plan_accepts_project_refs_without_claiming_render_adapter() {
        let plan = plan_glb_viewer("gemed-media://media/demo-model.glb", Some("demo-model.glb"))
            .expect("project GLB ref can be planned");

        assert_eq!(plan.source.uri_kind, MediaUriKind::ProjectReference);
        assert_eq!(plan.source.mime.as_deref(), Some("model/gltf-binary"));
        assert!(!plan.source.renderable_in_webview);
        assert_eq!(plan.filename.as_deref(), Some("demo-model.glb"));
        assert_eq!(plan.metadata, None);
        assert_eq!(plan.viewer_adapter, "webgl-glb-viewer");
        assert!(plan.requires_webgl_adapter);
        assert!(!plan.can_open_uri_directly);
        assert_eq!(plan.capture_output_mime, "image/png");
        assert!(plan.requires_capture_adapter);
    }

    #[test]
    fn glb_viewer_plan_parses_inline_glb_metadata() {
        let bytes = minimal_glb_bytes();
        let plan =
            plan_glb_viewer(&minimal_glb_data_url(), None).expect("inline GLB can be planned");

        assert_eq!(plan.source.uri_kind, MediaUriKind::InlineData);
        assert_eq!(plan.source.mime.as_deref(), Some("model/gltf-binary"));
        assert_eq!(plan.source.byte_length, Some(bytes.len()));
        assert!(plan.source.renderable_in_webview);
        assert!(plan.can_open_uri_directly);
        assert!(plan.requires_webgl_adapter);
        let metadata = plan.metadata.expect("inline GLB metadata is parsed");
        assert_eq!(metadata.version, 2);
        assert_eq!(metadata.declared_byte_length, bytes.len());
        assert_eq!(metadata.json_chunk_byte_length % 4, 0);
        assert_eq!(metadata.asset_version.as_deref(), Some("2.0"));
        assert_eq!(metadata.generator.as_deref(), Some("GemEd test"));
        assert_eq!(metadata.scene_count, 1);
        assert_eq!(metadata.node_count, 1);
        assert_eq!(metadata.mesh_count, 1);
        assert_eq!(metadata.material_count, 1);
        assert_eq!(metadata.animation_count, 0);
        assert_eq!(metadata.image_count, 0);
        assert_eq!(metadata.buffer_count, 1);
    }

    #[test]
    fn glb_viewer_plan_rejects_malformed_inline_glb() {
        let err = plan_glb_viewer("data:model/gltf-binary;base64,AAAA", None)
            .expect_err("malformed inline GLB should not be planned");

        assert!(err.to_string().contains("invalid inline GLB payload"));
    }

    #[test]
    fn glb_viewer_plan_rejects_non_model_data_urls() {
        let err = plan_glb_viewer("data:image/png;base64,AAAA", None)
            .expect_err("image data URL is not a 3D model");

        assert!(
            err.to_string()
                .contains("unsupported inline 3D model media type")
        );
    }

    fn minimal_glb_data_url() -> String {
        format!(
            "data:model/gltf-binary;base64,{}",
            general_purpose::STANDARD.encode(minimal_glb_bytes())
        )
    }

    fn minimal_glb_bytes() -> Vec<u8> {
        let mut json_chunk = br#"{"asset":{"version":"2.0","generator":"GemEd test"},"scenes":[{}],"nodes":[{}],"meshes":[{}],"materials":[{}],"animations":[],"images":[],"buffers":[{}]}"#.to_vec();
        while !json_chunk.len().is_multiple_of(4) {
            json_chunk.push(b' ');
        }

        let declared_len = 12 + 8 + json_chunk.len();
        let mut bytes = Vec::with_capacity(declared_len);
        bytes.extend_from_slice(b"glTF");
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(&(declared_len as u32).to_le_bytes());
        bytes.extend_from_slice(&(json_chunk.len() as u32).to_le_bytes());
        bytes.extend_from_slice(b"JSON");
        bytes.extend_from_slice(&json_chunk);
        bytes
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
    fn media_preview_reports_video_frame_grab_source_video() {
        let node = WorkflowNode::new(
            "frame",
            NodeType::VideoFrameGrab,
            Position { x: 0.0, y: 0.0 },
            json!({
                "label": "Frame Grab",
                "sourceVideo": "data:video/mp4;base64,AAAA",
                "outputImage": null
            }),
        );

        let previews = media_previews_for_node(&node);

        assert_eq!(previews.len(), 1);
        assert_eq!(previews[0].kind, MediaKind::Video);
        assert_eq!(previews[0].label, "Frame Grab");
        assert_eq!(previews[0].source_field, "sourceVideo");
        assert_eq!(previews[0].inline_mime().as_deref(), Some("video/mp4"));
        assert_eq!(previews[0].download_filename(), "frame-grab.mp4");
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

    #[test]
    fn inline_image_passthrough_validates_and_reencodes_png() {
        let tiny = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";

        let output = passthrough_inline_image_data_url(tiny).expect("png re-encodes");

        assert!(output.starts_with("data:image/png;base64,"));
        assert!(output.len() > "data:image/png;base64,".len());
    }

    #[test]
    fn inspect_inline_image_reports_dimensions_and_payload_size() {
        let tiny = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";

        let metadata = inspect_inline_image(tiny).expect("png metadata decodes");

        assert_eq!(metadata.mime, "image/png");
        assert_eq!(metadata.width, 1);
        assert_eq!(metadata.height, 1);
        assert!(metadata.byte_length > 0);
    }

    #[test]
    fn compare_inline_images_reports_exact_and_changed_pixels() {
        let red = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";
        let blue = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNgYPj/HwADAgH/5ncLrgAAAABJRU5ErkJggg==";

        let exact = compare_inline_images(red, red).expect("exact image comparison");
        let changed = compare_inline_images(red, blue).expect("changed image comparison");

        assert!(exact.dimensions_match);
        assert!(exact.exact_match);
        assert_eq!(exact.difference.unwrap().changed_pixels, 0);

        assert!(changed.dimensions_match);
        assert!(!changed.exact_match);
        let difference = changed
            .difference
            .expect("matching dimensions have metrics");
        assert_eq!(difference.pixel_count, 1);
        assert_eq!(difference.changed_pixels, 1);
        assert!(difference.mean_absolute_error > 0.0);
        assert!(difference.max_channel_delta > 0);
    }

    #[test]
    fn split_inline_image_grid_returns_cells_and_data_urls() {
        let tiny = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAFElEQVR4nGP4z8DwHwyBNBAw/AcAR8oI+ItOQ4UAAAAASUVORK5CYII=";

        let result = split_inline_image_grid(tiny, 2, 2, Some(3)).expect("grid splits");

        assert_eq!(result.rows, 2);
        assert_eq!(result.cols, 2);
        assert_eq!(result.cells.len(), 4);
        assert_eq!(result.images.len(), 3);
        assert!(
            result
                .images
                .iter()
                .all(|image| image.starts_with("data:image/png;base64,"))
        );
        assert_eq!(
            result.cells[0],
            GridCellRect {
                x: 0,
                y: 0,
                width: 1,
                height: 1
            }
        );
    }

    #[test]
    fn split_inline_image_grid_rejects_project_refs_until_storage_adapter_hydrates() {
        let err = split_inline_image_grid("gemed-media://media/image.png", 2, 2, None)
            .expect_err("project refs are not directly transformable");

        assert!(err.to_string().contains("only inline image data URLs"));
    }
}
