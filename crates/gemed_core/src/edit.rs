use crate::{
    GroupColor, NodeGroup, NodeType, Position, Size, WorkflowEdge, WorkflowFile, WorkflowNode,
};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

const DEFAULT_NODE_WIDTH: f64 = 248.0;
const DEFAULT_NODE_HEIGHT: f64 = 128.0;
const GROUP_PADDING_X: f64 = 32.0;
const GROUP_PADDING_TOP: f64 = 48.0;
const GROUP_PADDING_BOTTOM: f64 = 32.0;
const MIN_GROUP_WIDTH: f64 = 160.0;
const MIN_GROUP_HEIGHT: f64 = 120.0;

#[derive(Debug, Error)]
pub enum WorkflowEditError {
    #[error("node `{0}` was not found")]
    NodeNotFound(String),
    #[error("edge `{0}` was not found")]
    EdgeNotFound(String),
    #[error("edge id `{0}` already exists")]
    DuplicateEdgeId(String),
    #[error("group `{0}` was not found")]
    GroupNotFound(String),
    #[error("group `{0}` is locked")]
    GroupLocked(String),
    #[error("no nodes selected for {0}")]
    EmptySelection(String),
    #[error("node `{0}` is in a locked group")]
    NodeInLockedGroup(String),
    #[error("cannot connect node `{0}` to itself")]
    SelfConnection(String),
    #[error("edge from `{source_id}` to `{target_id}` already exists")]
    DuplicateConnection {
        source_id: String,
        target_id: String,
    },
    #[error("connecting `{source_id}` to `{target_id}` would create a cycle")]
    EdgeWouldCycle {
        source_id: String,
        target_id: String,
    },
    #[error("{0}")]
    InvalidOperation(String),
    #[error("{0} history is empty")]
    HistoryEmpty(String),
}

