use repin_core::model::provenance::Revision;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphState {
    Current,
    Stale,
    Building,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LexicalState {
    Current,
    BypassedLagging,
    Disabled,
    Failing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageState {
    Complete,
    Partial,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Freshness {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graph_revision: Option<Revision>,
    pub graph_state: GraphState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lexical_revision: Option<Revision>,
    pub lexical_state: LexicalState,
    pub coverage: CoverageState,
}

impl Default for Freshness {
    fn default() -> Self {
        Self {
            observed_at: None,
            graph_revision: None,
            graph_state: GraphState::Unknown,
            lexical_revision: None,
            lexical_state: LexicalState::Disabled,
            coverage: CoverageState::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TruncationReason {
    Limit,
    Bytes,
    Lines,
    Depth,
    Breadth,
    Provider,
    Timeout,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Truncation {
    pub truncated: bool,
    pub returned: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub available: Option<usize>,
    pub reason: TruncationReason,
}
