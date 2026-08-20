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
        let snapshot = fs.read_snapshot(path).ok()?;
        let content = String::from_utf8_lossy(&snapshot.content);
        let lines: Vec<&str> = content.lines().collect();
        if lines.is_empty() {
            return None;
        }

        let start_idx = (start_line.saturating_sub(1) as usize).min(lines.len() - 1);
        let end_idx = (end_line as usize).max(start_idx + 1).min(lines.len());

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

                let text = if let Some(ref range) = node.range
                    && let Some(fs_ref) = fs
                    && let Some(verbatim) = Self::extract_verbatim_lines(
                        fs_ref,
                        &node.path,
                        range.start.line,
                        range.end.line,
                    ) {
                    format!(
                        "Symbol: {} ({})\nFile: {} (L{}-L{})\nBlast Radius: {} incoming callers, {} outgoing relations\n----------------------------------------\n{}",
                        node.name,
                        node.kind.as_str(),
                        node.path,
                        range.start.line,
                        range.end.line,
                        incoming_count,
                        outgoing_count,
                        verbatim
                    )
                } else {
                    format!(
                        "Symbol: {} ({})\nFile: {}\nLine Range: {:?}\nBlast Radius: {} incoming callers, {} outgoing relations\nAttributes: {}",
                        node.name,
                        node.kind.as_str(),
                        node.path,
                        node.range.as_ref().map(|r| format!(
                            "{}:{}..{}:{}",
                            r.start.line, r.start.column, r.end.line, r.end.column
                        )),
                        incoming_count,
                        outgoing_count,
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
            let outgoing = read_view
                .edges_from(&node.id, &Default::default())
                .unwrap_or_default();
            for edge in outgoing {
                if let Ok(Some(neighbor)) = read_view.node(&edge.to)
                    && seen_ids.insert(neighbor.id)
                {
                    let text = if let Some(ref range) = neighbor.range
                        && let Some(fs_ref) = fs
                        && let Some(verbatim) = Self::extract_verbatim_lines(
                            fs_ref,
                            &neighbor.path,
                            range.start.line,
                            range.end.line,
                        ) {
                        format!(
                            "Outgoing Relation [{}]: {} ({}) in {} (L{}-L{})\n----------------------------------------\n{}",
                            edge.kind.as_str(),
                            neighbor.name,
                            neighbor.kind.as_str(),
                            neighbor.path,
                            range.start.line,
                            range.end.line,
                            verbatim
                        )
                    } else {
                        format!(
                            "Outgoing Relation [{}]: {} ({}) in {}",
                            edge.kind.as_str(),
                            neighbor.name,
                            neighbor.kind.as_str(),
                            neighbor.path
                        )
                    };

                    let bytes_len = text.len();
                    if total_bytes + bytes_len > budget_bytes {
                        truncated = true;
                        break;
                    }
                    total_bytes += bytes_len;
                    let start_l = neighbor.range.as_ref().map(|r| r.start.line).unwrap_or(1);
                    let end_l = neighbor
                        .range
                        .as_ref()
                        .map(|r| r.end.line)
                        .unwrap_or(start_l);
                    snippets.push(ContextSnippet {
                        root: neighbor.root,
                        path: neighbor.path,
                        start_line: start_l,
                        end_line: end_l,
                        content: text,
                    });
                }
            }

            // Find incoming neighbors
            let incoming = read_view
                .edges_to(&node.id, &Default::default())
                .unwrap_or_default();
            for edge in incoming {
                if let Ok(Some(neighbor)) = read_view.node(&edge.from)
                    && seen_ids.insert(neighbor.id)
                {
                    let text = if let Some(ref range) = neighbor.range
                        && let Some(fs_ref) = fs
                        && let Some(verbatim) = Self::extract_verbatim_lines(
                            fs_ref,
                            &neighbor.path,
                            range.start.line,
                            range.end.line,
                        ) {
                        format!(
                            "Incoming Relation [{}]: {} ({}) in {} (L{}-L{})\n----------------------------------------\n{}",
                            edge.kind.as_str(),
                            neighbor.name,
                            neighbor.kind.as_str(),
                            neighbor.path,
                            range.start.line,
                            range.end.line,
                            verbatim
                        )
                    } else {
                        format!(
                            "Incoming Relation [{}]: {} ({}) in {}",
                            edge.kind.as_str(),
                            neighbor.name,
                            neighbor.kind.as_str(),
                            neighbor.path
                        )
                    };

                    let bytes_len = text.len();
                    if total_bytes + bytes_len > budget_bytes {
                        truncated = true;
                        break;
                    }
                    total_bytes += bytes_len;
                    let start_l = neighbor.range.as_ref().map(|r| r.start.line).unwrap_or(1);
                    let end_l = neighbor
                        .range
                        .as_ref()
                        .map(|r| r.end.line)
                        .unwrap_or(start_l);
                    snippets.push(ContextSnippet {
                        root: neighbor.root,
                        path: neighbor.path,
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
    use repin_core::line_index::{ByteSpan, Position, Range};
    use repin_core::model::identity::NodeId;
    use repin_core::model::provenance::{Confidence, Derivation, Provenance, Revision};
    use repin_core::model::registries::NodeKind;
    use repin_core::ports::store::Store;
    use repin_store_sqlite::SqliteStore;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_verbatim_extraction_and_budget() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("main.rs");
        let mut file = std::fs::File::create(&file_path).unwrap();
        writeln!(file, "fn first() {{}}").unwrap();
        writeln!(file, "fn second() {{}}").unwrap();
        writeln!(file, "fn third() {{}}").unwrap();

        let cap_fs = CapabilityFs::open("root", dir.path()).unwrap();
        let verbatim = ContextBuilder::extract_verbatim_lines(&cap_fs, "main.rs", 2, 3).unwrap();
        assert!(verbatim.contains("   2: fn second() {}"));
        assert!(verbatim.contains("   3: fn third() {}"));
        assert!(!verbatim.contains("fn first"));

        let store = SqliteStore::open_in_memory().unwrap();
        let view = store.read_view().unwrap();

        let node = Node {
            id: NodeId::new(NodeKind::Function, "root", "main.rs", &[], "second", 0),
            kind: NodeKind::Function,
            name: "second".to_string(),
            qualified_name: Some("crate::second".to_string()),
            root: "root".to_string(),
            path: "main.rs".to_string(),
            range: Some(Range {
                span: ByteSpan::new(16, 33),
                start: Position { line: 2, column: 1 },
                end: Position {
                    line: 2,
                    column: 17,
                },
            }),
            language: Some("rust".to_string()),
            artifact_class: None,
            provenance: Provenance {
                root: "root".to_string(),
                path: "main.rs".to_string(),
                range: None,
                extractor: "test".to_string(),
                extractor_version: "1.0".to_string(),
                derivation: Derivation::Extracted,
                confidence: Confidence::EXACT,
                revision: Revision::INITIAL,
            },
            attributes: Default::default(),
        };

        // Budget enough for snippet
        let assembled = ContextBuilder::assemble_neighborhood_with_fs(
            &*view,
            Some(&cap_fs),
            &[node.clone()],
            1024,
        );
        assert_eq!(assembled.snippets.len(), 1);
        assert!(!assembled.truncated);
        assert!(assembled.snippets[0].content.contains("Blast Radius:"));
        assert!(assembled.snippets[0].content.contains("   2: fn second() {}"));

        // Tiny budget causing truncation
        let assembled_tiny =
            ContextBuilder::assemble_neighborhood_with_fs(&*view, Some(&cap_fs), &[node], 10);
        assert_eq!(assembled_tiny.snippets.len(), 0);
        assert!(assembled_tiny.truncated);
    }
}
