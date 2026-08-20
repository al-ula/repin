use repin_core::model::identity::NodeId;
use repin_core::model::node::Node;
use repin_core::model::registries::ArtifactClass;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RankExplanation {
    pub total_score: f64,
    pub reasons: Vec<RankReason>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RankReason {
    pub signal: String,
    pub score: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RankedCandidate {
    pub node: Node,
    pub explanation: RankExplanation,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct DeterministicRanker;

impl DeterministicRanker {
    pub fn rank(query: &str, candidates: Vec<Node>) -> Vec<RankedCandidate> {
        Self::rank_fusion(query, candidates, &HashMap::new(), &HashMap::new())
    }

    pub fn rank_fusion(
        query: &str,
        candidates: Vec<Node>,
        fts_ranks: &HashMap<NodeId, f64>,
        in_degrees: &HashMap<NodeId, usize>,
    ) -> Vec<RankedCandidate> {
        Self::rank_fusion_with_boost(query, candidates, fts_ranks, in_degrees, 0.15)
    }

    pub fn rank_fusion_with_boost(
        query: &str,
        candidates: Vec<Node>,
        fts_ranks: &HashMap<NodeId, f64>,
        in_degrees: &HashMap<NodeId, usize>,
        centrality_boost: f64,
    ) -> Vec<RankedCandidate> {
        let query_lower = query.to_lowercase();
        let mut ranked = Vec::with_capacity(candidates.len());
        let max_degree = in_degrees.values().copied().max().unwrap_or(1).max(1);

        for node in candidates {
            let mut score = 0.0;
            let mut reasons = Vec::new();
            let name_lower = node.name.to_lowercase();

            if name_lower == query_lower {
                score += 0.50;
                reasons.push(RankReason {
                    signal: "exact_name_match".to_string(),
                    score: 0.50,
                    detail: Some(format!("exact name '{}'", node.name)),
                });
            } else if name_lower.starts_with(&query_lower) {
                score += 0.30;
                reasons.push(RankReason {
                    signal: "name_prefix_match".to_string(),
                    score: 0.30,
                    detail: Some(format!("starts with '{}'", query)),
                });
            } else if name_lower.contains(&query_lower) {
                score += 0.15;
                reasons.push(RankReason {
                    signal: "name_substring_match".to_string(),
                    score: 0.15,
                    detail: Some(format!("contains '{}'", query)),
                });
            }

            let path_lower = node.path.to_lowercase();
            if path_lower.contains(&query_lower) {
                score += 0.10;
                reasons.push(RankReason {
                    signal: "path_match".to_string(),
                    score: 0.10,
                    detail: Some(format!("path contains '{}'", query)),
                });
            }

            if let Some(&fts_score) = fts_ranks.get(&node.id) {
                let normalized_fts = (fts_score.abs() * 0.1).min(0.25);
                score += normalized_fts;
                reasons.push(RankReason {
                    signal: "fts_lexical_match".to_string(),
                    score: normalized_fts,
                    detail: Some(format!("fts5 score {:.3}", fts_score)),
                });
            }

            if let Some(&in_degree) = in_degrees.get(&node.id)
                && in_degree > 0
                && centrality_boost > 0.0
            {
                let centrality_bonus =
                    ((in_degree as f64) / (max_degree as f64)).min(1.0) * centrality_boost;
                score += centrality_bonus;
                reasons.push(RankReason {
                    signal: "graph_degree_centrality".to_string(),
                    score: centrality_bonus,
                    detail: Some(format!("in-degree {in_degree} / max {max_degree}")),
                });
            }

            if let Some(artifact_class) = node.artifact_class
                && artifact_class == ArtifactClass::Code
            {
                score += 0.10;
                reasons.push(RankReason {
                    signal: "artifact_class_code".to_string(),
                    score: 0.10,
                    detail: None,
                });
            }

            ranked.push(RankedCandidate {
                node,
                explanation: RankExplanation {
                    total_score: score,
                    reasons,
                },
            });
        }

        ranked.sort_by(|a, b| {
            b.explanation
                .total_score
                .partial_cmp(&a.explanation.total_score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.node.id.cmp(&b.node.id))
        });
        ranked
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use repin_core::model::provenance::{Confidence, Derivation, Provenance, Revision};
    use repin_core::model::registries::NodeKind;

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
            attributes: Default::default(),
        }
    }

    #[test]
    fn centrality_boosting_is_deterministic() {
        let hub = make_test_node("Engine", "src/engine.rs");
        let leaf = make_test_node("Engine", "tests/helpers.rs");
        let in_degrees = HashMap::from([(hub.id, 25), (leaf.id, 1)]);
        let ranked = DeterministicRanker::rank_fusion(
            "Engine",
            vec![leaf.clone(), hub.clone()],
            &HashMap::new(),
            &in_degrees,
        );
        assert_eq!(ranked[0].node.id, hub.id);
        assert_eq!(ranked[1].node.id, leaf.id);
        assert!(
            ranked[0]
                .explanation
                .reasons
                .iter()
                .any(|reason| reason.signal == "graph_degree_centrality")
        );
    }
}
