use crate::line_index::Range;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Revision(pub u64);

impl Revision {
    pub const INITIAL: Revision = Revision(0);

    pub fn next(&self) -> Self {
        Revision(self.0 + 1)
    }
}

impl fmt::Display for Revision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "r{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Derivation {
    Extracted,
    Resolved,
    Heuristic,
    Inferred,
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Confidence(pub f32);

impl Confidence {
    pub const EXACT: Confidence = Confidence(1.0);
    pub const DEFAULT_HEURISTIC: Confidence = Confidence(0.7);

    pub fn new(val: f32) -> Self {
        Self(val.clamp(0.0, 1.0))
    }
}

impl Eq for Confidence {}

impl std::hash::Hash for Confidence {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Provenance {
    pub root: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<Range>,
    pub extractor: String,
    pub extractor_version: String,
    pub derivation: Derivation,
    pub confidence: Confidence,
    pub revision: Revision,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FactOwner {
    pub root: String,
    pub path: String,
    pub producer: String,
    pub producer_version: String,
}

impl FactOwner {
    pub fn new(
        root: impl Into<String>,
        path: impl Into<String>,
        producer: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        Self {
            root: root.into(),
            path: path.into(),
            producer: producer.into(),
            producer_version: version.into(),
        }
    }
}
