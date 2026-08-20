use repin_core::ports::model::{
    EmbeddingModel, GenerateRequest, GenerateResponse, ModelError, ModelIdentity, ModelLocation,
    RerankCandidate, RerankHit, Reranker, TextModel,
};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

/// Resolve credentials from a named environment variable without persisting
/// the value in configuration or model identity.
pub fn resolve_api_key(
    api_key_env: Option<&str>,
    provider: &str,
) -> Result<Option<String>, ModelError> {
    let Some(environment_name) = api_key_env else {
        return Ok(None);
    };
    if environment_name.is_empty() {
        return Ok(None);
    }
    std::env::var(environment_name)
        .map(Some)
        .map_err(|_| ModelError::AuthError {
            provider: provider.to_string(),
            env_var: environment_name.to_string(),
        })
}

#[derive(Debug, Clone)]
pub struct OpenAiProvider {
    pub endpoint: String,
    pub model: String,
    pub api_key_env: Option<String>,
    pub dimension: Option<usize>,
    pub timeout: Duration,
}

impl OpenAiProvider {
    pub fn new(
        endpoint: impl Into<String>,
        model: impl Into<String>,
        api_key_env: Option<String>,
        dimension: Option<usize>,
    ) -> Self {
        let endpoint = endpoint.into();
        Self {
            endpoint: if endpoint.is_empty() {
                "https://api.openai.com/v1".to_string()
            } else {
                endpoint
            },
            model: model.into(),
            api_key_env,
            dimension,
            timeout: Duration::from_secs(15),
        }
    }
}

#[derive(Deserialize)]
struct OpenAiEmbeddingData {
    embedding: Vec<f32>,
}

#[derive(Deserialize)]
struct OpenAiEmbeddingResponse {
    data: Vec<OpenAiEmbeddingData>,
}

impl EmbeddingModel for OpenAiProvider {
    fn identity(&self) -> ModelIdentity {
        ModelIdentity {
            provider: "openai".to_string(),
            model: self.model.clone(),
            version: None,
            location: ModelLocation::Remote,
        }
    }

    fn dimensions(&self) -> usize {
        self.dimension.unwrap_or(1536)
    }

    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, ModelError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let api_key = resolve_api_key(self.api_key_env.as_deref(), "openai")?;
        let url = format!("{}/embeddings", self.endpoint.trim_end_matches('/'));
        let mut body = json!({ "input": texts, "model": self.model });
        if let Some(dimension) = self.dimension {
            body["dimensions"] = json!(dimension);
        }
        let mut request = ureq::post(&url).timeout(self.timeout);
        if let Some(key) = api_key.as_deref() {
            request = request.set("Authorization", &format!("Bearer {key}"));
        }
        let response = request
            .send_json(body)
            .map_err(|error| provider_error("openai", format!("HTTP error: {error}")))?;
        let parsed: OpenAiEmbeddingResponse = response.into_json().map_err(|error| {
            provider_error("openai", format!("JSON response decode error: {error}"))
        })?;
        Ok(parsed.data.into_iter().map(|data| data.embedding).collect())
    }
}

#[derive(Deserialize)]
struct OpenAiRerankHit {
    index: usize,
    relevance_score: f32,
}

#[derive(Deserialize)]
struct OpenAiRerankResponse {
    results: Vec<OpenAiRerankHit>,
}

impl Reranker for OpenAiProvider {
    fn identity(&self) -> ModelIdentity {
        <Self as EmbeddingModel>::identity(self)
    }

