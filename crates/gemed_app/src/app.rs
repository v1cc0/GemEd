use dioxus::html::{
    InteractionElementOffset, InteractionLocation, ModifiersInteraction, MouseEvent,
    PointerInteraction, WheelEvent, geometry::WheelDelta, input_data::MouseButton,
};
use dioxus::prelude::*;
use gemed_core::{
    GroupColor, NodeGroup, NodeStatus, NodeType, Position, Size, WorkflowEdge, WorkflowFile,
    WorkflowNode, WorkflowUndoStack, add_edge_between, create_group_for_nodes,
    generate_split_grid_children, is_node_in_locked_group, move_group_by, move_node_by,
    remove_edge, resize_group_by, select_node, select_split_grid_child_set, selected_node_id,
    selected_node_ids, set_group_size, set_node_position, source_handle_options,
    split_grid_child_sets, target_handle_options, toggle_group_lock, toggle_node_selection,
};
use gemed_executor::{
    ExecutionControl, SimpleExecutionReport,
    execute_simple_workflow_with_control_and_progress_async,
    execute_workflow_with_providers_with_control_and_progress_async, execution_order,
};
use gemed_media::{MediaKind, MediaPreview, media_previews_for_node, workflow_media_summary};
#[cfg(all(feature = "desktop", feature = "providers-http"))]
use gemed_providers::{
    AnthropicMessagesProvider, GeminiGenerateContentProvider, OpenAiResponsesProvider,
};
use gemed_providers::{
    ProviderCapability, ProviderConfig, ProviderConfigSet, ProviderId, ProviderRegistry,
    ProviderRuntimeMode, ProviderSecretSource,
};
use gemed_storage::{
    DEFAULT_AUTOSAVE_SLOT, DEFAULT_PROVIDER_CONFIG_SLOT, ProviderConfigSnapshot,
    ProviderConfigStorage, WorkflowSnapshot, WorkflowStorage,
};
use serde_json::Value;

const CANVAS_WIDTH: f64 = 1400.0;
const CANVAS_HEIGHT: f64 = 900.0;
const NODE_CARD_WIDTH: f64 = 248.0;
const NODE_CARD_HEIGHT: f64 = 128.0;
const GROUP_SELECTION_MIN_SIZE: f64 = 18.0;
const MODEL_VIEWER_LOCAL_MODULE_URL: &str = "/vendor/model-viewer/4.3.1/model-viewer.min.js";
const MODEL_VIEWER_CDN_MODULE_URL: &str =
    "https://unpkg.com/@google/model-viewer@4.3.1/dist/model-viewer.min.js";

