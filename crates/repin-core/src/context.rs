//! Deterministic, budgeted context construction over Repin port contracts.
//!
//! This crate does not select a filesystem or store implementation. Source
//! reads use [`crate::ports::SourceFs`], while graph enrichment uses the
//! borrowed [`crate::ports::ReadView`] contract.

use crate::config::ContextConfig;
use crate::model::identity::NodeId;
use crate::model::node::Node;
use crate::ports::fs::SourceFs;
use crate::ports::store::ReadView;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextBudget {
    pub max_bytes: Option<usize>,
    pub max_lines: Option<usize>,
    pub max_units: Option<usize>,
}

impl ContextBudget {
    pub const fn bytes(max_bytes: usize) -> Self {
        Self {
            max_bytes: Some(max_bytes),
            max_lines: None,
            max_units: None,
        }
    }

    pub const fn unlimited() -> Self {
        Self {
            max_bytes: None,
            max_lines: None,
            max_units: None,
        }
    }

    pub const fn with_lines(self, max_lines: usize) -> Self {
        Self {
            max_lines: Some(max_lines),
            ..self
        }
    }

    pub const fn with_units(self, max_units: usize) -> Self {
        Self {
            max_units: Some(max_units),
            ..self
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetUsage {
    pub bytes: usize,
    pub lines: usize,
    pub units: Option<usize>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BudgetedContext {
    pub snippets: Vec<ContextSnippet>,
    pub usage: BudgetUsage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ContextBudgetError {
    #[error("a unit budget requires an explicit unit estimator")]
    MissingUnitEstimator,
}

/// Deterministic context construction over graph nodes and current source.
#[derive(Debug, Default, Clone, Copy)]
pub struct ContextBuilder;

impl ContextBuilder {
    pub const DEFAULT_BYTE_BUDGET: usize = 64 * 1024;

    pub fn extract_verbatim_lines(
        source: &dyn SourceFs,
        path: &str,
        start_line: u32,
        end_line: u32,
    ) -> Option<String> {
        Self::extract_verbatim_lines_with_padding(source, path, start_line, end_line, 0)
    }

    pub fn extract_verbatim_lines_with_padding(
        source: &dyn SourceFs,
        path: &str,
        start_line: u32,
        end_line: u32,
        padding_lines: usize,
    ) -> Option<String> {
        let snapshot = source.read_snapshot(path).ok()?;
        let content = String::from_utf8_lossy(&snapshot.content);
        let lines: Vec<&str> = content.lines().collect();
        if lines.is_empty() {
            return None;
        }

        let pad_u32 = padding_lines as u32;
        let start_padded = start_line.saturating_sub(1).saturating_sub(pad_u32);
        let start_idx = (start_padded as usize).min(lines.len() - 1);
        let end_idx = ((end_line as usize) + padding_lines)
            .max(start_idx + 1)
            .min(lines.len());

        let mut formatted = String::new();
        for (index, line) in lines[start_idx..end_idx].iter().enumerate() {
            let line_no = start_idx + index + 1;
            formatted.push_str(&format!("{line_no:>4}: {line}\n"));
        }
        Some(redact_sensitive(&formatted))
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
        source: Option<&dyn SourceFs>,
        nodes: &[Node],
        budget_bytes: usize,
    ) -> AssembledContext {
        let config = ContextConfig {
            default_token_budget: budget_bytes / 4,
            padding_lines: 0,
            include_blast_radius: true,
            include_verbatim_source: true,
        };
        Self::assemble_neighborhood_with_config(read_view, source, nodes, &config, budget_bytes)
    }

    pub fn assemble_neighborhood_with_config(
        read_view: &dyn ReadView,
        source: Option<&dyn SourceFs>,
        nodes: &[Node],
        config: &ContextConfig,
        budget_bytes: usize,
    ) -> AssembledContext {
        let mut snippets = Vec::new();
        let mut total_bytes = 0;
        let mut truncated = false;
        let mut seen_ids = std::collections::HashSet::new();

        let mut roots = nodes.to_vec();
        roots.sort_by_key(|node| node.id);

        for node in roots {
            if !append_node(
                &mut snippets,
                &mut total_bytes,
                &mut truncated,
                &mut seen_ids,
                read_view,
                source,
                &node,
                config,
                budget_bytes,
            ) {
                break;
            }

            let mut edges = read_view
                .edges_from(&node.id, &Default::default())
                .unwrap_or_default();
            edges.sort_by_key(|edge| edge.id);

            for edge in edges {
                if let Ok(Some(target_node)) = read_view.node(&edge.to)
                    && !append_node(
                        &mut snippets,
                        &mut total_bytes,
                        &mut truncated,
                        &mut seen_ids,
                        read_view,
                        source,
                        &target_node,
                        config,
                        budget_bytes,
                    )
                {
                    break;
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

    /// Assemble all eligible evidence, then pack it under exact byte, line,
    /// and optional caller-defined unit limits.
    pub fn assemble_neighborhood_with_budget(
        read_view: &dyn ReadView,
        source: Option<&dyn SourceFs>,
        nodes: &[Node],
        budget: ContextBudget,
        unit_estimator: Option<&dyn Fn(&str) -> usize>,
    ) -> Result<BudgetedContext, ContextBudgetError> {
        if budget.max_units.is_some() && unit_estimator.is_none() {
            return Err(ContextBudgetError::MissingUnitEstimator);
        }
        let assembled = Self::assemble_neighborhood_with_fs(read_view, source, nodes, usize::MAX);
        pack_snippets(&assembled.snippets, budget, unit_estimator)
    }
}

pub fn pack_snippets(
    snippets: &[ContextSnippet],
    budget: ContextBudget,
    unit_estimator: Option<&dyn Fn(&str) -> usize>,
) -> Result<BudgetedContext, ContextBudgetError> {
    if budget.max_units.is_some() && unit_estimator.is_none() {
        return Err(ContextBudgetError::MissingUnitEstimator);
    }

    let mut selected = Vec::new();
    let mut bytes: usize = 0;
    let mut lines: usize = 0;
    let mut units: Option<usize> = unit_estimator.map(|_| 0);
    let mut truncated = false;
    for snippet in snippets {
        let snippet_bytes = snippet.content.len();
        let snippet_lines = snippet.content.lines().count();
        let snippet_units = unit_estimator.map(|estimator| estimator(&snippet.content));
        let exceeds_bytes = budget
            .max_bytes
            .is_some_and(|limit| bytes.saturating_add(snippet_bytes) > limit);
        let exceeds_lines = budget
            .max_lines
            .is_some_and(|limit| lines.saturating_add(snippet_lines) > limit);
        let exceeds_units = budget.max_units.is_some_and(|limit| {
            units
                .unwrap_or_default()
                .saturating_add(snippet_units.unwrap_or_default())
                > limit
        });
        if exceeds_bytes || exceeds_lines || exceeds_units {
            truncated = true;
            break;
        }
        bytes += snippet_bytes;
        lines += snippet_lines;
        if let (Some(total), Some(snippet_units)) = (units.as_mut(), snippet_units) {
            *total += snippet_units;
        }
        selected.push(snippet.clone());
    }

    Ok(BudgetedContext {
        snippets: selected,
        usage: BudgetUsage {
            bytes,
            lines,
            units,
            truncated,
        },
    })
}

#[allow(clippy::too_many_arguments)]
fn append_node(
    snippets: &mut Vec<ContextSnippet>,
    total_bytes: &mut usize,
    truncated: &mut bool,
    seen_ids: &mut std::collections::HashSet<NodeId>,
    read_view: &dyn ReadView,
    source: Option<&dyn SourceFs>,
    node: &Node,
    config: &ContextConfig,
    budget_bytes: usize,
) -> bool {
    if !seen_ids.insert(node.id) {
        return true;
    }

    let incoming_count = read_view.incoming_edge_count(&node.id).unwrap_or(0);
    let outgoing_count = read_view
        .edges_from(&node.id, &Default::default())
        .map_or(0, |edges| edges.len());

    let blast_header = if config.include_blast_radius {
        format!(
            "Blast Radius: {incoming_count} incoming callers, {outgoing_count} outgoing relations\n"
        )
    } else {
        String::new()
    };

    let verbatim = if config.include_verbatim_source {
        node.range.as_ref().and_then(|range| {
            source.and_then(|source| {
                ContextBuilder::extract_verbatim_lines_with_padding(
                    source,
                    &node.path,
                    range.start.line,
                    range.end.line,
                    config.padding_lines,
                )
            })
        })
    } else {
        None
    };

    let text = if let Some(verbatim) = verbatim {
        let range = node.range.as_ref().map_or_else(String::new, |range| {
            format!(" (L{}-L{})", range.start.line, range.end.line)
        });
        format!(
            "Symbol: {} ({})\nFile: {}{}\n{}----------------------------------------\n{}",
            node.name,
            node.kind.as_str(),
            node.path,
            range,
            blast_header,
            verbatim
        )
    } else {
        let range = node.range.as_ref().map_or_else(String::new, |range| {
            format!(
                "{}:{}..{}:{}",
                range.start.line, range.start.column, range.end.line, range.end.column
            )
        });
        format!(
            "Symbol: {} ({})\nFile: {}\nLine Range: {}\n{}Attributes: {}",
            node.name,
            node.kind.as_str(),
            node.path,
            range,
            blast_header,
            serde_json::to_string(&node.attributes).unwrap_or_default()
        )
    };
    let text = redact_sensitive(&text);
    let bytes_len = text.len();
    if total_bytes.saturating_add(bytes_len) > budget_bytes {
        *truncated = true;
        return false;
    }

    *total_bytes += bytes_len;
    let start_line = node.range.as_ref().map_or(1, |range| range.start.line);
    let end_line = node
        .range
        .as_ref()
        .map_or(start_line, |range| range.end.line);
    snippets.push(ContextSnippet {
        root: node.root.clone(),
        path: node.path.clone(),
        start_line,
        end_line,
        content: text,
    });
    true
}

fn redact_sensitive(text: &str) -> String {
    let sensitive_keys = [
        "api_key",
        "access_key",
        "credential",
        "private_key",
        "secret",
        "password",
        "token",
        "auth",
        "bearer",
    ];
    let mut output = String::with_capacity(text.len());
    for segment in text.split_inclusive('\n') {
        let line = segment.strip_suffix('\n').unwrap_or(segment);
        let lower = line.to_ascii_lowercase();
        let key_end = sensitive_keys
            .iter()
            .filter_map(|key| lower.find(key).map(|index| index + key.len()))
            .min();
        let redacted = key_end
            .and_then(|end| line[end..].find(['=', ':']).map(|offset| end + offset))
            .map(|separator| format!("{} [REDACTED]", &line[..=separator]))
            .unwrap_or_else(|| line.to_string());
        output.push_str(&redacted);
        if segment.ends_with('\n') {
            output.push('\n');
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::ContentHash;
    use crate::model::provenance::{Confidence, Derivation, FactOwner, Provenance, Revision};
    use crate::model::registries::{ArtifactClass, NodeKind};
    use crate::ports::fs::{FileSnapshot, SourceError};
    use crate::ports::store::Store;
    use crate::store::SqliteStore;
    use std::collections::BTreeMap;
    use std::path::Path;
    use tempfile::tempdir;

    #[derive(Debug)]
    struct TestSource {
        root: String,
        snapshots: BTreeMap<String, Vec<u8>>,
    }

    impl SourceFs for TestSource {
        fn read_snapshot(&self, relative_path: &str) -> Result<FileSnapshot, SourceError> {
            let content =
                self.snapshots
                    .get(relative_path)
                    .cloned()
                    .ok_or_else(|| SourceError::Io {
                        path: relative_path.to_string(),
                        message: "missing test file".to_string(),
                    })?;
            Ok(FileSnapshot {
                root: self.root.clone(),
                path: relative_path.to_string(),
                content_hash: ContentHash::of_bytes(&content),
                content,
                artifact_class: ArtifactClass::Code,
            })
        }

        fn walk_files(
            &self,
            callback: &mut dyn FnMut(FileSnapshot) -> Result<(), SourceError>,
        ) -> Result<(), SourceError> {
            for path in self.snapshots.keys() {
                callback(self.read_snapshot(path)?)?;
            }
            Ok(())
        }
    }

    fn make_test_node(name: &str, path: &str) -> Node {
        Node {
            id: NodeId::new(NodeKind::Struct, "root", path, &[], name, 0),
            kind: NodeKind::Struct,
            name: name.to_string(),
            qualified_name: Some(format!("crate::{name}")),
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
    fn graph_free_source_reads_are_contained_and_redacted() {
        let source = TestSource {
            root: "root".to_string(),
            snapshots: BTreeMap::from([(
                "src/lib.rs".to_string(),
                b"pub fn run() {}\nAPI_KEY=secret-value\n".to_vec(),
            )]),
        };
        let text = ContextBuilder::extract_verbatim_lines(&source, "src/lib.rs", 1, 2).unwrap();
        assert!(text.contains("pub fn run()"));
        assert!(text.contains("[REDACTED]"));
        assert!(!text.contains("secret-value"));
        assert!(ContextBuilder::extract_verbatim_lines(&source, "../outside", 1, 1).is_none());
    }

    #[test]
    fn graph_enriched_context_is_deterministic_and_budgeted() {
        let dir = tempdir().unwrap();
        let store = SqliteStore::open(dir.path().join("test.sqlite3")).unwrap();
        let node1 = make_test_node("Alpha", "src/lib.rs");
        let node2 = make_test_node("Beta", "src/types.rs");
        let owner = FactOwner::new("root", "src/lib.rs", "test", "1.0");
        let mut tx = store.begin_write().unwrap();
        tx.put_nodes(&[
            crate::NodeClaim {
                node: node1.clone(),
                owner: owner.clone(),
            },
            crate::NodeClaim {
                node: node2.clone(),
                owner,
            },
        ])
        .unwrap();
        tx.commit().unwrap();

        let read_view = store.read_view().unwrap();
        let first = ContextBuilder::assemble_neighborhood(
            read_view.as_ref(),
            &[node2.clone(), node1.clone()],
            4096,
        );
        let second =
            ContextBuilder::assemble_neighborhood(read_view.as_ref(), &[node1, node2], 4096);
        assert_eq!(first, second);
        assert_eq!(first.snippets.len(), 2);

        let tiny = ContextBuilder::assemble_neighborhood(read_view.as_ref(), &[], 1);
        assert!(!tiny.truncated);
        assert!(Path::new("src/lib.rs").is_relative());
    }

    #[test]
    fn byte_line_and_unit_budgets_are_exact() {
        let snippets = vec![
            ContextSnippet {
                root: "root".to_string(),
                path: "a.rs".to_string(),
                start_line: 1,
                end_line: 2,
                content: "one\ntwo\n".to_string(),
            },
            ContextSnippet {
                root: "root".to_string(),
                path: "b.rs".to_string(),
                start_line: 1,
                end_line: 1,
                content: "three\n".to_string(),
            },
        ];
        let estimator = |text: &str| text.split_whitespace().count();
        let packed = pack_snippets(
            &snippets,
            ContextBudget::bytes(snippets[0].content.len() + snippets[1].content.len())
                .with_lines(2)
                .with_units(2),
            Some(&estimator),
        )
        .unwrap();
        assert_eq!(packed.snippets.len(), 1);
        assert_eq!(packed.usage.bytes, snippets[0].content.len());
        assert_eq!(packed.usage.lines, 2);
        assert_eq!(packed.usage.units, Some(2));
        assert!(packed.usage.truncated);
        assert!(matches!(
            pack_snippets(&snippets, ContextBudget::unlimited().with_units(2), None),
            Err(ContextBudgetError::MissingUnitEstimator)
        ));
    }
}
