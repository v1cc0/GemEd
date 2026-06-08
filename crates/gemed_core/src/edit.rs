use crate::{Position, WorkflowEdge, WorkflowFile};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorkflowEditError {
    #[error("node `{0}` was not found")]
    NodeNotFound(String),
    #[error("edge `{0}` was not found")]
    EdgeNotFound(String),
    #[error("edge id `{0}` already exists")]
    DuplicateEdgeId(String),
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

pub fn select_node(workflow: &mut WorkflowFile, node_id: Option<&str>) -> EditResult<()> {
    if let Some(node_id) = node_id {
        ensure_node_exists(workflow, node_id)?;
    }

    for node in &mut workflow.nodes {
        node.selected = (Some(node.id.as_str()) == node_id).then_some(true);
    }
    Ok(())
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
    use crate::{NodeType, WorkflowNode};
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
}
