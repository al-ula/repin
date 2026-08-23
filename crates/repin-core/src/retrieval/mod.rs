//! Reusable deterministic retrieval algorithms.
//!
//! The crate operates on borrowed core contracts. It does not select a store,
//! filesystem, language pack, or model provider.

pub mod hybrid;
pub mod ranking;
pub mod traversal;
pub mod vector;

pub use hybrid::{HybridRetriever, LexicalHit, LexicalSource, RetrievalMetadata, RetrievalResult};
pub use ranking::{DeterministicRanker, RankExplanation, RankReason, RankedCandidate};
pub use traversal::{
    GraphTraversal, ImpactData, ImpactItem, NeighborItem, NeighborsData, PathSegment, PathTrace,
    PathTraceData,
};
pub use vector::{ExactVectorIndex, VectorHit, cosine_similarity};
