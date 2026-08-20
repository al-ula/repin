pub mod agent;
pub mod embedded;
pub mod registry;
pub mod remote_api;

pub use agent::AgentRunnerReranker;
pub use embedded::{
    ensure_hf_model_assets, list_cached_models, normalize_l2, EmbeddedOnnxModel,
    EmbeddedOnnxReranker, LocalModelAssets, PoolingMode,
};
pub use registry::IntelligenceRegistry;
pub use remote_api::{resolve_api_key, GoogleGeminiProvider, OllamaProvider, OpenAiProvider};

#[cfg(test)]
mod tests {
    use super::*;
    use repin_core::config::RepinConfig;
    use repin_core::ports::model::{RerankCandidate, Reranker};

    #[test]
    fn test_normalize_l2_unit_length() {
        let mut vec = vec![3.0f32, 4.0f32];
        normalize_l2(&mut vec);
        let len_sq: f32 = vec.iter().map(|x| x * x).sum();
        assert!((len_sq.sqrt() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_intelligence_registry_defaults_none() {
        let config = RepinConfig::default();
        let emb = IntelligenceRegistry::build_embedding_model(&config).unwrap();
        assert!(emb.is_none());

        let rerank = IntelligenceRegistry::build_reranker(&config).unwrap();
        assert!(rerank.is_none());

        let text = IntelligenceRegistry::build_text_model(&config).unwrap();
        assert!(text.is_none());
    }

    #[test]
    fn test_intelligence_registry_builds_embedded() {
        let mut config = RepinConfig::default();
        config.intelligence.embedding.provider = "embedded".to_string();
        config.intelligence.embedding.model = "Alibaba-NLP/gte-modernbert-base".to_string();
        config.intelligence.embedding.dimension = Some(128);

        let emb = IntelligenceRegistry::build_embedding_model(&config).unwrap().expect("should build");
        assert_eq!(emb.identity().provider, "embedded");
        assert_eq!(emb.dimensions(), 128);

        // Test embedding generation produces unit vectors of requested dimension
        let texts = vec!["fn main() {}".to_string(), "struct User;".to_string()];
        let vecs = emb.embed(&texts).unwrap();
        assert_eq!(vecs.len(), 2);
        assert_eq!(vecs[0].len(), 128);

        let len_sq: f32 = vecs[0].iter().map(|x| x * x).sum();
        assert!((len_sq.sqrt() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_embedded_reranker_scoring() {
        let reranker = EmbeddedOnnxReranker::new("Alibaba-NLP/gte-reranker-modernbert-base", true, 100);
        let candidates = vec![
            RerankCandidate {
                id: "1".to_string(),
                content: "pub fn start_daemon_server() {}".to_string(),
            },
            RerankCandidate {
                id: "2".to_string(),
                content: "pub fn stop_daemon_server() {}".to_string(),
            },
            RerankCandidate {
                id: "3".to_string(),
                content: "pub struct UnrelatedConfig;".to_string(),
            },
        ];

        let hits = reranker.rerank("start daemon", &candidates).unwrap();
        assert_eq!(hits.len(), 3);
        // Candidate 1 contains both "start" and "daemon" so should rank first
        assert_eq!(hits[0].id, "1");
        assert!(hits[0].score >= hits[1].score);
    }

    #[test]
    fn test_agent_runner_rerank_with_echo() {
        // Agent command that echoes a valid JSON-RPC response
        let echo_cmd = "echo '{\"jsonrpc\":\"2.0\",\"result\":{\"ranked\":[{\"id\":\"c2\",\"score\":0.95},{\"id\":\"c1\",\"score\":0.40}]}}'";
        let agent_reranker = AgentRunnerReranker::new(echo_cmd, 2000);

        let candidates = vec![
            RerankCandidate { id: "c1".to_string(), content: "code 1".to_string() },
            RerankCandidate { id: "c2".to_string(), content: "code 2".to_string() },
        ];

        let hits = agent_reranker.rerank("search query", &candidates).unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].id, "c2");
        assert_eq!(hits[0].score, 0.95);
        assert_eq!(hits[1].id, "c1");
        assert_eq!(hits[1].score, 0.40);
    }

    #[test]
    fn test_resolve_api_key_env() {
        // Safe resolution: if env var not found, returns AuthError
        let res = resolve_api_key(Some("NON_EXISTENT_REPIN_TEST_KEY_12345"), "openai");
        assert!(res.is_err());

        // Empty or None returns Ok(None)
        let res_none = resolve_api_key(None, "openai").unwrap();
        assert_eq!(res_none, None);
    }
}
