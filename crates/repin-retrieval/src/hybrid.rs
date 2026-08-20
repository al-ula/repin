use crate::ranking::{DeterministicRanker, RankReason, RankedCandidate};
use crate::vector::VectorHit;
use repin_core::model::identity::NodeId;
use repin_core::ports::store::ReadView;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct LexicalHit {
    pub node_id: NodeId,
    pub score: f64,
}

/// Adapter contract for a lexical candidate source. The retrieval crate does
/// not select or depend on a concrete FTS implementation.
pub trait LexicalSource: Send + Sync {
    fn search(&self, query: &str, limit: usize) -> Result<Vec<LexicalHit>, String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetrievalMetadata {
    pub lexical_available: bool,
    pub lexical_failed: bool,
    pub graph_available: bool,
    pub vector_available: bool,
    pub candidate_count: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RetrievalResult {
    pub candidates: Vec<RankedCandidate>,
    pub metadata: RetrievalMetadata,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct HybridRetriever;

impl HybridRetriever {
    /// Merge lexical, graph-name, and optional vector candidates over one
    /// borrowed read view. Ordering and tie-breaking are deterministic.
    pub fn search(
        read_view: &dyn ReadView,
        lexical: Option<&dyn LexicalSource>,
        query: &str,
        limit: usize,
        vector_hits: Option<&[VectorHit]>,
    ) -> RetrievalResult {
        if limit == 0 {
            return RetrievalResult {
                candidates: Vec::new(),
                metadata: RetrievalMetadata {
                    lexical_available: lexical.is_some(),
                    lexical_failed: false,
                    graph_available: true,
                    vector_available: vector_hits.is_some(),
                    candidate_count: 0,
                    truncated: false,
                },
            };
        }

        let mut candidate_map = HashMap::new();
        let mut lexical_scores = HashMap::new();
        let mut lexical_failed = false;
        if let Some(source) = lexical {
            match source.search(query, limit.saturating_mul(3)) {
                Ok(hits) => {
                    for hit in hits {
                        if let Ok(Some(node)) = read_view.node(&hit.node_id) {
                            lexical_scores.insert(node.id, hit.score);
                            candidate_map.insert(node.id, node);
                        }
                    }
                }
                Err(_) => lexical_failed = true,
            }
        }

        if let Ok(nodes) = read_view.nodes_by_name(query, &Default::default()) {
            for node in nodes {
                candidate_map.insert(node.id, node);
            }
        }
        for token in query.split_whitespace().filter(|token| token.len() >= 3) {
            if let Ok(nodes) = read_view.nodes_by_name(token, &Default::default()) {
                for node in nodes {
                    candidate_map.insert(node.id, node);
                }
            }
        }

        let mut candidates: Vec<_> = candidate_map.into_values().collect();
        candidates.sort_by_key(|node| node.id);
        let candidate_count = candidates.len();
        let mut in_degrees = HashMap::new();
        for node in &candidates {
            if let Ok(count) = read_view.incoming_edge_count(&node.id) {
                in_degrees.insert(node.id, count);
            }
        }

        let mut ranked =
            DeterministicRanker::rank_fusion(query, candidates, &lexical_scores, &in_degrees);
        if let Some(vector_hits) = vector_hits {
            let max_score = vector_hits
                .iter()
                .map(|hit| hit.score)
                .fold(0.0_f32, f32::max)
                .max(f32::EPSILON);
            let vector_scores: HashMap<_, _> = vector_hits
                .iter()
                .map(|hit| (hit.node_id, f64::from(hit.score / max_score) * 0.2))
                .collect();
            for candidate in &mut ranked {
                if let Some(score) = vector_scores.get(&candidate.node.id) {
                    candidate.explanation.total_score += *score;
                    candidate.explanation.reasons.push(RankReason {
                        signal: "vector_similarity".to_string(),
                        score: *score,
                        detail: Some("exact or provider-supplied vector channel".to_string()),
                    });
                }
            }
            ranked.sort_by(|left, right| {
                right
                    .explanation
                    .total_score
                    .partial_cmp(&left.explanation.total_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| left.node.id.cmp(&right.node.id))
            });
        }

        let truncated = ranked.len() > limit;
        ranked.truncate(limit);
        RetrievalResult {
            candidates: ranked,
            metadata: RetrievalMetadata {
                lexical_available: lexical.is_some(),
                lexical_failed,
                graph_available: true,
                vector_available: vector_hits.is_some(),
                candidate_count,
                truncated,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use repin_core::model::node::Node;
    use repin_core::model::provenance::{Confidence, Derivation, FactOwner, Provenance, Revision};
    use repin_core::model::registries::{ArtifactClass, NodeKind};
    use repin_core::ports::store::Store;
    use repin_store_sqlite::SqliteStore;

    #[derive(Debug)]
    struct TestLexical {
        hits: Result<Vec<LexicalHit>, String>,
    }

    impl LexicalSource for TestLexical {
        fn search(&self, _query: &str, _limit: usize) -> Result<Vec<LexicalHit>, String> {
            self.hits.clone()
        }
    }

    fn test_node(name: &str, path: &str) -> Node {
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
            attributes: Default::default(),
        }
    }

    #[test]
    fn hybrid_search_reports_optional_channel_failure_and_stable_results() {
        let store = SqliteStore::open_in_memory().unwrap();
        let alpha = test_node("Alpha", "src/alpha.rs");
        let beta = test_node("Alpha", "src/beta.rs");
        let owner = FactOwner::new("root", "src/alpha.rs", "test", "1.0");
        let mut transaction = store.begin_write().unwrap();
        transaction
            .put_nodes(&[
                repin_core::model::node::NodeClaim {
                    node: alpha.clone(),
                    owner: owner.clone(),
                },
                repin_core::model::node::NodeClaim {
                    node: beta.clone(),
                    owner,
                },
            ])
            .unwrap();
        transaction.commit().unwrap();

        let view = store.read_view().unwrap();
        let lexical = TestLexical {
            hits: Err("lexical index unavailable".to_string()),
        };
        let result = HybridRetriever::search(
            view.as_ref(),
            Some(&lexical),
            "Alpha",
            1,
            Some(&[VectorHit {
                node_id: alpha.id,
                score: 1.0,
            }]),
        );

        assert!(result.metadata.lexical_available);
        assert!(result.metadata.lexical_failed);
        assert!(result.metadata.vector_available);
        assert!(result.metadata.truncated);
        assert_eq!(result.candidates[0].node.id, alpha.id);
        assert!(
            result.candidates[0]
                .explanation
                .reasons
                .iter()
                .any(|reason| reason.signal == "vector_similarity")
        );
    }
}
