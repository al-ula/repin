//! Runtime-owned provider selection and compatibility exports.

pub mod registry;

pub mod agent {
    pub use repin_intelligence::agent::*;
}

pub mod embedded {
    pub use repin_intelligence::embedded::*;
}

pub mod remote_api {
    pub use repin_intelligence::remote_api::*;
}

pub use repin_intelligence::{
    AgentRunnerReranker, EmbeddedOnnxModel, EmbeddedOnnxReranker, GoogleGeminiProvider,
    OllamaProvider, OpenAiProvider, ensure_hf_model_assets, list_cached_models, normalize_l2,
};
pub use registry::IntelligenceRegistry;
