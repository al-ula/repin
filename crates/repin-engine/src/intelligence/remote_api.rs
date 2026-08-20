use repin_core::ports::model::{
    EmbeddingModel, GenerateRequest, GenerateResponse, ModelError, ModelIdentity, ModelLocation,
    RerankCandidate, RerankHit, Reranker, TextModel,
};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

/// Resolver for API keys via environment variables (zero credential storage)
pub fn resolve_api_key(api_key_env: Option<&str>, provider: &str) -> Result<Option<String>, ModelError> {
    if let Some(env_name) = api_key_env {
        if env_name.is_empty() {
            return Ok(None);
        }
        match std::env::var(env_name) {
            Ok(val) => Ok(Some(val)),
            Err(_) => Err(ModelError::AuthError {
                provider: provider.to_string(),
                env_var: env_name.to_string(),
            }),
        }
    } else {
        Ok(None)
    }
}

// ==============================================================================
// 1. OpenAI-Compatible Provider
// ==============================================================================

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
        let ep = endpoint.into();
        Self {
            endpoint: if ep.is_empty() { "https://api.openai.com/v1".to_string() } else { ep },
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

        let mut body = json!({
            "input": texts,
            "model": self.model,
        });

        if let Some(dim) = self.dimension {
            body["dimensions"] = json!(dim);
        }

        let mut req = ureq::post(&url).timeout(self.timeout);
        if let Some(ref key) = api_key {
            req = req.set("Authorization", &format!("Bearer {key}"));
        }

        let response = req.send_json(body).map_err(|e| ModelError::ProviderError {
            provider: "openai".to_string(),
            message: format!("HTTP error: {e}"),
        })?;

        let parsed: OpenAiEmbeddingResponse = response.into_json().map_err(|e| {
            ModelError::ProviderError {
                provider: "openai".to_string(),
                message: format!("JSON response decode error: {e}"),
            }
        })?;

        Ok(parsed.data.into_iter().map(|d| d.embedding).collect())
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
        ModelIdentity {
            provider: "openai".to_string(),
            model: self.model.clone(),
            version: None,
            location: ModelLocation::Remote,
        }
    }

    fn rerank(&self, query: &str, candidates: &[RerankCandidate]) -> Result<Vec<RerankHit>, ModelError> {
        if candidates.is_empty() {
            return Ok(Vec::new());
        }

        let api_key = resolve_api_key(self.api_key_env.as_deref(), "openai")?;
        let url = format!("{}/rerank", self.endpoint.trim_end_matches('/'));
        let docs: Vec<&str> = candidates.iter().map(|c| c.content.as_str()).collect();

        let body = json!({
            "model": self.model,
            "query": query,
            "documents": docs,
            "top_n": candidates.len(),
        });

        let mut req = ureq::post(&url).timeout(self.timeout);
        if let Some(ref key) = api_key {
            req = req.set("Authorization", &format!("Bearer {key}"));
        }

        let response = req.send_json(body).map_err(|e| ModelError::ProviderError {
            provider: "openai".to_string(),
            message: format!("HTTP error: {e}"),
        })?;

        let parsed: OpenAiRerankResponse = response.into_json().map_err(|e| {
            ModelError::ProviderError {
                provider: "openai".to_string(),
                message: format!("JSON response decode error: {e}"),
            }
        })?;

        let mut hits = Vec::new();
        for (rank, res) in parsed.results.into_iter().enumerate() {
            if res.index < candidates.len() {
                hits.push(RerankHit {
                    id: candidates[res.index].id.clone(),
                    score: res.relevance_score,
                    rank,
                });
            }
        }

        Ok(hits)
    }
}

// ==============================================================================
// 2. Ollama Native Provider
// ==============================================================================

#[derive(Debug, Clone)]
pub struct OllamaProvider {
    pub endpoint: String,
    pub model: String,
    pub timeout: Duration,
}

