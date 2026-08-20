//! Compatibility re-exports for the extracted indexing capability.

pub use repin_runtime::invalidation::{BlastRadius, IndexingCoordinator};

/// Historical name retained for existing Rust callers.
pub type InvalidationCoordinator = IndexingCoordinator;
