use dioxus::html::{InteractionLocation, MouseEvent};
use dioxus::prelude::*;
use gemed_core::{
    NodeStatus, Position, WorkflowEdge, WorkflowFile, WorkflowNode, WorkflowUndoStack,
    add_edge_between, move_node_by, remove_edge, select_node, selected_node_id, set_node_position,
    source_handle_options, target_handle_options,
};
use gemed_executor::{
    SimpleExecutionReport, execute_simple_workflow, execute_workflow_with_providers,
    execution_order,
};
use gemed_providers::ProviderRegistry;
use gemed_storage::{DEFAULT_AUTOSAVE_SLOT, WorkflowSnapshot, WorkflowStorage};

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
.header { height: 4.5rem; display: flex; align-items: center; justify-content: space-between; padding: 0 1.5rem; border-bottom: 1px solid rgba(148, 163, 184, .18); background: rgba(11, 16, 32, .78); backdrop-filter: blur(14px); }
.brand { display: flex; align-items: baseline; gap: .75rem; }
.brand h1 { margin: 0; font-size: 1.45rem; letter-spacing: -.03em; }
.brand span { color: #93a4c8; font-size: .9rem; }
.actions { display: flex; gap: .65rem; align-items: center; }
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
.canvas { position: relative; width: 1400px; height: 900px; margin: 1.25rem; }
.edge-layer { position: absolute; inset: 0; width: 1400px; height: 900px; pointer-events: none; overflow: visible; }
.edge { stroke: rgba(125, 211, 252, .64); stroke-width: 2.5; fill: none; marker-end: url(#arrow); }
.node { position: absolute; width: 15.5rem; min-height: 8rem; border-radius: 1rem; border: 1px solid rgba(148, 163, 184, .24); background: linear-gradient(145deg, rgba(30, 41, 59, .96), rgba(15, 23, 42, .96)); box-shadow: 0 22px 60px rgba(0, 0, 0, .34); overflow: visible; }
.node.draggable { cursor: grab; user-select: none; }
.node.dragging { cursor: grabbing; opacity: .92; }
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
    let viewport = use_signal(CanvasViewport::default);
    let connection_draft = use_signal(|| None::<ConnectionDraft>);

    rsx! {
        style { "{APP_CSS}" }
        div { class: "app",
            Header { workflow, json_text, message, execution_report, undo_stack, drag_state, connection_draft }
            main { class: "main",
                Sidebar { workflow, json_text, message, execution_report, undo_stack, viewport, connection_draft }
                WorkflowCanvas { workflow, json_text, message, undo_stack, drag_state, viewport, connection_draft }
            }
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
                        let providers = ProviderRegistry::mock_all();
                        match execute_workflow_with_providers(&current, &providers) {
                            Ok(result) => {
                                let summary = result.report.summary();
                                match result.workflow.to_pretty_json() {
                                    Ok(json) => json_text.set(json),
                                    Err(err) => message.set(Message::err(format!("Executed with mocks but failed to export JSON: {err}"))),
                                }
                                workflow.set(result.workflow);
                                execution_report.set(Some(result.report));
                                undo_stack.write().clear();
                                drag_state.set(None);
                                connection_draft.set(None);
                                message.set(Message::ok(format!("Mock provider run finished: {summary}.")));
                            }
                            Err(err) => {
                                execution_report.set(None);
                                message.set(Message::err(format!("Mock provider run failed: {err}")));
                            }
                        }
                    },
                    "Run Mocks"
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
fn Sidebar(
    mut workflow: Signal<WorkflowFile>,
    mut json_text: Signal<String>,
    mut message: Signal<Message>,
    execution_report: Signal<Option<SimpleExecutionReport>>,
    mut undo_stack: Signal<WorkflowUndoStack>,
    mut viewport: Signal<CanvasViewport>,
    mut connection_draft: Signal<Option<ConnectionDraft>>,
) -> Element {
    let wf = workflow.read();
    let counts = wf.node_type_counts();
    let can_undo = undo_stack.read().can_undo();
    let can_redo = undo_stack.read().can_redo();
    let viewport_snapshot = *viewport.read();
    let zoom_percent = (viewport_snapshot.zoom * 100.0).round() as i32;
    let selected_id = selected_node_id(&wf).map(ToOwned::to_owned);
    let selected_index = selected_id
        .as_ref()
        .and_then(|id| wf.nodes.iter().position(|node| node.id == *id));
    let next_node_id = selected_index.and_then(|index| {
        (!wf.nodes.is_empty()).then(|| wf.nodes[(index + 1) % wf.nodes.len()].id.clone())
    });
    let selected_summary = selected_id
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
        .unwrap_or_else(|| "No node selected. Click a card in the canvas.".to_string());
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
                h2 { "Canvas MVP" }
                p { "{selected_summary}" }
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
                        disabled: selected_id.is_none(),
                        onclick: move |_| {
                            mutate_selected_node(&mut workflow, &mut json_text, &mut message, &mut undo_stack, -32.0, 0.0);
                        },
                        "←"
                    }
                    button {
                        class: "action",
                        disabled: selected_id.is_none(),
                        onclick: move |_| {
                            mutate_selected_node(&mut workflow, &mut json_text, &mut message, &mut undo_stack, 32.0, 0.0);
                        },
                        "→"
                    }
                    button {
                        class: "action",
                        disabled: selected_id.is_none(),
                        onclick: move |_| {
                            mutate_selected_node(&mut workflow, &mut json_text, &mut message, &mut undo_stack, 0.0, -32.0);
                        },
                        "↑"
                    }
                    button {
                        class: "action",
                        disabled: selected_id.is_none(),
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
                                                mutate_workflow(&mut workflow, &mut json_text, &mut message, &mut undo_stack, move |workflow| {
                                                    remove_edge(workflow, &edge_id)
                                                        .map(|edge| format!("Removed edge `{}` ({} → {}).", edge.id, edge.source, edge.target))
                                                        .map_err(|err| err.to_string())
                                                });
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
                    p { "Run Local executes pure Rust prompt/array/output/control nodes and explicitly skips unregistered providers. Run Mocks wires mock provider traits for generation/LLM smoke tests without secrets." }
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
    viewport: Signal<CanvasViewport>,
    connection_draft: Signal<Option<ConnectionDraft>>,
) -> Element {
    let wf = workflow.read();
    let viewport_snapshot = *viewport.read();
    let canvas_style = format!(
        "transform: translate({:.1}px, {:.1}px) scale({:.3}); transform-origin: 0 0;",
        viewport_snapshot.pan_x, viewport_snapshot.pan_y, viewport_snapshot.zoom
    );

    rsx! {
        section { class: "canvas-wrap",
            if wf.nodes.is_empty() {
                div { class: "empty",
                    div {
                        h2 { "Blank workflow" }
                        p { "Use the JSON panel or Sample button to load nodes into the Dioxus canvas." }
                    }
                }
            } else {
                div {
                    class: "canvas",
                    style: "{canvas_style}",
                    onmousemove: move |event: MouseEvent| {
                        update_dragged_node(event, workflow, json_text, drag_state);
                    },
                    onmouseup: move |_| {
                        finish_drag(workflow, json_text, message, drag_state);
                        cancel_canvas_connection(message, connection_draft);
                    },
                    svg { class: "edge-layer", view_box: "0 0 1400 900",
                        defs {
                            marker { id: "arrow", marker_width: "10", marker_height: "10", ref_x: "9", ref_y: "3", orient: "auto", marker_units: "strokeWidth",
                                path { d: "M0,0 L0,6 L9,3 z", fill: "rgba(125, 211, 252, .78)" }
                            }
                        }
                        for edge in wf.edges.iter() {
                            if let Some(path) = edge_path(&wf, edge) {
                                path { class: "edge", d: "{path}" }
                            }
                        }
                    }
                    for node in wf.nodes.iter() {
                        NodeCard { node: node.clone(), workflow, json_text, message, undo_stack, drag_state, viewport, connection_draft }
                    }
                }
            }
        }
    }
}

#[component]
fn NodeCard(
    node: WorkflowNode,
    mut workflow: Signal<WorkflowFile>,
    mut json_text: Signal<String>,
    mut message: Signal<Message>,
    mut undo_stack: Signal<WorkflowUndoStack>,
    drag_state: Signal<Option<DragState>>,
    viewport: Signal<CanvasViewport>,
    connection_draft: Signal<Option<ConnectionDraft>>,
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
    let node_class = classes.join(" ");
    let label = node.display_label();
    let preview = node.preview_text();
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
                    &mut workflow,
                    &mut json_text,
                    &mut undo_stack,
                    drag_state,
                    viewport,
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
                div { class: "node-id", "{node.id}" }
            }
        }
    }
}

fn edge_path(workflow: &WorkflowFile, edge: &WorkflowEdge) -> Option<String> {
    let source = workflow.nodes.iter().find(|node| node.id == edge.source)?;
    let target = workflow.nodes.iter().find(|node| node.id == edge.target)?;
    let x1 = source.position.x + 248.0;
    let y1 = handle_y(source, edge.source_handle.as_deref(), HandleSide::Source);
    let x2 = target.position.x;
    let y2 = handle_y(target, edge.target_handle.as_deref(), HandleSide::Target);
    let mid = ((x2 - x1).abs() * 0.5).clamp(80.0, 220.0);
    Some(format!(
        "M {x1:.1} {y1:.1} C {cx1:.1} {y1:.1}, {cx2:.1} {y2:.1}, {x2:.1} {y2:.1}",
        cx1 = x1 + mid,
        cx2 = x2 - mid
    ))
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
        let Some(node_id) = selected_node_id(workflow).map(ToOwned::to_owned) else {
            return Err("Select a node before moving it.".to_string());
        };
        let position = move_node_by(workflow, &node_id, dx, dy).map_err(|err| err.to_string())?;
        Ok(format!(
            "Moved `{node_id}` to ({:.0}, {:.0}).",
            position.x, position.y
        ))
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

fn begin_node_drag(
    event: MouseEvent,
    node_id: &str,
    workflow: &mut Signal<WorkflowFile>,
    json_text: &mut Signal<String>,
    undo_stack: &mut Signal<WorkflowUndoStack>,
    mut drag_state: Signal<Option<DragState>>,
    viewport: Signal<CanvasViewport>,
) {
    let before = workflow.read().clone();
    let Some(node) = before.nodes.iter().find(|node| node.id == node_id) else {
        return;
    };
    let point = event.data().client_coordinates();
    let viewport = *viewport.read();
    let mut next = before.clone();
    if select_node(&mut next, Some(node_id)).is_err() {
        return;
    }
    set_node_dragging(&mut next, node_id, true);

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
            start_position: node.position,
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
    let next_position = Position {
        x: drag.start_position.x + (point.x - drag.start_client_x) / zoom,
        y: drag.start_position.y + (point.y - drag.start_client_y) / zoom,
    };
    let mut next = workflow.read().clone();
    if set_node_position(&mut next, &drag.node_id, next_position).is_err() {
        return;
    }
    set_node_dragging(&mut next, &drag.node_id, true);
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
    set_node_dragging(&mut next, &drag.node_id, false);
    let Some(node) = next.nodes.iter().find(|node| node.id == drag.node_id) else {
        message.set(Message::err(format!(
            "Dragged node `{}` disappeared.",
            drag.node_id
        )));
        return;
    };
    let summary = format!(
        "Moved `{}` to ({:.0}, {:.0}).",
        drag.node_id, node.position.x, node.position.y
    );
    match next.to_pretty_json() {
        Ok(json) => {
            workflow.set(next);
            json_text.set(json);
            message.set(Message::ok(summary));
        }
        Err(err) => message.set(Message::err(format!("Drag finish export failed: {err}"))),
    }
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
    start_position: Position,
    start_viewport: CanvasViewport,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ConnectionDraft {
    source_node_id: String,
    source_handle: String,
}
