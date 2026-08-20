use repin_core::config::RepinConfig;
use repin_core::ports::model::{EmbeddingModel, ModelError, Reranker, TextModel};

use crate::intelligence::agent::AgentRunnerReranker;
use crate::intelligence::embedded::{EmbeddedOnnxModel, EmbeddedOnnxReranker};
use crate::intelligence::remote_api::{GoogleGeminiProvider, OllamaProvider, OpenAiProvider};

pub struct IntelligenceRegistry;

impl IntelligenceRegistry {
    pub fn build_embedding_model(
        config: &RepinConfig,
    ) -> Result<Option<Box<dyn EmbeddingModel>>, ModelError> {
        let emb = &config.intelligence.embedding;
        if !emb.is_enabled() {
            return Ok(None);
        }

        match emb.provider.as_str() {
            "embedded" => Ok(Some(Box::new(EmbeddedOnnxModel::new(
                &emb.model,
                emb.dimension,
                emb.auto_download,
            )))),
            "openai" => Ok(Some(Box::new(OpenAiProvider::new(
                emb.endpoint.as_deref().unwrap_or("https://api.openai.com/v1"),
                &emb.model,
                emb.api_key_env.clone(),
                emb.dimension,
            )))),
            "ollama" => Ok(Some(Box::new(OllamaProvider::new(
                emb.endpoint.as_deref().unwrap_or("http://localhost:11434"),
                &emb.model,
            )))),
            "google" => Ok(Some(Box::new(GoogleGeminiProvider::new(
                emb.endpoint.as_deref().unwrap_or("https://generativelanguage.googleapis.com"),
                &emb.model,
                emb.api_key_env.clone(),
            )))),
            other => Err(ModelError::Unsupported(format!(
                "unsupported embedding provider '{}'",
                other
            ))),
        }
    }

    pub fn build_reranker(
        config: &RepinConfig,
    ) -> Result<Option<Box<dyn Reranker>>, ModelError> {
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
                rerank.endpoint.as_deref().unwrap_or("https://api.openai.com/v1"),
                &rerank.model,
                rerank.api_key_env.clone(),
                None,
            )))),
            other => Err(ModelError::Unsupported(format!(
                "unsupported rerank provider '{}'",
                other
            ))),
        }
    }

    pub fn build_text_model(
        config: &RepinConfig,
    ) -> Result<Option<Box<dyn TextModel>>, ModelError> {
        let enrich = &config.intelligence.enrichment;
        if !enrich.is_enabled() {
            return Ok(None);
        }

        match enrich.provider.as_str() {
            "google" => Ok(Some(Box::new(GoogleGeminiProvider::new(
                enrich.endpoint.as_deref().unwrap_or("https://generativelanguage.googleapis.com"),
                &enrich.model,
                enrich.api_key_env.clone(),
            )))),
            "ollama" => Ok(Some(Box::new(OllamaProvider::new(
                enrich.endpoint.as_deref().unwrap_or("http://localhost:11434"),
                &enrich.model,
            )))),
            other => Err(ModelError::Unsupported(format!(
                "unsupported text/enrichment provider '{}'",
                other
            ))),
        }
    }
}
