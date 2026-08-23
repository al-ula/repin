pub mod config;
pub mod conformance;
pub mod context;
pub mod direct_search;
pub mod fs;
pub mod hash;
pub mod indexing;
pub mod intelligence;
pub mod line_index;
pub mod model;
pub mod packs;
pub mod ports;
pub mod protocol;
pub mod retrieval;
pub mod runtime;
pub mod store;
pub mod versions;

pub use config::{
    ConfigError, ContextConfig, DaemonConfig, ExtractionConfig, IndexingConfig, IntelligenceConfig,
    Merge, PartialRepinConfig, ProjectConfig, RepinConfig, RetrievalConfig, StorageConfig,
};

pub use hash::{ContentHash, HashAlgorithm};
pub use line_index::{ByteSpan, LineIndex, LineIndexError, Position, Range};
pub use model::{
    ArtifactClass, Attributes, Confidence, Derivation, Edge, EdgeClaim, EdgeId, FactClaimKey,
    FactOwner, Node, NodeClaim, NodeId, Provenance, Revision, UnresolvedKey, UnresolvedRef,
};
pub use ports::{
    ChangeOrigin, DerivedIndexIntent, DerivedIndexState, Diagnostic, EdgeFilters, ExtractedFacts,
    ExtractionError, FileChange, FileSnapshot, IndexKind, LanguagePack, NodeClassificationUpdate,
    NodeFilters, ReadView, Skip, SourceError, SourceFs, Store, StoreCapabilities, StoreError,
    Transaction, UpdateSummary, Vcs, VcsChangeSet, VcsError, VersionRecords,
};

// Re-exports for convenience
pub use context::{AssembledContext, ContextBuilder, ContextSnippet};
pub use runtime::{Engine, EngineOptions, Runtime, RuntimeOptions};
