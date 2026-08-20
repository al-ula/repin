use anyhow::{Context, Result};
use repin_engine::intelligence::{ensure_hf_model_assets, list_cached_models};
use std::fs;

pub fn execute_model_download(model_id: &str) -> Result<()> {
    println!("Fetching model '{}' from Hugging Face Hub...", model_id);
    let assets = ensure_hf_model_assets(model_id, true)
        .map_err(|e| anyhow::anyhow!("Failed to download model: {e}"))?;

    println!("✓ Model successfully downloaded to cache:");
    println!("  ONNX Weights: {:?}", assets.model_path);
    if let Some(tok) = assets.tokenizer_path {
        println!("  Tokenizer:    {:?}", tok);
    }
    if let Some(cfg) = assets.config_path {
        println!("  Config:       {:?}", cfg);
    }
    Ok(())
}

pub fn execute_model_list() -> Result<()> {
    let models =
        list_cached_models().map_err(|e| anyhow::anyhow!("Failed to list cached models: {e}"))?;

    if models.is_empty() {
        println!("No models found in local cache (~/.cache/repin/models/).");
        println!("Run 'repin model download <org/model>' to download a model.");
        return Ok(());
    }

    println!(
        "Cached Models ({}) in ~/.cache/repin/models/:",
        models.len()
    );
    for (name, path, size_bytes) in models {
        let size_mb = (size_bytes as f64) / (1024.0 * 1024.0);
        println!("  • {:<40} [{:>6.1} MB] ({:?})", name, size_mb, path);
    }

    Ok(())
}

pub fn execute_model_remove(model_id: &str) -> Result<()> {
    let cache_dir = repin_engine::intelligence::embedded::get_model_cache_dir(model_id)
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    if !cache_dir.exists() {
        println!(
            "Model '{}' is not present in local cache ({:?}).",
            model_id, cache_dir
        );
        return Ok(());
    }

    fs::remove_dir_all(&cache_dir)
        .with_context(|| format!("failed to remove cache directory {:?}", cache_dir))?;

    println!(
        "✓ Removed model '{}' from local cache ({:?}).",
        model_id, cache_dir
    );
    Ok(())
}
