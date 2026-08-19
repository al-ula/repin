use repin_core::model::node::Node;
use repin_core::ports::store::ReadView;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextSnippet {
    pub root: String,
    pub path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssembledContext {
    pub snippets: Vec<ContextSnippet>,
    pub total_bytes: usize,
    pub truncated: bool,
}

pub struct ContextBuilder;

impl ContextBuilder {
    pub const DEFAULT_BYTE_BUDGET: usize = 64 * 1024; // 64KB

    pub fn assemble_neighborhood(
        read_view: &dyn ReadView,
        nodes: &[Node],
        budget_bytes: usize,
    ) -> AssembledContext {
        let mut snippets = Vec::new();
        let mut total_bytes = 0;
        let mut truncated = false;

        for node in nodes {
            // Find edges and neighboring nodes
            let outgoing = read_view
                .edges_from(&node.id, &Default::default())
                .unwrap_or_default();
            for edge in outgoing {
                if let Ok(Some(neighbor)) = read_view.node(&edge.to)
                    && let Some(r) = &neighbor.range
                {
                    let text = format!(
                        "{}: {} ({})",
                        neighbor.path,
                        neighbor.name,
                        neighbor.kind.as_str()
                    );
                    let bytes_len = text.len();

                    if total_bytes + bytes_len > budget_bytes {
                        truncated = true;
                        break;
                    }

                    total_bytes += bytes_len;
                    snippets.push(ContextSnippet {
                        root: neighbor.root,
                        path: neighbor.path,
                        start_line: r.start.line,
                        end_line: r.end.line,
                        content: text,
                    });
                }
            }

            if truncated {
                break;
            }
        }

        AssembledContext {
            snippets,
            total_bytes,
            truncated,
        }
    }
}
