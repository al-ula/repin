use repin_core::config::RepinConfig;
use repin_core::ports::model::{EmbeddingModel, ModelError, Reranker, TextModel};

use repin_intelligence::{
    AgentRunnerReranker, EmbeddedOnnxModel, EmbeddedOnnxReranker, GoogleGeminiProvider,
    OllamaProvider, OpenAiProvider,
};

/// Default composition-layer provider registry.
pub struct IntelligenceRegistry;

impl IntelligenceRegistry {
    pub fn build_embedding_model(
        config: &RepinConfig,
    ) -> Result<Option<Box<dyn EmbeddingModel>>, ModelError> {
        let embedding = &config.intelligence.embedding;
        if !embedding.is_enabled() {
            return Ok(None);
        }
        match embedding.provider.as_str() {
            "embedded" => Ok(Some(Box::new(EmbeddedOnnxModel::new(
                &embedding.model,
                embedding.dimension,
                embedding.auto_download,
            )))),
            "openai" => Ok(Some(Box::new(OpenAiProvider::new(
                embedding
                    .endpoint
                    .as_deref()
                    .unwrap_or("https://api.openai.com/v1"),
                &embedding.model,
                embedding.api_key_env.clone(),
                embedding.dimension,
            )))),
            "ollama" => Ok(Some(Box::new(OllamaProvider::new(
                embedding
                    .endpoint
                    .as_deref()
                    .unwrap_or("http://localhost:11434"),
                &embedding.model,
            )))),
            "google" => Ok(Some(Box::new(GoogleGeminiProvider::new(
                embedding
                    .endpoint
                    .as_deref()
                    .unwrap_or("https://generativelanguage.googleapis.com"),
                &embedding.model,
                embedding.api_key_env.clone(),
            )))),
            provider => Err(ModelError::Unsupported(format!(
                "unsupported embedding provider '{provider}'"
            ))),
        }
    }

    pub fn build_reranker(config: &RepinConfig) -> Result<Option<Box<dyn Reranker>>, ModelError> {
        let rerank = &config.intelligence.rerank;
        if !rerank.is_enabled() {
            return Ok(None);
        }
        match rerank.provider.as_str() {
            "embedded" => Ok(Some(Box::new(EmbeddedOnnxReranker::new(
                &rerank.model,
                true,
                rerank.deadline_ms,
            )))),
            "agent" => {
                if rerank.agent_cmd.is_empty() {
                    return Err(ModelError::Unsupported(
                        "agent_cmd cannot be empty when rerank provider is 'agent'".to_string(),
                    ));
                }
                Ok(Some(Box::new(AgentRunnerReranker::new(
                    &rerank.agent_cmd,
                    rerank.deadline_ms,
                ))))
            }
            "openai" => Ok(Some(Box::new(OpenAiProvider::new(
                rerank
                    .endpoint
                    .as_deref()
                    .unwrap_or("https://api.openai.com/v1"),
                &rerank.model,
                rerank.api_key_env.clone(),
                None,
            )))),
            provider => Err(ModelError::Unsupported(format!(
                "unsupported rerank provider '{provider}'"
            ))),
        }
    }

    pub fn build_text_model(
        config: &RepinConfig,
    ) -> Result<Option<Box<dyn TextModel>>, ModelError> {
        let enrichment = &config.intelligence.enrichment;
        if !enrichment.is_enabled() {
            return Ok(None);
        }
        match enrichment.provider.as_str() {
            "google" => Ok(Some(Box::new(GoogleGeminiProvider::new(
                enrichment
                    .endpoint
                    .as_deref()
                    .unwrap_or("https://generativelanguage.googleapis.com"),
                &enrichment.model,
                enrichment.api_key_env.clone(),
            )))),
            "ollama" => Ok(Some(Box::new(OllamaProvider::new(
                enrichment
                    .endpoint
                    .as_deref()
                    .unwrap_or("http://localhost:11434"),
                &enrichment.model,
            )))),
            provider => Err(ModelError::Unsupported(format!(
                "unsupported text/enrichment provider '{provider}'"
            ))),
        }
    }
}
