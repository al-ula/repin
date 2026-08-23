use super::errors::ErrorCode;
use super::evidence::Evidence;
use super::freshness::{Freshness, Truncation};
use super::provider::ProviderId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Ok,
    Partial,
    NotFound,
    Unavailable,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Warning {
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    WorkingTree,
    Graph,
    Semantic,
    Enrichment,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ResultProvenance {
    pub sources: Vec<SourceKind>,
    pub providers: Vec<ProviderId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultEnvelope<T> {
    pub status: Status,
    pub data: T,
    pub warnings: Vec<Warning>,
    pub provenance: ResultProvenance,
    pub freshness: Freshness,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncation: Option<Truncation>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub evidence: Vec<Evidence>,
}

impl<T> ResultEnvelope<T> {
    pub fn ok(data: T) -> Self {
        Self {
            status: Status::Ok,
            data,
            warnings: Vec::new(),
            provenance: ResultProvenance::default(),
            freshness: Freshness::default(),
            truncation: None,
            evidence: Vec::new(),
        }
    }

    pub fn partial(data: T, warnings: Vec<Warning>) -> Self {
        Self {
            status: Status::Partial,
            data,
            warnings,
            provenance: ResultProvenance::default(),
            freshness: Freshness::default(),
            truncation: None,
            evidence: Vec::new(),
        }
    }

    pub fn not_found(data: T) -> Self {
        Self {
            status: Status::NotFound,
            data,
            warnings: Vec::new(),
            provenance: ResultProvenance::default(),
            freshness: Freshness::default(),
            truncation: None,
            evidence: Vec::new(),
        }
    }
}
