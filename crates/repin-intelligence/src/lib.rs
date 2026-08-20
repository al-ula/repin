//! Optional concrete implementations of Repin's model ports.
//!
//! Provider selection and configuration interpretation belong to the runtime
//! composition layer. This crate only supplies adapters and their capability
//! identities; all model ports remain defined by `repin-core`.

#[cfg(feature = "agent")]
pub mod agent;
#[cfg(feature = "embedded")]
pub mod embedded;
#[cfg(feature = "remote")]
pub mod remote_api;

#[cfg(feature = "agent")]
pub use agent::AgentRunnerReranker;
#[cfg(feature = "embedded")]
pub use embedded::{
    EmbeddedOnnxModel, EmbeddedOnnxReranker, LocalModelAssets, PoolingMode, ensure_hf_model_assets,
    get_model_cache_dir, list_cached_models, normalize_l2,
};
#[cfg(feature = "remote")]
pub use remote_api::{GoogleGeminiProvider, OllamaProvider, OpenAiProvider, resolve_api_key};

#[cfg(all(test, feature = "agent", feature = "embedded", feature = "remote"))]
mod tests {
    use super::*;
    use repin_core::ports::model::{EmbeddingModel, ModelError, RerankCandidate, Reranker};

    #[test]
    fn normalization_is_unit_length() {
        let mut vector = vec![3.0_f32, 4.0];
        normalize_l2(&mut vector);
        let length = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
        assert!((length - 1.0).abs() < 1e-5);
    }

    #[test]
    fn missing_credentials_are_explicit() {
        let result = resolve_api_key(Some("REPIN_INTELLIGENCE_TEST_MISSING"), "openai");
        assert!(result.is_err());
        assert_eq!(resolve_api_key(None, "openai").unwrap(), None);
    }

    #[test]
    fn embedded_model_absence_is_explicit_and_offline() {
        let model = EmbeddedOnnxModel::new(
            format!("repin-test-missing-{}", std::process::id()),
            Some(8),
            false,
        );
        let error = model.embed(&["query".to_string()]).unwrap_err();
        assert!(matches!(error, ModelError::ModelNotFound { .. }));
    }

    #[test]
    fn malformed_agent_response_is_a_provider_error() {
        let reranker = AgentRunnerReranker::new("printf malformed", 2000);
        let candidates = [RerankCandidate {
            id: "one".to_string(),
            content: "one".to_string(),
        }];
        let error = reranker.rerank("query", &candidates).unwrap_err();
        assert!(matches!(error, ModelError::ProviderError { .. }));
    }

    #[test]
    fn agent_deadline_returns_timeout() {
        let reranker = AgentRunnerReranker::new("sleep 1", 10);
        let candidates = [RerankCandidate {
            id: "one".to_string(),
            content: "one".to_string(),
        }];
        let error = reranker.rerank("query", &candidates).unwrap_err();
        assert!(matches!(error, ModelError::Timeout { .. }));
    }

    #[test]
    fn agent_round_trip_is_structured_and_bounded() {
        let reranker = AgentRunnerReranker::new(
            "echo '{\"jsonrpc\":\"2.0\",\"result\":{\"ranked\":[{\"id\":\"two\",\"score\":0.9},{\"id\":\"one\",\"score\":0.2}]}}'",
            2000,
        );
        let candidates = vec![
            RerankCandidate {
                id: "one".to_string(),
                content: "one".to_string(),
            },
            RerankCandidate {
                id: "two".to_string(),
                content: "two".to_string(),
            },
        ];
        let hits = reranker.rerank("query", &candidates).unwrap();
        assert_eq!(hits[0].id, "two");
        assert_eq!(hits[1].id, "one");
    }
}
