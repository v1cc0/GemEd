use crate::{GroupColor, NodeGroup, Position, Size, WorkflowEdge, WorkflowFile};
use indexmap::IndexMap;
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
    #[error("{0} history is empty")]
    HistoryEmpty(String),
}

pub type EditResult<T> = std::result::Result<T, WorkflowEditError>;

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
    let group = workflow
        .groups
        .get_mut(group_id)
        .ok_or_else(|| WorkflowEditError::GroupNotFound(group_id.to_string()))?;
    group.size.width = (group.size.width + width_delta).max(MIN_GROUP_WIDTH);
    group.size.height = (group.size.height + height_delta).max(MIN_GROUP_HEIGHT);
    Ok(group.size)
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
}
