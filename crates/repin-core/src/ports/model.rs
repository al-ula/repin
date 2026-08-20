use std::fmt::Debug;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ModelError {
    #[error("provider error ({provider}): {message}")]
    ProviderError { provider: String, message: String },
    #[error("authentication error for {provider}: missing environment variable {env_var}")]
    AuthError { provider: String, env_var: String },
    #[error("network timeout after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },
    #[error("rate limit exceeded for {provider}: retry after {retry_after_secs:?}s")]
    RateLimited { provider: String, retry_after_secs: Option<u64> },
    #[error("model not found: {model}")]
    ModelNotFound { model: String },
    #[error("unsupported task or operation: {0}")]
    Unsupported(String),
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelLocation {
    Local,
    Remote,
    HostSupplied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelIdentity {
    pub provider: String,
    pub model: String,
    pub version: Option<String>,
    pub location: ModelLocation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RerankCandidate {
    pub id: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RerankHit {
    pub id: String,
    pub score: f32,
    pub rank: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GenerateRequest {
    pub prompt: String,
    pub system_prompt: Option<String>,
    pub max_tokens: Option<usize>,
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GenerateResponse {
    pub text: String,
    pub model: ModelIdentity,
    pub token_usage: Option<usize>,
}

/// Port for generating vector embeddings
pub trait EmbeddingModel: Send + Sync + Debug {
    fn identity(&self) -> ModelIdentity;
    fn dimensions(&self) -> usize;
    fn max_input_tokens(&self) -> usize {
        8192
    }
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, ModelError>;
}

/// Port for cross-encoder reranking
pub trait Reranker: Send + Sync + Debug {
    fn identity(&self) -> ModelIdentity;
    fn rerank(&self, query: &str, candidates: &[RerankCandidate]) -> Result<Vec<RerankHit>, ModelError>;
}

/// Port for text model generation and relation derivation
pub trait TextModel: Send + Sync + Debug {
    fn identity(&self) -> ModelIdentity;
    fn generate(&self, request: &GenerateRequest) -> Result<GenerateResponse, ModelError>;
}
