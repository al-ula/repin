use repin_core::line_index::Position;
use repin_core::model::node::Node;
use repin_core::ports::store::ReadView;
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

pub struct Inspector;

impl Inspector {
    pub fn inspect_file(read_view: &dyn ReadView, root: &str, path: &str) -> FileOutline {
        let nodes = read_view.nodes_by_file(root, path).unwrap_or_default();
        let mut symbols = Vec::new();

        for n in nodes {
            if n.kind == repin_core::model::registries::NodeKind::File
                || n.kind == repin_core::model::registries::NodeKind::Document
            {
                continue;
            }
            let range_preview = n.range.as_ref().map(|r| format!("{}-{}", r.start, r.end));
            symbols.push(SymbolSummary {
                name: n.name,
                qualified_name: n.qualified_name,
                kind: n.kind.as_str().to_string(),
                range_preview,
            });
        }

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
        pos: Position,
    ) -> Option<Node> {
        let nodes = read_view.nodes_by_file(root, path).ok()?;

        // Find smallest enclosing node
        let mut best: Option<Node> = None;
        for n in nodes {
            if let Some(r) = &n.range
                && r.start.line <= pos.line
                && r.end.line >= pos.line
            {
                if let Some(prev) = &best {
                    let prev_span = prev
                        .range
                        .as_ref()
                        .map(|pr| pr.span.len())
                        .unwrap_or(usize::MAX);
                    if r.span.len() < prev_span {
                        best = Some(n);
                    }
                } else {
                    best = Some(n);
                }
            }
        }
        best
    }
}
