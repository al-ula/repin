use crate::model::edge::EdgeClaim;
use crate::model::node::NodeClaim;
use crate::model::unresolved::UnresolvedRef;
use crate::ports::fs::{Diagnostic, FileSnapshot, Skip};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExtractedFacts {
    pub nodes: Vec<NodeClaim>,
    pub edges: Vec<EdgeClaim>,
    pub unresolved: Vec<UnresolvedRef>,
    pub skips: Vec<Skip>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExtractionError {
    #[error("unsupported file language or syntax: {0}")]
    Unsupported(String),
    #[error("parser failure: {0}")]
    ParseFailure(String),
    #[error("timeout or limit exceeded: {0}")]
    BudgetExceeded(String),
}

pub trait LanguagePack: Send + Sync {
    fn name(&self) -> &'static str;
    fn version(&self) -> &'static str;
    fn can_handle(&self, path: &str, sample_content: &[u8]) -> bool;
    fn extract(&self, snapshot: &FileSnapshot) -> Result<ExtractedFacts, ExtractionError>;
}
