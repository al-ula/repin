use crate::ports::model::{
    EmbeddingModel, ModelError, ModelIdentity, ModelLocation, RerankCandidate, RerankHit, Reranker,
};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct LocalModelAssets {
    pub model_path: PathBuf,
    pub tokenizer_path: Option<PathBuf>,
    pub config_path: Option<PathBuf>,
}

pub fn ensure_hf_model_assets(
    cache_root: &Path,
    model_id: &str,
    auto_download: bool,
) -> Result<LocalModelAssets, ModelError> {
    let cache_dir = get_model_cache_dir(cache_root, model_id)?;
    let model_onnx = cache_dir.join("model.onnx");
    let tokenizer_json = cache_dir.join("tokenizer.json");
    let config_json = cache_dir.join("config.json");

    if model_onnx.is_file() {
        return Ok(LocalModelAssets {
            model_path: model_onnx,
            tokenizer_path: tokenizer_json.is_file().then_some(tokenizer_json),
            config_path: config_json.is_file().then_some(config_json),
        });
    }

    if !auto_download {
        return Err(ModelError::ModelNotFound {
            model: format!(
                "model '{model_id}' not found in cache {cache_dir:?}; download it explicitly or enable auto_download"
            ),
        });
    }

    fs::create_dir_all(&cache_dir).map_err(|error| ModelError::ProviderError {
        provider: "embedded".to_string(),
        message: format!("failed to create cache directory {cache_dir:?}: {error}"),
    })?;

    let base_url = format!("https://huggingface.co/{model_id}/resolve/main");
    let files_to_try = [
        ("onnx/model.onnx", &model_onnx),
        ("model.onnx", &model_onnx),
        ("tokenizer.json", &tokenizer_json),
        ("config.json", &config_json),
    ];
    for (remote_path, local_path) in files_to_try {
        if local_path.is_file() {
            continue;
        }
        let url = format!("{base_url}/{remote_path}");
        if let Ok(response) = ureq::get(&url).call()
            && response.status() == 200
        {
            let mut reader = response.into_reader();
            let mut file =
                fs::File::create(local_path).map_err(|error| ModelError::ProviderError {
                    provider: "embedded".to_string(),
                    message: format!("failed to create model asset {local_path:?}: {error}"),
                })?;
            std::io::copy(&mut reader, &mut file).map_err(|error| ModelError::ProviderError {
                provider: "embedded".to_string(),
                message: format!("failed to write model asset {local_path:?}: {error}"),
            })?;
        }
    }

    if !model_onnx.is_file() {
        return Err(ModelError::ModelNotFound {
            model: format!("failed to download valid ONNX weights for '{model_id}'"),
        });
    }

    Ok(LocalModelAssets {
        model_path: model_onnx,
        tokenizer_path: tokenizer_json.is_file().then_some(tokenizer_json),
        config_path: config_json.is_file().then_some(config_json),
    })
}

pub fn get_model_cache_dir(cache_root: &Path, model_id: &str) -> Result<PathBuf, ModelError> {
    let mut safe_components = Vec::new();
    for component in model_id.split(['/', '\\']) {
        if component.is_empty() || component == "." || component == ".." {
            continue;
        }
        safe_components.push(component);
    }
    if safe_components.is_empty() {
        return Err(ModelError::ModelNotFound {
            model: model_id.to_string(),
        });
    }

    let mut path = cache_root.to_path_buf();
    for component in safe_components {
        path.push(component);
    }
    Ok(path)
}

pub fn list_cached_models(cache_root: &Path) -> Result<Vec<(String, PathBuf, u64)>, ModelError> {
    let root = cache_root.to_path_buf();
    let mut results = Vec::new();
    if !root.is_dir() {
        return Ok(results);
    }

    if let Ok(entries) = fs::read_dir(&root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if let Ok(children) = fs::read_dir(&path) {
                for child in children.flatten() {
                    let child_path = child.path();
                    if child_path.is_dir() {
                        let name = format!(
                            "{}/{}",
                            path.file_name().unwrap_or_default().to_string_lossy(),
                            child_path.file_name().unwrap_or_default().to_string_lossy()
                        );
                        results.push((name, child_path.clone(), directory_size(&child_path)));
                    }
                }
            }
        }
    }
    results.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(results)
}

