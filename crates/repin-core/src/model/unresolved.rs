use crate::model::identity::NodeId;
use crate::model::provenance::Provenance;
use crate::model::registries::EdgeKind;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnresolvedRef {
    pub from: NodeId,
    pub seeking: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope_hint: Option<String>,
    pub edge_kind: EdgeKind,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct UnresolvedKey {
    pub from: NodeId,
    pub seeking: String,
    pub edge_kind: EdgeKind,
}