    fn rerank(
        &self,
        query: &str,
        candidates: &[RerankCandidate],
    ) -> Result<Vec<RerankHit>, ModelError> {
        if candidates.is_empty() {
            return Ok(Vec::new());
        }
        let api_key = resolve_api_key(self.api_key_env.as_deref(), "openai")?;
        let url = format!("{}/rerank", self.endpoint.trim_end_matches('/'));
        let documents: Vec<&str> = candidates
            .iter()
            .map(|candidate| candidate.content.as_str())
            .collect();
        let body = json!({
            "model": self.model,
            "query": query,
            "documents": documents,
            "top_n": candidates.len(),
        });
        let mut request = ureq::post(&url).timeout(self.timeout);
        if let Some(key) = api_key.as_deref() {
            request = request.set("Authorization", &format!("Bearer {key}"));
        }
        let response = request
            .send_json(body)
            .map_err(|error| provider_error("openai", format!("HTTP error: {error}")))?;
        let parsed: OpenAiRerankResponse = response.into_json().map_err(|error| {
            provider_error("openai", format!("JSON response decode error: {error}"))
        })?;
        Ok(parsed
            .results
            .into_iter()
            .enumerate()
            .filter_map(|(rank, result)| {
                candidates.get(result.index).map(|candidate| RerankHit {
                    id: candidate.id.clone(),
                    score: result.relevance_score,
                    rank,
                })
            })
            .collect())
    }
}

#[derive(Debug, Clone)]
pub struct OllamaProvider {
    pub endpoint: String,
    pub model: String,
    pub timeout: Duration,
}

impl OllamaProvider {
    pub fn new(endpoint: impl Into<String>, model: impl Into<String>) -> Self {
        let endpoint = endpoint.into();
        Self {
            endpoint: if endpoint.is_empty() {
                "http://localhost:11434".to_string()
            } else {
                endpoint
            },
            model: model.into(),
            timeout: Duration::from_secs(30),
        }
    }
}

#[derive(Deserialize)]
struct OllamaEmbeddingResponse {
    embedding: Vec<f32>,
}

impl EmbeddingModel for OllamaProvider {
    fn identity(&self) -> ModelIdentity {
        ModelIdentity {
            provider: "ollama".to_string(),
            model: self.model.clone(),
            version: None,
            location: ModelLocation::Local,
        }
    }

    fn dimensions(&self) -> usize {
        768
    }

    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, ModelError> {
        let url = format!("{}/api/embeddings", self.endpoint.trim_end_matches('/'));
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            let response = ureq::post(&url)
                .timeout(self.timeout)
                .send_json(json!({ "model": self.model, "prompt": text }))
                .map_err(|error| provider_error("ollama", format!("Ollama error: {error}")))?;
            let parsed: OllamaEmbeddingResponse = response.into_json().map_err(|error| {
                provider_error("ollama", format!("Ollama JSON decode error: {error}"))
            })?;
            results.push(parsed.embedding);
        }
        Ok(results)
    }
}

#[derive(Deserialize)]
struct OllamaGenerateResponse {
    response: String,
}

impl TextModel for OllamaProvider {
    fn identity(&self) -> ModelIdentity {
        <Self as EmbeddingModel>::identity(self)
    }

    fn generate(&self, request: &GenerateRequest) -> Result<GenerateResponse, ModelError> {
        let url = format!("{}/api/generate", self.endpoint.trim_end_matches('/'));
        let response = ureq::post(&url)
            .timeout(self.timeout)
            .send_json(json!({
                "model": self.model,
                "prompt": request.prompt,
                "system": request.system_prompt,
                "stream": false,
            }))
            .map_err(|error| provider_error("ollama", format!("Ollama generate error: {error}")))?;
        let parsed: OllamaGenerateResponse = response.into_json().map_err(|error| {
            provider_error("ollama", format!("Ollama JSON decode error: {error}"))
        })?;
        Ok(GenerateResponse {
            text: parsed.response,
            model: <Self as TextModel>::identity(self),
            token_usage: None,
        })
    }
}

#[derive(Debug, Clone)]
pub struct GoogleGeminiProvider {
    pub endpoint: String,
    pub model: String,
    pub api_key_env: Option<String>,
    pub timeout: Duration,
}

impl GoogleGeminiProvider {
    pub fn new(
        endpoint: impl Into<String>,
        model: impl Into<String>,
        api_key_env: Option<String>,
    ) -> Self {
        let endpoint = endpoint.into();
        Self {
            endpoint: if endpoint.is_empty() {
                "https://generativelanguage.googleapis.com".to_string()
            } else {
                endpoint
            },
            model: model.into(),
            api_key_env: api_key_env.or_else(|| Some("GEMINI_API_KEY".to_string())),
            timeout: Duration::from_secs(15),
        }
    }
}