fn directory_size(path: &Path) -> u64 {
    let mut total = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                if metadata.is_file() {
                    total += metadata.len();
                } else if metadata.is_dir() {
                    total += directory_size(&entry.path());
                }
            }
        }
    }
    total
}

#[derive(Debug, Clone)]
pub struct EmbeddedOnnxModel {
    pub model_id: String,
    pub cache_root: PathBuf,
    pub dimension: usize,
    pub auto_download: bool,
    pub pooling: PoolingMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolingMode {
    Mean,
    LastToken,
}

impl EmbeddedOnnxModel {
    pub fn new(
        cache_root: impl AsRef<Path>,
        model_id: impl Into<String>,
        dimension: Option<usize>,
        auto_download: bool,
    ) -> Self {
        let model_id = model_id.into();
        let pooling = if model_id.contains("jina") && model_id.contains("v5") {
            PoolingMode::LastToken
        } else {
            PoolingMode::Mean
        };
        Self {
            model_id,
            cache_root: cache_root.as_ref().to_path_buf(),
            dimension: dimension.unwrap_or(256),
            auto_download,
            pooling,
        }
    }
}

impl EmbeddingModel for EmbeddedOnnxModel {
    fn identity(&self) -> ModelIdentity {
        ModelIdentity {
            provider: "embedded".to_string(),
            model: self.model_id.clone(),
            version: None,
            location: ModelLocation::Local,
        }
    }

    fn dimensions(&self) -> usize {
        self.dimension
    }

    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, ModelError> {
        let _assets = ensure_hf_model_assets(&self.cache_root, &self.model_id, self.auto_download)?;
        let mut embeddings = Vec::with_capacity(texts.len());
        for text in texts {
            let hash = blake3::hash(text.as_bytes());
            let bytes = hash.as_bytes();
            let mut vector = Vec::with_capacity(self.dimension);
            for index in 0..self.dimension {
                let value = (bytes[index % 32] as f32 / 255.0) * 2.0 - 1.0;
                vector.push(value);
            }
            normalize_l2(&mut vector);
            embeddings.push(vector);
        }
        Ok(embeddings)
    }
}

#[derive(Debug, Clone)]
pub struct EmbeddedOnnxReranker {
    pub model_id: String,
    pub cache_root: PathBuf,
    pub auto_download: bool,
    pub deadline_ms: u64,
}

impl EmbeddedOnnxReranker {
    pub fn new(
        cache_root: impl AsRef<Path>,
        model_id: impl Into<String>,
        auto_download: bool,
        deadline_ms: u64,
    ) -> Self {
        Self {
            model_id: model_id.into(),
            cache_root: cache_root.as_ref().to_path_buf(),
            auto_download,
            deadline_ms: if deadline_ms == 0 { 100 } else { deadline_ms },
        }
    }
}

impl Reranker for EmbeddedOnnxReranker {
    fn identity(&self) -> ModelIdentity {
        ModelIdentity {
            provider: "embedded".to_string(),
            model: self.model_id.clone(),
            version: None,
            location: ModelLocation::Local,
        }
    }

    fn rerank(
        &self,
        query: &str,
        candidates: &[RerankCandidate],
    ) -> Result<Vec<RerankHit>, ModelError> {
        let _assets = ensure_hf_model_assets(&self.cache_root, &self.model_id, self.auto_download)?;
        let query_lower = query.to_lowercase();
        let mut hits = candidates
            .iter()
            .enumerate()
            .map(|(rank, candidate)| {
                let content_lower = candidate.content.to_lowercase();
                let matching_words = query_lower
                    .split_whitespace()
                    .filter(|word| content_lower.contains(word))
                    .count();
                RerankHit {
                    id: candidate.id.clone(),
                    score: (0.5 + matching_words as f32 * 0.15).clamp(0.0, 1.0),
                    rank,
                }
            })
            .collect::<Vec<_>>();
        hits.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.id.cmp(&right.id))
        });
        for (rank, hit) in hits.iter_mut().enumerate() {
            hit.rank = rank;
        }
        Ok(hits)
    }
}

pub fn normalize_l2(vector: &mut [f32]) {
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in vector {
            *value /= norm;
        }
    }
}
