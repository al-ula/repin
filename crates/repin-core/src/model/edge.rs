use crate::model::identity::{EdgeId, NodeId};
use crate::model::node::Attributes;
use crate::model::provenance::{FactOwner, Provenance};
use crate::model::registries::EdgeKind;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub id: EdgeId,
    pub from: NodeId,
    pub to: NodeId,
    pub kind: EdgeKind,
    pub provenance: Provenance,
    pub attributes: Attributes,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EdgeClaim {
    pub edge: Edge,
    pub owner: FactOwner,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FactClaimKey {
    pub fact_id: [u8; 32],
    pub owner: FactOwner,
}
