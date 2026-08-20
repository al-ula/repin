use crate::model::edge::{Edge, EdgeClaim, FactClaimKey};
use crate::model::identity::NodeId;
use crate::model::node::{Node, NodeClaim};
use crate::model::provenance::{FactOwner, Revision};
use crate::model::registries::{ArtifactClass, EdgeKind, NodeKind};
use crate::model::unresolved::{UnresolvedKey, UnresolvedRef};
use crate::ports::fs::{Diagnostic, Skip};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionRecords {
    #[serde(alias = "schema_version")]
    pub store_schema_version: u32,
    pub kind_registry_version: u32,
    pub attribute_registry_version: u32,
    #[serde(default = "default_component_version")]
    pub classification_version: u32,
    #[serde(default = "default_component_version")]
    pub resolution_version: u32,
    #[serde(alias = "producer_versions")]
    pub pack_versions: BTreeMap<String, String>,
    #[serde(default)]
    pub extractor_versions: BTreeMap<String, String>,
    pub engine_version: String,
    #[serde(default)]
    pub vcs_revision: Option<String>,
    #[serde(default)]
    pub observed_dirty_set: Option<Vec<String>>,
}

fn default_component_version() -> u32 {
    1
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreCapabilities {
    pub transactional_ddl: bool,
    pub concurrent_readers: bool,
    pub vectors_native: bool,
    pub lexical_native: bool,
    pub max_batch_size: Option<usize>,
    pub supports_savepoints: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexKind {
    Lexical,
    Vector,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DerivedIndexIntent {
    pub kind: IndexKind,
    pub revision: Revision,
    pub affected_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DerivedIndexState {
    pub kind: IndexKind,
    pub acknowledged_revision: Revision,
    pub is_current: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeClassificationUpdate {
    pub node_id: NodeId,
    pub owner: FactOwner,
    pub artifact_class: Option<ArtifactClass>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateSummary {
    pub revision: Revision,
    pub files_added: usize,
    pub files_modified: usize,
    pub files_deleted: usize,
    pub nodes_added: usize,
    pub nodes_removed: usize,
    pub edges_added: usize,
    pub edges_removed: usize,
    pub unresolved_promoted: usize,
    pub unresolved_demoted: usize,
}

#[derive(Debug, Clone, Default)]
pub struct NodeFilters {
    pub kinds: Option<Vec<NodeKind>>,
    pub roots: Option<Vec<String>>,
    pub paths: Option<Vec<String>>,
    pub artifact_classes: Option<Vec<ArtifactClass>>,
}

#[derive(Debug, Clone, Default)]
pub struct EdgeFilters {
    pub kinds: Option<Vec<EdgeKind>>,
    pub target_kinds: Option<Vec<NodeKind>>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StoreError {
    #[error(
        "revision conflict: base revision {expected} did not match actual store revision {actual}"
    )]
    RevisionConflict {
        expected: Revision,
        actual: Revision,
    },
    #[error("transaction already active or closed")]
    TransactionClosed,
    #[error("database I/O error: {0}")]
    Io(String),
    #[error("lock failure: {0}")]
    LockFailure(String),
    #[error("corrupt database: {0}")]
    Corrupt(String),
    #[error("schema version {found} is incompatible with supported version {supported}")]
    SchemaVersionMismatch { found: u32, supported: u32 },
}

pub trait Transaction: Send {
    fn expect_revision(&mut self, base: Revision) -> Result<(), StoreError>;
    fn put_nodes(&mut self, claims: &[NodeClaim]) -> Result<(), StoreError>;
    fn put_edges(&mut self, claims: &[EdgeClaim]) -> Result<(), StoreError>;
    fn remove_node_claims(&mut self, keys: &[FactClaimKey]) -> Result<(), StoreError>;
    fn remove_edge_claims(&mut self, keys: &[FactClaimKey]) -> Result<(), StoreError>;
    fn remove_claims(&mut self, owner: &FactOwner) -> Result<(), StoreError>;
    /// Remove all authoritative graph claims inside the current transaction.
    fn clear_graph(&mut self) -> Result<(), StoreError>;
    fn remove_by_file(&mut self, root: &str, path: &str) -> Result<(), StoreError>;
    fn put_unresolved(&mut self, refs: &[UnresolvedRef]) -> Result<(), StoreError>;
    fn remove_unresolved(&mut self, keys: &[UnresolvedKey]) -> Result<(), StoreError>;
    fn put_skips(&mut self, skips: &[Skip]) -> Result<(), StoreError>;
    fn put_diagnostics(&mut self, diagnostics: &[Diagnostic]) -> Result<(), StoreError>;
    fn put_update_summary(&mut self, summary: &UpdateSummary) -> Result<(), StoreError>;
    fn put_version_records(&mut self, records: &VersionRecords) -> Result<(), StoreError>;
    fn update_node_classifications(
        &mut self,
        updates: &[NodeClassificationUpdate],
    ) -> Result<(), StoreError>;
    fn put_index_intent(&mut self, intent: &DerivedIndexIntent) -> Result<(), StoreError>;
    fn acknowledge_index(&mut self, kind: IndexKind, revision: Revision) -> Result<(), StoreError>;
    fn set_revision(&mut self, revision: Revision) -> Result<(), StoreError>;
    fn commit(self: Box<Self>) -> Result<(), StoreError>;
    fn rollback(self: Box<Self>) -> Result<(), StoreError>;
}

pub trait ReadView: Send + Sync {
    fn node(&self, id: &NodeId) -> Result<Option<Node>, StoreError>;
    fn nodes_by_name(&self, name: &str, filters: &NodeFilters) -> Result<Vec<Node>, StoreError>;
    fn nodes_by_file(&self, root: &str, path: &str) -> Result<Vec<Node>, StoreError>;
    fn node_claims_by_file(&self, root: &str, path: &str) -> Result<Vec<NodeClaim>, StoreError>;
    /// Enumerate every persisted producer owner before scoped invalidation.
    fn owners_by_producer(
        &self,
        producer: &str,
        producer_version: Option<&str>,
    ) -> Result<Vec<FactOwner>, StoreError>;
    /// Enumerate every owner of persisted resolution-derived edge claims.
    /// Resolution invalidation must not assume one resolver name or version.
    fn resolution_owners(&self) -> Result<Vec<FactOwner>, StoreError>;
    /// Enumerate distinct persisted root/path pairs without reading source.
    fn files(&self) -> Result<Vec<(String, String)>, StoreError>;
    /// Enumerate every persisted fact owner for full registry invalidation.
    fn all_owners(&self) -> Result<Vec<FactOwner>, StoreError>;
    fn edges_from(&self, id: &NodeId, filters: &EdgeFilters) -> Result<Vec<Edge>, StoreError>;
    fn edges_to(&self, id: &NodeId, filters: &EdgeFilters) -> Result<Vec<Edge>, StoreError>;
    fn incoming_edge_count(&self, id: &NodeId) -> Result<usize, StoreError> {
        self.edges_to(id, &Default::default()).map(|e| e.len())
    }
    fn unresolved_seeking(&self, name: &str) -> Result<Vec<UnresolvedRef>, StoreError>;
    fn unresolved_refs(&self) -> Result<Vec<UnresolvedRef>, StoreError>;
    fn skips(&self, root: Option<&str>, path: Option<&str>) -> Result<Vec<Skip>, StoreError>;
    fn diagnostics(
        &self,
        root: Option<&str>,
        path: Option<&str>,
    ) -> Result<Vec<Diagnostic>, StoreError>;
    fn changes_since(&self, revision: Revision) -> Result<Vec<UpdateSummary>, StoreError>;
    fn version_records(&self) -> Result<Option<VersionRecords>, StoreError>;
    fn index_states(&self) -> Result<Vec<DerivedIndexState>, StoreError>;
    fn revision(&self) -> Result<Revision, StoreError>;
    fn node_count(&self) -> Result<usize, StoreError>;
    fn edge_count(&self) -> Result<usize, StoreError>;
}

pub trait Store: Send + Sync {
    fn begin_write(&self) -> Result<Box<dyn Transaction>, StoreError>;
    fn read_view(&self) -> Result<Box<dyn ReadView>, StoreError>;
    fn capabilities(&self) -> StoreCapabilities;
    fn checkpoint(&self) -> Result<(), StoreError> {
        Ok(())
    }
}
