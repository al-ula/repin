pub mod config;
pub mod extractor_util;
pub mod hash;
pub mod line_index;
pub mod model;
pub mod ports;
pub mod protocol;
pub mod versions;

pub use config::{
    ConfigError, ContextConfig, DaemonConfig, ExtractionConfig, IndexingConfig, IntelligenceConfig,
    Merge, PartialRepinConfig, ProjectConfig, RepinConfig, RetrievalConfig, StorageConfig,
};

pub use extractor_util::{DiscriminatorTracker, FactBuilder};
pub use hash::{ContentHash, HashAlgorithm};
pub use line_index::{ByteSpan, LineIndex, LineIndexError, Position, Range};
pub use model::{
    ArtifactClass, Attributes, Confidence, Derivation, Edge, EdgeClaim, EdgeId, EdgeKind,
    FactClaimKey, FactOwner, Node, NodeClaim, NodeId, NodeKind, Provenance, Revision,
    UnresolvedKey, UnresolvedRef,
};
pub use ports::{
    BranchInfo, ChangeOrigin, DerivedIndexIntent, DerivedIndexState, Diagnostic, EdgeFilters,
    EmbeddingModel, ExtractedFacts, ExtractionError, FileChange, FileSnapshot, GenerateRequest,
    GenerateResponse, IndexKind, LanguagePack, ModelError, ModelIdentity, ModelLocation,
    NodeClassificationUpdate, NodeFilters, ReadView, RerankCandidate, RerankHit, Reranker, Skip,
    SourceError, SourceFs, Store, StoreCapabilities, StoreError, TextModel, Transaction,
    UpdateSummary, Vcs, VcsChangeSet, VcsError, VersionRecords,
};
pub use protocol::{
    BOOTSTRAP_DEADLINE_MS, BOOTSTRAP_VERSION, BootstrapHandshake, BootstrapHandshakeOk,
    BootstrapRejected, CoverageState, ErrorCode, Evidence, Freshness, GraphState, IpcMessage,
    IpcRequest, IpcResponse, IpcResponseEnvelope, LexicalState, MAX_BOOTSTRAP_FRAME_BYTES,
    MAX_FRAME_BYTES, PROTOCOL_MAX, PROTOCOL_MIN, PROTOCOL_STATE_LIFECYCLE, ProviderId,
    ProviderInfo, ProviderKind, ProviderLocation, ResultEnvelope, ResultProvenance, SourceKind,
    Status, Truncation, TruncationReason, Warning, replacement_allowed, select_protocol,
};
