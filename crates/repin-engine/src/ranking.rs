use repin_core::model::node::Node;
use repin_core::model::registries::ArtifactClass;

#[derive(Debug, Clone, PartialEq)]
pub struct RankExplanation {
    pub total_score: f64,
    pub reasons: Vec<RankReason>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RankReason {
    pub signal: &'static str,
    pub score: f64,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RankedCandidate {
    pub node: Node,
    pub explanation: RankExplanation,
}

pub struct DeterministicRanker;

impl DeterministicRanker {
    pub fn rank(query: &str, candidates: Vec<Node>) -> Vec<RankedCandidate> {
        let query_lower = query.to_lowercase();
        let mut ranked = Vec::with_capacity(candidates.len());

        for node in candidates {
            let mut score = 0.0;
            let mut reasons = Vec::new();

            let name_lower = node.name.to_lowercase();

            // Exact match signal
            if name_lower == query_lower {
                score += 0.50;
                reasons.push(RankReason {
                    signal: "exact_name_match",
                    score: 0.50,
                    detail: Some(format!("exact name '{}'", node.name)),
                });
            } else if name_lower.starts_with(&query_lower) {
                score += 0.30;
                reasons.push(RankReason {
                    signal: "name_prefix_match",
                    score: 0.30,
                    detail: Some(format!("starts with '{}'", query)),
                });
            } else if name_lower.contains(&query_lower) {
                score += 0.15;
                reasons.push(RankReason {
                    signal: "name_substring_match",
                    score: 0.15,
                    detail: Some(format!("contains '{}'", query)),
                });
            }

            // Path proximity signal
            let path_lower = node.path.to_lowercase();
            if path_lower.contains(&query_lower) {
                score += 0.10;
                reasons.push(RankReason {
                    signal: "path_match",
                    score: 0.10,
                    detail: Some(format!("path contains '{}'", query)),
                });
            }

            // Artifact class preference
            if let Some(artifact_class) = node.artifact_class
                && artifact_class == ArtifactClass::Code
            {
                score += 0.10;
                reasons.push(RankReason {
                    signal: "artifact_class_code",
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

        // Deterministic stable sorting: highest score first, ties broken by node id bytes
        ranked.sort_by(|a, b| {
            b.explanation
                .total_score
                .partial_cmp(&a.explanation.total_score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.node.id.as_bytes().cmp(b.node.id.as_bytes()))
        });

        ranked
    }
}
