use dioxus::html::{
    InteractionElementOffset, InteractionLocation, ModifiersInteraction, MouseEvent,
    PointerInteraction, WheelEvent, geometry::WheelDelta, input_data::MouseButton,
};
use dioxus::prelude::*;
use gemed_core::{
    GroupColor, NodeGroup, NodeStatus, Position, Size, WorkflowEdge, WorkflowFile, WorkflowNode,
    WorkflowUndoStack, add_edge_between, create_group_for_nodes, is_node_in_locked_group,
    move_group_by, move_node_by, remove_edge, resize_group_by, select_node, selected_node_id,
    selected_node_ids, set_group_size, set_node_position, source_handle_options,
    target_handle_options, toggle_group_lock, toggle_node_selection,
};
use gemed_executor::{
    SimpleExecutionReport, execute_simple_workflow, execute_workflow_with_providers,
    execution_order,
};
use gemed_media::{MediaKind, MediaPreview, media_previews_for_node, workflow_media_summary};
#[cfg(all(feature = "desktop", feature = "providers-http"))]
use gemed_providers::OpenAiResponsesProvider;
use gemed_providers::{
    ProviderCapability, ProviderConfig, ProviderConfigSet, ProviderId, ProviderRegistry,
    ProviderRuntimeMode, ProviderSecretSource,
};
use gemed_storage::{
    DEFAULT_AUTOSAVE_SLOT, DEFAULT_PROVIDER_CONFIG_SLOT, ProviderConfigSnapshot,
    ProviderConfigStorage, WorkflowSnapshot, WorkflowStorage,
};

const CANVAS_WIDTH: f64 = 1400.0;
const CANVAS_HEIGHT: f64 = 900.0;
const NODE_CARD_WIDTH: f64 = 248.0;
const NODE_CARD_HEIGHT: f64 = 128.0;
const GROUP_SELECTION_MIN_SIZE: f64 = 18.0;

const APP_CSS: &str = r#"
:root {
  color-scheme: dark;
  font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  background: #0b1020;
  color: #e5ecff;
}
* { box-sizing: border-box; }
body { margin: 0; min-height: 100vh; background: radial-gradient(circle at top left, #1f2a44 0, #0b1020 36rem); }
button, textarea { font: inherit; }
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

    rsx! {
        style { "{APP_CSS}" }
        div { class: "app",
            Header { workflow, json_text, message, execution_report, undo_stack, drag_state, connection_draft, provider_config }
            main { class: "main",
                Sidebar { workflow, json_text, message, execution_report, undo_stack, viewport, connection_draft, provider_config }
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
) -> Element {
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
                    onclick: move |_| {
                        let current = workflow.read().clone();
                        match execute_simple_workflow(&current) {
                        Ok(result) => {
                            let summary = result.report.summary();
                            match result.workflow.to_pretty_json() {
                                Ok(json) => json_text.set(json),
                                Err(err) => message.set(Message::err(format!("Executed but failed to export JSON: {err}"))),
                            }
                            workflow.set(result.workflow);
                            execution_report.set(Some(result.report));
                            undo_stack.write().clear();
                            drag_state.set(None);
                            connection_draft.set(None);
                            message.set(Message::ok(format!("Local executor finished: {summary}.")));
                        }
                        Err(err) => {
                            execution_report.set(None);
                            message.set(Message::err(format!("Local executor failed: {err}")));
                        }
                    }
                    },
                    "Run Local"
                }
                button {
                    class: "action",
                    onclick: move |_| {
                        let current = workflow.read().clone();
                        let active_provider_config = provider_config.read().clone();
                        let provider_summary = active_provider_config
                            .summary_with(provider_secret_env_value)
                            .sentence();
                        match build_provider_registry(&active_provider_config) {
                            Ok(providers) => match execute_workflow_with_providers(&current, &providers) {
                            Ok(result) => {
                                let summary = result.report.summary();
                                match result.workflow.to_pretty_json() {
                                    Ok(json) => json_text.set(json),
                                    Err(err) => message.set(Message::err(format!("Executed with providers but failed to export JSON: {err}"))),
                                }
                                workflow.set(result.workflow);
                                execution_report.set(Some(result.report));
                                undo_stack.write().clear();
                                drag_state.set(None);
                                connection_draft.set(None);
                                message.set(Message::ok(format!("Provider run finished: {summary}. {provider_summary}")));
                            }
                            Err(err) => {
                                execution_report.set(None);
                                message.set(Message::err(format!("Provider run failed: {err}")));
                            }
                            },
                            Err(err) => {
                                execution_report.set(None);
                                message.set(Message::err(format!("Provider registry failed: {err}")));
                            }
                        }
                    },
                    "Run Providers"
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
                MediaPreviewStrip { previews: media_previews, media_overlay }
                div { class: "node-id", "{node.id}" }
            }
        }
    }
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
            } else if preview.kind == MediaKind::Model3d {
                div { class: "media-preview-placeholder",
                    "3D model reference detected. GLB/WebGL preview adapter is still pending."
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
                        } else {
                            div { class: "media-overlay-placeholder",
                                "No inline overlay adapter is available for this media kind yet."
                            }
                        }
                        if matches!(overlay.kind, MediaKind::Image | MediaKind::Audio | MediaKind::Video) && load_error() {
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
        MediaKind::Image | MediaKind::Audio | MediaKind::Video
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
            "3D preview requires a GLB/WebGL adapter before it can be loaded inline."
        }
    }
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
        if provider.id != ProviderId::OpenAi
            || provider.runtime_mode != ProviderRuntimeMode::DirectDesktop
        {
            continue;
        }
        match OpenAiResponsesProvider::from_config_with_secret(provider, &provider_secret_env_value)
        {
            Ok(Some(backend)) => registry.register(backend),
            Ok(None) => {}
            Err(err) => return Err(err.to_string()),
        }
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
    let replacement = match mode {
        ProviderSettingsMode::Mock => ProviderConfig::mock(id.clone()),
        ProviderSettingsMode::DesktopEnv => platform_env_provider_config(id.clone()),
        ProviderSettingsMode::Disabled => ProviderConfig::disabled(id.clone()),
    };
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
        CanvasRect, CopyStatus, copy_media_uri_script, ensure_json_extension, media_error_message,
        media_overlay_from_preview, media_preview_kind_class, node_ids_intersecting_rect,
        platform_provider_secret_setup_message, provider_capability_list,
        provider_secret_setup_hint, workflow_json_filename,
    };
    use gemed_core::{Position, WorkflowFile};
    use gemed_media::{MediaKind, MediaPreview, media_previews_for_node};
    use gemed_providers::{ProviderCapability, ProviderConfig, ProviderId};
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
        assert!(media_error_message(MediaKind::Model3d).contains("GLB/WebGL adapter"));
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
    fn media_overlay_excludes_adapter_refs_and_large_inline_payloads() {
        let model_preview = MediaPreview {
            kind: MediaKind::Model3d,
            label: "Project GLB".to_string(),
            uri: "https://example.test/demo.glb".to_string(),
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

        assert!(media_overlay_from_preview(&model_preview).is_none());
        assert!(media_overlay_from_preview(&project_ref_preview).is_none());
        assert!(media_overlay_from_preview(&large_inline_preview).is_none());
    }
}
