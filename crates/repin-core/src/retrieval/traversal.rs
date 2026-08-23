use crate::model::edge::Edge;
use crate::model::identity::NodeId;
use crate::model::node::Node;
use crate::ports::store::ReadView;
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NeighborItem {
    pub edge: Edge,
    pub node: Option<Node>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NeighborsData {
    pub target: Node,
    pub incoming: Vec<NeighborItem>,
    pub outgoing: Vec<NeighborItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImpactItem {
    pub node: Node,
    pub depth: usize,
    pub via_edge_kind: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImpactData {
    pub target: Node,
    pub max_depth: usize,
    pub items: Vec<ImpactItem>,
    pub total_impacted: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PathSegment {
    pub node: Node,
    pub edge_to_next: Option<Edge>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PathTrace {
    pub segments: Vec<PathSegment>,
    pub length: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PathTraceData {
    pub from: Node,
    pub to: Node,
    pub max_depth: usize,
    pub paths: Vec<PathTrace>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct GraphTraversal;

impl GraphTraversal {
    pub fn lookup_entity(read_view: &dyn ReadView, name_or_id: &str) -> Option<Node> {
        if let Ok(bytes) = hex::decode(name_or_id)
            && bytes.len() == 32
        {
            let mut array = [0_u8; 32];
            array.copy_from_slice(&bytes);
            if let Ok(Some(node)) = read_view.node(&NodeId::from_bytes(array)) {
                return Some(node);
            }
        }

        let mut nodes = read_view
            .nodes_by_name(name_or_id, &Default::default())
            .ok()?;
        nodes.sort_by_key(|node| node.id);
        nodes.into_iter().next()
    }

    pub fn lookup_neighbors(
        read_view: &dyn ReadView,
        name_or_id: &str,
        _max_depth: usize,
    ) -> Option<NeighborsData> {
        let target = Self::lookup_entity(read_view, name_or_id)?;
        let mut out_edges = read_view
            .edges_from(&target.id, &Default::default())
            .unwrap_or_default();
        out_edges.sort_by_key(|edge| edge.id);
        let outgoing = out_edges
            .into_iter()
            .map(|edge| NeighborItem {
                node: read_view.node(&edge.to).ok().flatten(),
                edge,
            })
            .collect();

        let mut in_edges = read_view
            .edges_to(&target.id, &Default::default())
            .unwrap_or_default();
        in_edges.sort_by_key(|edge| edge.id);
        let incoming = in_edges
            .into_iter()
            .map(|edge| NeighborItem {
                node: read_view.node(&edge.from).ok().flatten(),
                edge,
            })
            .collect();

        Some(NeighborsData {
            target,
            incoming,
            outgoing,
        })
    }

    pub fn lookup_impact(
        read_view: &dyn ReadView,
        name_or_id: &str,
        max_depth: usize,
    ) -> Option<ImpactData> {
        let target = Self::lookup_entity(read_view, name_or_id)?;
        let mut visited = HashSet::new();
        visited.insert(target.id);

        let mut queue = VecDeque::new();
        queue.push_back((target.id, 0));

        let mut items = Vec::new();
        while let Some((current, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }
            let mut in_edges = read_view
                .edges_to(&current, &Default::default())
                .unwrap_or_default();
            in_edges.sort_by_key(|edge| edge.id);
            for edge in in_edges {
                if visited.insert(edge.from) {
                    if let Ok(Some(node)) = read_view.node(&edge.from) {
                        items.push(ImpactItem {
                            node,
                            depth: depth + 1,
                            via_edge_kind: edge.kind.as_str().to_string(),
                        });
                    }
                    queue.push_back((edge.from, depth + 1));
                }
            }
        }

        let total_impacted = items.len();
        Some(ImpactData {
            target,
            max_depth,
            items,
            total_impacted,
        })
    }

    pub fn trace_paths(
        read_view: &dyn ReadView,
        from_id: &NodeId,
        to_id: &NodeId,
        max_depth: usize,
    ) -> Vec<Vec<NodeId>> {
        let mut paths = Vec::new();
        let mut queue = VecDeque::new();
        queue.push_back(vec![*from_id]);

        while let Some(path) = queue.pop_front() {
            if path.len() > max_depth {
                continue;
            }
            let current = path.last().expect("a queued path always has a node");
            if current == to_id && path.len() > 1 {
                paths.push(path.clone());
                continue;
            }

            let mut edges = read_view
                .edges_from(current, &Default::default())
                .unwrap_or_default();
            edges.sort_by_key(|edge| edge.id);
            for edge in edges {
                if !path.contains(&edge.to) {
                    let mut next_path = path.clone();
                    next_path.push(edge.to);
                    queue.push_back(next_path);
                }
            }
        }
        paths
    }

    pub fn lookup_paths(
        read_view: &dyn ReadView,
        from_name_or_id: &str,
        to_name_or_id: &str,
        max_depth: usize,
    ) -> Option<PathTraceData> {
        let from = Self::lookup_entity(read_view, from_name_or_id)?;
        let to = Self::lookup_entity(read_view, to_name_or_id)?;

        // Queue holds paths of (NodeId, Option<Edge_from_prev_to_this>)
        let mut queue: VecDeque<Vec<(NodeId, Option<Edge>)>> = VecDeque::new();
        queue.push_back(vec![(from.id, None)]);

        let mut traces = Vec::new();

        while let Some(path) = queue.pop_front() {
            if path.len() > max_depth + 1 {
                continue;
            }
            let (current_id, _) = path.last().expect("path has at least 1 node");
            if *current_id == to.id && path.len() > 1 {
                let mut segments = Vec::new();
                for i in 0..path.len() {
                    let (node_id, _) = path[i];
                    let next_edge = if i + 1 < path.len() {
                        path[i + 1].1.clone()
                    } else {
                        None
                    };
                    if let Ok(Some(node)) = read_view.node(&node_id) {
                        segments.push(PathSegment {
                            node,
                            edge_to_next: next_edge,
                        });
                    }
                }
                let length = segments.len();
                traces.push(PathTrace { segments, length });
                continue;
            }

            let mut edges = read_view
                .edges_from(current_id, &Default::default())
                .unwrap_or_default();
            edges.sort_by_key(|edge| edge.id);

            let node_ids_in_path: HashSet<NodeId> = path.iter().map(|(id, _)| *id).collect();
            for edge in edges {
                if !node_ids_in_path.contains(&edge.to) {
                    let mut next_path = path.clone();
                    next_path.push((edge.to, Some(edge)));
                    queue.push_back(next_path);
                }
            }
        }

        Some(PathTraceData {
            from,
            to,
            max_depth,
            paths: traces,
        })
    }

    pub fn impact_analysis(
        read_view: &dyn ReadView,
        root_nodes: &[NodeId],
        max_depth: usize,
    ) -> Vec<Node> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut roots = root_nodes.to_vec();
        roots.sort();
        for root in roots {
            visited.insert(root);
            queue.push_back((root, 0));
        }

        let mut impacted_nodes = Vec::new();
        while let Some((current, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }
            let mut in_edges = read_view
                .edges_to(&current, &Default::default())
                .unwrap_or_default();
            in_edges.sort_by_key(|edge| edge.id);
            for edge in in_edges {
                if visited.insert(edge.from) {
                    if let Ok(Some(node)) = read_view.node(&edge.from) {
                        impacted_nodes.push(node);
                    }
                    queue.push_back((edge.from, depth + 1));
                }
            }
        }
        impacted_nodes
    }
}