pub type EditResult<T> = std::result::Result<T, WorkflowEditError>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroupMove {
    pub position: Position,
    pub moved_node_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SplitGridChildSet {
    pub image_input: String,
    pub prompt: String,
    pub nano_banana: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitGridChildGeneration {
    pub split_node_id: String,
    pub child_node_ids: Vec<SplitGridChildSet>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitGridChildSelection {
    pub split_node_id: String,
    pub child_index: usize,
    pub child: SplitGridChildSet,
    pub selected_node_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowUndoStack {
    undo: Vec<WorkflowFile>,
    redo: Vec<WorkflowFile>,
    limit: usize,
}

impl WorkflowUndoStack {
    pub fn with_limit(limit: usize) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            limit: limit.max(1),
        }
    }

    pub fn record(&mut self, before: &WorkflowFile) {
        if self.undo.last() == Some(before) {
            return;
        }

        self.undo.push(before.clone());
        if self.undo.len() > self.limit {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn undo(&mut self, current: &mut WorkflowFile) -> EditResult<()> {
        let previous = self
            .undo
            .pop()
            .ok_or_else(|| WorkflowEditError::HistoryEmpty("undo".to_string()))?;
        self.redo.push(current.clone());
        *current = previous;
        Ok(())
    }

    pub fn redo(&mut self, current: &mut WorkflowFile) -> EditResult<()> {
        let next = self
            .redo
            .pop()
            .ok_or_else(|| WorkflowEditError::HistoryEmpty("redo".to_string()))?;
        self.undo.push(current.clone());
        *current = next;
        Ok(())
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

impl Default for WorkflowUndoStack {
    fn default() -> Self {
        Self::with_limit(100)
    }
}

pub fn selected_node_id(workflow: &WorkflowFile) -> Option<&str> {
    workflow
        .nodes
        .iter()
        .find(|node| node.selected.unwrap_or(false))
        .map(|node| node.id.as_str())
}

pub fn selected_node_ids(workflow: &WorkflowFile) -> Vec<&str> {
    workflow
        .nodes
        .iter()
        .filter(|node| node.selected.unwrap_or(false))
        .map(|node| node.id.as_str())
        .collect()
}

pub fn select_node(workflow: &mut WorkflowFile, node_id: Option<&str>) -> EditResult<()> {
    if let Some(node_id) = node_id {
        ensure_node_exists(workflow, node_id)?;
    }

    for node in &mut workflow.nodes {
        node.selected = (Some(node.id.as_str()) == node_id).then_some(true);
    }
    Ok(())
}

pub fn select_nodes(workflow: &mut WorkflowFile, node_ids: &[String]) -> EditResult<()> {
    for node_id in node_ids {
        ensure_node_exists(workflow, node_id)?;
    }

    let selected: HashSet<&str> = node_ids.iter().map(String::as_str).collect();
    for node in &mut workflow.nodes {
        node.selected = selected.contains(node.id.as_str()).then_some(true);
    }
    Ok(())
}

pub fn split_grid_child_sets(
    workflow: &WorkflowFile,
    split_node_id: &str,
) -> EditResult<Vec<SplitGridChildSet>> {
    let split_node = workflow
        .nodes
        .iter()
        .find(|node| node.id == split_node_id)
        .ok_or_else(|| WorkflowEditError::NodeNotFound(split_node_id.to_string()))?;
    if split_node.node_type != NodeType::SplitGrid {
        return Err(WorkflowEditError::InvalidOperation(format!(
            "`{split_node_id}` is not a split-grid node"
        )));
    }

    let sets = split_node
        .data
        .get("childNodeIds")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            Some(SplitGridChildSet {
                image_input: item.get("imageInput")?.as_str()?.to_string(),
                prompt: item.get("prompt")?.as_str()?.to_string(),
                nano_banana: item
                    .get("nanoBanana")
                    .or_else(|| item.get("generate"))?
                    .as_str()?
                    .to_string(),
            })
        })
        .collect();
    Ok(sets)
}

pub fn select_split_grid_child_set(
    workflow: &mut WorkflowFile,
    split_node_id: &str,
    child_index: usize,
) -> EditResult<SplitGridChildSelection> {
    if is_node_in_locked_group(workflow, split_node_id) {
        return Err(WorkflowEditError::NodeInLockedGroup(
            split_node_id.to_string(),
        ));
    }

    let child_sets = split_grid_child_sets(workflow, split_node_id)?;
    let child = child_sets.get(child_index).cloned().ok_or_else(|| {
        WorkflowEditError::InvalidOperation(format!(
            "split-grid node `{split_node_id}` has no child set {}",
            child_index + 1
        ))
    })?;
    let selected_node_ids = vec![
        child.image_input.clone(),
        child.prompt.clone(),
        child.nano_banana.clone(),
    ];
    select_nodes(workflow, &selected_node_ids)?;

    Ok(SplitGridChildSelection {
        split_node_id: split_node_id.to_string(),
        child_index,
        child,
        selected_node_ids,
    })
}

pub fn toggle_node_selection(workflow: &mut WorkflowFile, node_id: &str) -> EditResult<bool> {
    let node = workflow
        .nodes
        .iter_mut()
        .find(|node| node.id == node_id)
        .ok_or_else(|| WorkflowEditError::NodeNotFound(node_id.to_string()))?;
    let selected = !node.selected.unwrap_or(false);
    node.selected = selected.then_some(true);
    Ok(selected)
}

pub fn move_node_by(
    workflow: &mut WorkflowFile,
    node_id: &str,
    dx: f64,
    dy: f64,
) -> EditResult<Position> {
    let node = workflow
        .nodes
        .iter_mut()
        .find(|node| node.id == node_id)
        .ok_or_else(|| WorkflowEditError::NodeNotFound(node_id.to_string()))?;
    node.position.x = (node.position.x + dx).max(0.0);
    node.position.y = (node.position.y + dy).max(0.0);
    Ok(node.position)
}

pub fn set_node_position(
    workflow: &mut WorkflowFile,
    node_id: &str,
    position: Position,
) -> EditResult<()> {
    let node = workflow
        .nodes
        .iter_mut()
        .find(|node| node.id == node_id)
        .ok_or_else(|| WorkflowEditError::NodeNotFound(node_id.to_string()))?;
    node.position = Position {
        x: position.x.max(0.0),
        y: position.y.max(0.0),
    };
    Ok(())
}

pub fn is_node_in_locked_group(workflow: &WorkflowFile, node_id: &str) -> bool {
    workflow
        .nodes
        .iter()
        .find(|node| node.id == node_id)
        .and_then(|node| node.group_id.as_deref())
        .and_then(|group_id| workflow.groups.get(group_id))
        .is_some_and(|group| group.locked.unwrap_or(false))
}

pub fn toggle_group_lock(workflow: &mut WorkflowFile, group_id: &str) -> EditResult<bool> {
    let group = workflow
        .groups
        .get_mut(group_id)
        .ok_or_else(|| WorkflowEditError::GroupNotFound(group_id.to_string()))?;
    let next = !group.locked.unwrap_or(false);
    group.locked = next.then_some(true);
    Ok(next)
}

pub fn create_group_for_nodes(
    workflow: &mut WorkflowFile,
    node_ids: &[String],
) -> EditResult<NodeGroup> {
    let mut unique_node_ids = Vec::new();
    let mut seen = HashSet::new();
    for node_id in node_ids {
        ensure_node_exists(workflow, node_id)?;
        if is_node_in_locked_group(workflow, node_id) {
            return Err(WorkflowEditError::NodeInLockedGroup(node_id.clone()));
        }
        if seen.insert(node_id.as_str()) {
            unique_node_ids.push(node_id.clone());
        }
    }
    if unique_node_ids.is_empty() {
        return Err(WorkflowEditError::EmptySelection(
            "group creation".to_string(),
        ));
    }

    let group_id = next_group_id(workflow);
    let group = NodeGroup {
        id: group_id.clone(),
        name: next_group_name(workflow),
        color: GroupColor::Purple,
        position: group_position_for_nodes(workflow, &unique_node_ids)?,
        size: group_size_for_nodes(workflow, &unique_node_ids)?,
        locked: None,
        is_nbp_input: None,
        extra: IndexMap::new(),
    };

    let selected: HashSet<&str> = unique_node_ids.iter().map(String::as_str).collect();
    for node in &mut workflow.nodes {
        if selected.contains(node.id.as_str()) {
            node.group_id = Some(group_id.clone());
        }
    }
    workflow.groups.insert(group_id, group.clone());
    Ok(group)
}

pub fn resize_group_by(
    workflow: &mut WorkflowFile,
    group_id: &str,
    width_delta: f64,
    height_delta: f64,
) -> EditResult<Size> {
    let current_size = workflow
        .groups
        .get(group_id)
        .ok_or_else(|| WorkflowEditError::GroupNotFound(group_id.to_string()))?
        .size;
    set_group_size(
        workflow,
        group_id,
        Size {
            width: current_size.width + width_delta,
            height: current_size.height + height_delta,
        },
    )
}

pub fn set_group_size(workflow: &mut WorkflowFile, group_id: &str, size: Size) -> EditResult<Size> {
    let group = workflow
        .groups
        .get_mut(group_id)
        .ok_or_else(|| WorkflowEditError::GroupNotFound(group_id.to_string()))?;
    if group.locked.unwrap_or(false) {
        return Err(WorkflowEditError::GroupLocked(group_id.to_string()));
    }
    group.size.width = size.width.max(MIN_GROUP_WIDTH);
    group.size.height = size.height.max(MIN_GROUP_HEIGHT);
    Ok(group.size)
}

pub fn move_group_by(
    workflow: &mut WorkflowFile,
    group_id: &str,
    dx: f64,
    dy: f64,
) -> EditResult<GroupMove> {
    let current_position = {
        let group = workflow
            .groups
            .get(group_id)
            .ok_or_else(|| WorkflowEditError::GroupNotFound(group_id.to_string()))?;
        if group.locked.unwrap_or(false) {
            return Err(WorkflowEditError::GroupLocked(group_id.to_string()));
        }
        group.position
    };

    let next_position = Position {
        x: (current_position.x + dx).max(0.0),
        y: (current_position.y + dy).max(0.0),
    };
    let actual_dx = next_position.x - current_position.x;
    let actual_dy = next_position.y - current_position.y;

    let mut moved_node_count = 0;
    for node in &mut workflow.nodes {
        if node.group_id.as_deref() == Some(group_id) {
            node.position.x = (node.position.x + actual_dx).max(0.0);
            node.position.y = (node.position.y + actual_dy).max(0.0);
            moved_node_count += 1;
        }
    }

    let group = workflow
        .groups
        .get_mut(group_id)
        .ok_or_else(|| WorkflowEditError::GroupNotFound(group_id.to_string()))?;
    group.position = next_position;

    Ok(GroupMove {
        position: next_position,
        moved_node_count,
    })
}

pub fn generate_split_grid_children(
    workflow: &mut WorkflowFile,
    split_node_id: &str,
) -> EditResult<SplitGridChildGeneration> {
    let split_index = workflow
        .nodes
        .iter()
        .position(|node| node.id == split_node_id)
        .ok_or_else(|| WorkflowEditError::NodeNotFound(split_node_id.to_string()))?;
    let split_node = workflow.nodes[split_index].clone();
    if split_node.node_type != NodeType::SplitGrid {
        return Err(WorkflowEditError::InvalidOperation(format!(
            "`{split_node_id}` is not a split-grid node"
        )));
    }
    if is_node_in_locked_group(workflow, split_node_id) {
        return Err(WorkflowEditError::NodeInLockedGroup(
            split_node_id.to_string(),
        ));
    }
    if json_array_len(&split_node.data, "childNodeIds") > 0 {
        return Err(WorkflowEditError::InvalidOperation(format!(
            "split-grid node `{split_node_id}` already has child nodes"
        )));
    }

    let rows = positive_u32_field(&split_node.data, "gridRows").unwrap_or(2);
    let cols = positive_u32_field(&split_node.data, "gridCols").unwrap_or(2);
    let target_count = positive_usize_field(&split_node.data, "targetCount")
        .unwrap_or_else(|| rows.saturating_mul(cols) as usize);
    if target_count == 0 {
        return Err(WorkflowEditError::InvalidOperation(
            "split-grid target count must be greater than zero".to_string(),
        ));
    }
    if target_count > 12 {
        return Err(WorkflowEditError::InvalidOperation(
            "split-grid child generation currently supports at most 12 cells".to_string(),
        ));
    }

    let default_prompt = string_field(&split_node.data, "defaultPrompt").unwrap_or_default();
    let generate_settings = split_node
        .data
        .get("generateSettings")
        .cloned()
        .unwrap_or_else(default_split_grid_generate_settings);
    let start_x = split_node.position.x + 340.0;
    let start_y = split_node.position.y;
    let cluster_width = 600.0;
    let cluster_height = 390.0;
    let cluster_gap = 64.0;
    let image_to_generation_gap = 300.0;
    let prompt_y_gap = 176.0;
    let mut child_node_ids = Vec::with_capacity(target_count);

    for index in 0..target_count {
        let row = index / cols as usize;
        let col = index % cols as usize;
        let cluster_x = start_x + col as f64 * (cluster_width + cluster_gap);
        let cluster_y = start_y + row as f64 * (cluster_height + cluster_gap);
        let cell_number = index + 1;
        let image_input = next_node_id(
            workflow,
            &format!("{split_node_id}_cell_{cell_number}_image"),
        );
        let prompt = next_node_id(
            workflow,
            &format!("{split_node_id}_cell_{cell_number}_prompt"),
        );
        let nano_banana = next_node_id(
            workflow,
            &format!("{split_node_id}_cell_{cell_number}_generate"),
        );

        workflow.nodes.push(WorkflowNode::new(
            image_input.clone(),
            NodeType::ImageInput,
            Position {
                x: cluster_x,
                y: cluster_y,
            },
            json!({
                "label": format!("Cell {cell_number} Image"),
                "status": "idle",
                "image": null,
                "imageRef": null,
                "filename": null,
                "dimensions": { "width": 0, "height": 0 }
            }),
        ));
        workflow.nodes.push(WorkflowNode::new(
            nano_banana.clone(),
            NodeType::NanoBanana,
            Position {
                x: cluster_x + image_to_generation_gap,
                y: cluster_y,
            },
            nano_banana_child_data(cell_number, &generate_settings),
        ));
        workflow.nodes.push(WorkflowNode::new(
            prompt.clone(),
            NodeType::Prompt,
            Position {
                x: cluster_x,
                y: cluster_y + prompt_y_gap,
            },
            json!({
                "label": format!("Cell {cell_number} Prompt"),
                "status": "idle",
                "prompt": default_prompt
            }),
        ));

        let split_edge = add_edge_between(
            workflow,
            split_node_id,
            &image_input,
            Some(format!("image-{index}")),
            Some("image".to_string()),
        )?;
        if let Some(edge) = workflow
            .edges
            .iter_mut()
            .find(|edge| edge.id == split_edge.id)
        {
            edge.edge_type = Some("reference".to_string());
        }
        add_edge_between(
            workflow,
            &image_input,
            &nano_banana,
            Some("image".to_string()),
            Some("image".to_string()),
        )?;
        add_edge_between(
            workflow,
            &prompt,
            &nano_banana,
            Some("text".to_string()),
            Some("prompt".to_string()),
        )?;

        child_node_ids.push(SplitGridChildSet {
            image_input,
            prompt,
            nano_banana,
        });
    }

    let Some(split_node) = workflow
        .nodes
        .iter_mut()
        .find(|node| node.id == split_node_id)
    else {
        return Err(WorkflowEditError::NodeNotFound(split_node_id.to_string()));
    };
    ensure_object_data(split_node);
    let Some(data) = split_node.data.as_object_mut() else {
        unreachable!("ensure_object_data converts node data into an object")
    };
    data.insert("targetCount".to_string(), json!(target_count));
    data.insert("gridRows".to_string(), json!(rows));
    data.insert("gridCols".to_string(), json!(cols));
    data.insert("defaultPrompt".to_string(), json!(default_prompt));
    data.insert("generateSettings".to_string(), generate_settings);
    data.insert("childNodeIds".to_string(), json!(child_node_ids));
    data.insert("isConfigured".to_string(), json!(true));

    Ok(SplitGridChildGeneration {
        split_node_id: split_node_id.to_string(),
        child_node_ids,
    })
}

pub fn add_edge_between(
    workflow: &mut WorkflowFile,
    source: &str,
    target: &str,
    source_handle: Option<String>,
    target_handle: Option<String>,
) -> EditResult<WorkflowEdge> {
    ensure_node_exists(workflow, source)?;
    ensure_node_exists(workflow, target)?;
    if source == target {
        return Err(WorkflowEditError::SelfConnection(source.to_string()));
    }
    if workflow.edges.iter().any(|edge| {
        edge.source == source
            && edge.target == target
            && edge.source_handle == source_handle
            && edge.target_handle == target_handle
    }) {
        return Err(WorkflowEditError::DuplicateConnection {
            source_id: source.to_string(),
            target_id: target.to_string(),
        });
    }
    if path_exists(workflow, target, source) {
        return Err(WorkflowEditError::EdgeWouldCycle {
            source_id: source.to_string(),
            target_id: target.to_string(),
        });
    }

    let id = next_edge_id(workflow, source, target);
    let mut edge = WorkflowEdge::new(id, source, target);
    edge.source_handle = source_handle;
    edge.target_handle = target_handle;
    workflow.edges.push(edge.clone());
    Ok(edge)
}

pub fn remove_edge(workflow: &mut WorkflowFile, edge_id: &str) -> EditResult<WorkflowEdge> {
    let index = workflow
        .edges
        .iter()
        .position(|edge| edge.id == edge_id)
        .ok_or_else(|| WorkflowEditError::EdgeNotFound(edge_id.to_string()))?;
    Ok(workflow.edges.remove(index))
}

pub fn remove_edges_between_handles(
    workflow: &mut WorkflowFile,
    source: &str,
    target: &str,
    source_handle: Option<&str>,
    target_handle: Option<&str>,
) -> EditResult<Vec<WorkflowEdge>> {
    ensure_node_exists(workflow, source)?;
    ensure_node_exists(workflow, target)?;

    let mut removed = Vec::new();
    let mut index = 0;
    while index < workflow.edges.len() {
        if workflow.edges[index].source == source
            && workflow.edges[index].target == target
            && workflow.edges[index].source_handle.as_deref() == source_handle
            && workflow.edges[index].target_handle.as_deref() == target_handle
        {
            removed.push(workflow.edges.remove(index));
        } else {
            index += 1;
        }
    }

    if removed.is_empty() {
        return Err(WorkflowEditError::EdgeNotFound(format!(
            "{}:{} -> {}:{}",
            source,
            source_handle.unwrap_or(""),
            target,
            target_handle.unwrap_or("")
        )));
    }

    Ok(removed)
}

fn ensure_node_exists(workflow: &WorkflowFile, node_id: &str) -> EditResult<()> {
    workflow
        .nodes
        .iter()
        .any(|node| node.id == node_id)
        .then_some(())
        .ok_or_else(|| WorkflowEditError::NodeNotFound(node_id.to_string()))
}

fn ensure_object_data(node: &mut WorkflowNode) {
    if !node.data.is_object() {
        node.data = json!({});
    }
}

fn next_node_id(workflow: &WorkflowFile, base: &str) -> String {
    let base = sanitize_id_part(base).trim_matches('_').to_string();
    let base = if base.is_empty() {
        "node".to_string()
    } else {
        base
    };
    if !workflow.nodes.iter().any(|node| node.id == base) {
        return base;
    }

    for suffix in 2.. {
        let candidate = format!("{base}_{suffix}");
        if !workflow.nodes.iter().any(|node| node.id == candidate) {
            return candidate;
        }
    }
    unreachable!("unbounded suffix search always returns")
}

fn json_array_len(data: &Value, key: &str) -> usize {
    data.get(key).and_then(Value::as_array).map_or(0, Vec::len)
}

fn positive_u32_field(data: &Value, key: &str) -> Option<u32> {
    data.get(key)
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)
}

fn positive_usize_field(data: &Value, key: &str) -> Option<usize> {
    data.get(key)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .filter(|value| *value > 0)
}

fn string_field(data: &Value, key: &str) -> Option<String> {
    data.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn default_split_grid_generate_settings() -> Value {
    json!({
        "aspectRatio": "1:1",
        "resolution": "1K",
        "model": "nano-banana-pro",
        "useGoogleSearch": false,
        "useImageSearch": false
    })
}

fn nano_banana_child_data(cell_number: usize, generate_settings: &Value) -> Value {
    let mut data = json!({
        "label": format!("Cell {cell_number} Generate"),
        "status": "idle",
        "provider": "gemini"
    });
    if let (Some(target), Some(settings)) = (data.as_object_mut(), generate_settings.as_object()) {
        for (key, value) in settings {
            target.insert(key.clone(), value.clone());
        }
    }
    data
}

fn next_edge_id(workflow: &WorkflowFile, source: &str, target: &str) -> String {
    let base = format!(
        "edge_{}_{}",
        sanitize_id_part(source),
        sanitize_id_part(target)
    );
    if !workflow.edges.iter().any(|edge| edge.id == base) {
        return base;
    }

    for suffix in 2.. {
        let candidate = format!("{base}_{suffix}");
        if !workflow.edges.iter().any(|edge| edge.id == candidate) {
            return candidate;
        }
    }
    unreachable!("unbounded suffix search always returns")
}

fn next_group_id(workflow: &WorkflowFile) -> String {
    for suffix in 1.. {
        let candidate = format!("group_{suffix}");
        if !workflow.groups.contains_key(&candidate) {
            return candidate;
        }
    }
    unreachable!("unbounded suffix search always returns")
}

fn next_group_name(workflow: &WorkflowFile) -> String {
    let names = workflow
        .groups
        .values()
        .map(|group| group.name.as_str())
        .collect::<HashSet<_>>();
    for suffix in 1.. {
        let candidate = format!("Group {suffix}");
        if !names.contains(candidate.as_str()) {
            return candidate;
        }
    }
    unreachable!("unbounded suffix search always returns")
}

fn group_position_for_nodes(workflow: &WorkflowFile, node_ids: &[String]) -> EditResult<Position> {
    let (min_x, min_y, _, _) = group_bounds_for_nodes(workflow, node_ids)?;
    Ok(Position {
        x: (min_x - GROUP_PADDING_X).max(0.0),
        y: (min_y - GROUP_PADDING_TOP).max(0.0),
    })
}

fn group_size_for_nodes(workflow: &WorkflowFile, node_ids: &[String]) -> EditResult<Size> {
    let (min_x, min_y, max_x, max_y) = group_bounds_for_nodes(workflow, node_ids)?;
    Ok(Size {
        width: (max_x - min_x + GROUP_PADDING_X * 2.0).max(MIN_GROUP_WIDTH),
        height: (max_y - min_y + GROUP_PADDING_TOP + GROUP_PADDING_BOTTOM).max(MIN_GROUP_HEIGHT),
    })
}

fn group_bounds_for_nodes(
    workflow: &WorkflowFile,
    node_ids: &[String],
) -> EditResult<(f64, f64, f64, f64)> {
    let selected = node_ids.iter().map(String::as_str).collect::<HashSet<_>>();
    let mut bounds = None::<(f64, f64, f64, f64)>;

    for node in workflow
        .nodes
        .iter()
        .filter(|node| selected.contains(node.id.as_str()))
    {
        let min_x = node.position.x;
        let min_y = node.position.y;
        let max_x = node.position.x + node.width.unwrap_or(DEFAULT_NODE_WIDTH);
        let max_y = node.position.y + node.height.unwrap_or(DEFAULT_NODE_HEIGHT);
        bounds = Some(match bounds {
            Some((old_min_x, old_min_y, old_max_x, old_max_y)) => (
                old_min_x.min(min_x),
                old_min_y.min(min_y),
                old_max_x.max(max_x),
                old_max_y.max(max_y),
            ),
            None => (min_x, min_y, max_x, max_y),
        });
    }

    bounds.ok_or_else(|| WorkflowEditError::EmptySelection("group bounds".to_string()))
}

fn sanitize_id_part(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => ch,
            _ => '_',
        })
        .collect()
}

fn path_exists(workflow: &WorkflowFile, start: &str, end: &str) -> bool {
    let outgoing = outgoing_edges(workflow);
    let mut stack = vec![start];
    let mut seen = HashSet::new();

    while let Some(node_id) = stack.pop() {
        if node_id == end {
            return true;
        }
        if !seen.insert(node_id) {
            continue;
        }
        if let Some(targets) = outgoing.get(node_id) {
            stack.extend(targets.iter().copied());
        }
    }

    false
}

fn outgoing_edges(workflow: &WorkflowFile) -> HashMap<&str, Vec<&str>> {
    let mut outgoing: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in workflow
        .edges
        .iter()
        .filter(|edge| !edge.data.is_loop.unwrap_or(false))
    {
        outgoing
            .entry(edge.source.as_str())
            .or_default()
            .push(edge.target.as_str());
    }
    outgoing
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GroupColor, NodeGroup, NodeType, Size, WorkflowNode};
    use indexmap::IndexMap;
    use serde_json::json;

    fn two_node_workflow() -> WorkflowFile {
        WorkflowFile {
            name: "edit".to_string(),
            nodes: vec![
                WorkflowNode::new(
                    "a",
                    NodeType::Prompt,
                    Position { x: 10.0, y: 20.0 },
                    json!({}),
                ),
                WorkflowNode::new(
                    "b",
                    NodeType::Output,
                    Position { x: 100.0, y: 200.0 },
                    json!({}),
                ),
            ],
            ..WorkflowFile::blank()
        }
    }

    #[test]
    fn selecting_node_clears_previous_selection() {
        let mut workflow = two_node_workflow();
        select_node(&mut workflow, Some("a")).unwrap();
        select_node(&mut workflow, Some("b")).unwrap();

        assert_eq!(selected_node_id(&workflow), Some("b"));
        assert_eq!(workflow.nodes[0].selected, None);
        assert_eq!(workflow.nodes[1].selected, Some(true));
    }

    #[test]
    fn selecting_multiple_nodes_preserves_ordered_selection() {
        let mut workflow = two_node_workflow();
        select_nodes(&mut workflow, &["b".to_string(), "a".to_string()]).unwrap();

        assert_eq!(selected_node_id(&workflow), Some("a"));
        assert_eq!(selected_node_ids(&workflow), vec!["a", "b"]);
        assert_eq!(workflow.nodes[0].selected, Some(true));
        assert_eq!(workflow.nodes[1].selected, Some(true));
    }

    #[test]
    fn selecting_split_grid_child_set_selects_image_prompt_and_generate_nodes() {
        let mut workflow = WorkflowFile {
            nodes: vec![
                WorkflowNode::new(
                    "split",
                    NodeType::SplitGrid,
                    Position { x: 0.0, y: 0.0 },
                    json!({
                        "childNodeIds": [
                            {
                                "imageInput": "cell_1_image",
                                "prompt": "cell_1_prompt",
                                "nanoBanana": "cell_1_generate"
                            }
                        ]
                    }),
                ),
                WorkflowNode::new(
                    "cell_1_image",
                    NodeType::ImageInput,
                    Position { x: 100.0, y: 0.0 },
                    json!({}),
                ),
                WorkflowNode::new(
                    "cell_1_prompt",
                    NodeType::Prompt,
                    Position { x: 100.0, y: 180.0 },
                    json!({}),
                ),
                WorkflowNode::new(
                    "cell_1_generate",
                    NodeType::NanoBanana,
                    Position { x: 400.0, y: 0.0 },
                    json!({}),
                ),
            ],
            ..WorkflowFile::blank()
        };

        let selection = select_split_grid_child_set(&mut workflow, "split", 0).unwrap();

        assert_eq!(selection.split_node_id, "split");
        assert_eq!(selection.child_index, 0);
        assert_eq!(
            selection.selected_node_ids,
            ["cell_1_image", "cell_1_prompt", "cell_1_generate"]
        );
        assert_eq!(
            selected_node_ids(&workflow),
            vec!["cell_1_image", "cell_1_prompt", "cell_1_generate"]
        );
    }

    #[test]
    fn toggling_node_selection_flips_only_target_node() {
        let mut workflow = two_node_workflow();
        select_node(&mut workflow, Some("a")).unwrap();

        assert!(toggle_node_selection(&mut workflow, "b").unwrap());
        assert_eq!(selected_node_ids(&workflow), vec!["a", "b"]);

        assert!(!toggle_node_selection(&mut workflow, "a").unwrap());
        assert_eq!(selected_node_ids(&workflow), vec!["b"]);
        assert_eq!(workflow.nodes[0].selected, None);
        assert_eq!(workflow.nodes[1].selected, Some(true));
    }

    #[test]
    fn moving_node_clamps_to_canvas_origin() {
        let mut workflow = two_node_workflow();
        let position = move_node_by(&mut workflow, "a", -100.0, -5.0).unwrap();

        assert_eq!(position, Position { x: 0.0, y: 15.0 });
    }

    #[test]
    fn connecting_nodes_generates_stable_edge_id() {
        let mut workflow = two_node_workflow();
        let edge = add_edge_between(&mut workflow, "a", "b", None, None).unwrap();

        assert_eq!(edge.id, "edge_a_b");
        assert_eq!(workflow.edges.len(), 1);
    }

    #[test]
    fn duplicate_connection_is_rejected() {
        let mut workflow = two_node_workflow();
        add_edge_between(&mut workflow, "a", "b", None, None).unwrap();

        let err = add_edge_between(&mut workflow, "a", "b", None, None).unwrap_err();
        assert!(matches!(err, WorkflowEditError::DuplicateConnection { .. }));
    }

    #[test]
    fn connection_that_would_cycle_is_rejected() {
        let mut workflow = two_node_workflow();
        add_edge_between(&mut workflow, "a", "b", None, None).unwrap();

        let err = add_edge_between(&mut workflow, "b", "a", None, None).unwrap_err();
        assert!(matches!(err, WorkflowEditError::EdgeWouldCycle { .. }));
    }

    #[test]
    fn removing_edge_returns_removed_edge() {
        let mut workflow = two_node_workflow();
        add_edge_between(&mut workflow, "a", "b", None, None).unwrap();

        let removed = remove_edge(&mut workflow, "edge_a_b").unwrap();
        assert_eq!(removed.source, "a");
        assert!(workflow.edges.is_empty());
    }

    #[test]
    fn connecting_with_handles_persists_handle_ids() {
        let mut workflow = two_node_workflow();
        let edge = add_edge_between(
            &mut workflow,
            "a",
            "b",
            Some("text".to_string()),
            Some("prompt".to_string()),
        )
        .unwrap();

        assert_eq!(edge.source_handle.as_deref(), Some("text"));
        assert_eq!(edge.target_handle.as_deref(), Some("prompt"));
    }

    #[test]
    fn removing_edges_between_handles_only_removes_exact_handle_pair() {
        let mut workflow = two_node_workflow();
        add_edge_between(
            &mut workflow,
            "a",
            "b",
            Some("text".to_string()),
            Some("prompt".to_string()),
        )
        .unwrap();
        add_edge_between(
            &mut workflow,
            "a",
            "b",
            Some("image".to_string()),
            Some("image".to_string()),
        )
        .unwrap();

        let removed =
            remove_edges_between_handles(&mut workflow, "a", "b", Some("text"), Some("prompt"))
                .unwrap();
        assert_eq!(removed.len(), 1);
        assert_eq!(workflow.edges.len(), 1);
        assert_eq!(workflow.edges[0].source_handle.as_deref(), Some("image"));
    }

    #[test]
    fn undo_stack_restores_and_redoes_workflow_snapshots() {
        let mut workflow = two_node_workflow();
        let mut history = WorkflowUndoStack::default();

        history.record(&workflow);
        move_node_by(&mut workflow, "a", 10.0, 0.0).unwrap();
        assert_eq!(workflow.nodes[0].position.x, 20.0);

        history.undo(&mut workflow).unwrap();
        assert_eq!(workflow.nodes[0].position.x, 10.0);
        assert!(history.can_redo());

        history.redo(&mut workflow).unwrap();
        assert_eq!(workflow.nodes[0].position.x, 20.0);
    }

    #[test]
    fn undo_stack_clears_redo_after_new_record() {
        let mut workflow = two_node_workflow();
        let mut history = WorkflowUndoStack::default();

        history.record(&workflow);
        move_node_by(&mut workflow, "a", 10.0, 0.0).unwrap();
        history.undo(&mut workflow).unwrap();
        assert!(history.can_redo());

        history.record(&workflow);
        assert!(!history.can_redo());
    }

    #[test]
    fn locked_group_members_are_detected_and_toggled() {
        let mut workflow = two_node_workflow();
        workflow.nodes[0].group_id = Some("group-1".to_string());
        workflow.groups = IndexMap::from([(
            "group-1".to_string(),
            NodeGroup {
                id: "group-1".to_string(),
                name: "Locked".to_string(),
                color: GroupColor::Blue,
                position: Position { x: 0.0, y: 0.0 },
                size: Size {
                    width: 200.0,
                    height: 160.0,
                },
                locked: None,
                is_nbp_input: None,
                extra: IndexMap::new(),
            },
        )]);

        assert!(!is_node_in_locked_group(&workflow, "a"));
        assert!(toggle_group_lock(&mut workflow, "group-1").unwrap());
        assert!(is_node_in_locked_group(&workflow, "a"));
        assert!(!is_node_in_locked_group(&workflow, "b"));
        assert!(!toggle_group_lock(&mut workflow, "group-1").unwrap());
        assert!(!is_node_in_locked_group(&workflow, "a"));
    }

    #[test]
    fn creating_group_for_nodes_assigns_bounds_and_membership() {
        let mut workflow = two_node_workflow();
        let group = create_group_for_nodes(
            &mut workflow,
            &["a".to_string(), "b".to_string(), "a".to_string()],
        )
        .unwrap();

        assert_eq!(group.id, "group_1");
        assert_eq!(group.name, "Group 1");
        assert_eq!(group.position, Position { x: 0.0, y: 0.0 });
        assert!(group.size.width >= 338.0);
        assert!(group.size.height >= 360.0);
        assert_eq!(workflow.groups.len(), 1);
        assert_eq!(workflow.nodes[0].group_id.as_deref(), Some("group_1"));
        assert_eq!(workflow.nodes[1].group_id.as_deref(), Some("group_1"));
    }

    #[test]
    fn group_creation_rejects_locked_group_member() {
        let mut workflow = two_node_workflow();
        workflow.nodes[0].group_id = Some("locked".to_string());
        workflow.groups = IndexMap::from([(
            "locked".to_string(),
            NodeGroup {
                id: "locked".to_string(),
                name: "Locked".to_string(),
                color: GroupColor::Blue,
                position: Position { x: 0.0, y: 0.0 },
                size: Size {
                    width: 200.0,
                    height: 160.0,
                },
                locked: Some(true),
                is_nbp_input: None,
                extra: IndexMap::new(),
            },
        )]);

        let err = create_group_for_nodes(&mut workflow, &["a".to_string()]).unwrap_err();
        assert!(matches!(err, WorkflowEditError::NodeInLockedGroup(id) if id == "a"));
    }

    #[test]
    fn resizing_group_clamps_to_minimum_size() {
        let mut workflow = two_node_workflow();
        create_group_for_nodes(&mut workflow, &["a".to_string()]).unwrap();

        let size = resize_group_by(&mut workflow, "group_1", -1000.0, -1000.0).unwrap();
        assert_eq!(
            size,
            Size {
                width: MIN_GROUP_WIDTH,
                height: MIN_GROUP_HEIGHT
            }
        );
    }

    #[test]
    fn setting_group_size_clamps_and_rejects_locked_group() {
        let mut workflow = two_node_workflow();
        create_group_for_nodes(&mut workflow, &["a".to_string()]).unwrap();

        let size = set_group_size(
            &mut workflow,
            "group_1",
            Size {
                width: 240.0,
                height: 200.0,
            },
        )
        .unwrap();
        assert_eq!(
            size,
            Size {
                width: 240.0,
                height: 200.0,
            }
        );

        toggle_group_lock(&mut workflow, "group_1").unwrap();
        let err = set_group_size(
            &mut workflow,
            "group_1",
            Size {
                width: 320.0,
                height: 240.0,
            },
        )
        .unwrap_err();
        assert!(matches!(err, WorkflowEditError::GroupLocked(group_id) if group_id == "group_1"));
    }

    #[test]
    fn moving_group_moves_member_nodes_and_clamps_group_origin() {
        let mut workflow = two_node_workflow();
        create_group_for_nodes(&mut workflow, &["a".to_string(), "b".to_string()]).unwrap();

        let moved = move_group_by(&mut workflow, "group_1", 24.0, 16.0).unwrap();
        assert_eq!(moved.position, Position { x: 24.0, y: 16.0 });
        assert_eq!(moved.moved_node_count, 2);
        assert_eq!(workflow.nodes[0].position, Position { x: 34.0, y: 36.0 });
        assert_eq!(workflow.nodes[1].position, Position { x: 124.0, y: 216.0 });

        let moved = move_group_by(&mut workflow, "group_1", -100.0, -100.0).unwrap();
        assert_eq!(moved.position, Position { x: 0.0, y: 0.0 });
        assert_eq!(workflow.nodes[0].position, Position { x: 10.0, y: 20.0 });
        assert_eq!(workflow.nodes[1].position, Position { x: 100.0, y: 200.0 });
    }

    #[test]
    fn moving_locked_group_is_rejected() {
        let mut workflow = two_node_workflow();
        create_group_for_nodes(&mut workflow, &["a".to_string()]).unwrap();
        toggle_group_lock(&mut workflow, "group_1").unwrap();

        let err = move_group_by(&mut workflow, "group_1", 10.0, 10.0).unwrap_err();
        assert!(matches!(err, WorkflowEditError::GroupLocked(group_id) if group_id == "group_1"));
    }

    #[test]
    fn generating_split_grid_children_creates_legacy_child_sets_and_edges() {
        let mut workflow = WorkflowFile {
            name: "split children".to_string(),
            nodes: vec![WorkflowNode::new(
                "split",
                NodeType::SplitGrid,
                Position { x: 100.0, y: 120.0 },
                json!({
                    "targetCount": 2,
                    "gridRows": 1,
                    "gridCols": 2,
                    "defaultPrompt": "Enhance this cell",
                    "generateSettings": {
                        "aspectRatio": "2:3",
                        "resolution": "2K",
                        "model": "nano-banana-pro",
                        "useGoogleSearch": false,
                        "useImageSearch": false
                    },
                    "childNodeIds": [],
                    "isConfigured": false
                }),
            )],
            ..WorkflowFile::blank()
        };

        let generated = generate_split_grid_children(&mut workflow, "split").unwrap();

        assert_eq!(generated.child_node_ids.len(), 2);
        assert_eq!(workflow.nodes.len(), 7);
        assert_eq!(workflow.edges.len(), 6);
        let split = workflow
            .nodes
            .iter()
            .find(|node| node.id == "split")
            .unwrap();
        assert_eq!(
            split.data.get("isConfigured").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            split
                .data
                .get("childNodeIds")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(2)
        );
        assert!(workflow.edges.iter().any(|edge| {
            edge.source == "split"
                && edge.source_handle.as_deref() == Some("image-0")
                && edge.target == generated.child_node_ids[0].image_input
                && edge.target_handle.as_deref() == Some("image")
        }));
        assert!(workflow.nodes.iter().any(|node| {
            node.id == generated.child_node_ids[0].prompt
                && node.data.get("prompt").and_then(Value::as_str) == Some("Enhance this cell")
        }));
    }
}
