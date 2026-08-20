use repin_core::config::ContextConfig;
use repin_core::model::node::Node;
use repin_core::ports::store::ReadView;
use repin_fs::CapabilityFs;
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

    pub fn extract_verbatim_lines(
        fs: &CapabilityFs,
        path: &str,
        start_line: u32,
        end_line: u32,
    ) -> Option<String> {
        Self::extract_verbatim_lines_with_padding(fs, path, start_line, end_line, 0)
    }

    pub fn extract_verbatim_lines_with_padding(
        fs: &CapabilityFs,
        path: &str,
        start_line: u32,
        end_line: u32,
        padding_lines: usize,
    ) -> Option<String> {
        let snapshot = fs.read_snapshot(path).ok()?;
        let content = String::from_utf8_lossy(&snapshot.content);
        let lines: Vec<&str> = content.lines().collect();
        if lines.is_empty() {
            return None;
        }

        let pad_u32 = padding_lines as u32;
        let start_padded = start_line.saturating_sub(1).saturating_sub(pad_u32);
        let start_idx = (start_padded as usize).min(lines.len() - 1);
        let end_idx = ((end_line as usize) + padding_lines).max(start_idx + 1).min(lines.len());

        let mut formatted = String::new();
        for (i, line) in lines[start_idx..end_idx].iter().enumerate() {
            let line_no = start_idx + i + 1;
            formatted.push_str(&format!("{:>4}: {}\n", line_no, line));
        }
        Some(formatted)
    }

    pub fn assemble_neighborhood(
        read_view: &dyn ReadView,
        nodes: &[Node],
        budget_bytes: usize,
    ) -> AssembledContext {
        Self::assemble_neighborhood_with_fs(read_view, None, nodes, budget_bytes)
    }

    pub fn assemble_neighborhood_with_fs(
        read_view: &dyn ReadView,
        fs: Option<&CapabilityFs>,
        nodes: &[Node],
        budget_bytes: usize,
    ) -> AssembledContext {
        let config = ContextConfig {
            default_token_budget: budget_bytes / 4,
            padding_lines: 0,
            include_blast_radius: true,
            include_verbatim_source: true,
        };
        Self::assemble_neighborhood_with_config(read_view, fs, nodes, &config, budget_bytes)
    }

    pub fn assemble_neighborhood_with_config(
        read_view: &dyn ReadView,
        fs: Option<&CapabilityFs>,
        nodes: &[Node],
        config: &ContextConfig,
        budget_bytes: usize,
    ) -> AssembledContext {
        let mut snippets = Vec::new();
        let mut total_bytes = 0;
        let mut truncated = false;
        let mut seen_ids = std::collections::HashSet::new();

        for node in nodes {
            if seen_ids.insert(node.id) {
                let incoming_count = read_view.incoming_edge_count(&node.id).unwrap_or(0);
                let outgoing = read_view
                    .edges_from(&node.id, &Default::default())
                    .unwrap_or_default();
                let outgoing_count = outgoing.len();

                let blast_header = if config.include_blast_radius {
                    format!("Blast Radius: {} incoming callers, {} outgoing relations\n", incoming_count, outgoing_count)
                } else {
                    String::new()
                };

                let verbatim_opt = if config.include_verbatim_source {
                    if let Some(ref range) = node.range
                        && let Some(fs_ref) = fs
                    {
                        Self::extract_verbatim_lines_with_padding(
                            fs_ref,
                            &node.path,
                            range.start.line,
                            range.end.line,
                            config.padding_lines,
                        )
                    } else {
                        None
                    }
                } else {
                    None
                };

                let text = if let Some(verbatim) = verbatim_opt {
                    let range_str = if let Some(ref range) = node.range {
                        format!(" (L{}-L{})", range.start.line, range.end.line)
                    } else {
                        String::new()
                    };
                    format!(
                        "Symbol: {} ({})\nFile: {}{}\n{}----------------------------------------\n{}",
                        node.name,
                        node.kind.as_str(),
                        node.path,
                        range_str,
                        blast_header,
                        verbatim
                    )
                } else {
                    format!(
                        "Symbol: {} ({})\nFile: {}\nLine Range: {:?}\n{}Attributes: {}",
                        node.name,
                        node.kind.as_str(),
                        node.path,
                        node.range.as_ref().map(|r| format!(
                            "{}:{}..{}:{}",
                            r.start.line, r.start.column, r.end.line, r.end.column
                        )),
                        blast_header,
                        serde_json::to_string(&node.attributes).unwrap_or_default()
                    )
                };

                let bytes_len = text.len();
                if total_bytes + bytes_len > budget_bytes {
                    truncated = true;
                    break;
                }
                total_bytes += bytes_len;
                let start_l = node.range.as_ref().map(|r| r.start.line).unwrap_or(1);
                let end_l = node.range.as_ref().map(|r| r.end.line).unwrap_or(start_l);
                snippets.push(ContextSnippet {
                    root: node.root.clone(),
                    path: node.path.clone(),
                    start_line: start_l,
                    end_line: end_l,
                    content: text,
                });
            }

            // Find outgoing neighbors
            let edges = read_view
                .edges_from(&node.id, &Default::default())
                .unwrap_or_default();
            for edge in edges {
                if let Ok(Some(target_node)) = read_view.node(&edge.to)
                    && seen_ids.insert(target_node.id)
                {
                    let incoming_count = read_view.incoming_edge_count(&target_node.id).unwrap_or(0);
                        let outgoing = read_view
                            .edges_from(&target_node.id, &Default::default())
                            .unwrap_or_default();
                        let outgoing_count = outgoing.len();

                        let blast_header = if config.include_blast_radius {
                            format!("Blast Radius: {} incoming callers, {} outgoing relations\n", incoming_count, outgoing_count)
                        } else {
                            String::new()
                        };

                        let verbatim_opt = if config.include_verbatim_source {
                            if let Some(ref range) = target_node.range
                                && let Some(fs_ref) = fs
                            {
                                Self::extract_verbatim_lines_with_padding(
                                    fs_ref,
                                    &target_node.path,
                                    range.start.line,
                                    range.end.line,
                                    config.padding_lines,
                                )
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        let text = if let Some(verbatim) = verbatim_opt {
                            let range_str = if let Some(ref range) = target_node.range {
                                format!(" (L{}-L{})", range.start.line, range.end.line)
                            } else {
                                String::new()
                            };
                            format!(
                                "Symbol: {} ({})\nFile: {}{}\n{}----------------------------------------\n{}",
                                target_node.name,
                                target_node.kind.as_str(),
                                target_node.path,
                                range_str,
                                blast_header,
                                verbatim
                            )
                        } else {
                            format!(
                                "Symbol: {} ({})\nFile: {}\nLine Range: {:?}\n{}Attributes: {}",
                                target_node.name,
                                target_node.kind.as_str(),
                                target_node.path,
                                target_node.range.as_ref().map(|r| format!(
                                    "{}:{}..{}:{}",
                                    r.start.line, r.start.column, r.end.line, r.end.column
                                )),
                                blast_header,
                                serde_json::to_string(&target_node.attributes).unwrap_or_default()
                            )
                        };

                        let bytes_len = text.len();
                        if total_bytes + bytes_len > budget_bytes {
                            truncated = true;
                            break;
                        }
                        total_bytes += bytes_len;
                        let start_l = target_node.range.as_ref().map(|r| r.start.line).unwrap_or(1);
                        let end_l = target_node.range.as_ref().map(|r| r.end.line).unwrap_or(start_l);
                        snippets.push(ContextSnippet {
                            root: target_node.root.clone(),
                            path: target_node.path.clone(),
                            start_line: start_l,
                            end_line: end_l,
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

#[cfg(test)]
mod tests {
    use super::*;
    use repin_core::model::identity::NodeId;
    use repin_core::model::registries::{ArtifactClass, NodeKind};
    use repin_core::ports::store::Store;
    use repin_core::{Confidence, Derivation, FactOwner, Provenance, Revision};
    use repin_store_sqlite::SqliteStore;
    use std::collections::BTreeMap;
    use tempfile::tempdir;

    fn make_test_node(name: &str, path: &str) -> Node {
        Node {
            id: NodeId::new(NodeKind::Struct, "root", path, &[], name, 0),
            kind: NodeKind::Struct,
            name: name.to_string(),
            qualified_name: Some(format!("crate::{}", name)),
            root: "root".to_string(),
            path: path.to_string(),
            range: None,
            language: Some("rust".to_string()),
            artifact_class: Some(ArtifactClass::Code),
            provenance: Provenance {
                root: "root".to_string(),
                path: path.to_string(),
                range: None,
                extractor: "test".to_string(),
                extractor_version: "1.0".to_string(),
                derivation: Derivation::Extracted,
                confidence: Confidence::EXACT,
                revision: Revision::INITIAL,
            },
            attributes: BTreeMap::new(),
        }
    }

    #[test]
    fn test_context_builder_budgeting() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.sqlite3");
        let store = SqliteStore::open(&db_path).unwrap();

        let mut tx = store.begin_write().unwrap();
        let node1 = make_test_node("Alpha", "src/lib.rs");
        let node2 = make_test_node("Beta", "src/types.rs");
        let owner = FactOwner::new("root", "src/lib.rs", "test", "1.0");

        tx.put_nodes(&[
            repin_core::NodeClaim { node: node1.clone(), owner: owner.clone() },
            repin_core::NodeClaim { node: node2.clone(), owner },
        ]).unwrap();
        tx.commit().unwrap();

        let read_view = store.read_view().unwrap();

        // High budget should include both
        let context = ContextBuilder::assemble_neighborhood(
            read_view.as_ref(),
            &[node1.clone(), node2.clone()],
            4096,
        );
        assert_eq!(context.snippets.len(), 2);
        assert!(!context.truncated);

        // Very small budget should truncate
        let tiny_context = ContextBuilder::assemble_neighborhood(
            read_view.as_ref(),
            &[node1, node2],
            50,
        );
        assert!(tiny_context.truncated || tiny_context.snippets.len() <= 1);
    }
}
