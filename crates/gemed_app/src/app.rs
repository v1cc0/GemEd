use dioxus::prelude::*;
use gemed_core::{
    NodeStatus, WorkflowFile, WorkflowNode, add_edge_between, move_node_by, remove_edge,
    select_node, selected_node_id,
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
.node { position: absolute; width: 15.5rem; min-height: 8rem; border-radius: 1rem; border: 1px solid rgba(148, 163, 184, .24); background: linear-gradient(145deg, rgba(30, 41, 59, .96), rgba(15, 23, 42, .96)); box-shadow: 0 22px 60px rgba(0, 0, 0, .34); overflow: hidden; }
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

    rsx! {
        style { "{APP_CSS}" }
        div { class: "app",
            Header { workflow, json_text, message, execution_report }
            main { class: "main",
                Sidebar { workflow, json_text, message, execution_report }
                WorkflowCanvas { workflow, json_text, message }
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
) -> Element {
    let wf = workflow.read();
    let counts = wf.node_type_counts();
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
                            mutate_workflow(&mut workflow, &mut json_text, &mut message, |workflow| {
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
                            mutate_selected_node(&mut workflow, &mut json_text, &mut message, -32.0, 0.0);
                        },
                        "←"
                    }
                    button {
                        class: "action",
                        disabled: selected_id.is_none(),
                        onclick: move |_| {
                            mutate_selected_node(&mut workflow, &mut json_text, &mut message, 32.0, 0.0);
                        },
                        "→"
                    }
                    button {
                        class: "action",
                        disabled: selected_id.is_none(),
                        onclick: move |_| {
                            mutate_selected_node(&mut workflow, &mut json_text, &mut message, 0.0, -32.0);
                        },
                        "↑"
                    }
                    button {
                        class: "action",
                        disabled: selected_id.is_none(),
                        onclick: move |_| {
                            mutate_selected_node(&mut workflow, &mut json_text, &mut message, 0.0, 32.0);
                        },
                        "↓"
                    }
                    button {
                        class: "action",
                        disabled: selected_id.is_none() || next_node_id.is_none(),
                        onclick: move |_| {
                            mutate_workflow(&mut workflow, &mut json_text, &mut message, |workflow| {
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
                                        code { "{edge.source} → {edge.target}" }
                                        button {
                                            class: "mini-action",
                                            onclick: move |_| {
                                                let edge_id = edge_id.clone();
                                                mutate_workflow(&mut workflow, &mut json_text, &mut message, move |workflow| {
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
) -> Element {
    let wf = workflow.read();

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
                div { class: "canvas",
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
                        NodeCard { node: node.clone(), workflow, json_text, message }
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
) -> Element {
    let style = format!(
        "left: {}px; top: {}px;",
        node.position.x.max(0.0),
        node.position.y.max(0.0)
    );
    let status = node.status();
    let status_class = format!("badge {}", status.label());
    let node_class = if node.selected.unwrap_or(false) {
        format!("node {} selected", node.node_type.class_name())
    } else {
        format!("node {}", node.node_type.class_name())
    };
    let label = node.display_label();
    let preview = node.preview_text();
    let node_id = node.id.clone();

    rsx! {
        article {
            class: "{node_class}",
            style: "{style}",
            onclick: move |_| {
                let node_id = node_id.clone();
                mutate_workflow(&mut workflow, &mut json_text, &mut message, move |workflow| {
                    select_node(workflow, Some(&node_id))
                        .map_err(|err| err.to_string())?;
                    Ok(format!("Selected `{node_id}`."))
                });
            },
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

fn edge_path(workflow: &WorkflowFile, edge: &gemed_core::WorkflowEdge) -> Option<String> {
    let source = workflow.nodes.iter().find(|node| node.id == edge.source)?;
    let target = workflow.nodes.iter().find(|node| node.id == edge.target)?;
    let x1 = source.position.x + 248.0;
    let y1 = source.position.y + 64.0;
    let x2 = target.position.x;
    let y2 = target.position.y + 64.0;
    let mid = ((x2 - x1).abs() * 0.5).clamp(80.0, 220.0);
    Some(format!(
        "M {x1:.1} {y1:.1} C {cx1:.1} {y1:.1}, {cx2:.1} {y2:.1}, {x2:.1} {y2:.1}",
        cx1 = x1 + mid,
        cx2 = x2 - mid
    ))
}

fn mutate_selected_node(
    workflow: &mut Signal<WorkflowFile>,
    json_text: &mut Signal<String>,
    message: &mut Signal<Message>,
    dx: f64,
    dy: f64,
) {
    mutate_workflow(workflow, json_text, message, |workflow| {
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
    mut mutation: F,
) where
    F: FnMut(&mut WorkflowFile) -> Result<String, String>,
{
    let mut next = workflow.read().clone();
    match mutation(&mut next) {
        Ok(success) => match next.to_pretty_json() {
            Ok(json) => {
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