#[derive(Deserialize)]
struct GeminiEmbeddingValues {
    values: Vec<f32>,
}

#[derive(Deserialize)]
struct GeminiEmbeddingResponse {
    embedding: GeminiEmbeddingValues,
}

impl EmbeddingModel for GoogleGeminiProvider {
    fn identity(&self) -> ModelIdentity {
        ModelIdentity {
            provider: "google".to_string(),
            model: self.model.clone(),
            version: None,
            location: ModelLocation::Remote,
        }
    }

    fn dimensions(&self) -> usize {
        768
    }

    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, ModelError> {
        let api_key = resolve_api_key(self.api_key_env.as_deref(), "google")?.ok_or_else(|| {
            ModelError::AuthError {
                provider: "google".to_string(),
                env_var: "GEMINI_API_KEY".to_string(),
            }
        })?;
        let url = format!(
            "{}/v1beta/models/{}:embedContent?key={}",
            self.endpoint.trim_end_matches('/'),
            self.model,
            api_key
        );
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            let response = ureq::post(&url)
                .timeout(self.timeout)
                .send_json(json!({ "content": { "parts": [{ "text": text }] } }))
                .map_err(|error| provider_error("google", format!("Gemini API error: {error}")))?;
            let parsed: GeminiEmbeddingResponse = response.into_json().map_err(|error| {
                provider_error("google", format!("Gemini JSON decode error: {error}"))
            })?;
            results.push(parsed.embedding.values);
        }
        Ok(results)
    }
}

#[derive(Deserialize)]
struct GeminiCandidatePart {
    text: Option<String>,
}

#[derive(Deserialize)]
struct GeminiCandidateContent {
    parts: Vec<GeminiCandidatePart>,
}

#[derive(Deserialize)]
struct GeminiCandidate {
    content: Option<GeminiCandidateContent>,
}

#[derive(Deserialize)]
struct GeminiGenerateResponse {
    candidates: Option<Vec<GeminiCandidate>>,
}

impl TextModel for GoogleGeminiProvider {
    fn identity(&self) -> ModelIdentity {
        <Self as EmbeddingModel>::identity(self)
    }

    fn generate(&self, request: &GenerateRequest) -> Result<GenerateResponse, ModelError> {
        let api_key = resolve_api_key(self.api_key_env.as_deref(), "google")?.ok_or_else(|| {
            ModelError::AuthError {
                provider: "google".to_string(),
                env_var: "GEMINI_API_KEY".to_string(),
            }
        })?;
        let url = format!(
            "{}/v1beta/models/{}:generateContent?key={}",
            self.endpoint.trim_end_matches('/'),
            self.model,
            api_key
        );
        let mut parts = Vec::new();
        if let Some(system_prompt) = &request.system_prompt {
            parts.push(json!({ "text": format!("System instructions: {system_prompt}\n") }));
        }
        parts.push(json!({ "text": request.prompt }));
        let response = ureq::post(&url)
            .timeout(self.timeout)
            .send_json(json!({ "contents": [{ "parts": parts }] }))
            .map_err(|error| provider_error("google", format!("Gemini generate error: {error}")))?;
        let parsed: GeminiGenerateResponse = response.into_json().map_err(|error| {
            provider_error("google", format!("Gemini JSON decode error: {error}"))
        })?;
        let text = parsed
            .candidates
            .and_then(|candidates| candidates.into_iter().next())
            .and_then(|candidate| candidate.content)
            .and_then(|content| content.parts.into_iter().next())
            .and_then(|part| part.text)
            .unwrap_or_default();
        Ok(GenerateResponse {
            text,
            model: <Self as TextModel>::identity(self),
            token_usage: None,
        })
    }
}

fn provider_error(provider: &str, message: String) -> ModelError {
    ModelError::ProviderError {
        provider: provider.to_string(),
        message,
    }
}