impl OllamaProvider {
    pub fn new(endpoint: impl Into<String>, model: impl Into<String>) -> Self {
        let ep = endpoint.into();
        Self {
            endpoint: if ep.is_empty() { "http://localhost:11434".to_string() } else { ep },
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
        let mut results = Vec::with_capacity(texts.len());
        let url = format!("{}/api/embeddings", self.endpoint.trim_end_matches('/'));

        for text in texts {
            let body = json!({
                "model": self.model,
                "prompt": text,
            });

            let response = ureq::post(&url)
                .timeout(self.timeout)
                .send_json(body)
                .map_err(|e| ModelError::ProviderError {
                    provider: "ollama".to_string(),
                    message: format!("Ollama error: {e}"),
                })?;

            let parsed: OllamaEmbeddingResponse = response.into_json().map_err(|e| {
                ModelError::ProviderError {
                    provider: "ollama".to_string(),
                    message: format!("Ollama JSON decode error: {e}"),
                }
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
        ModelIdentity {
            provider: "ollama".to_string(),
            model: self.model.clone(),
            version: None,
            location: ModelLocation::Local,
        }
    }

    fn generate(&self, request: &GenerateRequest) -> Result<GenerateResponse, ModelError> {
        let url = format!("{}/api/generate", self.endpoint.trim_end_matches('/'));

        let body = json!({
            "model": self.model,
            "prompt": request.prompt,
            "system": request.system_prompt,
            "stream": false,
        });

        let response = ureq::post(&url)
            .timeout(self.timeout)
            .send_json(body)
            .map_err(|e| ModelError::ProviderError {
                provider: "ollama".to_string(),
                message: format!("Ollama generate error: {e}"),
            })?;

        let parsed: OllamaGenerateResponse = response.into_json().map_err(|e| {
            ModelError::ProviderError {
                provider: "ollama".to_string(),
                message: format!("Ollama JSON decode error: {e}"),
            }
        })?;

        Ok(GenerateResponse {
            text: parsed.response,
            model: TextModel::identity(self),
            token_usage: None,
        })
    }
}

// ==============================================================================
// 3. Google Gemini Provider
// ==============================================================================

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
        let ep = endpoint.into();
        Self {
            endpoint: if ep.is_empty() {
                "https://generativelanguage.googleapis.com".to_string()
            } else {
                ep
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
        let api_key = resolve_api_key(self.api_key_env.as_deref(), "google")?
            .ok_or_else(|| ModelError::AuthError {
                provider: "google".to_string(),
                env_var: "GEMINI_API_KEY".to_string(),
            })?;

        let mut results = Vec::with_capacity(texts.len());
        let url = format!(
            "{}/v1beta/models/{}:embedContent?key={}",
            self.endpoint.trim_end_matches('/'),
            self.model,
            api_key
        );

        for text in texts {
            let body = json!({
                "content": {
                    "parts": [{ "text": text }]
                }
            });

            let response = ureq::post(&url)
                .timeout(self.timeout)
                .send_json(body)
                .map_err(|e| ModelError::ProviderError {
                    provider: "google".to_string(),
                    message: format!("Gemini API error: {e}"),
                })?;

            let parsed: GeminiEmbeddingResponse = response.into_json().map_err(|e| {
                ModelError::ProviderError {
                    provider: "google".to_string(),
                    message: format!("Gemini JSON decode error: {e}"),
                }
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
        ModelIdentity {
            provider: "google".to_string(),
            model: self.model.clone(),
            version: None,
            location: ModelLocation::Remote,
        }
    }

    fn generate(&self, request: &GenerateRequest) -> Result<GenerateResponse, ModelError> {
        let api_key = resolve_api_key(self.api_key_env.as_deref(), "google")?
            .ok_or_else(|| ModelError::AuthError {
                provider: "google".to_string(),
                env_var: "GEMINI_API_KEY".to_string(),
            })?;

        let url = format!(
            "{}/v1beta/models/{}:generateContent?key={}",
            self.endpoint.trim_end_matches('/'),
            self.model,
            api_key
        );

        let mut parts = Vec::new();
        if let Some(ref sys) = request.system_prompt {
            parts.push(json!({ "text": format!("System instructions: {sys}\n") }));
        }
        parts.push(json!({ "text": request.prompt }));

        let body = json!({
            "contents": [{ "parts": parts }]
        });

        let response = ureq::post(&url)
            .timeout(self.timeout)
            .send_json(body)
            .map_err(|e| ModelError::ProviderError {
                provider: "google".to_string(),
                message: format!("Gemini generate error: {e}"),
            })?;

        let parsed: GeminiGenerateResponse = response.into_json().map_err(|e| {
            ModelError::ProviderError {
                provider: "google".to_string(),
                message: format!("Gemini JSON decode error: {e}"),
            }
        })?;

        let text = parsed
            .candidates
            .and_then(|c| c.into_iter().next())
            .and_then(|c| c.content)
            .and_then(|c| c.parts.into_iter().next())
            .and_then(|p| p.text)
            .unwrap_or_default();

        Ok(GenerateResponse {
            text,
            model: TextModel::identity(self),
            token_usage: None,
        })
    }
}
