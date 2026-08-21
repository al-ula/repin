//! Compatibility re-exports for runtime-owned provider selection and adapters.

pub use repin_runtime::intelligence::*;

#[cfg(test)]
mod tests {
    use super::*;
    use repin_core::config::RepinConfig;
    use repin_core::ports::model::{RerankCandidate, Reranker};

    #[test]
    fn provider_registry_defaults_to_absence() {
        let config = RepinConfig::default();
        assert!(
            IntelligenceRegistry::build_embedding_model(&config, std::path::Path::new("/tmp"))
                .unwrap()
                .is_none()
        );
        assert!(
            IntelligenceRegistry::build_reranker(&config, std::path::Path::new("/tmp"))
                .unwrap()
                .is_none()
        );
        assert!(
            IntelligenceRegistry::build_text_model(&config)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn agent_provider_round_trip_remains_compatible() {
        let reranker = AgentRunnerReranker::new(
            "echo '{\"jsonrpc\":\"2.0\",\"result\":{\"ranked\":[{\"id\":\"c2\",\"score\":0.95},{\"id\":\"c1\",\"score\":0.40}]}}'",
            2000,
        );
        let candidates = vec![
            RerankCandidate {
                id: "c1".to_string(),
                content: "code 1".to_string(),
            },
            RerankCandidate {
                id: "c2".to_string(),
                content: "code 2".to_string(),
            },
        ];
        let hits = reranker.rerank("search query", &candidates).unwrap();
        assert_eq!(hits[0].id, "c2");
        assert_eq!(hits[1].id, "c1");
    }
}
