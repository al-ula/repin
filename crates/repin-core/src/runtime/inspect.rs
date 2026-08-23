use crate::line_index::Position;
use crate::model::node::Node;
use crate::ports::store::ReadView;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SymbolSummary {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qualified_name: Option<String>,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range_preview: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileOutline {
    pub root: String,
    pub path: String,
    pub symbols: Vec<SymbolSummary>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Inspector;

impl Inspector {
    pub fn inspect_file(read_view: &dyn ReadView, root: &str, path: &str) -> FileOutline {
        let mut nodes = read_view.nodes_by_file(root, path).unwrap_or_default();
        nodes.sort_by_key(|node| node.id);
        let symbols = nodes
            .into_iter()
            .filter(|node| {
                node.kind != crate::model::registries::NodeKind::File
                    && node.kind != crate::model::registries::NodeKind::Document
            })
            .map(|node| SymbolSummary {
                name: node.name,
                qualified_name: node.qualified_name,
                kind: node.kind.as_str().to_string(),
                range_preview: node
                    .range
                    .map(|range| format!("{}-{}", range.start, range.end)),
            })
            .collect();
        FileOutline {
            root: root.to_string(),
            path: path.to_string(),
            symbols,
        }
    }

    pub fn at_position(
        read_view: &dyn ReadView,
        root: &str,
        path: &str,
        position: Position,
    ) -> Option<Node> {
        let nodes = read_view.nodes_by_file(root, path).ok()?;
        let mut best = None;
        for node in nodes {
            if let Some(range) = &node.range
                && range.start.line <= position.line
                && range.end.line >= position.line
            {
                let is_smaller = best.as_ref().is_none_or(|previous: &Node| {
                    range.span.len()
                        < previous
                            .range
                            .as_ref()
                            .map_or(usize::MAX, |previous_range| previous_range.span.len())
                });
                if is_smaller {
                    best = Some(node);
                }
            }
        }
        best
    }
}