const APP_CSS: &str = r#"
:root {
  color-scheme: dark;
  font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  background: #0b1020;
  color: #e5ecff;
}
* { box-sizing: border-box; }
body { margin: 0; min-height: 100vh; background: radial-gradient(circle at top left, #1f2a44 0, #0b1020 36rem); }
button, textarea, input { font: inherit; }
.app { min-height: 100vh; display: flex; flex-direction: column; }
.header { min-height: 4.5rem; display: flex; align-items: center; justify-content: space-between; gap: 1rem; padding: .75rem 1.5rem; border-bottom: 1px solid rgba(148, 163, 184, .18); background: rgba(11, 16, 32, .78); backdrop-filter: blur(14px); }
.brand { display: flex; align-items: baseline; gap: .75rem; }
.brand h1 { margin: 0; font-size: 1.45rem; letter-spacing: -.03em; }
.brand span { color: #93a4c8; font-size: .9rem; }
.actions { display: flex; gap: .65rem; align-items: center; justify-content: flex-end; flex-wrap: wrap; }
.action { border: 1px solid rgba(148, 163, 184, .28); background: rgba(15, 23, 42, .82); color: #dce6ff; border-radius: .75rem; padding: .58rem .82rem; cursor: pointer; }
.action.primary { border-color: rgba(96, 165, 250, .65); background: linear-gradient(135deg, #2563eb, #7c3aed); color: white; }
.action:disabled { opacity: .55; cursor: not-allowed; }
.main { flex: 1; display: grid; grid-template-columns: minmax(20rem, 25rem) minmax(0, 1fr); min-height: 0; }
.sidebar { border-right: 1px solid rgba(148, 163, 184, .16); background: rgba(15, 23, 42, .52); padding: 1rem; overflow: auto; }
.panel { border: 1px solid rgba(148, 163, 184, .18); background: rgba(15, 23, 42, .74); border-radius: 1rem; padding: 1rem; margin-bottom: 1rem; box-shadow: 0 20px 48px rgba(0, 0, 0, .24); }
.panel h2 { margin: 0 0 .7rem; font-size: 1rem; }
.panel p { margin: .45rem 0; color: #adbbd8; line-height: 1.45; }
.stats { display: grid; grid-template-columns: repeat(3, 1fr); gap: .5rem; }
.stat { border-radius: .85rem; padding: .75rem; background: rgba(30, 41, 59, .78); border: 1px solid rgba(148, 163, 184, .12); }
.stat strong { display: block; font-size: 1.25rem; }
.stat span { color: #91a3c5; font-size: .78rem; }
textarea.workflow-json { width: 100%; min-height: 13rem; resize: vertical; border: 1px solid rgba(148, 163, 184, .22); border-radius: .85rem; padding: .75rem; color: #e5ecff; background: rgba(2, 6, 23, .72); outline: none; }
textarea.workflow-json:focus { border-color: rgba(96, 165, 250, .65); box-shadow: 0 0 0 3px rgba(37, 99, 235, .18); }
.message { border-radius: .85rem; padding: .72rem .8rem; margin-top: .65rem; line-height: 1.35; }
.message.ok { color: #bbf7d0; background: rgba(22, 101, 52, .28); border: 1px solid rgba(74, 222, 128, .2); }
.message.err { color: #fecaca; background: rgba(127, 29, 29, .35); border: 1px solid rgba(248, 113, 113, .22); }
.canvas-wrap { position: relative; overflow: auto; background-image: linear-gradient(rgba(148,163,184,.055) 1px, transparent 1px), linear-gradient(90deg, rgba(148,163,184,.055) 1px, transparent 1px); background-size: 32px 32px; }
.canvas-wrap.panning { cursor: grabbing; }
.canvas { position: relative; width: 1400px; height: 900px; margin: 1.25rem; cursor: grab; user-select: none; }
.canvas.panning { cursor: grabbing; }
.canvas.selecting { cursor: crosshair; }
.selection-box { position: absolute; z-index: 4; pointer-events: none; border: 1.5px dashed rgba(186, 230, 253, .94); background: rgba(14, 165, 233, .16); border-radius: .45rem; box-shadow: 0 0 0 1px rgba(14, 165, 233, .12), inset 0 0 28px rgba(56, 189, 248, .12); }
.group-box { position: absolute; border-radius: 1rem; pointer-events: none; z-index: 0; box-shadow: inset 0 0 0 1px rgba(255,255,255,.08); }
.group-box.resizing { box-shadow: inset 0 0 0 1px rgba(255,255,255,.2), 0 0 0 3px rgba(125, 211, 252, .2); }
.group-box.locked { box-shadow: inset 0 0 0 1px rgba(255,255,255,.14), 0 0 0 2px rgba(250, 204, 21, .14); }
.group-box.nbp { border-style: dashed; }
.group-header { position: absolute; left: .65rem; top: -.95rem; display: inline-flex; align-items: center; gap: .35rem; padding: .22rem .5rem; border-radius: .55rem; color: white; font-size: .72rem; font-weight: 750; pointer-events: auto; box-shadow: 0 10px 24px rgba(0, 0, 0, .28); }
.group-header.movable { cursor: grab; }
.group-header.moving { cursor: grabbing; filter: brightness(1.1); }
.group-box.locked .group-header { cursor: not-allowed; }
.group-lock-toggle { border: 1px solid rgba(255,255,255,.18); border-radius: .45rem; padding: .08rem .35rem; background: rgba(15, 23, 42, .36); color: white; cursor: pointer; font-size: .68rem; }
.group-lock-toggle:hover { background: rgba(15, 23, 42, .62); }
.group-resize-handle { position: absolute; right: -.45rem; bottom: -.45rem; width: 1rem; height: 1rem; border-radius: .35rem; border: 2px solid rgba(224, 242, 254, .9); background: rgba(14, 165, 233, .8); box-shadow: 0 8px 18px rgba(0, 0, 0, .32); cursor: nwse-resize; pointer-events: auto; }
.group-resize-handle:hover { transform: scale(1.08); background: rgba(56, 189, 248, .95); }
.group-box.locked .group-resize-handle { cursor: not-allowed; opacity: .45; background: rgba(113, 63, 18, .85); border-color: rgba(250, 204, 21, .85); }
.edge-layer { position: absolute; inset: 0; width: 1400px; height: 900px; pointer-events: none; overflow: visible; z-index: 1; }
.edge-group { pointer-events: none; }
.edge-hit { stroke: transparent; stroke-width: 14; fill: none; pointer-events: stroke; cursor: pointer; }
.edge { stroke: rgba(125, 211, 252, .64); stroke-width: 2.5; fill: none; marker-end: url(#arrow); pointer-events: none; }
.edge-group:hover .edge { stroke: rgba(186, 230, 253, .95); stroke-width: 3; }
.edge-action { pointer-events: auto; cursor: pointer; opacity: .42; transition: opacity .12s ease, transform .12s ease; }
.edge-group:hover .edge-action { opacity: 1; }
.edge-action:hover { transform: scale(1.08); }
.edge-delete-dot { fill: rgba(127, 29, 29, .9); stroke: rgba(248, 113, 113, .68); stroke-width: 1.5; filter: drop-shadow(0 7px 14px rgba(0, 0, 0, .35)); }
.edge-delete-label { fill: #fecaca; font-size: 14px; font-weight: 800; pointer-events: none; user-select: none; }
.node { position: absolute; z-index: 2; width: 15.5rem; min-height: 8rem; border-radius: 1rem; border: 1px solid rgba(148, 163, 184, .24); background: linear-gradient(145deg, rgba(30, 41, 59, .96), rgba(15, 23, 42, .96)); box-shadow: 0 22px 60px rgba(0, 0, 0, .34); overflow: visible; }
.node.draggable { cursor: grab; user-select: none; }
.node.dragging { cursor: grabbing; opacity: .92; }
.node.locked { cursor: not-allowed; opacity: .86; }
.node.locked::after { content: "LOCKED"; position: absolute; right: .65rem; bottom: .55rem; border-radius: 999px; padding: .12rem .42rem; color: #fde68a; background: rgba(113, 63, 18, .58); border: 1px solid rgba(250, 204, 21, .22); font-size: .62rem; font-weight: 800; letter-spacing: .04em; }
.node.input { border-color: rgba(52, 211, 153, .38); }
.node.generation { border-color: rgba(168, 85, 247, .48); }
.node.processing { border-color: rgba(251, 191, 36, .38); }
.node.control { border-color: rgba(96, 165, 250, .45); }
.node.output { border-color: rgba(244, 114, 182, .45); }
.node.selected { outline: 3px solid rgba(96, 165, 250, .72); box-shadow: 0 0 0 6px rgba(37, 99, 235, .18), 0 22px 60px rgba(0, 0, 0, .34); }
.node-head { padding: .75rem .85rem; display: flex; align-items: center; justify-content: space-between; gap: .5rem; border-bottom: 1px solid rgba(148, 163, 184, .12); }
.node-title { font-weight: 700; font-size: .95rem; line-height: 1.2; }
.badge { white-space: nowrap; border-radius: 999px; padding: .18rem .5rem; font-size: .7rem; border: 1px solid rgba(148, 163, 184, .24); color: #b6c5e2; background: rgba(15, 23, 42, .8); }
.badge.complete { color: #bbf7d0; border-color: rgba(74, 222, 128, .28); }
.badge.error { color: #fecaca; border-color: rgba(248, 113, 113, .28); }
.badge.loading { color: #bfdbfe; border-color: rgba(96, 165, 250, .32); }
.node-body { padding: .8rem .85rem; color: #b9c6df; font-size: .82rem; line-height: 1.4; }
.node-insight { margin-top: .55rem; border: 1px solid rgba(148, 163, 184, .14); border-radius: .75rem; padding: .48rem .55rem; background: rgba(2, 6, 23, .38); }
.node-insight.ready { border-color: rgba(74, 222, 128, .22); background: rgba(22, 101, 52, .14); }
.node-insight.adapter { border-color: rgba(250, 204, 21, .26); background: rgba(113, 63, 18, .16); }
.node-insight.warn { border-color: rgba(248, 113, 113, .22); background: rgba(127, 29, 29, .16); }
.node-insight-title { color: #e5ecff; font-weight: 800; font-size: .72rem; margin-bottom: .25rem; letter-spacing: .01em; }
.node-insight p { margin: .16rem 0; color: #adbbd8; font-size: .7rem; line-height: 1.32; overflow-wrap: anywhere; }
.media-preview-list { display: flex; flex-direction: column; gap: .45rem; margin-top: .65rem; }
.media-preview { border: 1px solid rgba(148, 163, 184, .14); border-radius: .75rem; overflow: hidden; background: rgba(2, 6, 23, .42); }
.media-preview-head { display: flex; align-items: center; justify-content: space-between; gap: .4rem; padding: .36rem .48rem; color: #dbeafe; font-size: .68rem; font-weight: 750; }
.media-preview-kind { border-radius: 999px; padding: .1rem .38rem; border: 1px solid rgba(148, 163, 184, .2); color: #b6c5e2; background: rgba(15, 23, 42, .76); text-transform: uppercase; letter-spacing: .04em; }
.media-preview-kind.image { color: #bbf7d0; border-color: rgba(74, 222, 128, .24); }
.media-preview-kind.audio { color: #bfdbfe; border-color: rgba(96, 165, 250, .28); }
.media-preview-kind.video { color: #fbcfe8; border-color: rgba(244, 114, 182, .28); }
.media-preview-kind.model3d { color: #ddd6fe; border-color: rgba(196, 181, 253, .28); }
.media-preview img, .media-preview video { display: block; width: 100%; max-height: 8rem; object-fit: cover; background: #020617; }
.media-preview audio { display: block; width: 100%; height: 2.2rem; padding: 0 .35rem .35rem; }
.media-preview-model { display: block; width: 100%; min-height: 9rem; height: 9rem; border: 0; background: #020617; }
.media-preview-placeholder { min-height: 3.2rem; display: grid; place-items: center; padding: .7rem; color: #9fb0cf; text-align: center; font-size: .72rem; overflow-wrap: anywhere; background: repeating-linear-gradient(135deg, rgba(148, 163, 184, .06) 0, rgba(148, 163, 184, .06) 8px, transparent 8px, transparent 16px); }
.media-preview-error { margin: .42rem .48rem 0; border-radius: .55rem; padding: .42rem .5rem; color: #fecaca; background: rgba(127, 29, 29, .32); border: 1px solid rgba(248, 113, 113, .22); font-size: .68rem; line-height: 1.3; }
.media-copy-status { margin: .36rem .48rem 0; color: #bbf7d0; font-size: .66rem; line-height: 1.25; overflow-wrap: anywhere; }
.media-copy-status.err { color: #fecaca; }
.media-preview-hint { margin: 0; padding: .34rem .48rem .46rem; color: #8ea1c2; font-size: .68rem; overflow-wrap: anywhere; }
.media-preview-actions { display: flex; gap: .35rem; padding: .42rem .48rem 0; }
.media-preview-link { border: 1px solid rgba(125, 211, 252, .28); color: #dbeafe; background: rgba(14, 165, 233, .12); border-radius: .48rem; padding: .18rem .38rem; font-size: .68rem; text-decoration: none; cursor: pointer; }
.media-preview-link:hover { background: rgba(14, 165, 233, .22); }
.media-overlay-backdrop { position: fixed; inset: 0; z-index: 40; display: grid; place-items: center; padding: 2rem; background: rgba(2, 6, 23, .82); backdrop-filter: blur(12px); }
.media-overlay-panel { width: min(72rem, 94vw); max-height: 92vh; display: grid; grid-template-rows: auto minmax(0, 1fr) auto; border: 1px solid rgba(148, 163, 184, .22); border-radius: 1.1rem; background: rgba(15, 23, 42, .96); box-shadow: 0 36px 90px rgba(0, 0, 0, .55); overflow: hidden; }
.media-overlay-head { display: flex; align-items: center; justify-content: space-between; gap: 1rem; padding: .8rem 1rem; border-bottom: 1px solid rgba(148, 163, 184, .14); }
.media-overlay-title { display: flex; align-items: center; gap: .55rem; min-width: 0; font-weight: 800; color: #e5ecff; }
.media-overlay-title span:first-child { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.media-overlay-body { min-height: 0; display: grid; place-items: center; padding: 1rem; background: #020617; overflow: auto; }
.media-overlay-image { max-width: 100%; max-height: 70vh; object-fit: contain; border-radius: .6rem; box-shadow: 0 18px 56px rgba(0, 0, 0, .38); }
.media-overlay-video { display: block; width: min(100%, 64rem); max-height: 70vh; border-radius: .6rem; background: #020617; box-shadow: 0 18px 56px rgba(0, 0, 0, .38); }
.media-overlay-model { display: block; width: min(100%, 64rem); height: min(70vh, 42rem); min-height: 24rem; border: 0; border-radius: .6rem; background: #020617; box-shadow: 0 18px 56px rgba(0, 0, 0, .38); }
.media-overlay-audio-shell { width: min(42rem, 100%); display: grid; gap: .8rem; justify-items: stretch; padding: 1.1rem; border-radius: .9rem; border: 1px solid rgba(148, 163, 184, .18); background: rgba(15, 23, 42, .72); }
.media-overlay-audio-shell p { margin: 0; color: #bfdbfe; font-size: .84rem; text-align: center; }
.media-overlay-audio { width: 100%; }
.media-overlay-placeholder { min-height: 10rem; display: grid; place-items: center; padding: 1rem; color: #9fb0cf; text-align: center; }
.media-overlay-error { width: min(42rem, 100%); border-radius: .75rem; padding: .72rem .85rem; color: #fecaca; background: rgba(127, 29, 29, .34); border: 1px solid rgba(248, 113, 113, .24); font-size: .82rem; line-height: 1.35; text-align: center; }
.media-overlay-meta { padding: .65rem 1rem .9rem; color: #9fb0cf; font-size: .75rem; overflow-wrap: anywhere; }
.media-overlay-copy-status { padding: 0 1rem .75rem; color: #bbf7d0; font-size: .75rem; overflow-wrap: anywhere; }
.media-overlay-copy-status.err { color: #fecaca; }
.media-overlay-actions { display: flex; gap: .45rem; flex-wrap: wrap; justify-content: flex-end; }
.node-id { color: #64748b; font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: .72rem; margin-top: .65rem; overflow-wrap: anywhere; }
.handle-column { position: absolute; top: 3.25rem; bottom: .75rem; display: flex; flex-direction: column; justify-content: center; gap: .28rem; z-index: 4; pointer-events: auto; }
.handle-column.target { left: -.55rem; align-items: flex-start; }
.handle-column.source { right: -.55rem; align-items: flex-end; }
.handle-button { border: none; background: transparent; color: #dbeafe; cursor: crosshair; display: flex; align-items: center; gap: .28rem; padding: .02rem; max-width: 8rem; }
.handle-button.source { flex-direction: row-reverse; }
.handle-dot { width: .72rem; height: .72rem; flex: 0 0 .72rem; border-radius: 999px; border: 2px solid rgba(125, 211, 252, .78); background: #0f172a; box-shadow: 0 0 0 3px rgba(14, 165, 233, .12); }
.handle-button.target .handle-dot { border-color: rgba(96, 165, 250, .8); }
.handle-button.source .handle-dot { border-color: rgba(45, 212, 191, .8); }
.handle-button.pending .handle-dot { border-color: rgba(250, 204, 21, .95); box-shadow: 0 0 0 5px rgba(250, 204, 21, .2); }
.handle-button.ready .handle-dot { border-color: rgba(74, 222, 128, .95); box-shadow: 0 0 0 5px rgba(74, 222, 128, .18); }
.handle-button:hover .handle-dot { transform: scale(1.12); }
.handle-label { pointer-events: none; opacity: 0; white-space: nowrap; border-radius: 999px; padding: .14rem .38rem; color: #dbeafe; background: rgba(15, 23, 42, .94); border: 1px solid rgba(148, 163, 184, .22); font-size: .64rem; line-height: 1.1; box-shadow: 0 8px 20px rgba(0, 0, 0, .24); transition: opacity .12s ease; }
.handle-button:hover .handle-label, .handle-button.pending .handle-label, .handle-button.ready .handle-label { opacity: 1; }
.type-list { display: flex; flex-direction: column; gap: .35rem; }
.type-row { display: flex; justify-content: space-between; gap: .75rem; color: #b9c6df; font-size: .86rem; }
.type-row code { color: #dbeafe; }
.execution-log { display: flex; flex-direction: column; gap: .45rem; max-height: 14rem; overflow: auto; }
.event { border-radius: .75rem; padding: .55rem .65rem; background: rgba(30, 41, 59, .72); border: 1px solid rgba(148, 163, 184, .12); }
.event-head { display: flex; justify-content: space-between; gap: .6rem; align-items: center; margin-bottom: .25rem; }
.event-title { color: #dbeafe; font-weight: 650; font-size: .82rem; }
.event-message { color: #9fb0cf; font-size: .76rem; line-height: 1.35; }
.edit-grid { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: .45rem; }
.edit-grid .action { padding: .48rem .4rem; }
.handle-actions { display: grid; grid-template-columns: 1fr 1fr; gap: .45rem; margin-top: .5rem; }
.handle-hint { border: 1px dashed rgba(125, 211, 252, .28); border-radius: .75rem; padding: .55rem .65rem; color: #b9c6df; background: rgba(14, 165, 233, .08); font-size: .78rem; line-height: 1.35; }
.viewport-status { color: #9fb0cf; font-size: .78rem; margin-top: .55rem; }
.edge-list { display: flex; flex-direction: column; gap: .4rem; max-height: 9rem; overflow: auto; }
.edge-row { display: flex; align-items: center; justify-content: space-between; gap: .5rem; border-radius: .7rem; background: rgba(30, 41, 59, .72); border: 1px solid rgba(148, 163, 184, .12); padding: .45rem .55rem; }
.edge-row code { color: #dbeafe; font-size: .72rem; overflow-wrap: anywhere; }
.mini-action { border: 1px solid rgba(248, 113, 113, .28); color: #fecaca; background: rgba(127, 29, 29, .28); border-radius: .55rem; padding: .24rem .45rem; cursor: pointer; font-size: .72rem; }
.mini-action.neutral { border-color: rgba(125, 211, 252, .28); color: #dbeafe; background: rgba(14, 165, 233, .12); }
.mini-action.warn { border-color: rgba(250, 204, 21, .34); color: #fde68a; background: rgba(113, 63, 18, .28); }
.provider-list { display: flex; flex-direction: column; gap: .55rem; max-height: 18rem; overflow: auto; }
.provider-row { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: .55rem; align-items: start; border: 1px solid rgba(148, 163, 184, .14); background: rgba(30, 41, 59, .66); border-radius: .8rem; padding: .58rem .65rem; }
.provider-name { display: flex; align-items: center; gap: .4rem; color: #dbeafe; font-weight: 700; font-size: .82rem; }
.provider-meta { margin-top: .24rem; color: #98a9c7; font-size: .72rem; line-height: 1.35; overflow-wrap: anywhere; }
.provider-secret { margin-top: .2rem; color: #c4b5fd; font-size: .7rem; line-height: 1.35; overflow-wrap: anywhere; }
.provider-status { border-radius: 999px; padding: .12rem .42rem; border: 1px solid rgba(148, 163, 184, .2); color: #b6c5e2; background: rgba(15, 23, 42, .78); font-size: .64rem; font-weight: 800; text-transform: uppercase; letter-spacing: .04em; }
.provider-status.available { color: #bbf7d0; border-color: rgba(74, 222, 128, .26); background: rgba(22, 101, 52, .26); }
.provider-status.missing { color: #fde68a; border-color: rgba(250, 204, 21, .28); background: rgba(113, 63, 18, .28); }
.provider-status.disabled { color: #cbd5e1; border-color: rgba(148, 163, 184, .2); background: rgba(51, 65, 85, .36); }
.provider-actions { display: flex; gap: .28rem; flex-wrap: wrap; justify-content: flex-end; }
.provider-edit { grid-column: 1 / -1; display: grid; grid-template-columns: 1fr 1fr; gap: .45rem; margin-top: .48rem; }
.provider-edit label { display: grid; gap: .2rem; color: #98a9c7; font-size: .68rem; font-weight: 700; }
.provider-input { width: 100%; border: 1px solid rgba(148, 163, 184, .18); border-radius: .55rem; padding: .36rem .45rem; color: #e5ecff; background: rgba(2, 6, 23, .54); outline: none; font-size: .72rem; }
.provider-input:focus { border-color: rgba(125, 211, 252, .5); box-shadow: 0 0 0 2px rgba(14, 165, 233, .14); }
.provider-input::placeholder { color: #64748b; }
.secret-guide { border: 1px dashed rgba(196, 181, 253, .34); border-radius: .75rem; padding: .58rem .65rem; color: #ddd6fe; background: rgba(109, 40, 217, .12); font-size: .76rem; line-height: 1.4; }
.empty { height: 100%; display: grid; place-items: center; color: #93a4c8; text-align: center; padding: 2rem; }
@media (max-width: 900px) { .main { grid-template-columns: 1fr; } .sidebar { border-right: none; border-bottom: 1px solid rgba(148,163,184,.16); } .header { align-items: flex-start; height: auto; flex-direction: column; gap: .9rem; padding: 1rem; } .actions { flex-wrap: wrap; } }
"#;

#[component]
pub fn App() -> Element {
    let sample = use_memo(WorkflowFile::example);
    let initial_json = sample
        .read()
        .to_pretty_json()
        .unwrap_or_else(|err| format!("{{\n  \"error\": \"{err}\"\n}}"));
    let workflow = use_signal(|| sample.read().clone());
    let json_text = use_signal(|| initial_json);
    let message = use_signal(|| Message::ok("Loaded built-in starter workflow."));
    let execution_report = use_signal(|| None::<SimpleExecutionReport>);
    let undo_stack = use_signal(WorkflowUndoStack::default);
    let drag_state = use_signal(|| None::<DragState>);
    let pan_state = use_signal(|| None::<PanState>);
    let group_resize_state = use_signal(|| None::<GroupResizeState>);
    let group_move_state = use_signal(|| None::<GroupMoveState>);
    let group_selection_state = use_signal(|| None::<GroupSelectionState>);
    let viewport = use_signal(CanvasViewport::default);
    let connection_draft = use_signal(|| None::<ConnectionDraft>);
    let provider_config = use_signal(initial_provider_config);
    let media_overlay = use_signal(|| None::<MediaOverlay>);
    let active_execution = use_signal(|| None::<ExecutionControl>);

    rsx! {
        style { "{APP_CSS}" }
        div { class: "app",
            Header { workflow, json_text, message, execution_report, undo_stack, drag_state, connection_draft, provider_config, active_execution }
            main { class: "main",
                Sidebar { workflow, json_text, message, execution_report, undo_stack, viewport, connection_draft, provider_config, active_execution }
                WorkflowCanvas { workflow, json_text, message, undo_stack, drag_state, pan_state, group_resize_state, group_move_state, group_selection_state, viewport, connection_draft, media_overlay }
            }
            MediaOverlayLayer { media_overlay }
        }
    }
}

#[component]
fn Header(
    mut workflow: Signal<WorkflowFile>,
    mut json_text: Signal<String>,
    mut message: Signal<Message>,
    mut execution_report: Signal<Option<SimpleExecutionReport>>,
    mut undo_stack: Signal<WorkflowUndoStack>,
    mut drag_state: Signal<Option<DragState>>,
    mut connection_draft: Signal<Option<ConnectionDraft>>,
    provider_config: Signal<ProviderConfigSet>,
    mut active_execution: Signal<Option<ExecutionControl>>,
) -> Element {
    let is_execution_active = active_execution.read().is_some();
    rsx! {
        header { class: "header",
            div { class: "brand",
                h1 { "GemEd" }
                span { "Rust Dioxus workflow spine" }
            }
            div { class: "actions",
                button {
                    class: "action",
                    onclick: move |_| {
                        let next = WorkflowFile::blank();
                        match next.to_pretty_json() {
                            Ok(json) => {
                                workflow.set(next);
                                json_text.set(json);
                                execution_report.set(None);
                                undo_stack.write().clear();
                                drag_state.set(None);
                                connection_draft.set(None);
                                message.set(Message::ok("Started a blank workflow."));
                            }
                            Err(err) => message.set(Message::err(format!("Failed to serialize blank workflow: {err}"))),
                        }
                    },
                    "Blank"
                }
                button {
                    class: "action",
                    onclick: move |_| {
                        let next = WorkflowFile::example();
                        match next.to_pretty_json() {
                            Ok(json) => {
                                workflow.set(next);
                                json_text.set(json);
                                execution_report.set(None);
                                undo_stack.write().clear();
                                drag_state.set(None);
                                connection_draft.set(None);
                                message.set(Message::ok("Reset to built-in starter workflow."));
                            }
                            Err(err) => message.set(Message::err(format!("Failed to serialize sample workflow: {err}"))),
                        }
                    },
                    "Sample"
                }
                button {
                    class: "action",
                    onclick: move |_| {
                        let next = WorkflowFile::media_preview_example();
                        match next.to_pretty_json() {
                            Ok(json) => {
                                workflow.set(next);
                                json_text.set(json);
                                execution_report.set(None);
                                undo_stack.write().clear();
                                drag_state.set(None);
                                connection_draft.set(None);
                                message.set(Message::ok("Loaded built-in media preview sample."));
                            }
                            Err(err) => message.set(Message::err(format!("Failed to serialize media sample workflow: {err}"))),
                        }
                    },
                    "Media Sample"
                }
                button {
                    class: "action",
                    onclick: move |_| {
                        let next = WorkflowFile::media_transform_example();
                        match next.to_pretty_json() {
                            Ok(json) => {
                                workflow.set(next);
                                json_text.set(json);
                                execution_report.set(None);
                                undo_stack.write().clear();
                                drag_state.set(None);
                                connection_draft.set(None);
                                message.set(Message::ok("Loaded built-in media transform sample. Run Local to split the inline image grid."));
                            }
                            Err(err) => message.set(Message::err(format!("Failed to serialize media transform workflow: {err}"))),
                        }
                    },
                    "Transform Sample"
                }
                button {
                    class: "action",
                    onclick: move |_| {
                        let next = WorkflowFile::video_frame_grab_example();
                        match next.to_pretty_json() {
                            Ok(json) => {
                                workflow.set(next);
                                json_text.set(json);
                                execution_report.set(None);
                                undo_stack.write().clear();
                                drag_state.set(None);
                                connection_draft.set(None);
                                message.set(Message::ok("Loaded built-in video frame-grab sample. Run Local to plan source/seek metadata; true PNG capture still needs a platform decode adapter."));
                            }
                            Err(err) => message.set(Message::err(format!("Failed to serialize video frame sample workflow: {err}"))),
                        }
                    },
                    "Frame Sample"
                }
                button {
                    class: "action",
                    onclick: move |_| {
                        let next = WorkflowFile::llm_provider_example();
                        match next.to_pretty_json() {
                            Ok(json) => {
                                workflow.set(next);
                                json_text.set(json);
                                execution_report.set(None);
                                undo_stack.write().clear();
                                drag_state.set(None);
                                connection_draft.set(None);
                                message.set(Message::ok("Loaded built-in LLM provider sample. Use Mock Defaults + Run Providers for offline coverage, or Env + providers-http for opt-in live desktop calls."));
                            }
                            Err(err) => message.set(Message::err(format!("Failed to serialize provider sample workflow: {err}"))),
                        }
                    },
                    "Provider Sample"
                }
                button {
                    class: "action",
                    onclick: move |_| {
                        let next = WorkflowFile::multimodal_provider_example();
                        match next.to_pretty_json() {
                            Ok(json) => {
                                workflow.set(next);
                                json_text.set(json);
                                execution_report.set(None);
                                undo_stack.write().clear();
                                drag_state.set(None);
                                connection_draft.set(None);
                                message.set(Message::ok("Loaded built-in multimodal provider sample. Use Run Providers with mock defaults to exercise image/video/audio/3D provider traits offline."));
                            }
                            Err(err) => message.set(Message::err(format!("Failed to serialize multimodal provider sample workflow: {err}"))),
                        }
                    },
                    "Provider Media"
                }
                button {
                    class: "action primary",
                    onclick: move |_| match WorkflowFile::from_json_str(&json_text.read()) {
                        Ok(parsed) => {
                            let summary = format!(
                                "Loaded `{}` with {} nodes and {} edges.",
                                parsed.name,
                                parsed.nodes.len(),
                                parsed.edges.len()
                            );
                            workflow.set(parsed);
                            execution_report.set(None);
                            undo_stack.write().clear();
                            drag_state.set(None);
                            connection_draft.set(None);
                            message.set(Message::ok(summary));
                        }
                        Err(err) => message.set(Message::err(format!("Workflow JSON rejected: {err}"))),
                    },
                    "Load JSON"
                }

                button {
                    class: "action primary",
                    disabled: is_execution_active,
                    onclick: move |_| {
                        async move {
                            if active_execution.read().is_some() {
                                message.set(Message::ok("Execution is already running. Use Cancel Run to request a stop."));
                                return;
                            }
                            let current = workflow.read().clone();
                            let control = ExecutionControl::new();
                            active_execution.set(Some(control.clone()));
                            execution_report.set(Some(SimpleExecutionReport::default()));
                            message.set(Message::ok("Local executor started."));

                            let mut progress_report = SimpleExecutionReport::default();
                            match execute_simple_workflow_with_control_and_progress_async(
                                &current,
                                &control,
                                |event| {
                                    progress_report.events.push(event);
                                    execution_report.set(Some(progress_report.clone()));
                                },
                            ).await {
                                Ok(result) => {
                                    let summary = result.report.summary();
                                    let cancelled = control.is_cancelled();
                                    match result.workflow.to_pretty_json() {
                                        Ok(json) => json_text.set(json),
                                        Err(err) => message.set(Message::err(format!("Executed but failed to export JSON: {err}"))),
                                    }
                                    workflow.set(result.workflow);
                                    execution_report.set(Some(result.report));
                                    undo_stack.write().clear();
                                    drag_state.set(None);
                                    connection_draft.set(None);
                                    active_execution.set(None);
                                    if cancelled {
                                        message.set(Message::ok(format!(
                                            "Local executor cancelled: {summary}. Remaining nodes were skipped after the active node finished."
                                        )));
                                    } else {
                                        message.set(Message::ok(format!("Local executor finished: {summary}.")));
                                    }
                                }
                                Err(err) => {
                                    execution_report.set(None);
                                    active_execution.set(None);
                                    message.set(Message::err(format!("Local executor failed: {err}")));
                                }
                            }
                        }
                    },
                    "Run Local"
                }
                button {
                    class: "action",
                    disabled: is_execution_active,
                    onclick: move |_| {
                        async move {
                            if active_execution.read().is_some() {
                                message.set(Message::ok("Execution is already running. Use Cancel Run to request a stop."));
                                return;
                            }
                            let current = workflow.read().clone();
                            let active_provider_config = provider_config.read().clone();
                            let provider_summary = active_provider_config
                                .summary_with(provider_secret_env_value)
                                .sentence();
                            let control = ExecutionControl::new();
                            active_execution.set(Some(control.clone()));
                            execution_report.set(Some(SimpleExecutionReport::default()));
                            message.set(Message::ok(format!("Provider run started. {provider_summary}")));

                            match build_provider_registry(&active_provider_config) {
                                Ok(providers) => {
                                    let mut progress_report = SimpleExecutionReport::default();
                                    match execute_workflow_with_providers_with_control_and_progress_async(
                                        &current,
                                        &providers,
                                        &control,
                                        |event| {
                                            progress_report.events.push(event);
                                            execution_report.set(Some(progress_report.clone()));
                                        },
                                    ).await {
                                Ok(result) => {
                                    let summary = result.report.summary();
                                    let cancelled = control.is_cancelled();
                                    match result.workflow.to_pretty_json() {
                                        Ok(json) => json_text.set(json),
                                        Err(err) => message.set(Message::err(format!("Executed with providers but failed to export JSON: {err}"))),
                                    }
                                    workflow.set(result.workflow);
                                    execution_report.set(Some(result.report));
                                    undo_stack.write().clear();
                                    drag_state.set(None);
                                    connection_draft.set(None);
                                    active_execution.set(None);
                                    if cancelled {
                                        message.set(Message::ok(format!(
                                            "Provider run cancelled: {summary}. Remaining nodes were skipped after the active provider/media node finished. {provider_summary}"
                                        )));
                                    } else {
                                        message.set(Message::ok(format!("Provider run finished: {summary}. {provider_summary}")));
                                    }
                                }
                                Err(err) => {
                                    execution_report.set(None);
                                    active_execution.set(None);
                                    message.set(Message::err(format!("Provider run failed: {err}")));
                                }
                                    }
                                }
                                Err(err) => {
                                    execution_report.set(None);
                                    active_execution.set(None);
                                    message.set(Message::err(format!("Provider registry failed: {err}")));
                                }
                            }
                        }
                    },
                    "Run Providers"
                }
                button {
                    class: "action",
                    disabled: !is_execution_active,
                    onclick: move |_| {
                        if let Some(control) = active_execution.read().clone() {
                            control.cancel();
                            message.set(Message::ok(
                                "Cancellation requested. The active node may finish before remaining nodes are skipped."
                            ));
                        } else {
                            message.set(Message::ok("No workflow execution is currently running."));
                        }
                    },
                    "Cancel Run"
                }
                button {
                    class: "action",
                    onclick: move |_| match workflow.read().to_pretty_json() {
                        Ok(json) => {
                            json_text.set(json);
                            message.set(Message::ok("Exported current workflow into the JSON editor."));
                        }
                        Err(err) => message.set(Message::err(format!("Export failed: {err}"))),
                    },
                    "Export JSON"
                }
                DesktopFileActions {
                    workflow,
                    json_text,
                    message,
                    execution_report,
                    undo_stack,
                    drag_state,
                    connection_draft,
                }
                button {
                    class: "action",
                    onclick: move |_| {
                        let current = workflow.read().clone();
                        match save_autosave_workflow(&current) {
                            Ok(snapshot) => {
                                message.set(Message::ok(format!(
                                    "Saved `{}` to {} slot `{}`.",
                                    snapshot.name,
                                    storage_backend_label(),
                                    snapshot.slot
                                )));
                            }
                            Err(err) => message.set(Message::err(format!("Save failed: {err}"))),
                        }
                    },
                    "Save Slot"
                }
                button {
                    class: "action",
                    onclick: move |_| {
                        match load_autosave_workflow() {
                            Ok(next) => match next.to_pretty_json() {
                                Ok(json) => {
                                    workflow.set(next);
                                    json_text.set(json);
                                    execution_report.set(None);
                                    undo_stack.write().clear();
                                    drag_state.set(None);
                                    connection_draft.set(None);
                                    message.set(Message::ok(format!(
                                        "Loaded {} slot `{DEFAULT_AUTOSAVE_SLOT}`.",
                                        storage_backend_label()
                                    )));
                                }
                                Err(err) => message.set(Message::err(format!(
                                    "Loaded slot but failed to export JSON: {err}"
                                ))),
                            },
                            Err(err) => message.set(Message::err(format!("Load slot failed: {err}"))),
                        }
                    },
                    "Load Slot"
                }
            }
        }
    }
}

#[component]
fn DesktopFileActions(
    workflow: Signal<WorkflowFile>,
    json_text: Signal<String>,
    message: Signal<Message>,
    execution_report: Signal<Option<SimpleExecutionReport>>,
    undo_stack: Signal<WorkflowUndoStack>,
    drag_state: Signal<Option<DragState>>,
    connection_draft: Signal<Option<ConnectionDraft>>,
) -> Element {
    #[cfg(feature = "desktop")]
    {
        let mut workflow = workflow;
        let mut json_text = json_text;
        let mut message = message;
        let mut execution_report = execution_report;
        let mut undo_stack = undo_stack;
        let mut drag_state = drag_state;
        let mut connection_draft = connection_draft;

        rsx! {
            button {
                class: "action",
                onclick: move |_| match open_workflow_from_dialog() {
                    Ok(Some((next, path))) => match next.to_pretty_json() {
                        Ok(json) => {
                            let summary = format!(
                                "Opened `{}` from `{}` with {} nodes and {} edges.",
                                next.name,
                                path.display(),
                                next.nodes.len(),
                                next.edges.len()
                            );
                            workflow.set(next);
                            json_text.set(json);
                            execution_report.set(None);
                            undo_stack.write().clear();
                            drag_state.set(None);
                            connection_draft.set(None);
                            message.set(Message::ok(summary));
                        }
                        Err(err) => message.set(Message::err(format!(
                            "Opened file but failed to export JSON: {err}"
                        ))),
                    },
                    Ok(None) => message.set(Message::ok("Open file cancelled.")),
                    Err(err) => message.set(Message::err(format!("Open file failed: {err}"))),
                },
                "Open File"
            }
            button {
                class: "action",
                onclick: move |_| {
                    let current = workflow.read().clone();
                    match save_workflow_to_dialog(&current) {
                        Ok(Some((path, json))) => {
                            json_text.set(json);
                            message.set(Message::ok(format!(
                                "Saved `{}` to `{}`.",
                                current.name,
                                path.display()
                            )));
                        }
                        Ok(None) => message.set(Message::ok("Save As cancelled.")),
                        Err(err) => message.set(Message::err(format!("Save As failed: {err}"))),
                    }
                },
                "Save As"
            }
            button {
                class: "action",
                onclick: move |_| match open_project_from_dialog() {
                    Ok(Some(snapshot)) => match snapshot.workflow.to_pretty_json() {
                        Ok(json) => {
                            let summary = format!(
                                "Opened project `{}` from `{}` using `{}`.",
                                snapshot.manifest.name,
                                snapshot.root.display(),
                                snapshot.manifest.workflow_file
                            );
                            workflow.set(snapshot.workflow);
                            json_text.set(json);
                            execution_report.set(None);
                            undo_stack.write().clear();
                            drag_state.set(None);
                            connection_draft.set(None);
                            message.set(Message::ok(summary));
                        }
                        Err(err) => message.set(Message::err(format!(
                            "Opened project but failed to export JSON: {err}"
                        ))),
                    },
                    Ok(None) => message.set(Message::ok("Open Project cancelled.")),
                    Err(err) => message.set(Message::err(format!("Open Project failed: {err}"))),
                },
                "Open Project"
            }
            button {
                class: "action",
                onclick: move |_| {
                    let current = workflow.read().clone();
                    match save_project_to_dialog(&current) {
                        Ok(Some((snapshot, json))) => {
                            json_text.set(json);
                            message.set(Message::ok(format!(
                                "Saved project `{}` to `{}` with `{}` and `{}/`.",
                                snapshot.manifest.name,
                                snapshot.root.display(),
                                snapshot.manifest.workflow_file,
                                snapshot.manifest.media_dir
                            )));
                        }
                        Ok(None) => message.set(Message::ok("Save Project cancelled.")),
                        Err(err) => message.set(Message::err(format!("Save Project failed: {err}"))),
                    }
                },
                "Save Project"
            }
        }
    }
    #[cfg(not(feature = "desktop"))]
    {
        let _ = (
            &workflow,
            &json_text,
            &message,
            &execution_report,
            &undo_stack,
            &drag_state,
            &connection_draft,
        );
        rsx! {}
    }
}

#[component]
fn Sidebar(
    mut workflow: Signal<WorkflowFile>,
    mut json_text: Signal<String>,
    mut message: Signal<Message>,
    execution_report: Signal<Option<SimpleExecutionReport>>,
    mut undo_stack: Signal<WorkflowUndoStack>,
    mut viewport: Signal<CanvasViewport>,
    mut connection_draft: Signal<Option<ConnectionDraft>>,
    mut provider_config: Signal<ProviderConfigSet>,
    active_execution: Signal<Option<ExecutionControl>>,
) -> Element {
    let wf = workflow.read();
    let counts = wf.node_type_counts();
    let can_undo = undo_stack.read().can_undo();
    let can_redo = undo_stack.read().can_redo();
    let viewport_snapshot = *viewport.read();
    let zoom_percent = (viewport_snapshot.zoom * 100.0).round() as i32;
    let selected_id = selected_node_id(&wf).map(ToOwned::to_owned);
    let selected_ids = selected_node_ids(&wf)
        .into_iter()
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let selected_count = selected_ids.len();
    let selected_index = selected_id
        .as_ref()
        .and_then(|id| wf.nodes.iter().position(|node| node.id == *id));
    let next_node_id = selected_index.and_then(|index| {
        (!wf.nodes.is_empty()).then(|| wf.nodes[(index + 1) % wf.nodes.len()].id.clone())
    });
    let selected_summary = match selected_count {
        0 => "No node selected. Click a card in the canvas.".to_string(),
        1 => selected_id
            .as_ref()
            .and_then(|id| wf.nodes.iter().find(|node| node.id == *id))
            .map(|node| {
                format!(
                    "{} at ({:.0}, {:.0})",
                    node.display_label(),
                    node.position.x,
                    node.position.y
                )
            })
            .unwrap_or_else(|| "1 node selected.".to_string()),
        count => format!("{count} nodes selected: {}.", selected_ids.join(", ")),
    };
    let selected_split_grid_id = selected_id
        .as_ref()
        .and_then(|id| wf.nodes.iter().find(|node| node.id == *id))
        .filter(|node| node.node_type == NodeType::SplitGrid)
        .and_then(|node| {
            let has_children = node
                .data
                .get("childNodeIds")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|items| !items.is_empty());
            (!has_children && !is_node_in_locked_group(&wf, &node.id)).then(|| node.id.clone())
        });
    let can_generate_split_grid_children = selected_split_grid_id.is_some();
    let selected_split_grid_child_sets = selected_id
        .as_ref()
        .and_then(|id| wf.nodes.iter().find(|node| node.id == *id))
        .filter(|node| node.node_type == NodeType::SplitGrid)
        .and_then(|node| {
            split_grid_child_sets(&wf, &node.id)
                .ok()
                .map(|sets| (node.id.clone(), sets))
        });
    let selected_frame_capture = selected_id
        .as_ref()
        .and_then(|id| wf.nodes.iter().find(|node| node.id == *id))
        .filter(|node| node.node_type == NodeType::VideoFrameGrab)
        .map(|node| {
            (
                node.id.clone(),
                video_frame_capture_request(node.id.clone(), node),
            )
        });
    let selected_glb_capture = selected_id
        .as_ref()
        .and_then(|id| wf.nodes.iter().find(|node| node.id == *id))
        .filter(|node| node.node_type == NodeType::GlbViewer)
        .map(|node| (node.id.clone(), glb_capture_request(node.id.clone(), node)));
    let draft_summary = connection_draft
        .read()
        .as_ref()
        .map(|draft| {
            format!(
                "Connecting from `{}`:{}.",
                draft.source_node_id, draft.source_handle
            )
        })
        .unwrap_or_else(|| {
            "Drag or press a right-side source handle, then release on a left-side target handle."
                .to_string()
        });
    let msg = message.read();
    let order_text = match execution_order(&wf) {
        Ok(items) if items.is_empty() => "Order: no nodes".to_string(),
        Ok(items) => format!("Order: {}", items.join(" → ")),
        Err(err) => format!("Order blocked: {err}"),
    };
    let mock_provider_summary = ProviderConfigSet::mock_all()
        .summary_with(|_| None::<String>)
        .sentence();
    let provider_config_snapshot = provider_config.read().clone();
    let provider_settings_summary = provider_config_snapshot
        .summary_with(provider_secret_env_value)
        .sentence();
    let media_summary = workflow_media_summary(&wf);
    let media_sentence = media_summary.sentence();
    let extra_media_profiles = media_summary.profiles.len().saturating_sub(6);
    let report = execution_report.read();
    let is_execution_active = active_execution.read().is_some();

    rsx! {
        aside { class: "sidebar",
            section { class: "panel",
                h2 { "Workflow" }
                p { "{wf.name}" }
                div { class: "stats",
                    div { class: "stat", strong { "{wf.nodes.len()}" } span { "nodes" } }
                    div { class: "stat", strong { "{wf.edges.len()}" } span { "edges" } }
                    div { class: "stat", strong { "{wf.groups.len()}" } span { "groups" } }
                }
            }
            section { class: "panel",
                h2 { "Node Types" }
                if counts.is_empty() {
                    p { "No nodes yet." }
                } else {
                    div { class: "type-list",
                        for (node_type, count) in counts.iter() {
                            div { class: "type-row",
                                code { "{node_type.title()}" }
                                span { "× {count}" }
                            }
                        }
                    }
                }
            }
            section { class: "panel",
                h2 { "Media Capabilities" }
                p { "{media_sentence}" }
                if media_summary.profiles.is_empty() {
                    p { "No media-specific nodes yet. Media adapters stay idle for this workflow." }
                } else {
                    div { class: "edge-list",
                        for profile in media_summary.profiles.iter().take(6) {
                            {
                                let label = profile.label.clone();
                                let kind_labels = profile.kind_labels();
                                let platform_label = profile.platform_label();
                                let notes = profile.notes.clone();
                                rsx! {
                                    div { class: "edge-row",
                                        code { "{label}: {kind_labels} · {platform_label}" }
                                    }
                                    p { class: "viewport-status", "{notes}" }
                                }
                            }
                        }
                    }
                    if extra_media_profiles > 0 {
                        p { class: "viewport-status",
                            "... and {extra_media_profiles} more media profile(s)."
                        }
                    }
                }
            }
            if !wf.groups.is_empty() {
                section { class: "panel",
                    h2 { "Groups" }
                    div { class: "edge-list",
                        for group in wf.groups.values() {
                            {
                                let lock_group_id = group.id.clone();
                                let widen_group_id = group.id.clone();
                                let narrow_group_id = group.id.clone();
                                let taller_group_id = group.id.clone();
                                let shorter_group_id = group.id.clone();
                                let label = if group.locked.unwrap_or(false) { "Unlock" } else { "Lock" };
                                let state = if group.locked.unwrap_or(false) { "locked" } else { "unlocked" };
                                let group_summary = format!(
                                    "{} · {state} · {:.0}×{:.0}",
                                    group.name,
                                    group.size.width,
                                    group.size.height
                                );
                                rsx! {
                                    div { class: "edge-row",
                                        code { "{group_summary}" }
                                        button {
                                            class: "mini-action neutral",
                                            onclick: move |_| {
                                                let group_id = lock_group_id.clone();
                                                toggle_group_lock_by_id(
                                                    &group_id,
                                                    &mut workflow,
                                                    &mut json_text,
                                                    &mut message,
                                                    &mut undo_stack,
                                                );
                                            },
                                            "{label}"
                                        }
                                        button {
                                            class: "mini-action neutral",
                                            title: "Wider",
                                            onclick: move |_| {
                                                let group_id = widen_group_id.clone();
                                                resize_group_by_id(
                                                    &group_id,
                                                    48.0,
                                                    0.0,
                                                    GroupEditSignals {
                                                        workflow,
                                                        json_text,
                                                        message,
                                                        undo_stack,
                                                    },
                                                );
                                            },
                                            "W+"
                                        }
                                        button {
                                            class: "mini-action neutral",
                                            title: "Narrower",
                                            onclick: move |_| {
                                                let group_id = narrow_group_id.clone();
                                                resize_group_by_id(
                                                    &group_id,
                                                    -48.0,
                                                    0.0,
                                                    GroupEditSignals {
                                                        workflow,
                                                        json_text,
                                                        message,
                                                        undo_stack,
                                                    },
                                                );
                                            },
                                            "W-"
                                        }
                                        button {
                                            class: "mini-action neutral",
                                            title: "Taller",
                                            onclick: move |_| {
                                                let group_id = taller_group_id.clone();
                                                resize_group_by_id(
                                                    &group_id,
                                                    0.0,
                                                    48.0,
                                                    GroupEditSignals {
                                                        workflow,
                                                        json_text,
                                                        message,
                                                        undo_stack,
                                                    },
                                                );
                                            },
                                            "H+"
                                        }
                                        button {
                                            class: "mini-action neutral",
                                            title: "Shorter",
                                            onclick: move |_| {
                                                let group_id = shorter_group_id.clone();
                                                resize_group_by_id(
                                                    &group_id,
                                                    0.0,
                                                    -48.0,
                                                    GroupEditSignals {
                                                        workflow,
                                                        json_text,
                                                        message,
                                                        undo_stack,
                                                    },
                                                );
                                            },
                                            "H-"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            section { class: "panel",
                h2 { "Canvas MVP" }
                p { "{selected_summary}" }
                p { class: "handle-hint",
                    "Ctrl/Cmd-click nodes to add or remove them from the selection; drag a selected node or use arrows to move the whole selection. Shift-drag blank canvas to draw a group box."
                }
                div { class: "edit-grid",
                    button {
                        class: "action",
                        onclick: move |_| {
                            mutate_workflow(&mut workflow, &mut json_text, &mut message, &mut undo_stack, |workflow| {
                                if workflow.nodes.is_empty() {
                                    return Err("No nodes to select.".to_string());
                                }
                                let next_id = selected_node_id(workflow)
                                    .and_then(|id| workflow.nodes.iter().position(|node| node.id == id))
                                    .map(|index| workflow.nodes[(index + 1) % workflow.nodes.len()].id.clone())
                                    .unwrap_or_else(|| workflow.nodes[0].id.clone());
                                select_node(workflow, Some(&next_id))
                                    .map_err(|err| err.to_string())?;
                                Ok(format!("Selected `{next_id}`."))
                            });
                        },
                        "Select Next"
                    }
                    button {
                        class: "action",
                        disabled: selected_count == 0,
                        onclick: move |_| {
                            mutate_workflow(&mut workflow, &mut json_text, &mut message, &mut undo_stack, |workflow| {
                                let selected_ids = selected_node_ids(workflow)
                                    .into_iter()
                                    .map(ToOwned::to_owned)
                                    .collect::<Vec<_>>();
                                let count = selected_ids.len();
                                let group = create_group_for_nodes(workflow, &selected_ids)
                                    .map_err(|err| err.to_string())?;
                                Ok(format!("Created group `{}` for {count} node(s).", group.name))
                            });
                        },
                        "Create Group"
                    }
                    button {
                        class: "action",
                        disabled: !can_generate_split_grid_children,
                        onclick: move |_| {
                            let Some(split_node_id) = selected_split_grid_id.clone() else {
                                message.set(Message::err("Select an unconfigured Split Grid node first."));
                                return;
                            };
                            mutate_workflow(&mut workflow, &mut json_text, &mut message, &mut undo_stack, move |workflow| {
                                let generated = generate_split_grid_children(workflow, &split_node_id)
                                    .map_err(|err| err.to_string())?;
                                Ok(format!(
                                    "Generated {} split-grid child set(s) for `{}`.",
                                    generated.child_node_ids.len(),
                                    generated.split_node_id
                                ))
                            });
                        },
                        "Split Children"
                    }
                    button {
                        class: "action",
                        disabled: selected_count == 0,
                        onclick: move |_| {
                            mutate_selected_node(&mut workflow, &mut json_text, &mut message, &mut undo_stack, -32.0, 0.0);
                        },
                        "←"
                    }
                    button {
                        class: "action",
                        disabled: selected_count == 0,
                        onclick: move |_| {
                            mutate_selected_node(&mut workflow, &mut json_text, &mut message, &mut undo_stack, 32.0, 0.0);
                        },
                        "→"
                    }
                    button {
                        class: "action",
                        disabled: selected_count == 0,
                        onclick: move |_| {
                            mutate_selected_node(&mut workflow, &mut json_text, &mut message, &mut undo_stack, 0.0, -32.0);
                        },
                        "↑"
                    }
                    button {
                        class: "action",
                        disabled: selected_count == 0,
                        onclick: move |_| {
                            mutate_selected_node(&mut workflow, &mut json_text, &mut message, &mut undo_stack, 0.0, 32.0);
                        },
                        "↓"
                    }
                    button {
                        class: "action",
                        disabled: selected_id.is_none() || next_node_id.is_none(),
                        onclick: move |_| {
                            mutate_workflow(&mut workflow, &mut json_text, &mut message, &mut undo_stack, |workflow| {
                                let Some(source) = selected_node_id(workflow).map(ToOwned::to_owned) else {
                                    return Err("Select a source node first.".to_string());
                                };
                                let Some(index) = workflow.nodes.iter().position(|node| node.id == source) else {
                                    return Err(format!("Selected node `{source}` disappeared."));
                                };
                                let target = workflow.nodes[(index + 1) % workflow.nodes.len()].id.clone();
                                let edge = add_edge_between(workflow, &source, &target, None, None)
                                    .map_err(|err| err.to_string())?;
                                Ok(format!("Connected `{}` → `{}` as `{}`.", edge.source, edge.target, edge.id))
                            });
                        },
                        "Connect Next"
                    }
                    button {
                        class: "action",
                        disabled: !can_undo,
                        onclick: move |_| {
                            apply_history_action(
                                &mut workflow,
                                &mut json_text,
                                &mut message,
                                &mut undo_stack,
                                HistoryDirection::Undo,
                            );
                        },
                        "Undo"
                    }
                    button {
                        class: "action",
                        disabled: !can_redo,
                        onclick: move |_| {
                            apply_history_action(
                                &mut workflow,
                                &mut json_text,
                                &mut message,
                                &mut undo_stack,
                                HistoryDirection::Redo,
                            );
                        },
                        "Redo"
                    }
                }
                div { class: "edit-grid",
                    button {
                        class: "action",
                        onclick: move |_| {
                            viewport.with_mut(|viewport| viewport.zoom_by(1.15));
                        },
                        "Zoom +"
                    }
                    button {
                        class: "action",
                        onclick: move |_| {
                            viewport.with_mut(|viewport| viewport.zoom_by(1.0 / 1.15));
                        },
                        "Zoom -"
                    }
                    button {
                        class: "action",
                        onclick: move |_| viewport.set(CanvasViewport::default()),
                        "Reset View"
                    }
                    button {
                        class: "action",
                        onclick: move |_| viewport.with_mut(|viewport| viewport.pan_by(-64.0, 0.0)),
                        "Pan ←"
                    }
                    button {
                        class: "action",
                        onclick: move |_| viewport.with_mut(|viewport| viewport.pan_by(0.0, -64.0)),
                        "Pan ↑"
                    }
                    button {
                        class: "action",
                        onclick: move |_| viewport.with_mut(|viewport| viewport.pan_by(64.0, 0.0)),
                        "Pan →"
                    }
                    button {
                        class: "action",
                        onclick: move |_| viewport.with_mut(|viewport| viewport.pan_by(0.0, 64.0)),
                        "Pan ↓"
                    }
                }
                if let Some((split_node_id, child_sets)) = selected_split_grid_child_sets.as_ref() {
                    if !child_sets.is_empty() {
                        p { class: "handle-hint",
                            "Split Grid `{split_node_id}` has {child_sets.len()} generated child set(s). Select one to inspect its ImageInput, Prompt, and Generate nodes together."
                        }
                        div { class: "edge-list",
                            for (index, child) in child_sets.iter().enumerate() {
                                {
                                    let child_number = index + 1;
                                    let split_node_id = split_node_id.clone();
                                    let child_summary = format!(
                                        "Cell {child_number}: {} · {} · {}",
                                        child.image_input,
                                        child.prompt,
                                        child.nano_banana
                                    );
                                    rsx! {
                                        div { class: "edge-row",
                                            code { "{child_summary}" }
                                            button {
                                                class: "mini-action neutral",
                                                onclick: move |_| {
                                                    focus_split_grid_child_set(
                                                        &split_node_id,
                                                        index,
                                                        &mut workflow,
                                                        &mut json_text,
                                                        &mut message,
                                                        &mut undo_stack,
                                                        &mut viewport,
                                                    );
                                                },
                                                "Select"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if let Some((frame_node_id, capture_request)) = selected_frame_capture.as_ref() {
                    p { class: "handle-hint",
                        "Video Frame Grab `{frame_node_id}` can use the browser/WebView adapter only after Run Local records a renderable frameGrabPlan."
                    }
                    match capture_request {
                        Ok(request) => {
                            let frame_node_id = frame_node_id.clone();
                            let source_summary = request.source_summary();
                            rsx! {
                                div { class: "edge-row",
                                    code { "{source_summary}" }
                                    button {
                                        class: "mini-action neutral",
                                        onclick: move |event: MouseEvent| {
                                            event.stop_propagation();
                                            let frame_node_id = frame_node_id.clone();
                                            async move {
                                                capture_video_frame_with_webview_adapter(
                                                    frame_node_id,
                                                    workflow,
                                                    json_text,
                                                    message,
                                                    execution_report,
                                                ).await;
                                            }
                                        },
                                        "Capture"
                                    }
                                }
                            }
                        }
                        Err(reason) => {
                            rsx! {
                                p { class: "viewport-status",
                                    "Adapter unavailable: {reason}"
                                }
                            }
                        }
                    }
                }
                if let Some((viewer_node_id, capture_request)) = selected_glb_capture.as_ref() {
                    p { class: "handle-hint",
                        "GLB Viewer `{viewer_node_id}` can capture a PNG snapshot after Run Local records a renderable glbViewerPlan."
                    }
                    match capture_request {
                        Ok(request) => {
                            let viewer_node_id = viewer_node_id.clone();
                            let source_summary = request.source_summary();
                            rsx! {
                                div { class: "edge-row",
                                    code { "{source_summary}" }
                                    button {
                                        class: "mini-action neutral",
                                        onclick: move |event: MouseEvent| {
                                            event.stop_propagation();
                                            let viewer_node_id = viewer_node_id.clone();
                                            async move {
                                                capture_glb_snapshot_with_webview_adapter(
                                                    viewer_node_id,
                                                    workflow,
                                                    json_text,
                                                    message,
                                                    execution_report,
                                                ).await;
                                            }
                                        },
                                        "Capture PNG"
                                    }
                                }
                            }
                        }
                        Err(reason) => {
                            rsx! {
                                p { class: "viewport-status",
                                    "GLB capture unavailable: {reason}"
                                }
                            }
                        }
                    }
                }
                p { class: "viewport-status",
                    "View: {zoom_percent}% · pan ({viewport_snapshot.pan_x:.0}, {viewport_snapshot.pan_y:.0})"
                }
                if wf.edges.is_empty() {
                    p { "No edges yet." }
                } else {
                    div { class: "edge-list",
                        for edge in wf.edges.iter() {
                            {
                                let edge_id = edge.id.clone();
                                rsx! {
                                    div { class: "edge-row",
                                        code { "{edge_label(edge)}" }
                                        button {
                                            class: "mini-action",
                                            onclick: move |_| {
                                                let edge_id = edge_id.clone();
                                                remove_edge_by_id(
                                                    &edge_id,
                                                    &mut workflow,
                                                    &mut json_text,
                                                    &mut message,
                                                    &mut undo_stack,
                                                );
                                            },
                                            "Remove"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                p { class: "handle-hint", "{draft_summary}" }
                div { class: "handle-actions",
                    button {
                        class: "action",
                        disabled: connection_draft.read().is_none(),
                        onclick: move |_| {
                            connection_draft.set(None);
                            message.set(Message::ok("Cancelled pending handle connection."));
                        },
                        "Cancel Connect"
                    }
                }
            }

            section { class: "panel",
                h2 { "Execution Spine" }
                p { "{order_text}" }
                if is_execution_active {
                    p { class: "handle-hint",
                        "Execution is running. Use Cancel Run in the header to request a stop; cancellation is applied before the next node starts."
                    }
                }
                if let Some(report) = report.as_ref() {
                    p { "Last run: {report.summary()}" }
                    div { class: "execution-log",
                        for event in report.events.iter() {
                            div { class: "event",
                                div { class: "event-head",
                                    span { class: "event-title", "{event.node_type}" }
                                    span { class: "badge {event.status.as_str()}", "{event.status.as_str()}" }
                                }
                                div { class: "event-message", "{event.node_id}: {event.message}" }
                            }
                        }
                    }
                } else {
                    p { "Run Local executes pure Rust prompt/array/output/control nodes and skips unregistered providers. Run Providers wires the configured provider registry; mock mode stays offline, and live desktop HTTP providers require explicit feature/env opt-in." }
                    p { "{mock_provider_summary}" }
                }
            }
            section { class: "panel",
                h2 { "Provider Settings" }
                p { "{provider_settings_summary}" }
                p { "Only provider mode and secret source labels are saved. Raw API keys stay outside GemEd config state." }
                p { class: "secret-guide", "{provider_secret_setup_overview()}" }
                div { class: "handle-actions",
                    button {
                        class: "action",
                        onclick: move |_| {
                            provider_config.set(default_provider_config());
                            message.set(Message::ok("Reset provider settings to platform defaults."));
                        },
                        "Platform Defaults"
                    }
                    button {
                        class: "action",
                        onclick: move |_| {
                            provider_config.set(ProviderConfigSet::mock_all());
                            message.set(Message::ok("Reset provider settings to mock providers."));
                        },
                        "Mock Defaults"
                    }
                    button {
                        class: "action",
                        onclick: move |_| {
                            let current = provider_config.read().clone();
                            match save_provider_settings(&current) {
                                Ok(snapshot) => message.set(Message::ok(format!(
                                    "Saved provider settings to {} slot `{}`.",
                                    storage_backend_label(),
                                    snapshot.slot
                                ))),
                                Err(err) => message.set(Message::err(format!("Save provider settings failed: {err}"))),
                            }
                        },
                        "Save Providers"
                    }
                    button {
                        class: "action",
                        onclick: move |_| match load_provider_settings() {
                            Ok(config) => {
                                provider_config.set(config);
                                message.set(Message::ok(format!(
                                    "Loaded provider settings from {} slot `{DEFAULT_PROVIDER_CONFIG_SLOT}`.",
                                    storage_backend_label()
                                )));
                            }
                            Err(err) => message.set(Message::err(format!("Load provider settings failed: {err}"))),
                        },
                        "Load Providers"
                    }
                }
                div { class: "provider-list",
                    for provider in provider_config_snapshot.providers.iter() {
                        {
                            let provider_id = provider.id.clone();
                            let mock_id = provider.id.clone();
                            let env_id = provider.id.clone();
                            let disabled_id = provider.id.clone();
                            let setup_id = provider.id.clone();
                            let name = provider.id.display_name();
                            let mode = provider_runtime_mode_label(provider.runtime_mode);
                            let source = provider.secret_source.public_label();
                            let model_id = provider.id.clone();
                            let base_url_id = provider.id.clone();
                            let default_model = provider.default_model.clone().unwrap_or_default();
                            let base_url = provider.base_url.clone().unwrap_or_default();
                            let capabilities = provider_capability_list(&provider.capabilities);
                            let secret_hint = provider_secret_setup_hint(provider);
                            let can_show_setup = provider.runtime_mode != ProviderRuntimeMode::Mock
                                && provider.runtime_mode != ProviderRuntimeMode::Disabled;
                            let status = provider_status(provider);
                            let status_class = provider_status_class(provider);
                            rsx! {
                                div { class: "provider-row",
                                    div {
                                        div { class: "provider-name",
                                            span { "{name}" }
                                            span { class: "provider-status {status_class}", "{status}" }
                                        }
                                        div { class: "provider-meta",
                                            "{provider_id.as_wire()} · {mode} · {source} · {capabilities}"
                                        }
                                        div { class: "provider-secret",
                                            "{secret_hint}"
                                        }
                                    }
                                    div { class: "provider-actions",
                                        button {
                                            class: "mini-action neutral",
                                            onclick: move |_| {
                                                set_provider_config_mode(
                                                    &mut provider_config,
                                                    mock_id.clone(),
                                                    ProviderSettingsMode::Mock,
                                                    &mut message,
                                                );
                                            },
                                            "Mock"
                                        }
                                        button {
                                            class: "mini-action warn",
                                            onclick: move |_| {
                                                set_provider_config_mode(
                                                    &mut provider_config,
                                                    env_id.clone(),
                                                    ProviderSettingsMode::DesktopEnv,
                                                    &mut message,
                                                );
                                            },
                                            "Env"
                                        }
                                        if can_show_setup {
                                            button {
                                                class: "mini-action neutral",
                                                onclick: move |_| {
                                                    message.set(Message::ok(provider_secret_setup_message(&setup_id)));
                                                },
                                                "Setup"
                                            }
                                        }
                                        button {
                                            class: "mini-action",
                                            onclick: move |_| {
                                                set_provider_config_mode(
                                                    &mut provider_config,
                                                    disabled_id.clone(),
                                                    ProviderSettingsMode::Disabled,
                                                    &mut message,
                                                );
                                            },
                                            "Off"
                                        }
                                    }
                                    div { class: "provider-edit",
                                        label {
                                            "Default model"
                                            input {
                                                class: "provider-input",
                                                r#type: "text",
                                                value: "{default_model}",
                                                placeholder: provider_default_model_placeholder(&provider_id),
                                                oninput: move |event| {
                                                    set_provider_default_model(
                                                        &mut provider_config,
                                                        model_id.clone(),
                                                        &event.value(),
                                                        &mut message,
                                                    );
                                                },
                                            }
                                        }
                                        label {
                                            "Base URL / endpoint"
                                            input {
                                                class: "provider-input",
                                                r#type: "url",
                                                value: "{base_url}",
                                                placeholder: provider_base_url_placeholder(&provider_id),
                                                oninput: move |event| {
                                                    set_provider_base_url(
                                                        &mut provider_config,
                                                        base_url_id.clone(),
                                                        &event.value(),
                                                        &mut message,
                                                    );
                                                },
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            section { class: "panel",
                h2 { "Workflow JSON" }
                p { "Paste a compatible v1 workflow JSON, then load it. This is the first compatibility gate from the rewrite plan." }
                textarea {
                    class: "workflow-json",
                    value: "{json_text}",
                    oninput: move |event| json_text.set(event.value()),
                }
                div { class: if msg.ok { "message ok" } else { "message err" }, "{msg.text}" }
            }
        }
    }
}

#[component]
fn WorkflowCanvas(
    workflow: Signal<WorkflowFile>,
    json_text: Signal<String>,
    message: Signal<Message>,
    undo_stack: Signal<WorkflowUndoStack>,
    drag_state: Signal<Option<DragState>>,
    pan_state: Signal<Option<PanState>>,
    group_resize_state: Signal<Option<GroupResizeState>>,
    group_move_state: Signal<Option<GroupMoveState>>,
    group_selection_state: Signal<Option<GroupSelectionState>>,
    viewport: Signal<CanvasViewport>,
    connection_draft: Signal<Option<ConnectionDraft>>,
    media_overlay: Signal<Option<MediaOverlay>>,
) -> Element {
    let wf = workflow.read();
    let viewport_snapshot = *viewport.read();
    let is_panning = pan_state.read().is_some();
    let is_moving_group = group_move_state.read().is_some();
    let active_selection_rect = group_selection_state
        .read()
        .as_ref()
        .map(GroupSelectionState::rect);
    let wrap_class = if is_panning {
        "canvas-wrap panning"
    } else {
        "canvas-wrap"
    };
    let mut canvas_classes = vec!["canvas"];
    if is_panning {
        canvas_classes.push("panning");
    }
    if is_moving_group {
        canvas_classes.push("panning");
    }
    if active_selection_rect.is_some() {
        canvas_classes.push("selecting");
    }
    let canvas_class = canvas_classes.join(" ");
    let canvas_style = format!(
        "transform: translate({:.1}px, {:.1}px) scale({:.3}); transform-origin: 0 0;",
        viewport_snapshot.pan_x, viewport_snapshot.pan_y, viewport_snapshot.zoom
    );

    rsx! {
        section {
            class: "{wrap_class}",
            onwheel: move |event: WheelEvent| {
                handle_canvas_wheel(event, viewport);
            },
            onmousedown: move |event: MouseEvent| {
                begin_canvas_pan(
                    event,
                    CanvasGestureSignals {
                        drag_state,
                        pan_state,
                        connection_draft,
                        group_resize_state,
                        group_move_state,
                        group_selection_state,
                        viewport,
                    },
                );
            },
            onmousemove: move |event: MouseEvent| {
                update_dragged_node(event.clone(), workflow, json_text, drag_state);
                update_group_resize(event.clone(), workflow, json_text, group_resize_state);
                update_group_move(event.clone(), workflow, json_text, group_move_state);
                update_group_selection(event.clone(), group_selection_state);
                update_canvas_pan(event, viewport, pan_state);
            },
            onmouseup: move |_| {
                finish_drag(workflow, json_text, message, drag_state);
                finish_group_resize(workflow, message, undo_stack, group_resize_state);
                finish_group_move(workflow, message, undo_stack, group_move_state);
                finish_group_selection(workflow, json_text, message, undo_stack, group_selection_state);
                finish_canvas_pan(pan_state);
                cancel_canvas_connection(message, connection_draft);
            },
            onmouseleave: move |_| {
                finish_drag(workflow, json_text, message, drag_state);
                finish_group_resize(workflow, message, undo_stack, group_resize_state);
                finish_group_move(workflow, message, undo_stack, group_move_state);
                cancel_group_selection(message, group_selection_state);
                finish_canvas_pan(pan_state);
            },
            if wf.nodes.is_empty() {
                div { class: "empty",
                    div {
                        h2 { "Blank workflow" }
                        p { "Use the JSON panel or Sample button to load nodes into the Dioxus canvas." }
                    }
                }
            } else {
                div {
                    class: "{canvas_class}",
                    style: "{canvas_style}",
                    onmousedown: move |event: MouseEvent| {
                        begin_group_selection(
                            event,
                            CanvasGestureSignals {
                                drag_state,
                                pan_state,
                                connection_draft,
                                group_resize_state,
                                group_move_state,
                                group_selection_state,
                                viewport,
                            },
                        );
                    },
                    for group in wf.groups.values() {
                        GroupBox { group: group.clone(), workflow, json_text, message, undo_stack, group_resize_state, group_move_state, viewport }
                    }
                    if let Some(rect) = active_selection_rect {
                        div {
                            class: "selection-box",
                            style: "{rect.style()}",
                        }
                    }
                    svg { class: "edge-layer", view_box: "0 0 1400 900",
                        defs {
                            marker { id: "arrow", marker_width: "10", marker_height: "10", ref_x: "9", ref_y: "3", orient: "auto", marker_units: "strokeWidth",
                                path { d: "M0,0 L0,6 L9,3 z", fill: "rgba(125, 211, 252, .78)" }
                            }
                        }
                        for edge in wf.edges.iter() {
                            if let Some(path) = edge_path(&wf, edge) {
                                {
                                    let edge_id = edge.id.clone();
                                    let action = edge_delete_action(&wf, edge);
                                    rsx! {
                                        g { class: "edge-group",
                                            path {
                                                class: "edge-hit",
                                                d: "{path}",
                                                onmousedown: move |event: MouseEvent| {
                                                    event.stop_propagation();
                                                },
                                                onmouseup: move |event: MouseEvent| {
                                                    event.stop_propagation();
                                                },
                                                onclick: {
                                                    let edge_id = edge_id.clone();
                                                    move |event: MouseEvent| {
                                                        event.stop_propagation();
                                                        remove_edge_by_id(
                                                            &edge_id,
                                                            &mut workflow,
                                                            &mut json_text,
                                                            &mut message,
                                                            &mut undo_stack,
                                                        );
                                                    }
                                                },
                                            }
                                            path { class: "edge", d: "{path}" }
                                            if let Some(action) = action {
                                                g {
                                                    class: "edge-action",
                                                    transform: "translate({action.x:.1} {action.y:.1})",
                                                    onmousedown: move |event: MouseEvent| {
                                                        event.stop_propagation();
                                                    },
                                                    onmouseup: move |event: MouseEvent| {
                                                        event.stop_propagation();
                                                    },
                                                    onclick: {
                                                        let edge_id = edge_id.clone();
                                                        move |event: MouseEvent| {
                                                            event.stop_propagation();
                                                            remove_edge_by_id(
                                                                &edge_id,
                                                                &mut workflow,
                                                                &mut json_text,
                                                                &mut message,
                                                                &mut undo_stack,
                                                            );
                                                        }
                                                    },
                                                    circle { class: "edge-delete-dot", r: "10" }
                                                    text {
                                                        class: "edge-delete-label",
                                                        text_anchor: "middle",
                                                        dominant_baseline: "central",
                                                        "×"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    for node in wf.nodes.iter() {
                        NodeCard {
                            node: node.clone(),
                            locked: is_node_in_locked_group(&wf, &node.id),
                            workflow,
                            json_text,
                            message,
                            undo_stack,
                            drag_state,
                            viewport,
                            connection_draft,
                            media_overlay,
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn GroupBox(
    group: NodeGroup,
    mut workflow: Signal<WorkflowFile>,
    mut json_text: Signal<String>,
    mut message: Signal<Message>,
    mut undo_stack: Signal<WorkflowUndoStack>,
    mut group_resize_state: Signal<Option<GroupResizeState>>,
    mut group_move_state: Signal<Option<GroupMoveState>>,
    viewport: Signal<CanvasViewport>,
) -> Element {
    let color = group_color_style(group.color);
    let background = group_background_style(group.color);
    let border = group_border_style(&group);
    let style = format!(
        "left: {:.1}px; top: {:.1}px; width: {:.1}px; height: {:.1}px; background: {background}; border: {border};",
        group.position.x.max(0.0),
        group.position.y.max(0.0),
        group.size.width.max(80.0),
        group.size.height.max(48.0),
    );
    let mut class = vec!["group-box"];
    if group.locked.unwrap_or(false) {
        class.push("locked");
    }
    if group_resize_state
        .read()
        .as_ref()
        .is_some_and(|state| state.group_id == group.id)
    {
        class.push("resizing");
    }
    let is_moving = group_move_state
        .read()
        .as_ref()
        .is_some_and(|state| state.group_id == group.id);
    if group.is_nbp_input.unwrap_or(false) {
        class.push("nbp");
    }
    let class = class.join(" ");
    let group_locked = group.locked.unwrap_or(false);
    let header_class = if group_locked {
        "group-header"
    } else if is_moving {
        "group-header movable moving"
    } else {
        "group-header movable"
    };
    let header_style = format!("background: {color};");
    let lock_label = if group.locked.unwrap_or(false) {
        "Unlock"
    } else {
        "Lock"
    };
    let group_id = group.id.clone();
    let resize_group_id = group.id.clone();
    let move_group_id = group.id.clone();
    let start_size = group.size;

    rsx! {
        div { class: "{class}", style: "{style}",
            div {
                class: "{header_class}",
                style: "{header_style}",
                title: if group_locked { "Unlock group before moving" } else { "Drag to move group and its member nodes" },
                onmousedown: move |event: MouseEvent| {
                    event.stop_propagation();
                    event.prevent_default();
                    if group_locked {
                        message.set(Message::err(format!(
                            "Group `{move_group_id}` is locked. Unlock it before moving."
                        )));
                        return;
                    }
                    let point = event.data().client_coordinates();
                    let before = workflow.read().clone();
                    group_move_state.set(Some(GroupMoveState {
                        group_id: move_group_id.clone(),
                        start_client_x: point.x,
                        start_client_y: point.y,
                        start_viewport: *viewport.read(),
                        before,
                    }));
                },
                onmouseup: move |event: MouseEvent| {
                    event.stop_propagation();
                    finish_group_move(workflow, message, undo_stack, group_move_state);
                },
                span { "{group.name}" }
                if group.locked.unwrap_or(false) {
                    span { "🔒" }
                }
                button {
                    class: "group-lock-toggle",
                    onmousedown: move |event: MouseEvent| {
                        event.stop_propagation();
                    },
                    onmouseup: move |event: MouseEvent| {
                        event.stop_propagation();
                    },
                    onclick: move |event: MouseEvent| {
                        event.stop_propagation();
                        let group_id = group_id.clone();
                        toggle_group_lock_by_id(
                            &group_id,
                            &mut workflow,
                            &mut json_text,
                            &mut message,
                            &mut undo_stack,
                        );
                    },
                    "{lock_label}"
                }
            }
            div {
                class: "group-resize-handle",
                title: if group_locked { "Unlock group before resizing" } else { "Drag to resize group" },
                onmousedown: move |event: MouseEvent| {
                    event.stop_propagation();
                    event.prevent_default();
                    if group_locked {
                        message.set(Message::err(format!(
                            "Group `{resize_group_id}` is locked. Unlock it before resizing."
                        )));
                        return;
                    }
                    let point = event.data().client_coordinates();
                    let before = workflow.read().clone();
                    group_resize_state.set(Some(GroupResizeState {
                        group_id: resize_group_id.clone(),
                        start_client_x: point.x,
                        start_client_y: point.y,
                        start_size,
                        start_viewport: *viewport.read(),
                        before,
                    }));
                },
                onmouseup: move |event: MouseEvent| {
                    event.stop_propagation();
                    finish_group_resize(workflow, message, undo_stack, group_resize_state);
                }
            }
        }
    }
}

#[component]
fn NodeCard(
    node: WorkflowNode,
    locked: bool,
    mut workflow: Signal<WorkflowFile>,
    mut json_text: Signal<String>,
    mut message: Signal<Message>,
    mut undo_stack: Signal<WorkflowUndoStack>,
    drag_state: Signal<Option<DragState>>,
    viewport: Signal<CanvasViewport>,
    connection_draft: Signal<Option<ConnectionDraft>>,
    media_overlay: Signal<Option<MediaOverlay>>,
) -> Element {
    let style = format!(
        "left: {}px; top: {}px;",
        node.position.x.max(0.0),
        node.position.y.max(0.0)
    );
    let status = node.status();
    let status_class = format!("badge {}", status.label());
    let mut classes = vec!["node", node.node_type.class_name(), "draggable"];
    if node.selected.unwrap_or(false) {
        classes.push("selected");
    }
    if node.dragging.unwrap_or(false) {
        classes.push("dragging");
    }
    if locked {
        classes.push("locked");
    }
    let node_class = classes.join(" ");
    let label = node.display_label();
    let preview = node.preview_text();
    let media_previews = media_previews_for_node(&node);
    let node_insight = node_card_insight(&node);
    let node_id = node.id.clone();
    let source_handles = source_handle_options(&node);
    let target_handles = target_handle_options(&node);
    let draft = connection_draft.read().clone();

    rsx! {
        article {
            class: "{node_class}",
            style: "{style}",
            onmousedown: move |event: MouseEvent| {
                event.stop_propagation();
                let node_id = node_id.clone();
                begin_node_drag(
                    event,
                    &node_id,
                    NodeDragSignals {
                        workflow,
                        json_text,
                        undo_stack,
                        drag_state,
                        viewport,
                        message,
                    },
                );
            },
            div { class: "handle-column target",
                for handle in target_handles.iter() {
                    {
                        let target_node_id = node.id.clone();
                        let target_handle = handle.id.clone();
                        let title = format!("Target: {}", handle.label);
                        let is_ready = draft
                            .as_ref()
                            .is_some_and(|draft| draft.source_node_id != target_node_id);
                        let class = if is_ready {
                            "handle-button target ready"
                        } else {
                            "handle-button target"
                        };
                        rsx! {
                            button {
                                class: "{class}",
                                title: "{title}",
                                onmousedown: move |event: MouseEvent| {
                                    event.stop_propagation();
                                },
                                onmouseup: move |event: MouseEvent| {
                                    event.stop_propagation();
                                    finish_handle_connection(
                                        &target_node_id,
                                        &target_handle,
                                        &mut workflow,
                                        &mut json_text,
                                        &mut message,
                                        &mut undo_stack,
                                        connection_draft,
                                    );
                                },
                                span { class: "handle-dot" }
                                span { class: "handle-label", "{handle.label}" }
                            }
                        }
                    }
                }
            }
            div { class: "handle-column source",
                for handle in source_handles.iter() {
                    {
                        let source_node_id = node.id.clone();
                        let source_handle = handle.id.clone();
                        let title = format!("Source: {}", handle.label);
                        let is_pending = draft
                            .as_ref()
                            .is_some_and(|draft| {
                                draft.source_node_id == source_node_id
                                    && draft.source_handle == source_handle
                            });
                        let class = if is_pending {
                            "handle-button source pending"
                        } else {
                            "handle-button source"
                        };
                        rsx! {
                            button {
                                class: "{class}",
                                title: "{title}",
                                onmousedown: move |event: MouseEvent| {
                                    event.stop_propagation();
                                    begin_handle_connection(
                                        &source_node_id,
                                        &source_handle,
                                        &mut message,
                                        connection_draft,
                                    );
                                },
                                onmouseup: move |event: MouseEvent| {
                                    event.stop_propagation();
                                },
                                span { class: "handle-dot" }
                                span { class: "handle-label", "{handle.label}" }
                            }
                        }
                    }
                }
            }
            div { class: "node-head",
                div { class: "node-title", "{label}" }
                span { class: "{status_class}", "{status.label()}" }
            }
            div { class: "node-body",
                div { "{node.node_type.title()}" }
                if let Some(text) = preview {
                    p { "{text}" }
                }
                if let Some(insight) = node_insight {
                    NodeInsightCard { insight }
                }
                MediaPreviewStrip { previews: media_previews, media_overlay }
                div { class: "node-id", "{node.id}" }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NodeInsight {
    class: &'static str,
    title: String,
    lines: Vec<String>,
}

#[component]
fn NodeInsightCard(insight: NodeInsight) -> Element {
    rsx! {
        div { class: "{insight.class}",
            div { class: "node-insight-title", "{insight.title}" }
            for line in insight.lines.iter() {
                p { "{line}" }
            }
        }
    }
}

fn node_card_insight(node: &WorkflowNode) -> Option<NodeInsight> {
    match node.node_type {
        NodeType::VideoFrameGrab => Some(video_frame_grab_insight(node)),
        NodeType::SplitGrid => split_grid_insight(node),
        NodeType::GlbViewer => Some(glb_viewer_insight(node)),
        _ => None,
    }
}

fn video_frame_grab_insight(node: &WorkflowNode) -> NodeInsight {
    if let Some(plan) = node.data.get("frameGrabPlan").and_then(Value::as_object) {
        let captured_output = node
            .data
            .get("outputImage")
            .and_then(Value::as_str)
            .filter(|value| value.starts_with("data:image/"));
        let capture_result = node
            .data
            .get("frameCaptureResult")
            .and_then(Value::as_object);
        let source = plan.get("source").unwrap_or(&Value::Null);
        let uri_kind = source
            .get("uriKind")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let mime = source
            .get("mime")
            .and_then(Value::as_str)
            .unwrap_or("unknown MIME");
        let size = source
            .get("byteLength")
            .and_then(Value::as_u64)
            .map(|bytes| format!(" · {}", human_media_bytes(bytes)));
        let renderable = source
            .get("renderableInWebview")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let frame_position = plan
            .get("framePosition")
            .and_then(Value::as_str)
            .unwrap_or("first");
        let seek_requires_duration = plan
            .get("seekRequiresDuration")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let seek_line = if seek_requires_duration {
            format!("seek: {frame_position} frame waits for duration metadata")
        } else if let Some(seconds) = plan.get("requestedSeekSeconds").and_then(Value::as_f64) {
            format!("seek: {frame_position} frame at {seconds:.3}s")
        } else {
            format!("seek: {frame_position} frame")
        };
        let preview_line = if renderable {
            "preview: source can be handed to the WebView player".to_string()
        } else {
            "preview: source must be hydrated or handled by a platform adapter".to_string()
        };
        let requires_decode = plan
            .get("requiresDecodeAdapter")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let mut lines = vec![
            format!("source: {uri_kind} · {mime}{}", size.unwrap_or_default()),
            seek_line,
            preview_line,
        ];
        if captured_output.is_some() {
            let adapter = capture_result
                .and_then(|result| result.get("adapter"))
                .and_then(Value::as_str)
                .unwrap_or("webview-video-canvas");
            let dimensions = capture_result.and_then(|result| {
                let width = result.get("width").and_then(Value::as_u64)?;
                let height = result.get("height").and_then(Value::as_u64)?;
                Some(format!(" · {width}×{height}"))
            });
            lines.push(format!(
                "capture: {adapter} emitted PNG output{}",
                dimensions.unwrap_or_default()
            ));
        } else if requires_decode {
            lines.push(
                "adapter: decode/canvas PNG capture pending; no output image emitted".to_string(),
            );
        }
        return NodeInsight {
            class: if captured_output.is_some() || !requires_decode {
                "node-insight ready"
            } else {
                "node-insight adapter"
            },
            title: "Frame grab plan".to_string(),
            lines,
        };
    }

    if let Some(error) = node
        .data
        .get("error")
        .and_then(Value::as_str)
        .filter(|error| !error.trim().is_empty())
    {
        return NodeInsight {
            class: "node-insight warn",
            title: "Frame grab plan".to_string(),
            lines: vec![format!("planning failed: {}", truncate_chars(error, 120))],
        };
    }

    NodeInsight {
        class: "node-insight adapter",
        title: "Frame grab plan".to_string(),
        lines: vec![
            "Run Local to inspect source/seek metadata.".to_string(),
            "PNG capture still needs a browser/native decode adapter.".to_string(),
        ],
    }
}

fn split_grid_insight(node: &WorkflowNode) -> Option<NodeInsight> {
    let images = node
        .data
        .get("images")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let populated = images
        .iter()
        .filter(|image| image.as_str().is_some_and(|value| !value.trim().is_empty()))
        .count();
    let children = node
        .data
        .get("childNodeIds")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let adapter = node
        .data
        .get("__mediaAdapter")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty());

    if populated == 0 && children == 0 && adapter.is_none() {
        return None;
    }

    let target_count = node
        .data
        .get("targetCount")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or_else(|| images.len().max(populated));
    let mut lines = vec![format!(
        "cells: {populated}/{} populated",
        target_count.max(populated)
    )];
    if populated > 0 {
        lines.push(format!(
            "routing: image-0{} handles available",
            if populated > 1 {
                format!("..image-{}", populated - 1)
            } else {
                String::new()
            }
        ));
    } else {
        lines.push("Run Local to populate split cell images.".to_string());
    }
    if children > 0 {
        lines.push(format!("children: {children} generated cell set(s)"));
    }
    if let Some(adapter) = adapter {
        lines.push(format!("adapter: {adapter}"));
    }

    Some(NodeInsight {
        class: if populated > 0 {
            "node-insight ready"
        } else {
            "node-insight adapter"
        },
        title: "Split grid cells".to_string(),
        lines,
    })
}

fn glb_viewer_insight(node: &WorkflowNode) -> NodeInsight {
    if let Some(plan) = node.data.get("glbViewerPlan").and_then(Value::as_object) {
        let captured_output = node
            .data
            .get("capturedImage")
            .and_then(Value::as_str)
            .filter(|value| value.starts_with("data:image/png;base64,"));
        let capture_result = node.data.get("glbCaptureResult").and_then(Value::as_object);
        let source = plan.get("source").unwrap_or(&Value::Null);
        let uri_kind = source
            .get("uriKind")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let mime = source
            .get("mime")
            .and_then(Value::as_str)
            .unwrap_or("unknown MIME");
        let size = source
            .get("byteLength")
            .and_then(Value::as_u64)
            .map(|bytes| format!(" · {}", human_media_bytes(bytes)));
        let renderable = source
            .get("renderableInWebview")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let filename = plan
            .get("filename")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(|value| format!("file: {value}"));
        let can_open = plan
            .get("canOpenUriDirectly")
            .and_then(Value::as_bool)
            .unwrap_or(renderable);
        let requires_webgl = plan
            .get("requiresWebglAdapter")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let requires_capture = plan
            .get("requiresCaptureAdapter")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let mut lines = vec![format!(
            "source: {uri_kind} · {mime}{}",
            size.unwrap_or_default()
        )];
        if let Some(filename) = filename {
            lines.push(filename);
        }
        if let Some(metadata) = plan.get("metadata") {
            lines.extend(glb_metadata_lines(metadata));
        }
        lines.push(if can_open {
            "preview: WebView model-viewer adapter can open this URI".to_string()
        } else {
            "preview: project media must be hydrated before model-viewer handoff".to_string()
        });
        if requires_webgl {
            lines.push("adapter: model URI hydration/WebGL handoff pending".to_string());
        }
        if captured_output.is_some() {
            let adapter = capture_result
                .and_then(|result| result.get("adapter"))
                .and_then(Value::as_str)
                .unwrap_or("webview-model-viewer");
            let dimensions = capture_result.and_then(|result| {
                let width = result.get("width").and_then(Value::as_u64)?;
                let height = result.get("height").and_then(Value::as_u64)?;
                Some(format!(" · {width}×{height}"))
            });
            lines.push(format!(
                "capture: {adapter} emitted PNG snapshot{}",
                dimensions.unwrap_or_default()
            ));
        } else if requires_capture {
            lines.push(
                "capture: PNG snapshot adapter pending; no captured image emitted".to_string(),
            );
        }
        return NodeInsight {
            class: if captured_output.is_some() || (!requires_webgl && !requires_capture) {
                "node-insight ready"
            } else if requires_webgl || requires_capture {
                "node-insight adapter"
            } else {
                "node-insight ready"
            },
            title: "GLB viewer plan".to_string(),
            lines,
        };
    }

    if let Some(error) = node
        .data
        .get("error")
        .and_then(Value::as_str)
        .filter(|error| !error.trim().is_empty())
    {
        return NodeInsight {
            class: "node-insight warn",
            title: "GLB viewer plan".to_string(),
            lines: vec![format!("planning failed: {}", truncate_chars(error, 120))],
        };
    }

    NodeInsight {
        class: "node-insight adapter",
        title: "GLB viewer plan".to_string(),
        lines: vec![
            "Run Local to inspect GLB URI metadata.".to_string(),
            "Renderable GLB URIs can preview in WebView and expose an opt-in model-viewer PNG capture action."
                .to_string(),
        ],
    }
}

fn glb_metadata_lines(metadata: &Value) -> Vec<String> {
    let Some(metadata) = metadata.as_object() else {
        return Vec::new();
    };
    let count = |field: &str| {
        metadata
            .get(field)
            .and_then(Value::as_u64)
            .unwrap_or_default()
    };

    let version = metadata
        .get("version")
        .and_then(Value::as_u64)
        .map(|version| format!("GLB v{version}"))
        .unwrap_or_else(|| "GLB version unknown".to_string());
    let declared = metadata
        .get("declaredByteLength")
        .and_then(Value::as_u64)
        .map(human_media_bytes)
        .unwrap_or_else(|| "unknown size".to_string());
    let json_chunk = metadata
        .get("jsonChunkByteLength")
        .and_then(Value::as_u64)
        .map(human_media_bytes)
        .unwrap_or_else(|| "unknown JSON chunk".to_string());
    let asset_version = metadata
        .get("assetVersion")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!(" · glTF {value}"))
        .unwrap_or_default();

    let mut lines = vec![
        format!("metadata: {version}{asset_version} · {declared} declared · JSON {json_chunk}"),
        format!(
            "assets: scenes {} · nodes {} · meshes {} · materials {} · animations {} · images {} · buffers {}",
            count("sceneCount"),
            count("nodeCount"),
            count("meshCount"),
            count("materialCount"),
            count("animationCount"),
            count("imageCount"),
            count("bufferCount"),
        ),
    ];
    if let Some(generator) = metadata
        .get("generator")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        lines.push(format!("generator: {}", truncate_chars(generator, 80)));
    }
    lines
}

fn human_media_bytes(bytes: u64) -> String {
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

fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let keep = max_chars.saturating_sub(1);
    format!("{}…", value.chars().take(keep).collect::<String>())
}

#[component]
fn MediaPreviewStrip(
    previews: Vec<MediaPreview>,
    media_overlay: Signal<Option<MediaOverlay>>,
) -> Element {
    let preview_count = previews.len();
    let extra_count = preview_count.saturating_sub(3);

    rsx! {
        if preview_count > 0 {
            div { class: "media-preview-list",
                for preview in previews.iter().take(3) {
                    MediaPreviewCard { preview: preview.clone(), media_overlay }
                }
                if extra_count > 0 {
                    div { class: "media-preview-placeholder",
                        "{extra_count} more media item(s) available in node data."
                    }
                }
            }
        }
    }
}

#[component]
fn MediaPreviewCard(
    preview: MediaPreview,
    mut media_overlay: Signal<Option<MediaOverlay>>,
) -> Element {
    let mut load_error = use_signal(|| false);
    let mut copy_status = use_signal(|| None::<CopyStatus>);
    let label = preview.label.clone();
    let kind_label = preview.kind.label();
    let kind_class = media_preview_kind_class(preview.kind);
    let uri = preview.uri.clone();
    let uri_hint = preview.uri_hint();
    let source_field = preview.source_field.clone();
    let renderable = preview.is_renderable_uri();
    let inline_preview = preview.should_inline_preview();
    let download_filename = preview.download_filename();
    let overlay = media_overlay_from_preview(&preview);
    let error_message = media_error_message(preview.kind);
    let copy_uri = uri.clone();
    let copy_status_snapshot = copy_status.read().clone();

    rsx! {
        div {
            class: "media-preview",
            onmousedown: move |event: MouseEvent| {
                event.stop_propagation();
            },
            onmouseup: move |event: MouseEvent| {
                event.stop_propagation();
            },
            div { class: "media-preview-head",
                span { "{label}" }
                span { class: "{kind_class}", "{kind_label}" }
            }
            if inline_preview && preview.kind == MediaKind::Image {
                img {
                    src: "{uri}",
                    alt: "{label}",
                    loading: "lazy",
                    onload: move |_| {
                        load_error.set(false);
                    },
                    onerror: move |_| {
                        load_error.set(true);
                    },
                }
            } else if inline_preview && preview.kind == MediaKind::Video {
                video {
                    src: "{uri}",
                    controls: true,
                    preload: "metadata",
                    onloadedmetadata: move |_| {
                        load_error.set(false);
                    },
                    onerror: move |_| {
                        load_error.set(true);
                    },
                }
            } else if inline_preview && preview.kind == MediaKind::Audio {
                audio {
                    src: "{uri}",
                    controls: true,
                    preload: "metadata",
                    onloadedmetadata: move |_| {
                        load_error.set(false);
                    },
                    onerror: move |_| {
                        load_error.set(true);
                    },
                }
            } else if renderable && preview.is_large_inline() {
                div { class: "media-preview-placeholder",
                    "Large inline media detected. Use Open or Download instead of rendering it inside the node card."
                }
            } else if inline_preview && preview.kind == MediaKind::Model3d {
                {
                    let srcdoc = glb_model_viewer_srcdoc(&uri, &label);
                    rsx! {
                        iframe {
                            class: "media-preview-model",
                            title: "{label} GLB preview",
                            srcdoc: "{srcdoc}",
                            allow: "fullscreen; xr-spatial-tracking",
                            onload: move |_| {
                                load_error.set(false);
                            },
                        }
                    }
                }
            } else if preview.kind == MediaKind::Model3d {
                div { class: "media-preview-placeholder",
                    "3D model reference detected. Project refs must be hydrated before the WebView model-viewer preview can load."
                }
            } else {
                div { class: "media-preview-placeholder",
                    "Media reference detected, but this URI must be hydrated or handled by a platform adapter before inline preview."
                }
            }
            if inline_preview && load_error() {
                div { class: "media-preview-error",
                    "{error_message}"
                }
            }
            div { class: "media-preview-actions",
                if let Some(overlay) = overlay.clone() {
                    button {
                        class: "media-preview-link",
                        onclick: move |event: MouseEvent| {
                            event.stop_propagation();
                            media_overlay.set(Some(overlay.clone()));
                        },
                        "Preview"
                    }
                }
                if renderable {
                    a {
                        class: "media-preview-link",
                        href: "{uri}",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        onclick: move |event: MouseEvent| {
                            event.stop_propagation();
                        },
                        "Open"
                    }
                    a {
                        class: "media-preview-link",
                        href: "{uri}",
                        download: "{download_filename}",
                        onclick: move |event: MouseEvent| {
                            event.stop_propagation();
                        },
                        "Download"
                    }
                }
                button {
                    class: "media-preview-link",
                    onclick: move |event: MouseEvent| {
                        event.stop_propagation();
                        copy_status.set(Some(CopyStatus::copying()));
                        let uri = copy_uri.clone();
                        async move {
                            let status = copy_media_uri(uri).await;
                            copy_status.set(Some(status));
                        }
                    },
                    "Copy URI"
                }
            }
            if let Some(status) = copy_status_snapshot.as_ref() {
                div {
                    class: "{status.class_name(\"media-copy-status\")}",
                    "{status.message}"
                }
            }
            p { class: "media-preview-hint",
                "{source_field} · {uri_hint}"
            }
        }
    }
}

#[component]
fn MediaOverlayLayer(mut media_overlay: Signal<Option<MediaOverlay>>) -> Element {
    let snapshot = media_overlay.read().clone();
    let mut load_error = use_signal(|| false);
    let mut copy_status = use_signal(|| None::<CopyStatus>);
    let overlay_error_message = snapshot
        .as_ref()
        .map(|overlay| {
            format!(
                "{} Try Open or Download to inspect the source directly.",
                media_error_message(overlay.kind)
            )
        })
        .unwrap_or_default();
    let overlay_copy_uri = snapshot
        .as_ref()
        .map(|overlay| overlay.uri.clone())
        .unwrap_or_default();
    let overlay_copy_status_snapshot = copy_status.read().clone();

    rsx! {
        if let Some(overlay) = snapshot {
            div {
                class: "media-overlay-backdrop",
                onclick: move |_| {
                    load_error.set(false);
                    copy_status.set(None);
                    media_overlay.set(None);
                },
                div {
                    class: "media-overlay-panel",
                    onclick: move |event: MouseEvent| {
                        event.stop_propagation();
                    },
                    div { class: "media-overlay-head",
                        div { class: "media-overlay-title",
                            span { "{overlay.label}" }
                            span { class: "{media_preview_kind_class(overlay.kind)}", "{overlay.kind.label()}" }
                        }
                        div { class: "media-overlay-actions",
                            a {
                                class: "media-preview-link",
                                href: "{overlay.uri}",
                                target: "_blank",
                                rel: "noopener noreferrer",
                                onclick: move |event: MouseEvent| {
                                    event.stop_propagation();
                                },
                                "Open"
                            }
                            a {
                                class: "media-preview-link",
                                href: "{overlay.uri}",
                                download: "{overlay.download_filename}",
                                onclick: move |event: MouseEvent| {
                                    event.stop_propagation();
                                },
                                "Download"
                            }
                            button {
                                class: "media-preview-link",
                                onclick: move |event: MouseEvent| {
                                    event.stop_propagation();
                                    copy_status.set(Some(CopyStatus::copying()));
                                    let uri = overlay_copy_uri.clone();
                                    async move {
                                        let status = copy_media_uri(uri).await;
                                        copy_status.set(Some(status));
                                    }
                                },
                                "Copy URI"
                            }
                            button {
                                class: "media-preview-link",
                                onclick: move |event: MouseEvent| {
                                    event.stop_propagation();
                                    load_error.set(false);
                                    copy_status.set(None);
                                    media_overlay.set(None);
                                },
                                "Close"
                            }
                        }
                    }
                    div { class: "media-overlay-body",
                        if overlay.kind == MediaKind::Image {
                            img {
                                class: "media-overlay-image",
                                src: "{overlay.uri}",
                                alt: "{overlay.label}",
                                onload: move |_| {
                                    load_error.set(false);
                                },
                                onerror: move |_| {
                                    load_error.set(true);
                                },
                            }
                        } else if overlay.kind == MediaKind::Video {
                            video {
                                class: "media-overlay-video",
                                src: "{overlay.uri}",
                                controls: true,
                                preload: "metadata",
                                onloadedmetadata: move |_| {
                                    load_error.set(false);
                                },
                                onerror: move |_| {
                                    load_error.set(true);
                                },
                            }
                        } else if overlay.kind == MediaKind::Audio {
                            div { class: "media-overlay-audio-shell",
                                p { "Audio preview" }
                                audio {
                                    class: "media-overlay-audio",
                                    src: "{overlay.uri}",
                                    controls: true,
                                    preload: "metadata",
                                    onloadedmetadata: move |_| {
                                        load_error.set(false);
                                    },
                                    onerror: move |_| {
                                        load_error.set(true);
                                    },
                                }
                            }
                        } else if overlay.kind == MediaKind::Model3d {
                            {
                                let srcdoc = glb_model_viewer_srcdoc(&overlay.uri, &overlay.label);
                                rsx! {
                                    iframe {
                                        class: "media-overlay-model",
                                        title: "{overlay.label} GLB preview",
                                        srcdoc: "{srcdoc}",
                                        allow: "fullscreen; xr-spatial-tracking",
                                        onload: move |_| {
                                            load_error.set(false);
                                        },
                                    }
                                }
                            }
                        } else {
                            div { class: "media-overlay-placeholder",
                                "No inline overlay adapter is available for this media kind yet."
                            }
                        }
                        if matches!(overlay.kind, MediaKind::Image | MediaKind::Audio | MediaKind::Video | MediaKind::Model3d) && load_error() {
                            div { class: "media-overlay-error",
                                "{overlay_error_message}"
                            }
                        }
                    }
                    div { class: "media-overlay-meta",
                        "{overlay.source_field} · {overlay.uri_hint}"
                    }
                    if let Some(status) = overlay_copy_status_snapshot.as_ref() {
                        div {
                            class: "{status.class_name(\"media-overlay-copy-status\")}",
                            "{status.message}"
                        }
                    }
                }
            }
        }
    }
}

fn media_overlay_from_preview(preview: &MediaPreview) -> Option<MediaOverlay> {
    if !matches!(
        preview.kind,
        MediaKind::Image | MediaKind::Audio | MediaKind::Video | MediaKind::Model3d
    ) || !preview.should_inline_preview()
    {
        return None;
    }

    Some(MediaOverlay {
        kind: preview.kind,
        label: preview.label.clone(),
        uri: preview.uri.clone(),
        uri_hint: preview.uri_hint(),
        source_field: preview.source_field.clone(),
        download_filename: preview.download_filename(),
    })
}

fn media_preview_kind_class(kind: MediaKind) -> &'static str {
    match kind {
        MediaKind::Image => "media-preview-kind image",
        MediaKind::Audio => "media-preview-kind audio",
        MediaKind::Video => "media-preview-kind video",
        MediaKind::Model3d => "media-preview-kind model3d",
    }
}

fn media_error_message(kind: MediaKind) -> &'static str {
    match kind {
        MediaKind::Image => {
            "Image preview failed to load. The URI may be missing, blocked, or unsupported."
        }
        MediaKind::Audio => {
            "Audio preview failed to load. The URI may be missing, blocked, or unsupported."
        }
        MediaKind::Video => {
            "Video preview failed to load. The URI may be missing, blocked, or unsupported."
        }
        MediaKind::Model3d => {
            "3D preview failed to load. The GLB URI may need project hydration or WebView/WebGL support."
        }
    }
}

fn glb_model_viewer_srcdoc(uri: &str, label: &str) -> String {
    let uri = html_attr_escape(uri);
    let label = html_attr_escape(label);
    let local_module_url = html_attr_escape(MODEL_VIEWER_LOCAL_MODULE_URL);
    let fallback_module_url = html_attr_escape(MODEL_VIEWER_CDN_MODULE_URL);
    format!(
        r#"<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <style>
    html, body {{
      width: 100%;
      height: 100%;
      margin: 0;
      background: #020617;
      color: #dbeafe;
      overflow: hidden;
      font-family: Inter, ui-sans-serif, system-ui, sans-serif;
    }}
    model-viewer {{
      width: 100%;
      height: 100%;
      min-height: 100%;
      --poster-color: #020617;
      background: radial-gradient(circle at 30% 20%, rgba(96, 165, 250, .20), transparent 34%), #020617;
    }}
    .poster, .fallback {{
      position: absolute;
      inset: auto .7rem .65rem .7rem;
      padding: .42rem .5rem;
      border-radius: .55rem;
      border: 1px solid rgba(148, 163, 184, .22);
      background: rgba(15, 23, 42, .78);
      color: #bfdbfe;
      font-size: 12px;
      line-height: 1.35;
      text-align: center;
    }}
  </style>
  <script type="module">
    const localModuleUrl = "{local_module_url}";
    const fallbackModuleUrl = "{fallback_module_url}";
    const loadModule = (src) => new Promise((resolve, reject) => {{
      const script = document.createElement("script");
      script.type = "module";
      script.src = src;
      script.onload = () => resolve(src);
      script.onerror = () => reject(new Error(`Failed to load ${{src}}`));
      document.head.appendChild(script);
    }});
    if (!customElements.get("model-viewer")) {{
      loadModule(localModuleUrl).catch(() => loadModule(fallbackModuleUrl)).catch((error) => {{
        const fallback = document.querySelector(".fallback");
        if (fallback) {{
          fallback.textContent = String(error && (error.message || error));
        }}
      }});
    }}
  </script>
</head>
<body>
  <model-viewer
    src="{uri}"
    alt="{label}"
    camera-controls
    auto-rotate
    interaction-prompt="auto"
    shadow-intensity="0.7"
    exposure="1"
    loading="eager"
    reveal="auto">
    <div slot="poster" class="poster">Loading GLB preview...</div>
    <div class="fallback">If the WebGL viewer does not load, use Open or Download.</div>
  </model-viewer>
</body>
</html>"#
    )
}

fn html_attr_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

fn copy_media_uri_script(uri: &str) -> String {
    let uri = serde_json::to_string(uri).unwrap_or_else(|_| "\"\"".to_string());
    format!(
        r#"
const text = {uri};
try {{
    if (navigator.clipboard && navigator.clipboard.writeText) {{
        await navigator.clipboard.writeText(text);
        return {{ ok: true }};
    }}

    const textarea = document.createElement("textarea");
    textarea.value = text;
    textarea.setAttribute("readonly", "");
    textarea.style.position = "fixed";
    textarea.style.left = "-9999px";
    textarea.style.top = "0";
    document.body.appendChild(textarea);
    textarea.focus();
    textarea.select();
    const copied = document.execCommand("copy");
    textarea.remove();
    return copied
        ? {{ ok: true }}
        : {{ ok: false, error: "document.execCommand('copy') returned false" }};
}} catch (error) {{
    return {{ ok: false, error: String(error && (error.message || error)) }};
}}
"#
    )
}

async fn copy_media_uri(uri: String) -> CopyStatus {
    if uri.trim().is_empty() {
        return CopyStatus::failed("Copy failed: media URI is empty.");
    }

    let script = copy_media_uri_script(&uri);
    match document::eval(&script).await {
        Ok(value) => CopyStatus::from_eval_value(&value),
        Err(err) => CopyStatus::failed(format!("Copy unavailable: {err}")),
    }
}

#[derive(Debug, Clone, PartialEq)]
struct GlbCaptureRequest {
    node_id: String,
    source_uri: String,
    label: String,
    timeout_ms: u32,
}

impl GlbCaptureRequest {
    fn source_summary(&self) -> String {
        format!("model-viewer PNG · {}", truncate_chars(&self.label, 48))
    }
}

#[derive(Debug, Clone, PartialEq)]
struct GlbCaptureSuccess {
    image: String,
    width: Option<u64>,
    height: Option<u64>,
}

fn glb_capture_request(node_id: String, node: &WorkflowNode) -> Result<GlbCaptureRequest, String> {
    let source_uri = node
        .data
        .get("glbUrl")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| "Run Local first so glbViewerPlan records the source GLB.".to_string())?;
    let plan = node
        .data
        .get("glbViewerPlan")
        .and_then(Value::as_object)
        .ok_or_else(|| "Run Local first so glbViewerPlan records GLB metadata.".to_string())?;
    let source = plan
        .get("source")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "glbViewerPlan is missing source metadata; run local planning again.".to_string()
        })?;
    let renderable = source
        .get("renderableInWebview")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !renderable {
        let uri_kind = source
            .get("uriKind")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        return Err(format!(
            "source `{uri_kind}` must be hydrated to a WebView-renderable URI before capture"
        ));
    }

    let label = node
        .data
        .get("filename")
        .and_then(Value::as_str)
        .or_else(|| node.data.get("label").and_then(Value::as_str))
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("GLB snapshot")
        .to_string();

    Ok(GlbCaptureRequest {
        node_id,
        source_uri,
        label,
        timeout_ms: 45_000,
    })
}

fn glb_capture_script(request: &GlbCaptureRequest) -> String {
    let source_uri =
        serde_json::to_string(&request.source_uri).unwrap_or_else(|_| "\"\"".to_string());
    let label = serde_json::to_string(&request.label).unwrap_or_else(|_| "\"GLB\"".to_string());
    let local_module_url =
        serde_json::to_string(MODEL_VIEWER_LOCAL_MODULE_URL).unwrap_or_else(|_| "\"\"".to_string());
    let fallback_module_url =
        serde_json::to_string(MODEL_VIEWER_CDN_MODULE_URL).unwrap_or_else(|_| "\"\"".to_string());
    let timeout_ms = request.timeout_ms;

    format!(
        r#"
const sourceUri = {source_uri};
const label = {label};
const localModuleUrl = {local_module_url};
const fallbackModuleUrl = {fallback_module_url};
const timeoutMs = {timeout_ms};
let objectUrl = null;
let model = null;
let timeoutId = null;

const cleanup = () => {{
    if (timeoutId !== null) {{
        clearTimeout(timeoutId);
        timeoutId = null;
    }}
    if (model) {{
        model.remove();
        model = null;
    }}
    if (objectUrl !== null) {{
        URL.revokeObjectURL(objectUrl);
        objectUrl = null;
    }}
}};

try {{
    const result = await new Promise((resolve, reject) => {{
        const fail = (error) => reject(error instanceof Error ? error : new Error(String(error)));
        timeoutId = setTimeout(() => fail(new Error("GLB snapshot timed out")), timeoutMs);

        const start = () => {{
            model = document.createElement("model-viewer");
            model.style.position = "fixed";
            model.style.left = "-10000px";
            model.style.top = "0";
            model.style.width = "640px";
            model.style.height = "480px";
            model.style.pointerEvents = "none";
            model.setAttribute("alt", label);
            model.setAttribute("camera-controls", "");
            model.setAttribute("reveal", "auto");
            model.setAttribute("loading", "eager");
            model.setAttribute("shadow-intensity", "0.7");
            model.setAttribute("exposure", "1");
            model.addEventListener("error", () => fail(new Error("model-viewer failed to load GLB")), {{ once: true }});
            model.addEventListener("load", async () => {{
                try {{
                    if (typeof model.toDataURL !== "function") {{
                        throw new Error("model-viewer toDataURL API is unavailable");
                    }}
                    if (typeof model.updateComplete !== "undefined") {{
                        await model.updateComplete;
                    }}
                    await new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done)));
                    const image = await model.toDataURL("image/png");
                    resolve({{
                        ok: true,
                        image,
                        width: model.clientWidth || 640,
                        height: model.clientHeight || 480,
                        adapter: "webview-model-viewer"
                    }});
                }} catch (error) {{
                    fail(error);
                }}
            }}, {{ once: true }});
            document.body.appendChild(model);

            if (sourceUri.startsWith("data:")) {{
                fetch(sourceUri)
                    .then((response) => response.blob())
                    .then((blob) => {{
                        objectUrl = URL.createObjectURL(blob);
                        model.src = objectUrl;
                    }})
                    .catch(() => {{
                        model.src = sourceUri;
                    }});
            }} else {{
                model.src = sourceUri;
            }}
        }};

        const loadModelViewerModule = (src) => new Promise((resolve, reject) => {{
            const script = document.createElement("script");
            script.type = "module";
            script.src = src;
            script.onload = () => resolve(src);
            script.onerror = () => reject(new Error(`Failed to load model-viewer module: ${{src}}`));
            document.head.appendChild(script);
        }});

        if (customElements.get("model-viewer")) {{
            start();
        }} else {{
            loadModelViewerModule(localModuleUrl)
                .catch(() => loadModelViewerModule(fallbackModuleUrl))
                .then(() => start())
                .catch((error) => fail(error));
        }}
    }});
    cleanup();
    return result;
}} catch (error) {{
    cleanup();
    return {{ ok: false, error: String(error && (error.message || error)), adapter: "webview-model-viewer" }};
}}
"#
    )
}

fn glb_capture_success_from_eval_value(value: &Value) -> Result<GlbCaptureSuccess, String> {
    if !value.get("ok").and_then(Value::as_bool).unwrap_or(false) {
        return Err(value
            .get("error")
            .and_then(Value::as_str)
            .filter(|error| !error.trim().is_empty())
            .unwrap_or("GLB capture adapter returned an unknown error")
            .to_string());
    }
    let image = value
        .get("image")
        .and_then(Value::as_str)
        .filter(|image| image.starts_with("data:image/png;base64,"))
        .ok_or_else(|| "GLB capture adapter did not return a PNG data URL".to_string())?;
    Ok(GlbCaptureSuccess {
        image: image.to_string(),
        width: value.get("width").and_then(Value::as_u64),
        height: value.get("height").and_then(Value::as_u64),
    })
}

async fn capture_glb_snapshot_with_webview_adapter(
    node_id: String,
    mut workflow: Signal<WorkflowFile>,
    mut json_text: Signal<String>,
    mut message: Signal<Message>,
    mut execution_report: Signal<Option<SimpleExecutionReport>>,
) {
    let request = {
        let snapshot = workflow.read();
        let Some(node) = snapshot.nodes.iter().find(|node| node.id == node_id) else {
            message.set(Message::err(format!("GLB Viewer `{node_id}` disappeared.")));
            return;
        };
        match glb_capture_request(node_id.clone(), node) {
            Ok(request) => request,
            Err(err) => {
                message.set(Message::err(format!("GLB capture unavailable: {err}")));
                return;
            }
        }
    };
    message.set(Message::ok(format!(
        "Capturing `{}` via WebView model-viewer adapter...",
        request.node_id
    )));

    let script = glb_capture_script(&request);
    let eval_value = match document::eval(&script).await {
        Ok(value) => value,
        Err(err) => {
            message.set(Message::err(format!("GLB capture eval failed: {err}")));
            return;
        }
    };
    let success = match glb_capture_success_from_eval_value(&eval_value) {
        Ok(success) => success,
        Err(err) => {
            message.set(Message::err(format!("GLB capture failed: {err}")));
            return;
        }
    };

    let mut next = workflow.read().clone();
    match apply_glb_capture_success(&mut next, &request.node_id, &success) {
        Ok(routed_count) => match next.to_pretty_json() {
            Ok(json) => {
                workflow.set(next);
                json_text.set(json);
                execution_report.set(None);
                let size = match (success.width, success.height) {
                    (Some(width), Some(height)) => format!(" · {width}×{height}"),
                    _ => String::new(),
                };
                message.set(Message::ok(format!(
                    "Captured GLB snapshot for `{}`{size}; routed to {routed_count} downstream image output node(s).",
                    request.node_id
                )));
            }
            Err(err) => message.set(Message::err(format!(
                "Captured GLB snapshot but failed to export JSON: {err}"
            ))),
        },
        Err(err) => message.set(Message::err(err)),
    }
}

#[derive(Debug, Clone, PartialEq)]
struct VideoFrameCaptureRequest {
    node_id: String,
    source_uri: String,
    frame_position: String,
    requested_seek_seconds: Option<f64>,
    timeout_ms: u32,
}

impl VideoFrameCaptureRequest {
    fn source_summary(&self) -> String {
        let seek = self
            .requested_seek_seconds
            .map(|seconds| format!("{seconds:.3}s"))
            .unwrap_or_else(|| "duration-based".to_string());
        format!("{} frame · seek {seek}", self.frame_position)
    }
}

#[derive(Debug, Clone, PartialEq)]
struct VideoFrameCaptureSuccess {
    image: String,
    width: Option<u64>,
    height: Option<u64>,
    seek_seconds: Option<f64>,
}

fn video_frame_capture_request(
    node_id: String,
    node: &WorkflowNode,
) -> Result<VideoFrameCaptureRequest, String> {
    let source_uri = node
        .data
        .get("sourceVideo")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| "Run Local first so frameGrabPlan records the source video.".to_string())?;
    let plan = node
        .data
        .get("frameGrabPlan")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "Run Local first so frameGrabPlan records source/seek metadata.".to_string()
        })?;
    let source = plan
        .get("source")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "frameGrabPlan is missing source metadata; run local planning again.".to_string()
        })?;
    let renderable = source
        .get("renderableInWebview")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !renderable {
        let uri_kind = source
            .get("uriKind")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        return Err(format!(
            "source `{uri_kind}` must be hydrated to a WebView-renderable URI before capture"
        ));
    }
    let frame_position = plan
        .get("framePosition")
        .and_then(Value::as_str)
        .or_else(|| node.data.get("framePosition").and_then(Value::as_str))
        .unwrap_or("first")
        .to_string();
    let requested_seek_seconds = plan.get("requestedSeekSeconds").and_then(Value::as_f64);

    Ok(VideoFrameCaptureRequest {
        node_id,
        source_uri,
        frame_position,
        requested_seek_seconds,
        timeout_ms: 30_000,
    })
}

fn video_frame_capture_script(request: &VideoFrameCaptureRequest) -> String {
    let source_uri =
        serde_json::to_string(&request.source_uri).unwrap_or_else(|_| "\"\"".to_string());
    let frame_position =
        serde_json::to_string(&request.frame_position).unwrap_or_else(|_| "\"first\"".to_string());
    let requested_seek_seconds = request
        .requested_seek_seconds
        .map_or_else(|| "null".to_string(), |seconds| seconds.to_string());
    let timeout_ms = request.timeout_ms;

    format!(
        r#"
const sourceUri = {source_uri};
const framePosition = {frame_position};
const requestedSeekSeconds = {requested_seek_seconds};
const timeoutMs = {timeout_ms};
let blobUrl = null;
let timeoutId = null;
const video = document.createElement("video");
video.crossOrigin = "anonymous";
video.preload = "auto";
video.muted = true;
video.playsInline = true;

const cleanup = () => {{
    if (timeoutId !== null) {{
        clearTimeout(timeoutId);
        timeoutId = null;
    }}
    if (blobUrl !== null) {{
        URL.revokeObjectURL(blobUrl);
        blobUrl = null;
    }}
    video.removeAttribute("src");
    try {{ video.load(); }} catch (_) {{}}
}};

try {{
    const result = await new Promise((resolve, reject) => {{
        const fail = (error) => reject(error instanceof Error ? error : new Error(String(error)));
        timeoutId = setTimeout(() => fail(new Error("Frame extraction timed out")), timeoutMs);

        video.onloadedmetadata = () => {{
            const duration = Number.isFinite(video.duration) ? video.duration : null;
            const seekSeconds = requestedSeekSeconds !== null
                ? requestedSeekSeconds
                : (framePosition === "last" && duration !== null ? Math.max(0, duration - 0.1) : 0.001);
            video.currentTime = seekSeconds;
        }};

        video.onseeked = () => {{
            try {{
                const width = video.videoWidth;
                const height = video.videoHeight;
                if (!width || !height) {{
                    throw new Error("Video metadata did not expose frame dimensions");
                }}
                const canvas = document.createElement("canvas");
                canvas.width = width;
                canvas.height = height;
                const context = canvas.getContext("2d");
                if (!context) {{
                    throw new Error("Could not get canvas 2D context");
                }}
                context.drawImage(video, 0, 0, width, height);
                const image = canvas.toDataURL("image/png");
                resolve({{
                    ok: true,
                    image,
                    width,
                    height,
                    seekSeconds: video.currentTime,
                    duration: Number.isFinite(video.duration) ? video.duration : null,
                    adapter: "webview-video-canvas"
                }});
            }} catch (error) {{
                fail(error);
            }}
        }};

        video.onerror = () => fail(new Error("Failed to load video for frame extraction"));

        if (sourceUri.startsWith("data:")) {{
            fetch(sourceUri)
                .then((response) => response.blob())
                .then((blob) => {{
                    blobUrl = URL.createObjectURL(blob);
                    video.src = blobUrl;
                }})
                .catch(() => {{
                    video.src = sourceUri;
                }});
        }} else {{
            video.src = sourceUri;
        }}
    }});
    cleanup();
    return result;
}} catch (error) {{
    cleanup();
    return {{ ok: false, error: String(error && (error.message || error)), adapter: "webview-video-canvas" }};
}}
"#
    )
}

fn video_frame_capture_success_from_eval_value(
    value: &Value,
) -> Result<VideoFrameCaptureSuccess, String> {
    if !value.get("ok").and_then(Value::as_bool).unwrap_or(false) {
        return Err(value
            .get("error")
            .and_then(Value::as_str)
            .filter(|error| !error.trim().is_empty())
            .unwrap_or("frame capture adapter returned an unknown error")
            .to_string());
    }
    let image = value
        .get("image")
        .and_then(Value::as_str)
        .filter(|image| image.starts_with("data:image/png;base64,"))
        .ok_or_else(|| "frame capture adapter did not return a PNG data URL".to_string())?;
    Ok(VideoFrameCaptureSuccess {
        image: image.to_string(),
        width: value.get("width").and_then(Value::as_u64),
        height: value.get("height").and_then(Value::as_u64),
        seek_seconds: value.get("seekSeconds").and_then(Value::as_f64),
    })
}

async fn capture_video_frame_with_webview_adapter(
    node_id: String,
    mut workflow: Signal<WorkflowFile>,
    mut json_text: Signal<String>,
    mut message: Signal<Message>,
    mut execution_report: Signal<Option<SimpleExecutionReport>>,
) {
    let request = {
        let snapshot = workflow.read();
        let Some(node) = snapshot.nodes.iter().find(|node| node.id == node_id) else {
            message.set(Message::err(format!(
                "Video Frame Grab `{node_id}` disappeared."
            )));
            return;
        };
        match video_frame_capture_request(node_id.clone(), node) {
            Ok(request) => request,
            Err(err) => {
                message.set(Message::err(format!("Frame capture unavailable: {err}")));
                return;
            }
        }
    };
    message.set(Message::ok(format!(
        "Capturing `{}` via WebView video/canvas adapter...",
        request.node_id
    )));

    let script = video_frame_capture_script(&request);
    let eval_value = match document::eval(&script).await {
        Ok(value) => value,
        Err(err) => {
            message.set(Message::err(format!("Frame capture eval failed: {err}")));
            return;
        }
    };
    let success = match video_frame_capture_success_from_eval_value(&eval_value) {
        Ok(success) => success,
        Err(err) => {
            message.set(Message::err(format!("Frame capture failed: {err}")));
            return;
        }
    };

    let mut next = workflow.read().clone();
    match apply_video_frame_capture_success(&mut next, &request.node_id, &success) {
        Ok(routed_count) => match next.to_pretty_json() {
            Ok(json) => {
                workflow.set(next);
                json_text.set(json);
                execution_report.set(None);
                let size = match (success.width, success.height) {
                    (Some(width), Some(height)) => format!(" · {width}×{height}"),
                    _ => String::new(),
                };
                message.set(Message::ok(format!(
                    "Captured frame for `{}`{size}; routed to {routed_count} downstream output node(s).",
                    request.node_id
                )));
            }
            Err(err) => message.set(Message::err(format!(
                "Captured frame but failed to export JSON: {err}"
            ))),
        },
        Err(err) => message.set(Message::err(err)),
    }
}

fn apply_video_frame_capture_success(
    workflow: &mut WorkflowFile,
    node_id: &str,
    success: &VideoFrameCaptureSuccess,
) -> Result<usize, String> {
    let Some(index) = workflow
        .nodes
        .iter()
        .position(|node| node.id == node_id && node.node_type == NodeType::VideoFrameGrab)
    else {
        return Err(format!("Video Frame Grab `{node_id}` was not found."));
    };
    let image = success.image.clone();
    set_node_data_field(
        &mut workflow.nodes[index],
        "outputImage",
        Value::String(image.clone()),
    );
    set_node_data_field(&mut workflow.nodes[index], "outputImageRef", Value::Null);
    set_node_data_field(
        &mut workflow.nodes[index],
        "frameCaptureResult",
        serde_json::json!({
            "adapter": "webview-video-canvas",
            "width": success.width,
            "height": success.height,
            "seekSeconds": success.seek_seconds,
            "outputMime": "image/png"
        }),
    );
    set_node_data_field(
        &mut workflow.nodes[index],
        "__mediaAdapter",
        Value::String("webview-video-canvas".to_string()),
    );
    set_node_data_field(
        &mut workflow.nodes[index],
        "status",
        Value::String("complete".to_string()),
    );
    set_node_data_field(&mut workflow.nodes[index], "error", Value::Null);

    route_captured_image_to_downstream_outputs(workflow, node_id, &image, true)
}

fn apply_glb_capture_success(
    workflow: &mut WorkflowFile,
    node_id: &str,
    success: &GlbCaptureSuccess,
) -> Result<usize, String> {
    let Some(index) = workflow
        .nodes
        .iter()
        .position(|node| node.id == node_id && node.node_type == NodeType::GlbViewer)
    else {
        return Err(format!("GLB Viewer `{node_id}` was not found."));
    };
    let image = success.image.clone();
    set_node_data_field(
        &mut workflow.nodes[index],
        "capturedImage",
        Value::String(image.clone()),
    );
    set_node_data_field(&mut workflow.nodes[index], "capturedImageRef", Value::Null);
    set_node_data_field(
        &mut workflow.nodes[index],
        "glbCaptureResult",
        serde_json::json!({
            "adapter": "webview-model-viewer",
            "width": success.width,
            "height": success.height,
            "outputMime": "image/png"
        }),
    );
    set_node_data_field(
        &mut workflow.nodes[index],
        "__mediaAdapter",
        Value::String("webview-model-viewer".to_string()),
    );
    set_node_data_field(
        &mut workflow.nodes[index],
        "status",
        Value::String("complete".to_string()),
    );
    set_node_data_field(&mut workflow.nodes[index], "error", Value::Null);

    route_captured_image_to_downstream_outputs(workflow, node_id, &image, false)
}

fn route_captured_image_to_downstream_outputs(
    workflow: &mut WorkflowFile,
    node_id: &str,
    image: &str,
    allow_unhandled_source_edges: bool,
) -> Result<usize, String> {
    let downstream_targets = workflow
        .edges
        .iter()
        .filter(|edge| {
            edge.source == node_id
                && edge
                    .source_handle
                    .as_deref()
                    .map(|handle| handle == "image")
                    .unwrap_or(allow_unhandled_source_edges)
        })
        .map(|edge| (edge.target.clone(), edge.target_handle.clone()))
        .collect::<Vec<_>>();
    let mut routed_count = 0;
    for (target_id, target_handle) in downstream_targets {
        if !target_handle
            .as_deref()
            .map(|handle| handle == "image")
            .unwrap_or(true)
        {
            continue;
        }
        let Some(target) = workflow.nodes.iter_mut().find(|node| node.id == target_id) else {
            continue;
        };
        match target.node_type {
            NodeType::Output => {
                set_node_data_field(target, "image", Value::String(image.to_string()));
                set_node_data_field(target, "contentType", Value::String("image".to_string()));
                set_node_data_field(target, "status", Value::String("complete".to_string()));
                routed_count += 1;
            }
            NodeType::OutputGallery => {
                set_node_data_field(target, "images", serde_json::json!([image]));
                set_node_data_field(target, "status", Value::String("complete".to_string()));
                routed_count += 1;
            }
            _ => {}
        }
    }
    Ok(routed_count)
}

fn set_node_data_field(node: &mut WorkflowNode, key: &str, value: Value) {
    if !node.data.is_object() {
        node.data = serde_json::json!({});
    }
    if let Some(map) = node.data.as_object_mut() {
        map.insert(key.to_string(), value);
    }
}

fn edge_path(workflow: &WorkflowFile, edge: &WorkflowEdge) -> Option<String> {
    let points = edge_points(workflow, edge)?;
    let x1 = points.source.x;
    let y1 = points.source.y;
    let x2 = points.target.x;
    let y2 = points.target.y;
    let mid = ((x2 - x1).abs() * 0.5).clamp(80.0, 220.0);
    Some(format!(
        "M {x1:.1} {y1:.1} C {cx1:.1} {y1:.1}, {cx2:.1} {y2:.1}, {x2:.1} {y2:.1}",
        cx1 = x1 + mid,
        cx2 = x2 - mid
    ))
}

fn edge_delete_action(workflow: &WorkflowFile, edge: &WorkflowEdge) -> Option<Position> {
    let points = edge_points(workflow, edge)?;
    Some(Position {
        x: (points.source.x + points.target.x) / 2.0,
        y: (points.source.y + points.target.y) / 2.0 - 16.0,
    })
}

fn edge_points(workflow: &WorkflowFile, edge: &WorkflowEdge) -> Option<EdgePoints> {
    let source = workflow.nodes.iter().find(|node| node.id == edge.source)?;
    let target = workflow.nodes.iter().find(|node| node.id == edge.target)?;
    let x1 = source.position.x + 248.0;
    let y1 = handle_y(source, edge.source_handle.as_deref(), HandleSide::Source);
    let x2 = target.position.x;
    let y2 = handle_y(target, edge.target_handle.as_deref(), HandleSide::Target);
    Some(EdgePoints {
        source: Position { x: x1, y: y1 },
        target: Position { x: x2, y: y2 },
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct EdgePoints {
    source: Position,
    target: Position,
}

fn handle_y(node: &WorkflowNode, handle_id: Option<&str>, side: HandleSide) -> f64 {
    let handles = match side {
        HandleSide::Source => source_handle_options(node),
        HandleSide::Target => target_handle_options(node),
    };
    let count = handles.len().max(1) as f64;
    let index = handle_id
        .and_then(|handle_id| handles.iter().position(|handle| handle.id == handle_id))
        .map_or((count - 1.0) / 2.0, |index| index as f64);
    let usable = 76.0;
    let offset = if count <= 1.0 {
        64.0
    } else {
        42.0 + (usable * index / (count - 1.0))
    };
    node.position.y + offset
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HandleSide {
    Source,
    Target,
}

fn optional_handle(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_string())
}

fn edge_label(edge: &WorkflowEdge) -> String {
    format!(
        "{}:{} → {}:{}",
        edge.source,
        edge.source_handle.as_deref().unwrap_or(""),
        edge.target,
        edge.target_handle.as_deref().unwrap_or("")
    )
}

fn group_color_style(color: GroupColor) -> &'static str {
    match color {
        GroupColor::Neutral => "rgba(82, 82, 91, .95)",
        GroupColor::Blue => "rgba(37, 99, 235, .95)",
        GroupColor::Green => "rgba(22, 163, 74, .95)",
        GroupColor::Purple => "rgba(124, 58, 237, .95)",
        GroupColor::Orange => "rgba(234, 88, 12, .95)",
        GroupColor::Red => "rgba(220, 38, 38, .95)",
    }
}

fn group_background_style(color: GroupColor) -> &'static str {
    match color {
        GroupColor::Neutral => "rgba(82, 82, 91, .20)",
        GroupColor::Blue => "rgba(37, 99, 235, .18)",
        GroupColor::Green => "rgba(22, 163, 74, .18)",
        GroupColor::Purple => "rgba(124, 58, 237, .18)",
        GroupColor::Orange => "rgba(234, 88, 12, .18)",
        GroupColor::Red => "rgba(220, 38, 38, .18)",
    }
}

fn group_border_style(group: &NodeGroup) -> String {
    let style = if group.is_nbp_input.unwrap_or(false) {
        "2px dashed"
    } else {
        "1px solid"
    };
    format!("{style} {}", group_color_style(group.color))
}

fn remove_edge_by_id(
    edge_id: &str,
    workflow: &mut Signal<WorkflowFile>,
    json_text: &mut Signal<String>,
    message: &mut Signal<Message>,
    undo_stack: &mut Signal<WorkflowUndoStack>,
) {
    let edge_id = edge_id.to_string();
    mutate_workflow(workflow, json_text, message, undo_stack, move |workflow| {
        remove_edge(workflow, &edge_id)
            .map(|edge| {
                format!(
                    "Removed edge `{}` ({} → {}).",
                    edge.id, edge.source, edge.target
                )
            })
            .map_err(|err| err.to_string())
    });
}

fn toggle_group_lock_by_id(
    group_id: &str,
    workflow: &mut Signal<WorkflowFile>,
    json_text: &mut Signal<String>,
    message: &mut Signal<Message>,
    undo_stack: &mut Signal<WorkflowUndoStack>,
) {
    let group_id = group_id.to_string();
    mutate_workflow(workflow, json_text, message, undo_stack, move |workflow| {
        let locked = toggle_group_lock(workflow, &group_id).map_err(|err| err.to_string())?;
        Ok(format!(
            "{} group `{group_id}`.",
            if locked { "Locked" } else { "Unlocked" }
        ))
    });
}

fn resize_group_by_id(
    group_id: &str,
    width_delta: f64,
    height_delta: f64,
    signals: GroupEditSignals,
) {
    let mut workflow = signals.workflow;
    let mut json_text = signals.json_text;
    let mut message = signals.message;
    let mut undo_stack = signals.undo_stack;
    let group_id = group_id.to_string();
    mutate_workflow(
        &mut workflow,
        &mut json_text,
        &mut message,
        &mut undo_stack,
        move |workflow| {
            let size = resize_group_by(workflow, &group_id, width_delta, height_delta)
                .map_err(|err| err.to_string())?;
            Ok(format!(
                "Resized group `{group_id}` to {:.0}×{:.0}.",
                size.width, size.height
            ))
        },
    );
}

fn focus_split_grid_child_set(
    split_node_id: &str,
    child_index: usize,
    workflow: &mut Signal<WorkflowFile>,
    json_text: &mut Signal<String>,
    message: &mut Signal<Message>,
    undo_stack: &mut Signal<WorkflowUndoStack>,
    viewport: &mut Signal<CanvasViewport>,
) {
    let split_node_id = split_node_id.to_string();
    mutate_workflow(workflow, json_text, message, undo_stack, move |workflow| {
        let selection = select_split_grid_child_set(workflow, &split_node_id, child_index)
            .map_err(|err| err.to_string())?;
        let viewport_target =
            viewport_for_node_ids(workflow, &selection.selected_node_ids).unwrap_or_default();
        viewport.set(viewport_target);
        Ok(format!(
            "Selected split-grid cell {} child set: {}.",
            selection.child_index + 1,
            selection.selected_node_ids.join(", ")
        ))
    });
}

fn viewport_for_node_ids(workflow: &WorkflowFile, node_ids: &[String]) -> Option<CanvasViewport> {
    let bounds = bounds_for_node_ids(workflow, node_ids)?;
    let center_x = bounds.x + bounds.width / 2.0;
    let center_y = bounds.y + bounds.height / 2.0;
    Some(CanvasViewport {
        zoom: 0.88,
        pan_x: 520.0 - center_x * 0.88,
        pan_y: 330.0 - center_y * 0.88,
    })
}

fn bounds_for_node_ids(workflow: &WorkflowFile, node_ids: &[String]) -> Option<CanvasRect> {
    let selected: std::collections::HashSet<&str> = node_ids.iter().map(String::as_str).collect();
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    let mut found = false;

    for node in workflow
        .nodes
        .iter()
        .filter(|node| selected.contains(node.id.as_str()))
    {
        found = true;
        min_x = min_x.min(node.position.x);
        min_y = min_y.min(node.position.y);
        max_x = max_x.max(node.position.x + NODE_CARD_WIDTH);
        max_y = max_y.max(node.position.y + NODE_CARD_HEIGHT);
    }

    found.then(|| CanvasRect {
        x: min_x,
        y: min_y,
        width: (max_x - min_x).max(0.0),
        height: (max_y - min_y).max(0.0),
    })
}

fn begin_handle_connection(
    source_node_id: &str,
    source_handle: &str,
    message: &mut Signal<Message>,
    mut connection_draft: Signal<Option<ConnectionDraft>>,
) {
    connection_draft.set(Some(ConnectionDraft {
        source_node_id: source_node_id.to_string(),
        source_handle: source_handle.to_string(),
    }));
    message.set(Message::ok(format!(
        "Connecting from `{source_node_id}`:{source_handle}. Release on a target handle."
    )));
}

fn finish_handle_connection(
    target_node_id: &str,
    target_handle: &str,
    workflow: &mut Signal<WorkflowFile>,
    json_text: &mut Signal<String>,
    message: &mut Signal<Message>,
    undo_stack: &mut Signal<WorkflowUndoStack>,
    mut connection_draft: Signal<Option<ConnectionDraft>>,
) {
    let Some(draft) = connection_draft.read().clone() else {
        message.set(Message::err(
            "Start from a source handle before connecting to a target handle.",
        ));
        return;
    };

    let source_node_id = draft.source_node_id.clone();
    let source_handle = draft.source_handle.clone();
    let target_node_id = target_node_id.to_string();
    let target_handle = target_handle.to_string();
    mutate_workflow(workflow, json_text, message, undo_stack, move |workflow| {
        let edge = add_edge_between(
            workflow,
            &source_node_id,
            &target_node_id,
            optional_handle(&source_handle),
            optional_handle(&target_handle),
        )
        .map_err(|err| err.to_string())?;
        Ok(format!(
            "Connected `{}`:{} → `{}`:{} as `{}`.",
            edge.source,
            edge.source_handle.as_deref().unwrap_or(""),
            edge.target,
            edge.target_handle.as_deref().unwrap_or(""),
            edge.id
        ))
    });
    connection_draft.set(None);
}

fn cancel_canvas_connection(
    mut message: Signal<Message>,
    mut connection_draft: Signal<Option<ConnectionDraft>>,
) {
    if connection_draft.read().is_none() {
        return;
    }
    connection_draft.set(None);
    message.set(Message::ok("Cancelled pending handle connection."));
}

fn mutate_selected_node(
    workflow: &mut Signal<WorkflowFile>,
    json_text: &mut Signal<String>,
    message: &mut Signal<Message>,
    undo_stack: &mut Signal<WorkflowUndoStack>,
    dx: f64,
    dy: f64,
) {
    mutate_workflow(workflow, json_text, message, undo_stack, |workflow| {
        let selected_ids = selected_node_ids(workflow)
            .into_iter()
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        if selected_ids.is_empty() {
            return Err("Select a node before moving it.".to_string());
        }

        let locked_ids = locked_node_ids(workflow, &selected_ids);
        if !locked_ids.is_empty() {
            return Err(format!(
                "Selection contains locked node(s): {}. Unlock their group before moving.",
                locked_ids.join(", ")
            ));
        }

        let mut last_position = None;
        for node_id in &selected_ids {
            last_position =
                Some(move_node_by(workflow, node_id, dx, dy).map_err(|err| err.to_string())?);
        }

        if selected_ids.len() == 1 {
            let node_id = &selected_ids[0];
            let position = last_position.expect("single selected node was moved");
            Ok(format!(
                "Moved `{node_id}` to ({:.0}, {:.0}).",
                position.x, position.y
            ))
        } else {
            Ok(format!(
                "Moved {} selected nodes by ({dx:.0}, {dy:.0}).",
                selected_ids.len()
            ))
        }
    });
}

fn mutate_workflow<F>(
    workflow: &mut Signal<WorkflowFile>,
    json_text: &mut Signal<String>,
    message: &mut Signal<Message>,
    undo_stack: &mut Signal<WorkflowUndoStack>,
    mut mutation: F,
) where
    F: FnMut(&mut WorkflowFile) -> Result<String, String>,
{
    let before = workflow.read().clone();
    let mut next = before.clone();
    match mutation(&mut next) {
        Ok(success) => match next.to_pretty_json() {
            Ok(json) => {
                if next != before {
                    undo_stack.write().record(&before);
                }
                workflow.set(next);
                json_text.set(json);
                message.set(Message::ok(success));
            }
            Err(err) => message.set(Message::err(format!(
                "Workflow edited but export failed: {err}"
            ))),
        },
        Err(err) => message.set(Message::err(err)),
    }
}

fn begin_node_drag(event: MouseEvent, node_id: &str, signals: NodeDragSignals) {
    let mut workflow = signals.workflow;
    let mut json_text = signals.json_text;
    let mut undo_stack = signals.undo_stack;
    let mut drag_state = signals.drag_state;
    let viewport = signals.viewport;
    let mut message = signals.message;
    let before = workflow.read().clone();
    let Some(node) = before.nodes.iter().find(|node| node.id == node_id) else {
        return;
    };
    let point = event.data().client_coordinates();
    let viewport = *viewport.read();
    let additive_selection = is_additive_selection(&event);
    let mut next = before.clone();

    if additive_selection {
        let Ok(selected) = toggle_node_selection(&mut next, node_id) else {
            return;
        };
        if let Ok(json) = next.to_pretty_json() {
            if next != before {
                undo_stack.write().record(&before);
            }
            workflow.set(next);
            json_text.set(json);
            message.set(Message::ok(format!(
                "{} `{node_id}` {} the selection.",
                if selected { "Added" } else { "Removed" },
                if selected { "to" } else { "from" }
            )));
        }
        return;
    }

    let node_was_selected = node.selected.unwrap_or(false);
    if !node_was_selected && select_node(&mut next, Some(node_id)).is_err() {
        return;
    }

    let selected_ids = selected_node_ids(&next)
        .into_iter()
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let locked_ids = locked_node_ids(&next, &selected_ids);
    if !locked_ids.is_empty() {
        if let Ok(json) = next.to_pretty_json() {
            if next != before {
                undo_stack.write().record(&before);
            }
            workflow.set(next);
            json_text.set(json);
        }
        drag_state.set(None);
        message.set(Message::err(format!(
            "Selection contains locked node(s): {}. Unlock their group before moving.",
            locked_ids.join(", ")
        )));
        return;
    }

    for selected_id in &selected_ids {
        set_node_dragging(&mut next, selected_id, true);
    }
    let start_positions = selected_ids
        .iter()
        .filter_map(|selected_id| {
            next.nodes
                .iter()
                .find(|node| node.id == *selected_id)
                .map(|node| DraggedNodeStart {
                    node_id: selected_id.clone(),
                    start_position: node.position,
                })
        })
        .collect::<Vec<_>>();

    if let Ok(json) = next.to_pretty_json() {
        if next != before {
            undo_stack.write().record(&before);
        }
        workflow.set(next);
        json_text.set(json);
        drag_state.set(Some(DragState {
            node_id: node_id.to_string(),
            start_client_x: point.x,
            start_client_y: point.y,
            start_positions,
            start_viewport: viewport,
        }));
    }
}

fn update_dragged_node(
    event: MouseEvent,
    mut workflow: Signal<WorkflowFile>,
    mut json_text: Signal<String>,
    drag_state: Signal<Option<DragState>>,
) {
    let Some(drag) = drag_state.read().clone() else {
        return;
    };
    let point = event.data().client_coordinates();
    let zoom = drag.start_viewport.zoom;
    let dx = (point.x - drag.start_client_x) / zoom;
    let dy = (point.y - drag.start_client_y) / zoom;
    let mut next = workflow.read().clone();
    for origin in &drag.start_positions {
        let next_position = Position {
            x: origin.start_position.x + dx,
            y: origin.start_position.y + dy,
        };
        if set_node_position(&mut next, &origin.node_id, next_position).is_err() {
            return;
        }
        set_node_dragging(&mut next, &origin.node_id, true);
    }
    if let Ok(json) = next.to_pretty_json() {
        workflow.set(next);
        json_text.set(json);
    }
}

fn finish_drag(
    mut workflow: Signal<WorkflowFile>,
    mut json_text: Signal<String>,
    mut message: Signal<Message>,
    mut drag_state: Signal<Option<DragState>>,
) {
    let Some(drag) = drag_state.read().clone() else {
        return;
    };
    drag_state.set(None);
    let mut next = workflow.read().clone();
    for origin in &drag.start_positions {
        set_node_dragging(&mut next, &origin.node_id, false);
    }
    let Some(node) = next.nodes.iter().find(|node| node.id == drag.node_id) else {
        message.set(Message::err(format!(
            "Dragged node `{}` disappeared.",
            drag.node_id
        )));
        return;
    };
    let summary = if drag.start_positions.len() == 1 {
        format!(
            "Moved `{}` to ({:.0}, {:.0}).",
            drag.node_id, node.position.x, node.position.y
        )
    } else {
        format!("Moved {} selected nodes.", drag.start_positions.len())
    };
    match next.to_pretty_json() {
        Ok(json) => {
            workflow.set(next);
            json_text.set(json);
            message.set(Message::ok(summary));
        }
        Err(err) => message.set(Message::err(format!("Drag finish export failed: {err}"))),
    }
}

fn update_group_resize(
    event: MouseEvent,
    mut workflow: Signal<WorkflowFile>,
    mut json_text: Signal<String>,
    group_resize_state: Signal<Option<GroupResizeState>>,
) {
    let Some(resize) = group_resize_state.read().clone() else {
        return;
    };

    event.prevent_default();
    let point = event.data().client_coordinates();
    let zoom = resize.start_viewport.zoom;
    let next_size = Size {
        width: resize.start_size.width + (point.x - resize.start_client_x) / zoom,
        height: resize.start_size.height + (point.y - resize.start_client_y) / zoom,
    };
    let mut next = workflow.read().clone();
    if set_group_size(&mut next, &resize.group_id, next_size).is_err() {
        return;
    }
    if let Ok(json) = next.to_pretty_json() {
        workflow.set(next);
        json_text.set(json);
    }
}

fn finish_group_resize(
    workflow: Signal<WorkflowFile>,
    mut message: Signal<Message>,
    mut undo_stack: Signal<WorkflowUndoStack>,
    mut group_resize_state: Signal<Option<GroupResizeState>>,
) {
    let Some(resize) = group_resize_state.read().clone() else {
        return;
    };
    group_resize_state.set(None);
    let current = workflow.read().clone();
    if current != resize.before {
        undo_stack.write().record(&resize.before);
    }
    if let Some(group) = current.groups.get(&resize.group_id) {
        message.set(Message::ok(format!(
            "Resized group `{}` to {:.0}×{:.0}.",
            group.id, group.size.width, group.size.height
        )));
    } else {
        message.set(Message::err(format!(
            "Resized group `{}` disappeared.",
            resize.group_id
        )));
    }
}

fn update_group_move(
    event: MouseEvent,
    mut workflow: Signal<WorkflowFile>,
    mut json_text: Signal<String>,
    group_move_state: Signal<Option<GroupMoveState>>,
) {
    let Some(group_move) = group_move_state.read().clone() else {
        return;
    };

    event.prevent_default();
    let point = event.data().client_coordinates();
    let dx = (point.x - group_move.start_client_x) / group_move.start_viewport.zoom;
    let dy = (point.y - group_move.start_client_y) / group_move.start_viewport.zoom;
    let mut next = group_move.before.clone();
    if move_group_by(&mut next, &group_move.group_id, dx, dy).is_err() {
        return;
    }
    if let Ok(json) = next.to_pretty_json() {
        workflow.set(next);
        json_text.set(json);
    }
}

fn finish_group_move(
    workflow: Signal<WorkflowFile>,
    mut message: Signal<Message>,
    mut undo_stack: Signal<WorkflowUndoStack>,
    mut group_move_state: Signal<Option<GroupMoveState>>,
) {
    let Some(group_move) = group_move_state.read().clone() else {
        return;
    };
    group_move_state.set(None);
    let current = workflow.read().clone();
    if current != group_move.before {
        undo_stack.write().record(&group_move.before);
    }
    if let Some(group) = current.groups.get(&group_move.group_id) {
        let moved_node_count = current
            .nodes
            .iter()
            .filter(|node| node.group_id.as_deref() == Some(group_move.group_id.as_str()))
            .count();
        message.set(Message::ok(format!(
            "Moved group `{}` to ({:.0}, {:.0}) with {} member node(s).",
            group.id, group.position.x, group.position.y, moved_node_count
        )));
    } else {
        message.set(Message::err(format!(
            "Moved group `{}` disappeared.",
            group_move.group_id
        )));
    }
}

fn begin_group_selection(event: MouseEvent, signals: CanvasGestureSignals) {
    let mut group_selection_state = signals.group_selection_state;
    let modifiers = event.data().modifiers();
    if signals.drag_state.read().is_some()
        || signals.pan_state.read().is_some()
        || signals.connection_draft.read().is_some()
        || signals.group_resize_state.read().is_some()
        || signals.group_move_state.read().is_some()
        || group_selection_state.read().is_some()
        || !modifiers.shift()
        || !matches!(event.data().trigger_button(), Some(MouseButton::Primary))
    {
        return;
    }

    event.prevent_default();
    event.stop_propagation();
    let point = canvas_point_from_event(&event);
    let client = event.data().client_coordinates();
    group_selection_state.set(Some(GroupSelectionState {
        start: point,
        current: point,
        start_client_x: client.x,
        start_client_y: client.y,
        start_viewport: *signals.viewport.read(),
    }));
}

fn update_group_selection(
    event: MouseEvent,
    mut group_selection_state: Signal<Option<GroupSelectionState>>,
) {
    let Some(mut selection) = group_selection_state.read().clone() else {
        return;
    };

    event.prevent_default();
    let point = event.data().client_coordinates();
    selection.current = Position {
        x: (selection.start.x
            + (point.x - selection.start_client_x) / selection.start_viewport.zoom)
            .clamp(0.0, CANVAS_WIDTH),
        y: (selection.start.y
            + (point.y - selection.start_client_y) / selection.start_viewport.zoom)
            .clamp(0.0, CANVAS_HEIGHT),
    };
    group_selection_state.set(Some(selection));
}

fn finish_group_selection(
    workflow: Signal<WorkflowFile>,
    json_text: Signal<String>,
    mut message: Signal<Message>,
    undo_stack: Signal<WorkflowUndoStack>,
    mut group_selection_state: Signal<Option<GroupSelectionState>>,
) {
    let Some(selection) = group_selection_state.read().clone() else {
        return;
    };
    group_selection_state.set(None);

    let rect = selection.rect();
    if rect.width < GROUP_SELECTION_MIN_SIZE || rect.height < GROUP_SELECTION_MIN_SIZE {
        message.set(Message::ok("Group selection cancelled: drag a larger box."));
        return;
    }

    let mut workflow = workflow;
    let mut json_text = json_text;
    let mut message = message;
    let mut undo_stack = undo_stack;
    mutate_workflow(
        &mut workflow,
        &mut json_text,
        &mut message,
        &mut undo_stack,
        move |workflow| {
            let selected_ids = node_ids_intersecting_rect(workflow, rect);
            if selected_ids.is_empty() {
                return Err("Group selection did not include any nodes.".to_string());
            }
            let count = selected_ids.len();
            let group =
                create_group_for_nodes(workflow, &selected_ids).map_err(|err| err.to_string())?;
            Ok(format!(
                "Created group `{}` from box selection for {count} node(s).",
                group.name
            ))
        },
    );
}

fn cancel_group_selection(
    mut message: Signal<Message>,
    mut group_selection_state: Signal<Option<GroupSelectionState>>,
) {
    if group_selection_state.read().is_none() {
        return;
    }
    group_selection_state.set(None);
    message.set(Message::ok("Cancelled group box selection."));
}

fn begin_canvas_pan(event: MouseEvent, signals: CanvasGestureSignals) {
    let mut pan_state = signals.pan_state;
    if signals.drag_state.read().is_some()
        || pan_state.read().is_some()
        || signals.connection_draft.read().is_some()
        || signals.group_resize_state.read().is_some()
        || signals.group_move_state.read().is_some()
        || signals.group_selection_state.read().is_some()
        || event.data().modifiers().shift()
        || !matches!(
            event.data().trigger_button(),
            Some(MouseButton::Primary | MouseButton::Auxiliary)
        )
    {
        return;
    }

    event.prevent_default();
    let point = event.data().client_coordinates();
    pan_state.set(Some(PanState {
        start_client_x: point.x,
        start_client_y: point.y,
        start_viewport: *signals.viewport.read(),
    }));
}

fn update_canvas_pan(
    event: MouseEvent,
    mut viewport: Signal<CanvasViewport>,
    pan_state: Signal<Option<PanState>>,
) {
    let Some(pan) = pan_state.read().clone() else {
        return;
    };

    event.prevent_default();
    let point = event.data().client_coordinates();
    viewport.set(CanvasViewport {
        zoom: pan.start_viewport.zoom,
        pan_x: pan.start_viewport.pan_x + point.x - pan.start_client_x,
        pan_y: pan.start_viewport.pan_y + point.y - pan.start_client_y,
    });
}

fn finish_canvas_pan(mut pan_state: Signal<Option<PanState>>) {
    pan_state.set(None);
}

fn handle_canvas_wheel(event: WheelEvent, mut viewport: Signal<CanvasViewport>) {
    event.prevent_default();
    let delta_y = normalized_wheel_delta_y(event.data().delta());
    if delta_y.abs() < f64::EPSILON {
        return;
    }

    let factor = (-delta_y * 0.0015).exp().clamp(0.5, 1.5);
    viewport.with_mut(|viewport| viewport.zoom_by(factor));
}

fn normalized_wheel_delta_y(delta: WheelDelta) -> f64 {
    match delta {
        WheelDelta::Pixels(delta) => delta.y,
        WheelDelta::Lines(delta) => delta.y * 36.0,
        WheelDelta::Pages(delta) => delta.y * 720.0,
    }
}

fn canvas_point_from_event(event: &MouseEvent) -> Position {
    let point = event.data().element_coordinates();
    Position {
        x: point.x.clamp(0.0, CANVAS_WIDTH),
        y: point.y.clamp(0.0, CANVAS_HEIGHT),
    }
}

fn node_ids_intersecting_rect(workflow: &WorkflowFile, rect: CanvasRect) -> Vec<String> {
    workflow
        .nodes
        .iter()
        .filter(|node| rect.intersects(CanvasRect::from_node(node)))
        .map(|node| node.id.clone())
        .collect()
}

fn is_additive_selection(event: &MouseEvent) -> bool {
    let modifiers = event.data().modifiers();
    modifiers.ctrl() || modifiers.meta()
}

fn locked_node_ids(workflow: &WorkflowFile, node_ids: &[String]) -> Vec<String> {
    node_ids
        .iter()
        .filter(|node_id| is_node_in_locked_group(workflow, node_id))
        .cloned()
        .collect()
}

#[derive(Clone, Copy)]
enum HistoryDirection {
    Undo,
    Redo,
}

fn apply_history_action(
    workflow: &mut Signal<WorkflowFile>,
    json_text: &mut Signal<String>,
    message: &mut Signal<Message>,
    undo_stack: &mut Signal<WorkflowUndoStack>,
    direction: HistoryDirection,
) {
    let mut next = workflow.read().clone();
    let result = match direction {
        HistoryDirection::Undo => undo_stack.write().undo(&mut next),
        HistoryDirection::Redo => undo_stack.write().redo(&mut next),
    };
    if let Err(err) = result {
        message.set(Message::err(err.to_string()));
        return;
    }

    clear_dragging_flags(&mut next);
    match next.to_pretty_json() {
        Ok(json) => {
            workflow.set(next);
            json_text.set(json);
            message.set(Message::ok(match direction {
                HistoryDirection::Undo => "Undid last canvas edit.",
                HistoryDirection::Redo => "Redid canvas edit.",
            }));
        }
        Err(err) => message.set(Message::err(format!("History export failed: {err}"))),
    }
}

fn set_node_dragging(workflow: &mut WorkflowFile, node_id: &str, dragging: bool) {
    if let Some(node) = workflow.nodes.iter_mut().find(|node| node.id == node_id) {
        node.dragging = dragging.then_some(true);
    }
}

fn clear_dragging_flags(workflow: &mut WorkflowFile) {
    for node in &mut workflow.nodes {
        node.dragging = None;
    }
}

fn save_autosave_workflow(
    workflow: &WorkflowFile,
) -> Result<WorkflowSnapshot, gemed_storage::StorageError> {
    let mut storage = platform_storage()?;
    storage.save_workflow(DEFAULT_AUTOSAVE_SLOT, workflow)
}

fn load_autosave_workflow() -> Result<WorkflowFile, gemed_storage::StorageError> {
    platform_storage()?.load_workflow(DEFAULT_AUTOSAVE_SLOT)
}

fn initial_provider_config() -> ProviderConfigSet {
    load_provider_settings().unwrap_or_else(|_| default_provider_config())
}

fn save_provider_settings(
    config: &ProviderConfigSet,
) -> Result<ProviderConfigSnapshot, gemed_storage::StorageError> {
    let mut storage = platform_storage()?;
    storage.save_provider_config(DEFAULT_PROVIDER_CONFIG_SLOT, config)
}

fn load_provider_settings() -> Result<ProviderConfigSet, gemed_storage::StorageError> {
    platform_storage()?.load_provider_config(DEFAULT_PROVIDER_CONFIG_SLOT)
}

fn build_provider_registry(config: &ProviderConfigSet) -> Result<ProviderRegistry, String> {
    let mut registry = ProviderRegistry::mock_from_config(config);
    register_platform_provider_backends(config, &mut registry)?;
    Ok(registry)
}

#[cfg(all(feature = "desktop", feature = "providers-http"))]
fn register_platform_provider_backends(
    config: &ProviderConfigSet,
    registry: &mut ProviderRegistry,
) -> Result<(), String> {
    for provider in &config.providers {
        if provider.runtime_mode != ProviderRuntimeMode::DirectDesktop {
            continue;
        }
        match provider.id {
            ProviderId::Gemini | ProviderId::Google => register_provider_backend(
                registry,
                GeminiGenerateContentProvider::from_config_with_secret(
                    provider,
                    &provider_secret_env_value,
                ),
            )?,
            ProviderId::OpenAi => register_provider_backend(
                registry,
                OpenAiResponsesProvider::from_config_with_secret(
                    provider,
                    &provider_secret_env_value,
                ),
            )?,
            ProviderId::Anthropic => register_provider_backend(
                registry,
                AnthropicMessagesProvider::from_config_with_secret(
                    provider,
                    &provider_secret_env_value,
                ),
            )?,
            _ => {}
        }
    }
    Ok(())
}

#[cfg(all(feature = "desktop", feature = "providers-http"))]
fn register_provider_backend(
    registry: &mut ProviderRegistry,
    backend: Result<
        Option<impl gemed_providers::ProviderBackend + 'static>,
        gemed_providers::ProviderError,
    >,
) -> Result<(), String> {
    match backend {
        Ok(Some(backend)) => registry.register(backend),
        Ok(None) => {}
        Err(err) => return Err(err.to_string()),
    }
    Ok(())
}

#[cfg(not(all(feature = "desktop", feature = "providers-http")))]
fn register_platform_provider_backends(
    config: &ProviderConfigSet,
    _registry: &mut ProviderRegistry,
) -> Result<(), String> {
    let live_candidates = config
        .providers
        .iter()
        .filter(|provider| {
            provider.enabled
                && !matches!(
                    provider.runtime_mode,
                    ProviderRuntimeMode::Mock | ProviderRuntimeMode::Disabled
                )
        })
        .map(|provider| provider.id.display_name())
        .collect::<Vec<_>>();
    if live_candidates.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "live provider backend feature is not enabled for: {}",
            live_candidates.join(", ")
        ))
    }
}

#[cfg(feature = "desktop")]
fn default_provider_config() -> ProviderConfigSet {
    ProviderConfigSet::desktop_env_defaults()
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
fn default_provider_config() -> ProviderConfigSet {
    ProviderConfigSet::web_backend_defaults()
}

#[cfg(feature = "desktop")]
fn provider_secret_env_value(variable: &str) -> Option<String> {
    std::env::var(variable).ok()
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
fn provider_secret_env_value(_variable: &str) -> Option<String> {
    None
}

#[derive(Clone, Copy)]
enum ProviderSettingsMode {
    Mock,
    DesktopEnv,
    Disabled,
}

fn set_provider_config_mode(
    provider_config: &mut Signal<ProviderConfigSet>,
    id: ProviderId,
    mode: ProviderSettingsMode,
    message: &mut Signal<Message>,
) {
    let mut next = provider_config.read().clone();
    let existing_customization = next
        .providers
        .iter()
        .find(|config| config.id == id)
        .map(|config| (config.default_model.clone(), config.base_url.clone()));
    let mut replacement = match mode {
        ProviderSettingsMode::Mock => ProviderConfig::mock(id.clone()),
        ProviderSettingsMode::DesktopEnv => platform_env_provider_config(id.clone()),
        ProviderSettingsMode::Disabled => ProviderConfig::disabled(id.clone()),
    };
    if let Some((default_model, base_url)) = existing_customization {
        replacement.default_model = default_model;
        replacement.base_url = base_url;
    }
    let display_name = replacement.id.display_name();
    let mode_label = provider_runtime_mode_label(replacement.runtime_mode);

    if let Some(config) = next.providers.iter_mut().find(|config| config.id == id) {
        *config = replacement;
    } else {
        next.providers.push(replacement);
    }
    provider_config.set(next);
    message.set(Message::ok(format!(
        "Set `{display_name}` provider mode to {mode_label}."
    )));
}

fn set_provider_default_model(
    provider_config: &mut Signal<ProviderConfigSet>,
    id: ProviderId,
    value: &str,
    message: &mut Signal<Message>,
) {
    let sanitized = sanitize_optional_provider_text(value);
    let label = sanitized
        .clone()
        .unwrap_or_else(|| "provider default".to_string());
    update_provider_config(provider_config, id.clone(), |config| {
        config.default_model = sanitized;
    });
    message.set(Message::ok(format!(
        "Set `{}` default model to `{label}`.",
        id.display_name()
    )));
}

fn set_provider_base_url(
    provider_config: &mut Signal<ProviderConfigSet>,
    id: ProviderId,
    value: &str,
    message: &mut Signal<Message>,
) {
    match sanitize_optional_provider_base_url(value) {
        Ok(base_url) => {
            let label = base_url
                .clone()
                .unwrap_or_else(|| "provider default".to_string());
            update_provider_config(provider_config, id.clone(), |config| {
                config.base_url = base_url;
            });
            message.set(Message::ok(format!(
                "Set `{}` base URL to `{label}`.",
                id.display_name()
            )));
        }
        Err(err) => message.set(Message::err(err)),
    }
}

fn update_provider_config(
    provider_config: &mut Signal<ProviderConfigSet>,
    id: ProviderId,
    update: impl FnOnce(&mut ProviderConfig),
) {
    let mut next = provider_config.read().clone();
    if let Some(config) = next.providers.iter_mut().find(|config| config.id == id) {
        update(config);
    } else {
        let mut config = ProviderConfig::mock(id);
        update(&mut config);
        next.providers.push(config);
    }
    provider_config.set(next);
}

fn sanitize_optional_provider_text(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn sanitize_optional_provider_base_url(value: &str) -> Result<Option<String>, String> {
    let value = value.trim().trim_end_matches('/');
    if value.is_empty() {
        return Ok(None);
    }
    if !(value.starts_with("https://") || value.starts_with("http://")) {
        return Err("Provider base URL must start with http:// or https://.".to_string());
    }
    let authority = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))
        .unwrap_or(value)
        .split('/')
        .next()
        .unwrap_or_default();
    if authority.is_empty() {
        return Err("Provider base URL needs a host.".to_string());
    }
    if authority.contains('@') {
        return Err(
            "Provider base URL must not include username/password credentials.".to_string(),
        );
    }
    if value.contains('?') || value.contains('#') {
        return Err("Provider base URL must not include query strings or fragments.".to_string());
    }
    Ok(Some(value.to_string()))
}

fn provider_default_model_placeholder(id: &ProviderId) -> &'static str {
    match id {
        ProviderId::Gemini | ProviderId::Google => "gemini-3.5-flash",
        ProviderId::OpenAi => "gpt-5.5",
        ProviderId::Anthropic => "claude-sonnet-4-6",
        ProviderId::Mock => "mock-llm",
        _ => "provider default",
    }
}

fn provider_base_url_placeholder(id: &ProviderId) -> &'static str {
    match id {
        ProviderId::Gemini | ProviderId::Google => {
            "https://generativelanguage.googleapis.com/v1beta"
        }
        ProviderId::OpenAi => "https://api.openai.com/v1/responses",
        ProviderId::Anthropic => "https://api.anthropic.com/v1/messages",
        _ => "https://provider.example/api",
    }
}

#[cfg(feature = "desktop")]
fn platform_env_provider_config(id: ProviderId) -> ProviderConfig {
    let variable = provider_env_variable(&id).unwrap_or("CUSTOM_PROVIDER_API_KEY");
    ProviderConfig::direct_desktop_env(id, variable, None)
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
fn platform_env_provider_config(id: ProviderId) -> ProviderConfig {
    let variable = provider_env_variable(&id).unwrap_or("CUSTOM_PROVIDER_API_KEY");
    ProviderConfig::web_backend(id, variable, None)
}

fn provider_env_variable(id: &ProviderId) -> Option<&'static str> {
    match id {
        ProviderId::Gemini => Some("GEMINI_API_KEY"),
        ProviderId::Google => Some("GOOGLE_API_KEY"),
        ProviderId::OpenAi => Some("OPENAI_API_KEY"),
        ProviderId::Anthropic => Some("ANTHROPIC_API_KEY"),
        ProviderId::Replicate => Some("REPLICATE_API_TOKEN"),
        ProviderId::Fal => Some("FAL_KEY"),
        ProviderId::Kie => Some("KIE_API_KEY"),
        ProviderId::WaveSpeed => Some("WAVESPEED_API_KEY"),
        ProviderId::Mock | ProviderId::Custom(_) => None,
    }
}

fn provider_runtime_mode_label(mode: ProviderRuntimeMode) -> &'static str {
    match mode {
        ProviderRuntimeMode::Mock => "mock",
        ProviderRuntimeMode::Disabled => "disabled",
        ProviderRuntimeMode::DirectDesktop => "direct desktop",
        ProviderRuntimeMode::WebBackend => "web backend",
    }
}

fn provider_status(config: &ProviderConfig) -> &'static str {
    if !config.enabled || config.runtime_mode == ProviderRuntimeMode::Disabled {
        "disabled"
    } else if config.missing_required_secret_with(&provider_secret_env_value) {
        "missing"
    } else if config.is_available_with(&provider_secret_env_value) {
        "available"
    } else {
        "pending"
    }
}

fn provider_status_class(config: &ProviderConfig) -> &'static str {
    match provider_status(config) {
        "available" => "available",
        "missing" => "missing",
        "disabled" => "disabled",
        _ => "",
    }
}

fn provider_capability_list(capabilities: &[ProviderCapability]) -> String {
    if capabilities.is_empty() {
        return "no declared capabilities".to_string();
    }
    capabilities
        .iter()
        .map(|capability| match capability {
            ProviderCapability::Llm => "llm",
            ProviderCapability::Image => "image",
            ProviderCapability::Video => "video",
            ProviderCapability::Audio => "audio",
            ProviderCapability::Model3d => "3d",
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(feature = "desktop")]
fn provider_secret_setup_overview() -> &'static str {
    "Desktop live providers read secrets from the process environment at runtime. Set the listed variable before launching GemEd; Save Providers stores only the variable name, never the secret value."
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
fn provider_secret_setup_overview() -> &'static str {
    "Web builds treat provider secrets as backend bindings. Static WASM should not store API keys; Save Providers stores only binding names and runtime modes."
}

fn provider_secret_setup_hint(config: &ProviderConfig) -> String {
    match config.runtime_mode {
        ProviderRuntimeMode::Mock => "Mock provider: no secret required.".to_string(),
        ProviderRuntimeMode::Disabled => "Provider disabled: no secret required.".to_string(),
        ProviderRuntimeMode::DirectDesktop => match &config.secret_source {
            ProviderSecretSource::Environment { variable } => {
                if provider_secret_env_value(variable).is_some() {
                    format!("Environment `{variable}` is present for this process.")
                } else {
                    format!(
                        "Set `{variable}` before launching GemEd. This value is not saved in provider settings."
                    )
                }
            }
            ProviderSecretSource::DesktopKeychain { service, account } => {
                format!(
                    "Desktop keychain source `{service}/{account}` is modeled, but keychain writes are not implemented yet."
                )
            }
            ProviderSecretSource::None | ProviderSecretSource::WebBackend { .. } => {
                "Direct desktop mode needs an environment or keychain secret source.".to_string()
            }
        },
        ProviderRuntimeMode::WebBackend => match &config.secret_source {
            ProviderSecretSource::WebBackend { binding } => {
                format!(
                    "Backend binding `{binding}` must be configured server-side; no browser secret is stored."
                )
            }
            _ => "Web backend mode expects a server-side secret binding.".to_string(),
        },
    }
}

fn provider_secret_setup_message(id: &ProviderId) -> String {
    let name = id.display_name();
    let variable = provider_env_variable(id).unwrap_or("CUSTOM_PROVIDER_API_KEY");
    platform_provider_secret_setup_message(&name, variable)
}

#[cfg(feature = "desktop")]
fn platform_provider_secret_setup_message(name: &str, variable: &str) -> String {
    format!(
        "{name} setup: export {variable}=... before launching GemEd, then run `dx serve --desktop --features desktop,providers-http` for live HTTP providers. GemEd saves only `{variable}`, not the secret value."
    )
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
fn platform_provider_secret_setup_message(name: &str, variable: &str) -> String {
    format!(
        "{name} setup: configure backend binding `{variable}` on the server/fullstack deployment. Do not place provider API keys in static WASM or browser localStorage."
    )
}

#[cfg(feature = "desktop")]
fn open_workflow_from_dialog() -> Result<Option<(WorkflowFile, std::path::PathBuf)>, String> {
    let Some(path) = workflow_json_dialog().pick_file() else {
        return Ok(None);
    };
    let json = std::fs::read_to_string(&path)
        .map_err(|err| format!("failed to read `{}`: {err}", path.display()))?;
    let workflow = WorkflowFile::from_json_str(&json)
        .map_err(|err| format!("workflow JSON rejected in `{}`: {err}", path.display()))?;
    Ok(Some((workflow, path)))
}

#[cfg(feature = "desktop")]
fn save_workflow_to_dialog(
    workflow: &WorkflowFile,
) -> Result<Option<(std::path::PathBuf, String)>, String> {
    let Some(path) = workflow_json_dialog()
        .set_file_name(workflow_json_filename(&workflow.name))
        .save_file()
    else {
        return Ok(None);
    };
    let path = ensure_json_extension(path);
    let json = workflow
        .to_pretty_json()
        .map_err(|err| format!("failed to export workflow JSON: {err}"))?;
    std::fs::write(&path, json.as_bytes())
        .map_err(|err| format!("failed to write `{}`: {err}", path.display()))?;
    Ok(Some((path, json)))
}

#[cfg(feature = "desktop")]
fn open_project_from_dialog()
-> Result<Option<gemed_storage::desktop::WorkflowProjectSnapshot>, String> {
    let Some(root) = rfd::FileDialog::new().pick_folder() else {
        return Ok(None);
    };
    gemed_storage::desktop::DesktopWorkflowProject::at_dir(root)
        .load()
        .map(Some)
        .map_err(|err| err.to_string())
}

#[cfg(feature = "desktop")]
fn save_project_to_dialog(
    workflow: &WorkflowFile,
) -> Result<Option<(gemed_storage::desktop::WorkflowProjectSnapshot, String)>, String> {
    let Some(root) = rfd::FileDialog::new().pick_folder() else {
        return Ok(None);
    };
    let snapshot = gemed_storage::desktop::DesktopWorkflowProject::at_dir(root)
        .save(workflow)
        .map_err(|err| err.to_string())?;
    let json = workflow
        .to_pretty_json()
        .map_err(|err| format!("failed to refresh project workflow JSON: {err}"))?;
    Ok(Some((snapshot, json)))
}

#[cfg(feature = "desktop")]
fn workflow_json_dialog() -> rfd::FileDialog {
    rfd::FileDialog::new().add_filter("GemEd workflow JSON", &["json"])
}

#[cfg(any(feature = "desktop", test))]
fn workflow_json_filename(name: &str) -> String {
    let mut base = String::new();
    let mut previous_separator = false;
    for ch in name.trim().chars() {
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
            base.push(ch);
        }
    }

    let base = base.trim_matches(['-', '_']).trim();
    let base = if base.is_empty() { "workflow" } else { base };
    format!("{base}.json")
}

#[cfg(any(feature = "desktop", test))]
fn ensure_json_extension(path: std::path::PathBuf) -> std::path::PathBuf {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some(extension) if !extension.is_empty() => path,
        _ => path.with_extension("json"),
    }
}

#[cfg(feature = "desktop")]
fn platform_storage()
-> Result<gemed_storage::desktop::DesktopWorkflowStorage, gemed_storage::StorageError> {
    gemed_storage::desktop::DesktopWorkflowStorage::new()
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
fn platform_storage() -> Result<gemed_storage::web::WebLocalStorage, gemed_storage::StorageError> {
    Ok(gemed_storage::web::WebLocalStorage::new())
}

#[cfg(feature = "desktop")]
fn storage_backend_label() -> &'static str {
    "desktop filesystem"
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
fn storage_backend_label() -> &'static str {
    "browser localStorage"
}

#[derive(Clone, PartialEq)]
struct Message {
    ok: bool,
    text: String,
}

impl Message {
    fn ok(text: impl Into<String>) -> Self {
        Self {
            ok: true,
            text: text.into(),
        }
    }

    fn err(text: impl Into<String>) -> Self {
        Self {
            ok: false,
            text: text.into(),
        }
    }
}

#[allow(dead_code)]
fn _status_used_for_exhaustiveness(status: NodeStatus) -> &'static str {
    status.label()
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CanvasViewport {
    zoom: f64,
    pan_x: f64,
    pan_y: f64,
}

impl CanvasViewport {
    const MIN_ZOOM: f64 = 0.35;
    const MAX_ZOOM: f64 = 2.5;

    fn zoom_by(&mut self, factor: f64) {
        self.zoom = (self.zoom * factor).clamp(Self::MIN_ZOOM, Self::MAX_ZOOM);
    }

    fn pan_by(&mut self, dx: f64, dy: f64) {
        self.pan_x += dx;
        self.pan_y += dy;
    }
}

impl Default for CanvasViewport {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct DragState {
    node_id: String,
    start_client_x: f64,
    start_client_y: f64,
    start_positions: Vec<DraggedNodeStart>,
    start_viewport: CanvasViewport,
}

#[derive(Clone, Debug, PartialEq)]
struct DraggedNodeStart {
    node_id: String,
    start_position: Position,
}

#[derive(Clone, Copy)]
struct NodeDragSignals {
    workflow: Signal<WorkflowFile>,
    json_text: Signal<String>,
    undo_stack: Signal<WorkflowUndoStack>,
    drag_state: Signal<Option<DragState>>,
    viewport: Signal<CanvasViewport>,
    message: Signal<Message>,
}

#[derive(Clone, Copy)]
struct GroupEditSignals {
    workflow: Signal<WorkflowFile>,
    json_text: Signal<String>,
    message: Signal<Message>,
    undo_stack: Signal<WorkflowUndoStack>,
}

#[derive(Clone, Copy)]
struct CanvasGestureSignals {
    drag_state: Signal<Option<DragState>>,
    pan_state: Signal<Option<PanState>>,
    connection_draft: Signal<Option<ConnectionDraft>>,
    group_resize_state: Signal<Option<GroupResizeState>>,
    group_move_state: Signal<Option<GroupMoveState>>,
    group_selection_state: Signal<Option<GroupSelectionState>>,
    viewport: Signal<CanvasViewport>,
}

#[derive(Clone, Debug, PartialEq)]
struct GroupResizeState {
    group_id: String,
    start_client_x: f64,
    start_client_y: f64,
    start_size: Size,
    start_viewport: CanvasViewport,
    before: WorkflowFile,
}

#[derive(Clone, Debug, PartialEq)]
struct GroupMoveState {
    group_id: String,
    start_client_x: f64,
    start_client_y: f64,
    start_viewport: CanvasViewport,
    before: WorkflowFile,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CanvasRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl CanvasRect {
    fn from_points(start: Position, current: Position) -> Self {
        let left = start.x.min(current.x).clamp(0.0, CANVAS_WIDTH);
        let top = start.y.min(current.y).clamp(0.0, CANVAS_HEIGHT);
        let right = start.x.max(current.x).clamp(0.0, CANVAS_WIDTH);
        let bottom = start.y.max(current.y).clamp(0.0, CANVAS_HEIGHT);
        Self {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        }
    }

    fn from_node(node: &WorkflowNode) -> Self {
        Self {
            x: node.position.x,
            y: node.position.y,
            width: NODE_CARD_WIDTH,
            height: NODE_CARD_HEIGHT,
        }
    }

    fn right(self) -> f64 {
        self.x + self.width
    }

    fn bottom(self) -> f64 {
        self.y + self.height
    }

    fn intersects(self, other: Self) -> bool {
        self.x <= other.right()
            && self.right() >= other.x
            && self.y <= other.bottom()
            && self.bottom() >= other.y
    }

    fn style(self) -> String {
        format!(
            "left: {:.1}px; top: {:.1}px; width: {:.1}px; height: {:.1}px;",
            self.x, self.y, self.width, self.height
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
struct GroupSelectionState {
    start: Position,
    current: Position,
    start_client_x: f64,
    start_client_y: f64,
    start_viewport: CanvasViewport,
}

impl GroupSelectionState {
    fn rect(&self) -> CanvasRect {
        CanvasRect::from_points(self.start, self.current)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct PanState {
    start_client_x: f64,
    start_client_y: f64,
    start_viewport: CanvasViewport,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ConnectionDraft {
    source_node_id: String,
    source_handle: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MediaOverlay {
    kind: MediaKind,
    label: String,
    uri: String,
    uri_hint: String,
    source_field: String,
    download_filename: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CopyStatus {
    ok: bool,
    message: String,
}

impl CopyStatus {
    fn copying() -> Self {
        Self {
            ok: true,
            message: "Copying media URI…".to_string(),
        }
    }

    fn copied() -> Self {
        Self {
            ok: true,
            message: "Media URI copied to clipboard.".to_string(),
        }
    }

    fn failed(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            message: message.into(),
        }
    }

    fn from_eval_value(value: &serde_json::Value) -> Self {
        if value
            .get("ok")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            Self::copied()
        } else {
            let detail = value
                .get("error")
                .and_then(serde_json::Value::as_str)
                .filter(|detail| !detail.trim().is_empty())
                .unwrap_or("clipboard API rejected the request");
            Self::failed(format!("Copy failed: {detail}."))
        }
    }

    fn class_name(&self, base: &'static str) -> &'static str {
        if self.ok {
            base
        } else if base == "media-overlay-copy-status" {
            "media-overlay-copy-status err"
        } else {
            "media-copy-status err"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CanvasRect, CopyStatus, GlbCaptureRequest, GlbCaptureSuccess, NODE_CARD_HEIGHT,
        NODE_CARD_WIDTH, VideoFrameCaptureRequest, VideoFrameCaptureSuccess,
        apply_glb_capture_success, apply_video_frame_capture_success, bounds_for_node_ids,
        copy_media_uri_script, ensure_json_extension, glb_capture_request, glb_capture_script,
        glb_capture_success_from_eval_value, media_error_message, media_overlay_from_preview,
        media_preview_kind_class, node_card_insight, node_ids_intersecting_rect,
        platform_provider_secret_setup_message, provider_capability_list,
        provider_default_model_placeholder, provider_secret_setup_hint,
        sanitize_optional_provider_base_url, sanitize_optional_provider_text,
        video_frame_capture_request, video_frame_capture_script,
        video_frame_capture_success_from_eval_value, viewport_for_node_ids, workflow_json_filename,
    };
    use gemed_core::{
        NodeType, Position, WorkflowEdge, WorkflowFile, WorkflowNode, add_edge_between,
        generate_split_grid_children,
    };
    use gemed_media::{MediaKind, MediaPreview, media_previews_for_node};
    use gemed_providers::{ProviderCapability, ProviderConfig, ProviderId};
    use gemed_storage::{MemoryWorkflowStorage, WorkflowStorage};
    use serde_json::Value;
    use std::path::PathBuf;

    #[test]
    fn workflow_json_filename_keeps_safe_names_predictable() {
        assert_eq!(
            workflow_json_filename("GemEd Dioxus Starter"),
            "gemed-dioxus-starter.json"
        );
        assert_eq!(
            workflow_json_filename("  Media/Provider: v1  "),
            "media-provider-v1.json"
        );
        assert_eq!(workflow_json_filename(""), "workflow.json");
    }

    #[test]
    fn ensure_json_extension_only_adds_missing_extension() {
        assert_eq!(
            ensure_json_extension(PathBuf::from("workflow")),
            PathBuf::from("workflow.json")
        );
        assert_eq!(
            ensure_json_extension(PathBuf::from("workflow.gemed")),
            PathBuf::from("workflow.gemed")
        );
    }

    #[test]
    fn canvas_rect_normalizes_drag_direction() {
        let rect = CanvasRect::from_points(
            Position { x: 340.0, y: 260.0 },
            Position { x: 80.0, y: 110.0 },
        );

        assert_eq!(
            rect,
            CanvasRect {
                x: 80.0,
                y: 110.0,
                width: 260.0,
                height: 150.0,
            }
        );
    }

    #[test]
    fn node_ids_intersecting_rect_uses_canvas_card_bounds() {
        let workflow = WorkflowFile::example();
        let rect = CanvasRect {
            x: 60.0,
            y: 80.0,
            width: 300.0,
            height: 220.0,
        };

        assert_eq!(
            node_ids_intersecting_rect(&workflow, rect),
            vec!["node_prompt"]
        );
    }

    #[test]
    fn provider_capability_list_is_human_readable() {
        assert_eq!(
            provider_capability_list(&[ProviderCapability::Llm, ProviderCapability::Image]),
            "llm, image"
        );
        assert_eq!(provider_capability_list(&[]), "no declared capabilities");
    }

    #[test]
    fn provider_secret_hint_never_contains_secret_values() {
        let config = ProviderConfig::direct_desktop_env(
            ProviderId::OpenAi,
            "OPENAI_API_KEY",
            Some("gpt-test".to_string()),
        );

        let hint = provider_secret_setup_hint(&config);

        assert!(hint.contains("OPENAI_API_KEY"));
        assert!(!hint.contains("sk-live-secret"));
    }

    #[test]
    fn provider_model_and_base_url_text_is_sanitized_without_secrets() {
        assert_eq!(
            sanitize_optional_provider_text("  gemini-test  ").as_deref(),
            Some("gemini-test")
        );
        assert_eq!(sanitize_optional_provider_text("  "), None);
        assert_eq!(
            sanitize_optional_provider_base_url(" https://api.example.test/v1/ ").unwrap(),
            Some("https://api.example.test/v1".to_string())
        );
        assert!(sanitize_optional_provider_base_url("api.example.test").is_err());
        assert!(sanitize_optional_provider_base_url("https://token@example.test").is_err());
        assert!(sanitize_optional_provider_base_url("https://example.test?key=secret").is_err());
    }

    #[test]
    fn provider_default_model_placeholders_cover_live_llm_backends() {
        assert_eq!(
            provider_default_model_placeholder(&ProviderId::Gemini),
            "gemini-3.5-flash"
        );
        assert_eq!(
            provider_default_model_placeholder(&ProviderId::OpenAi),
            "gpt-5.5"
        );
        assert_eq!(
            provider_default_model_placeholder(&ProviderId::Anthropic),
            "claude-sonnet-4-6"
        );
    }

    #[test]
    fn provider_secret_setup_message_points_out_external_secret_boundary() {
        let message = platform_provider_secret_setup_message("OpenAI", "OPENAI_API_KEY");

        assert!(message.contains("OpenAI"));
        assert!(message.contains("OPENAI_API_KEY"));
        assert!(!message.contains("sk-live-secret"));
    }

    #[test]
    fn media_preview_kind_class_is_stable_for_css() {
        assert_eq!(
            media_preview_kind_class(MediaKind::Image),
            "media-preview-kind image"
        );
        assert_eq!(
            media_preview_kind_class(MediaKind::Model3d),
            "media-preview-kind model3d"
        );
    }

    #[test]
    fn media_error_messages_are_specific_and_stable() {
        assert!(media_error_message(MediaKind::Image).starts_with("Image preview failed"));
        assert!(media_error_message(MediaKind::Audio).starts_with("Audio preview failed"));
        assert!(media_error_message(MediaKind::Video).starts_with("Video preview failed"));
        assert!(media_error_message(MediaKind::Model3d).contains("3D preview failed"));
    }

    #[test]
    fn glb_model_viewer_srcdoc_escapes_inputs_and_loads_local_model_viewer_first() {
        let html = super::glb_model_viewer_srcdoc(
            "data:model/gltf-binary;base64,AA\"<&",
            "Inline \"GLB\" <test>",
        );

        assert!(html.contains("/vendor/model-viewer/4.3.1/model-viewer.min.js"));
        assert!(html.contains("@google/model-viewer@4.3.1"));
        assert!(
            html.contains("loadModule(localModuleUrl).catch(() => loadModule(fallbackModuleUrl))")
        );
        assert!(html.contains("<model-viewer"));
        assert!(html.contains("camera-controls"));
        assert!(html.contains("data:model/gltf-binary;base64,AA&quot;&lt;&amp;"));
        assert!(html.contains("Inline &quot;GLB&quot; &lt;test&gt;"));
    }

    #[test]
    fn copy_media_uri_script_escapes_uri_and_uses_clipboard_fallback() {
        let script = copy_media_uri_script("gemed-media://media/quote\"line\n.png");

        assert!(script.contains("navigator.clipboard.writeText"));
        assert!(script.contains("document.execCommand(\"copy\")"));
        assert!(script.contains(r#"const text = "gemed-media://media/quote\"line\n.png";"#));
    }

    #[test]
    fn copy_status_maps_eval_results_to_user_messages_and_css() {
        let copied = CopyStatus::from_eval_value(&serde_json::json!({ "ok": true }));
        let rejected =
            CopyStatus::from_eval_value(&serde_json::json!({ "ok": false, "error": "denied" }));

        assert_eq!(copied, CopyStatus::copied());
        assert_eq!(copied.class_name("media-copy-status"), "media-copy-status");
        assert_eq!(rejected.message, "Copy failed: denied.");
        assert_eq!(
            rejected.class_name("media-overlay-copy-status"),
            "media-overlay-copy-status err"
        );
    }

    #[test]
    fn glb_capture_request_requires_renderable_plan() {
        let workflow = WorkflowFile {
            name: "glb capture".to_string(),
            nodes: vec![WorkflowNode::new(
                "viewer",
                NodeType::GlbViewer,
                Position { x: 0.0, y: 0.0 },
                serde_json::json!({
                    "glbUrl": test_inline_glb_data_url(),
                    "filename": "inline.glb"
                }),
            )],
            ..WorkflowFile::blank()
        };
        let planned = gemed_executor::execute_simple_workflow(&workflow).expect("GLB plans");
        let viewer = planned
            .workflow
            .nodes
            .iter()
            .find(|node| node.id == "viewer")
            .expect("viewer exists");

        let request =
            glb_capture_request("viewer".to_string(), viewer).expect("inline GLB is renderable");

        assert_eq!(request.node_id, "viewer");
        assert!(
            request
                .source_uri
                .starts_with("data:model/gltf-binary;base64,")
        );
        assert_eq!(request.label, "inline.glb");

        let project_ref = WorkflowNode::new(
            "viewer",
            NodeType::GlbViewer,
            Position { x: 0.0, y: 0.0 },
            serde_json::json!({
                "glbUrl": "gemed-media://media/model.glb",
                "glbViewerPlan": {
                    "source": {
                        "uriKind": "projectReference",
                        "mime": "model/gltf-binary",
                        "renderableInWebview": false
                    },
                    "viewerAdapter": "webview-model-viewer",
                    "requiresWebglAdapter": true,
                    "canOpenUriDirectly": false,
                    "captureOutputMime": "image/png",
                    "requiresCaptureAdapter": true
                }
            }),
        );
        let err = glb_capture_request("viewer".to_string(), &project_ref)
            .expect_err("project refs need hydration first");
        assert!(err.contains("projectReference"));
    }

    #[test]
    fn glb_capture_script_declares_model_viewer_snapshot_contract() {
        let request = GlbCaptureRequest {
            node_id: "viewer".to_string(),
            source_uri: "data:model/gltf-binary;base64,AAAA".to_string(),
            label: "inline.glb".to_string(),
            timeout_ms: 1234,
        };

        let script = glb_capture_script(&request);

        assert!(script.contains("document.createElement(\"model-viewer\")"));
        assert!(script.contains("/vendor/model-viewer/4.3.1/model-viewer.min.js"));
        assert!(script.contains("@google/model-viewer@4.3.1"));
        assert!(script.contains("loadModelViewerModule(localModuleUrl)"));
        assert!(script.contains(".catch(() => loadModelViewerModule(fallbackModuleUrl))"));
        assert!(script.contains("model.toDataURL(\"image/png\")"));
        assert!(script.contains("URL.revokeObjectURL(objectUrl)"));
        assert!(script.contains("timeoutMs = 1234"));
        assert!(script.contains("\"webview-model-viewer\""));
    }

    #[test]
    fn glb_capture_success_updates_node_and_downstream_output() {
        let mut workflow = WorkflowFile {
            name: "glb capture".to_string(),
            nodes: vec![
                WorkflowNode::new(
                    "viewer",
                    NodeType::GlbViewer,
                    Position { x: 0.0, y: 0.0 },
                    serde_json::json!({
                        "glbUrl": "data:model/gltf-binary;base64,AAAA",
                        "capturedImage": null,
                        "glbViewerPlan": {}
                    }),
                ),
                WorkflowNode::new(
                    "output",
                    NodeType::Output,
                    Position { x: 0.0, y: 0.0 },
                    serde_json::json!({}),
                ),
                WorkflowNode::new(
                    "gallery",
                    NodeType::OutputGallery,
                    Position { x: 0.0, y: 0.0 },
                    serde_json::json!({ "images": [] }),
                ),
                WorkflowNode::new(
                    "legacy",
                    NodeType::Output,
                    Position { x: 0.0, y: 0.0 },
                    serde_json::json!({}),
                ),
            ],
            edges: vec![
                WorkflowEdge::with_handles("e1", "viewer", "output", "image", "image"),
                WorkflowEdge::with_handles("e2", "viewer", "gallery", "image", "image"),
                WorkflowEdge::new("e3", "viewer", "legacy"),
            ],
            ..WorkflowFile::blank()
        };
        let success = GlbCaptureSuccess {
            image: "data:image/png;base64,AAAA".to_string(),
            width: Some(640),
            height: Some(480),
        };

        let routed =
            apply_glb_capture_success(&mut workflow, "viewer", &success).expect("capture applies");
        let viewer = workflow
            .nodes
            .iter()
            .find(|node| node.id == "viewer")
            .unwrap();
        let output = workflow
            .nodes
            .iter()
            .find(|node| node.id == "output")
            .unwrap();
        let gallery = workflow
            .nodes
            .iter()
            .find(|node| node.id == "gallery")
            .unwrap();
        let legacy = workflow
            .nodes
            .iter()
            .find(|node| node.id == "legacy")
            .unwrap();

        assert_eq!(routed, 2);
        assert_eq!(
            viewer.data.get("capturedImage").and_then(Value::as_str),
            Some("data:image/png;base64,AAAA")
        );
        assert_eq!(
            viewer.data.get("__mediaAdapter").and_then(Value::as_str),
            Some("webview-model-viewer")
        );
        assert_eq!(
            viewer
                .data
                .get("glbCaptureResult")
                .and_then(|result| result.get("width"))
                .and_then(Value::as_u64),
            Some(640)
        );
        assert_eq!(
            output.data.get("image").and_then(Value::as_str),
            Some("data:image/png;base64,AAAA")
        );
        assert_eq!(
            gallery
                .data
                .get("images")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(Value::as_str),
            Some("data:image/png;base64,AAAA")
        );
        assert_eq!(
            legacy.data.get("image").and_then(Value::as_str),
            None,
            "GLB capture must not route snapshots through legacy/no-handle edges; those remain 3D model edges"
        );

        let insight = node_card_insight(viewer).expect("GLB insight remains available");
        assert_eq!(insight.class, "node-insight ready");
        assert!(
            insight
                .lines
                .iter()
                .any(|line| line.contains("webview-model-viewer emitted PNG snapshot"))
        );
    }

    #[test]
    fn glb_capture_eval_parser_rejects_failed_or_non_png_results() {
        let failed = glb_capture_success_from_eval_value(
            &serde_json::json!({ "ok": false, "error": "snapshot failed" }),
        )
        .expect_err("adapter errors propagate");
        let non_png = glb_capture_success_from_eval_value(
            &serde_json::json!({ "ok": true, "image": "data:image/jpeg;base64,AAAA" }),
        )
        .expect_err("only PNG GLB captures are accepted");

        assert_eq!(failed, "snapshot failed");
        assert!(non_png.contains("PNG data URL"));
    }

    #[test]
    fn video_frame_capture_request_requires_renderable_plan() {
        let workflow = WorkflowFile::video_frame_grab_example();
        let planned =
            gemed_executor::execute_simple_workflow(&workflow).expect("frame sample plans");
        let grab = planned
            .workflow
            .nodes
            .iter()
            .find(|node| node.id == "frame_grab")
            .expect("frame grab exists");

        let request = video_frame_capture_request("frame_grab".to_string(), grab)
            .expect("inline video plan is renderable");

        assert_eq!(request.node_id, "frame_grab");
        assert!(request.source_uri.starts_with("data:video/mp4;base64,"));
        assert_eq!(request.frame_position, "first");
        assert_eq!(request.requested_seek_seconds, Some(0.001));

        let project_ref = WorkflowNode::new(
            "grab",
            NodeType::VideoFrameGrab,
            Position { x: 0.0, y: 0.0 },
            serde_json::json!({
                "sourceVideo": "gemed-media://media/clip.mp4",
                "frameGrabPlan": {
                    "source": {
                        "uriKind": "projectReference",
                        "mime": "video/mp4",
                        "renderableInWebview": false
                    },
                    "framePosition": "first",
                    "requestedSeekSeconds": 0.001,
                    "seekRequiresDuration": false,
                    "outputMime": "image/png",
                    "requiresDecodeAdapter": true
                }
            }),
        );
        let err = video_frame_capture_request("grab".to_string(), &project_ref)
            .expect_err("project refs need hydration first");
        assert!(err.contains("projectReference"));
    }

    #[test]
    fn video_frame_capture_script_declares_real_video_canvas_contract() {
        let request = VideoFrameCaptureRequest {
            node_id: "grab".to_string(),
            source_uri: "data:video/mp4;base64,AAAA".to_string(),
            frame_position: "last".to_string(),
            requested_seek_seconds: None,
            timeout_ms: 1234,
        };

        let script = video_frame_capture_script(&request);

        assert!(script.contains("document.createElement(\"video\")"));
        assert!(script.contains("document.createElement(\"canvas\")"));
        assert!(script.contains("context.drawImage(video"));
        assert!(script.contains("canvas.toDataURL(\"image/png\")"));
        assert!(script.contains("URL.revokeObjectURL(blobUrl)"));
        assert!(script.contains("timeoutMs = 1234"));
        assert!(script.contains("\"webview-video-canvas\""));
    }

    #[test]
    fn video_frame_capture_success_updates_node_and_downstream_output() {
        let mut workflow = WorkflowFile {
            name: "capture".to_string(),
            nodes: vec![
                WorkflowNode::new(
                    "grab",
                    NodeType::VideoFrameGrab,
                    Position { x: 0.0, y: 0.0 },
                    serde_json::json!({
                        "sourceVideo": "data:video/mp4;base64,AAAA",
                        "outputImage": null,
                        "frameGrabPlan": {}
                    }),
                ),
                WorkflowNode::new(
                    "output",
                    NodeType::Output,
                    Position { x: 0.0, y: 0.0 },
                    serde_json::json!({}),
                ),
            ],
            edges: vec![WorkflowEdge::with_handles(
                "e1", "grab", "output", "image", "image",
            )],
            ..WorkflowFile::blank()
        };
        let success = VideoFrameCaptureSuccess {
            image: "data:image/png;base64,AAAA".to_string(),
            width: Some(16),
            height: Some(9),
            seek_seconds: Some(0.001),
        };

        let routed = apply_video_frame_capture_success(&mut workflow, "grab", &success)
            .expect("capture applies");
        let grab = workflow
            .nodes
            .iter()
            .find(|node| node.id == "grab")
            .unwrap();
        let output = workflow
            .nodes
            .iter()
            .find(|node| node.id == "output")
            .unwrap();

        assert_eq!(routed, 1);
        assert_eq!(
            grab.data.get("outputImage").and_then(Value::as_str),
            Some("data:image/png;base64,AAAA")
        );
        assert_eq!(
            grab.data.get("__mediaAdapter").and_then(Value::as_str),
            Some("webview-video-canvas")
        );
        assert_eq!(
            grab.data
                .get("frameCaptureResult")
                .and_then(|result| result.get("width"))
                .and_then(Value::as_u64),
            Some(16)
        );
        assert_eq!(
            output.data.get("image").and_then(Value::as_str),
            Some("data:image/png;base64,AAAA")
        );
        assert_eq!(
            output.data.get("contentType").and_then(Value::as_str),
            Some("image")
        );

        let insight = node_card_insight(grab).expect("frame grab insight remains available");
        assert_eq!(insight.class, "node-insight ready");
        assert!(
            insight
                .lines
                .iter()
                .any(|line| line.contains("webview-video-canvas emitted PNG output"))
        );
    }

    #[test]
    fn video_frame_capture_eval_parser_rejects_failed_or_non_png_results() {
        let failed = video_frame_capture_success_from_eval_value(
            &serde_json::json!({ "ok": false, "error": "decode failed" }),
        )
        .expect_err("adapter errors propagate");
        let non_png = video_frame_capture_success_from_eval_value(
            &serde_json::json!({ "ok": true, "image": "data:image/jpeg;base64,AAAA" }),
        )
        .expect_err("only PNG frame captures are accepted");

        assert_eq!(failed, "decode failed");
        assert!(non_png.contains("PNG data URL"));
    }

    #[test]
    fn media_overlay_supports_inline_renderable_image_audio_video() {
        let workflow = WorkflowFile::media_preview_example();
        let image_preview = workflow
            .nodes
            .iter()
            .flat_map(media_previews_for_node)
            .find(|preview| preview.kind == MediaKind::Image && preview.should_inline_preview())
            .expect("media sample has an inline image");
        let audio_preview = workflow
            .nodes
            .iter()
            .flat_map(media_previews_for_node)
            .find(|preview| preview.kind == MediaKind::Audio)
            .expect("media sample has audio");
        let video_preview = workflow
            .nodes
            .iter()
            .flat_map(media_previews_for_node)
            .find(|preview| preview.kind == MediaKind::Video)
            .expect("media sample has video");

        let image_overlay = media_overlay_from_preview(&image_preview).expect("image has overlay");
        let audio_overlay = media_overlay_from_preview(&audio_preview).expect("audio has overlay");
        let video_overlay = media_overlay_from_preview(&video_preview).expect("video has overlay");

        assert_eq!(image_overlay.kind, MediaKind::Image);
        assert_eq!(image_overlay.label, "Inline SVG Image");
        assert_eq!(image_overlay.source_field, "image");
        assert_eq!(image_overlay.download_filename, "inline-svg-image.svg");

        assert_eq!(audio_overlay.kind, MediaKind::Audio);
        assert_eq!(audio_overlay.source_field, "audioFile");
        assert_eq!(audio_overlay.download_filename, "inline-wav-audio.wav");

        assert_eq!(video_overlay.kind, MediaKind::Video);
        assert_eq!(video_overlay.source_field, "video");
        assert_eq!(video_overlay.download_filename, "inline-mp4-video.mp4");
    }

    #[test]
    fn media_overlay_supports_renderable_glb_and_excludes_refs_or_large_payloads() {
        let model_preview = MediaPreview {
            kind: MediaKind::Model3d,
            label: "Inline GLB".to_string(),
            uri: test_inline_glb_data_url().to_string(),
            source_field: "glbUrl".to_string(),
        };
        let project_ref_preview = MediaPreview {
            kind: MediaKind::Image,
            label: "Project Image".to_string(),
            uri: "gemed-media://media/example.png".to_string(),
            source_field: "imageRef".to_string(),
        };
        let large_inline_preview = MediaPreview {
            kind: MediaKind::Image,
            label: "Large Inline".to_string(),
            uri: format!("data:image/png;base64,{}", "A".repeat(800 * 1024)),
            source_field: "image".to_string(),
        };

        let model_overlay = media_overlay_from_preview(&model_preview).expect("GLB has overlay");
        assert_eq!(model_overlay.kind, MediaKind::Model3d);
        assert_eq!(model_overlay.download_filename, "inline-glb.glb");
        assert!(media_overlay_from_preview(&project_ref_preview).is_none());
        assert!(media_overlay_from_preview(&large_inline_preview).is_none());
    }

    #[test]
    fn provider_sample_runs_offline_with_mock_defaults() {
        let workflow = WorkflowFile::llm_provider_example();
        let providers = gemed_providers::ProviderRegistry::mock_from_config(
            &gemed_providers::ProviderConfigSet::mock_all(),
        );

        let result = gemed_executor::execute_workflow_with_providers(&workflow, &providers)
            .expect("provider sample runs through mock registry");

        assert_eq!(result.report.error_count(), 0);
        assert_eq!(result.report.skipped_count(), 0);
        for node_id in [
            "provider_gemini_output",
            "provider_openai_output",
            "provider_anthropic_output",
        ] {
            assert!(
                result
                    .workflow
                    .nodes
                    .iter()
                    .find(|node| node.id == node_id)
                    .and_then(|node| node.data.get("text"))
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|text| text.starts_with("[mock:")),
                "{node_id} should receive deterministic mock provider text"
            );
        }
    }

    #[test]
    fn multimodal_provider_sample_runs_image_video_audio_and_3d_mocks() {
        let workflow = WorkflowFile::multimodal_provider_example();
        let providers = gemed_providers::ProviderRegistry::mock_from_config(
            &gemed_providers::ProviderConfigSet::mock_all(),
        );

        let result = gemed_executor::execute_workflow_with_providers(&workflow, &providers)
            .expect("multimodal provider sample runs through mock registry");

        assert_eq!(result.report.error_count(), 0);
        assert_eq!(result.report.skipped_count(), 0);
        assert_eq!(result.report.loading_count(), workflow.nodes.len());
        let node_value = |node_id: &str, key: &str| {
            result
                .workflow
                .nodes
                .iter()
                .find(|node| node.id == node_id)
                .and_then(|node| node.data.get(key))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        };

        assert!(
            node_value("provider_image_output", "image")
                .is_some_and(|value| value.starts_with("mock://image/mock/mock-image"))
        );
        assert!(
            node_value("provider_video_output", "video")
                .is_some_and(|value| value.starts_with("mock://video/mock/mock-video"))
        );
        assert!(
            node_value("provider_audio_output", "audio")
                .is_some_and(|value| value.starts_with("mock://audio/mock/mock-audio"))
        );
        assert!(
            node_value("provider_3d_output", "model3d")
                .is_some_and(|value| value.starts_with("mock://3d/mock/mock-3d"))
        );
    }

    #[test]
    fn release_smoke_create_save_load_run_local_and_mock_provider_paths() {
        let opened_example = WorkflowFile::example();
        opened_example.validate().expect("built-in example opens");

        let mut created = WorkflowFile {
            name: "Release Smoke Created Workflow".to_string(),
            nodes: vec![
                WorkflowNode::new(
                    "smoke_prompt",
                    NodeType::Prompt,
                    Position { x: 80.0, y: 100.0 },
                    serde_json::json!({
                        "label": "Smoke Prompt",
                        "text": "release smoke text"
                    }),
                ),
                WorkflowNode::new(
                    "smoke_output",
                    NodeType::Output,
                    Position { x: 420.0, y: 100.0 },
                    serde_json::json!({
                        "label": "Smoke Output",
                        "contentType": "text"
                    }),
                ),
            ],
            ..WorkflowFile::blank()
        };
        let edge = add_edge_between(
            &mut created,
            "smoke_prompt",
            "smoke_output",
            Some("text".to_string()),
            Some("text".to_string()),
        )
        .expect("created workflow connects handles");
        assert_eq!(edge.source_handle.as_deref(), Some("text"));
        assert_eq!(edge.target_handle.as_deref(), Some("text"));

        let mut storage = MemoryWorkflowStorage::new();
        let snapshot = storage
            .save_workflow("release-smoke", &created)
            .expect("created workflow saves");
        assert_eq!(snapshot.slot, "release-smoke");
        assert!(snapshot.json.contains("Release Smoke Created Workflow"));

        let loaded = storage
            .load_workflow("release-smoke")
            .expect("created workflow loads");
        assert_eq!(loaded.name, created.name);
        assert_eq!(loaded.nodes.len(), 2);
        assert_eq!(loaded.edges.len(), 1);

        let local_result =
            gemed_executor::execute_simple_workflow(&loaded).expect("created workflow runs");
        assert_eq!(local_result.report.error_count(), 0);
        assert_eq!(local_result.report.skipped_count(), 0);
        assert_eq!(local_result.report.loading_count(), loaded.nodes.len());
        assert_eq!(local_result.report.executed_count(), loaded.nodes.len());
        let output = local_result
            .workflow
            .nodes
            .iter()
            .find(|node| node.id == "smoke_output")
            .expect("smoke output exists");
        assert_eq!(
            output.data.get("text").and_then(Value::as_str),
            Some("release smoke text")
        );

        let provider_workflow = WorkflowFile::llm_provider_example();
        let providers = gemed_providers::ProviderRegistry::mock_from_config(
            &gemed_providers::ProviderConfigSet::mock_all(),
        );
        let provider_result =
            gemed_executor::execute_workflow_with_providers(&provider_workflow, &providers)
                .expect("mock provider workflow runs");

        assert_eq!(provider_result.report.error_count(), 0);
        assert_eq!(provider_result.report.skipped_count(), 0);
        assert_eq!(
            provider_result.report.loading_count(),
            provider_workflow.nodes.len()
        );
        assert!(
            provider_result
                .workflow
                .nodes
                .iter()
                .find(|node| node.id == "provider_gemini_output")
                .and_then(|node| node.data.get("text"))
                .and_then(Value::as_str)
                .is_some_and(|text| text.starts_with("[mock:gemini:gemini-3.5-flash]"))
        );
    }

    #[test]
    fn video_frame_grab_sample_runs_as_planning_boundary() {
        let workflow = WorkflowFile::video_frame_grab_example();
        let result = gemed_executor::execute_simple_workflow(&workflow)
            .expect("video frame sample planning runs");
        let grab = result
            .workflow
            .nodes
            .iter()
            .find(|node| node.id == "frame_grab")
            .expect("frame grab node exists");

        assert_eq!(result.report.error_count(), 0);
        assert_eq!(result.report.skipped_count(), 0);
        assert_eq!(
            grab.data
                .get("__mediaAdapter")
                .and_then(serde_json::Value::as_str),
            Some("rust-video-frame-grab-plan")
        );
        assert!(
            grab.data
                .get("frameGrabPlan")
                .and_then(|plan| plan.get("requiresDecodeAdapter"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        );
        assert!(
            grab.data
                .get("outputImage")
                .is_some_and(serde_json::Value::is_null)
        );
    }

    #[test]
    fn node_card_insight_summarizes_frame_grab_plan_without_fake_output() {
        let workflow = WorkflowFile::video_frame_grab_example();
        let result =
            gemed_executor::execute_simple_workflow(&workflow).expect("frame sample plans");
        let grab = result
            .workflow
            .nodes
            .iter()
            .find(|node| node.id == "frame_grab")
            .expect("frame grab node exists");

        let insight = node_card_insight(grab).expect("frame grab insight exists");

        assert_eq!(insight.title, "Frame grab plan");
        assert_eq!(insight.class, "node-insight adapter");
        assert!(
            insight
                .lines
                .iter()
                .any(|line| line.contains("inlineData") && line.contains("video/mp4"))
        );
        assert!(
            insight
                .lines
                .iter()
                .any(|line| line.contains("first frame at 0.001s"))
        );
        assert!(
            insight
                .lines
                .iter()
                .any(|line| line.contains("no output image emitted"))
        );
    }

    #[test]
    fn node_card_insight_summarizes_glb_viewer_plan_without_fake_capture() {
        let workflow = WorkflowFile {
            name: "glb insight".to_string(),
            nodes: vec![WorkflowNode::new(
                "viewer",
                NodeType::GlbViewer,
                Position { x: 0.0, y: 0.0 },
                serde_json::json!({
                    "glbUrl": test_inline_glb_data_url(),
                    "filename": "inline.glb"
                }),
            )],
            ..WorkflowFile::blank()
        };
        let result = gemed_executor::execute_simple_workflow(&workflow).expect("glb viewer plans");
        let viewer = result
            .workflow
            .nodes
            .iter()
            .find(|node| node.id == "viewer")
            .expect("viewer exists");

        let insight = node_card_insight(viewer).expect("glb viewer insight exists");

        assert_eq!(insight.title, "GLB viewer plan");
        assert_eq!(insight.class, "node-insight adapter");
        assert!(
            insight
                .lines
                .iter()
                .any(|line| line.contains("inlineData") && line.contains("model/gltf-binary"))
        );
        assert!(
            insight
                .lines
                .iter()
                .any(|line| line.contains("metadata: GLB v2") && line.contains("glTF 2.0"))
        );
        assert!(insight.lines.iter().any(|line| {
            line.contains("assets: scenes 1")
                && line.contains("nodes 1")
                && line.contains("meshes 1")
        }));
        assert!(
            insight
                .lines
                .iter()
                .any(|line| line.contains("model-viewer adapter can open this URI"))
        );
        assert!(
            insight
                .lines
                .iter()
                .any(|line| line.contains("no captured image emitted"))
        );
    }

    fn test_inline_glb_data_url() -> &'static str {
        "data:model/gltf-binary;base64,Z2xURgIAAACsAAAAmAAAAEpTT057ImFzc2V0Ijp7InZlcnNpb24iOiIyLjAiLCJnZW5lcmF0b3IiOiJHZW1FZCB0ZXN0In0sInNjZW5lcyI6W3t9XSwibm9kZXMiOlt7fV0sIm1lc2hlcyI6W3t9XSwibWF0ZXJpYWxzIjpbe31dLCJhbmltYXRpb25zIjpbXSwiaW1hZ2VzIjpbXSwiYnVmZmVycyI6W119IA=="
    }

    #[test]
    fn node_card_insight_summarizes_split_grid_cells_and_children() {
        let node = WorkflowNode::new(
            "split",
            NodeType::SplitGrid,
            Position { x: 0.0, y: 0.0 },
            serde_json::json!({
                "targetCount": 3,
                "images": [
                    "data:image/png;base64,a",
                    "data:image/png;base64,b"
                ],
                "childNodeIds": [
                    {
                        "imageInput": "cell_1_image",
                        "prompt": "cell_1_prompt",
                        "nanoBanana": "cell_1_generate"
                    }
                ],
                "__mediaAdapter": "rust-inline-image-grid"
            }),
        );

        let insight = node_card_insight(&node).expect("split grid insight exists");

        assert_eq!(insight.title, "Split grid cells");
        assert_eq!(insight.class, "node-insight ready");
        assert_eq!(insight.lines[0], "cells: 2/3 populated");
        assert!(
            insight
                .lines
                .iter()
                .any(|line| line == "children: 1 generated cell set(s)")
        );
        assert!(
            insight
                .lines
                .iter()
                .any(|line| line == "adapter: rust-inline-image-grid")
        );
    }

    #[test]
    fn split_grid_child_bounds_and_viewport_cover_generated_child_cluster() {
        let mut workflow = WorkflowFile::media_transform_example();
        let generated = generate_split_grid_children(&mut workflow, "transform_split")
            .expect("split children generate");
        let first = generated.child_node_ids.first().expect("first child set");
        let node_ids = vec![
            first.image_input.clone(),
            first.prompt.clone(),
            first.nano_banana.clone(),
        ];

        let bounds = bounds_for_node_ids(&workflow, &node_ids).expect("bounds exist");
        let viewport = viewport_for_node_ids(&workflow, &node_ids).expect("viewport exists");

        assert!(bounds.width > NODE_CARD_WIDTH);
        assert!(bounds.height > NODE_CARD_HEIGHT);
        assert_eq!(viewport.zoom, 0.88);
        assert!(viewport.pan_x.is_finite());
        assert!(viewport.pan_y.is_finite());
    }

    #[cfg(feature = "desktop")]
    #[test]
    fn desktop_project_transform_sample_runs_after_project_roundtrip() {
        let workflow = WorkflowFile::media_transform_example();
        let executed = gemed_executor::execute_simple_workflow(&workflow)
            .expect("transform sample runs before save")
            .workflow;
        let root = unique_test_project_dir("gemed-transform-project-roundtrip");
        let project = gemed_storage::desktop::DesktopWorkflowProject::at_dir(&root);

        let snapshot = project.save(&executed).expect("save transform project");

        assert!(!snapshot.manifest.media_files.is_empty());
        let saved_json = std::fs::read_to_string(root.join(gemed_storage::PROJECT_WORKFLOW_FILE))
            .expect("saved workflow json exists");
        assert!(saved_json.contains(gemed_storage::PROJECT_MEDIA_URL_PREFIX));
        assert!(!saved_json.contains("data:image/png"));

        let loaded = project.load().expect("load transform project").workflow;
        let rerun = gemed_executor::execute_simple_workflow(&loaded)
            .expect("transform sample reruns after hydration");
        let split = rerun
            .workflow
            .nodes
            .iter()
            .find(|node| node.id == "transform_split")
            .expect("split node exists");
        let images = split
            .data
            .get("images")
            .and_then(serde_json::Value::as_array)
            .expect("split images remain hydrated and executable");

        assert_eq!(images.len(), 4);
        assert!(images.iter().all(|image| {
            image
                .as_str()
                .is_some_and(|value| value.starts_with("data:image/png;base64,"))
        }));
        assert_eq!(rerun.report.error_count(), 0);

        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(feature = "desktop")]
    fn unique_test_project_dir(prefix: &str) -> std::path::PathBuf {
        let unique = format!(
            "{}-{}",
            prefix,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        std::env::temp_dir().join(unique)
    }
}
