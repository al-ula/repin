use crate::line_index::Range;
use crate::model::identity::NodeId;
use crate::model::provenance::{FactOwner, Provenance};
use crate::model::registries::{ArtifactClass, NodeKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type Attributes = BTreeMap<String, serde_json::Value>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qualified_name: Option<String>,
    pub root: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<Range>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_class: Option<ArtifactClass>,
    pub provenance: Provenance,
    pub attributes: Attributes,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeClaim {
    pub node: Node,
    pub owner: FactOwner,
}
