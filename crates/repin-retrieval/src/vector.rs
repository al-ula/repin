use repin_core::model::identity::NodeId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorHit {
    pub node_id: NodeId,
    pub score: f32,
}

#[derive(Debug, Clone)]
pub struct ExactVectorIndex {
    dimension: usize,
    vectors: Vec<(NodeId, Vec<f32>)>,
}

impl ExactVectorIndex {
    pub fn new(dimension: usize) -> Self {
        Self {
            dimension,
            vectors: Vec::new(),
        }
    }

    pub fn insert(&mut self, node_id: NodeId, vector: Vec<f32>) {
        if vector.len() == self.dimension {
            self.vectors.retain(|(id, _)| *id != node_id);
            self.vectors.push((node_id, vector));
        }
    }

    pub fn search(&self, query_vec: &[f32], top_k: usize) -> Vec<VectorHit> {
        if query_vec.len() != self.dimension || self.vectors.is_empty() {
            return Vec::new();
        }

        let mut hits: Vec<_> = self
            .vectors
            .iter()
            .map(|(id, vector)| VectorHit {
                node_id: *id,
                score: cosine_similarity(query_vec, vector),
            })
            .collect();

        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.node_id.cmp(&b.node_id))
        });
        hits.truncate(top_k);
        hits
    }

    /// Deterministic feature-hash embedding for an offline vector baseline.
    pub fn deterministic_embed(text: &str, dim: usize) -> Vec<f32> {
        if dim == 0 {
            return Vec::new();
        }
        let mut vector = vec![0.0_f32; dim];
        for word in text.split_whitespace() {
            let hash = blake3::hash(word.to_lowercase().as_bytes());
            let bytes = hash.as_bytes();
            let bucket = ((usize::from(bytes[0]) << 8) | usize::from(bytes[1])) % dim;
            let sign = if bytes[2].is_multiple_of(2) {
                1.0
            } else {
                -1.0
            };
            vector[bucket] += sign;
        }
        let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
        if norm > 0.0 {
            for value in &mut vector {
                *value /= norm;
            }
        }
        vector
    }
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let (mut dot, mut norm_a, mut norm_b) = (0.0_f32, 0.0_f32, 0.0_f32);
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    let denominator = norm_a.sqrt() * norm_b.sqrt();
    if denominator > 0.0 {
        dot / denominator
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_vector_similarity_is_stable() {
        let mut index = ExactVectorIndex::new(64);
        let id1 = NodeId::from_bytes([1; 32]);
        let id2 = NodeId::from_bytes([2; 32]);
        index.insert(
            id1,
            ExactVectorIndex::deterministic_embed("compute mathematical sum", 64),
        );
        index.insert(
            id2,
            ExactVectorIndex::deterministic_embed("database storage transaction", 64),
        );
        let query = ExactVectorIndex::deterministic_embed("compute sum", 64);
        let hits = index.search(&query, 2);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].node_id, id1);
        assert!(hits[0].score > hits[1].score);
    }

    #[test]
    fn zero_dimension_is_safe() {
        assert!(ExactVectorIndex::deterministic_embed("query", 0).is_empty());
        let index = ExactVectorIndex::new(0);
        assert!(index.search(&[], 1).is_empty());
    }
}
