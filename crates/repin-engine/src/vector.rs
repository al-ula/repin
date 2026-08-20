use repin_core::model::identity::NodeId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorHit {
    pub node_id: NodeId,
    pub score: f32,
}

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

        let mut hits = Vec::with_capacity(self.vectors.len());
        for (id, vec) in &self.vectors {
            let sim = cosine_similarity(query_vec, vec);
            hits.push(VectorHit {
                node_id: *id,
                score: sim,
            });
        }

        // Sort descending by similarity score
        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        hits.truncate(top_k);
        hits
    }

    /// Pure-Rust deterministic feature-hash embedding for fallback without external neural weights
    pub fn deterministic_embed(text: &str, dim: usize) -> Vec<f32> {
        let mut vec = vec![0.0f32; dim];
        for word in text.split_whitespace() {
            let hash = blake3::hash(word.to_lowercase().as_bytes());
            let bytes = hash.as_bytes();
            let bucket = ((bytes[0] as usize) << 8 | (bytes[1] as usize)) % dim;
            let sign = if bytes[2].is_multiple_of(2) {
                1.0
            } else {
                -1.0
            };
            vec[bucket] += sign;
        }

        // L2 normalize
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for val in &mut vec {
                *val /= norm;
            }
        }
        vec
    }
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;

    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }

    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom > 0.0 { dot / denom } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_vector_similarity() {
        let mut index = ExactVectorIndex::new(64);
        let id1 = NodeId::from_bytes([1; 32]);
        let id2 = NodeId::from_bytes([2; 32]);

        let v1 = ExactVectorIndex::deterministic_embed("compute mathematical sum", 64);
        let v2 = ExactVectorIndex::deterministic_embed("database storage transaction", 64);

        index.insert(id1, v1.clone());
        index.insert(id2, v2);

        let query = ExactVectorIndex::deterministic_embed("compute sum", 64);
        let hits = index.search(&query, 2);

        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].node_id, id1);
        assert!(hits[0].score > hits[1].score);
    }
}
