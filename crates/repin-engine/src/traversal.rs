use repin_core::model::edge::Edge;
use repin_core::model::identity::NodeId;
use repin_core::model::node::Node;
use repin_core::ports::store::ReadView;
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

pub struct GraphTraversal;

impl GraphTraversal {
    pub fn lookup_entity(read_view: &dyn ReadView, name_or_id: &str) -> Option<Node> {
        // Try parsing as NodeId hex
        if let Ok(bytes) = hex::decode(name_or_id)
            && bytes.len() == 32
        {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&bytes);
            if let Ok(Some(n)) = read_view.node(&NodeId::from_bytes(arr)) {
                return Some(n);
            }
        }

        // Try lookup by name
        let nodes = read_view
            .nodes_by_name(name_or_id, &Default::default())
            .ok()?;
        nodes.into_iter().next()
    }

    pub fn lookup_neighbors(
        read_view: &dyn ReadView,
        name_or_id: &str,
        _max_depth: usize,
    ) -> Option<NeighborsData> {
        let target = Self::lookup_entity(read_view, name_or_id)?;

        let out_edges = read_view
            .edges_from(&target.id, &Default::default())
            .unwrap_or_default();
        let mut outgoing = Vec::new();
        for e in out_edges {
            let neighbor_node = read_view.node(&e.to).ok().flatten();
            outgoing.push(NeighborItem {
                edge: e,
                node: neighbor_node,
            });
        }

        let in_edges = read_view
            .edges_to(&target.id, &Default::default())
            .unwrap_or_default();
        let mut incoming = Vec::new();
        for e in in_edges {
            let neighbor_node = read_view.node(&e.from).ok().flatten();
            incoming.push(NeighborItem {
                edge: e,
                node: neighbor_node,
            });
        }

        Some(NeighborsData {
            target,
            incoming,
            outgoing,
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
            let current = path.last().unwrap();
            if current == to_id && path.len() > 1 {
                paths.push(path.clone());
                continue;
            }

            let edges = read_view
                .edges_from(current, &Default::default())
                .unwrap_or_default();
            for e in edges {
                if !path.contains(&e.to) {
                    let mut next_path = path.clone();
                    next_path.push(e.to);
                    queue.push_back(next_path);
                }
            }
        }

        paths
    }

    pub fn impact_analysis(
        read_view: &dyn ReadView,
        root_nodes: &[NodeId],
        max_depth: usize,
    ) -> Vec<Node> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        for root in root_nodes {
            visited.insert(*root);
            queue.push_back((*root, 0));
        }

        let mut impacted_nodes = Vec::new();

        while let Some((curr, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }

            // Find incoming edges (callers / referrers that are impacted by this node)
            let in_edges = read_view
                .edges_to(&curr, &Default::default())
                .unwrap_or_default();
            for e in in_edges {
                if visited.insert(e.from) {
                    if let Ok(Some(n)) = read_view.node(&e.from) {
                        impacted_nodes.push(n);
                    }
                    queue.push_back((e.from, depth + 1));
                }
            }
        }

        impacted_nodes
    }
}
