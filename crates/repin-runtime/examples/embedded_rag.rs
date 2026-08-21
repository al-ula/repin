//! Small embedded RAG proof: indexing and retrieval are Repin capabilities;
//! inference remains owned by the caller.

use repin_context::{AssembledContext, ContextBuilder};
use repin_core::ports::model::{EmbeddingModel, ModelError, ModelIdentity, ModelLocation};
use repin_core::ports::store::Store;
use repin_fs::CapabilityFs;
use repin_indexing::IndexingCoordinator;
use repin_packs::default_packs;
use repin_protocol::envelope::{ResultEnvelope, SourceKind};
use repin_protocol::freshness::{
    CoverageState, Freshness, GraphState, LexicalState, Truncation, TruncationReason,
};
use repin_retrieval::{ExactVectorIndex, HybridRetriever, LexicalHit, LexicalSource};
use repin_runtime::EmbeddedOnnxModel;
use repin_store_sqlite::SqliteStore;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy)]
struct FakeEmbedder;

impl EmbeddingModel for FakeEmbedder {
    fn identity(&self) -> ModelIdentity {
        ModelIdentity {
            provider: "fake".to_string(),
            model: "hash-v1".to_string(),
            version: Some("1".to_string()),
            location: ModelLocation::Local,
        }
    }

    fn dimensions(&self) -> usize {
        32
    }

    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, ModelError> {
        Ok(texts
            .iter()
            .map(|text| ExactVectorIndex::deterministic_embed(text, self.dimensions()))
            .collect())
    }
}

trait CallerInference {
    fn infer(&self, context: &AssembledContext) -> String;
}

#[derive(Debug, Clone, Copy)]
struct FakeInference;

impl CallerInference for FakeInference {
    fn infer(&self, context: &AssembledContext) -> String {
        context
            .snippets
            .first()
            .map(|snippet| format!("answer derived from {}", snippet.path))
            .unwrap_or_else(|| "no evidence".to_string())
    }
}

struct StoreLexical<'a> {
    store: &'a SqliteStore,
}

impl LexicalSource for StoreLexical<'_> {
    fn search(&self, query: &str, limit: usize) -> Result<Vec<LexicalHit>, String> {
        self.store
            .search_fts(query, limit)
            .map(|hits| {
                hits.into_iter()
                    .map(|hit| LexicalHit {
                        node_id: hit.node_id,
                        score: hit.rank,
                    })
                    .collect()
            })
            .map_err(|error| error.to_string())
    }
}

#[derive(Debug)]
struct RagOutput {
    context: ResultEnvelope<AssembledContext>,
    answer: String,
    vector_hits: usize,
}

fn run_rag<E: EmbeddingModel, I: CallerInference>(
    root_path: &Path,
    embedder: &E,
    inference: &I,
) -> Result<RagOutput, String> {
    let source = CapabilityFs::open("root", root_path).map_err(|error| error.to_string())?;
    let store = SqliteStore::open_in_memory().map_err(|error| error.to_string())?;
    let packs = default_packs();
    IndexingCoordinator::index_source(&store, &source, &packs)
        .map_err(|error| error.to_string())?;

    let view = store.read_view().map_err(|error| error.to_string())?;
    let lexical = StoreLexical { store: &store };
    let ranked =
        HybridRetriever::search(view.as_ref(), Some(&lexical), "answer", 16, None, None).candidates;

    let query_vector = embedder
        .embed(&["answer".to_string()])
        .map_err(|error| error.to_string())?
        .into_iter()
        .next()
        .ok_or_else(|| "embedding model returned no query vector".to_string())?;
    let mut vector_index = ExactVectorIndex::new(embedder.dimensions());
    for candidate in &ranked {
        let text = format!("{} {}", candidate.node.name, candidate.node.path);
        let vector = embedder
            .embed(&[text])
            .map_err(|error| error.to_string())?
            .into_iter()
            .next()
            .ok_or_else(|| "embedding model returned no candidate vector".to_string())?;
        vector_index.insert(candidate.node.id, vector);
    }
    let vector_hits = vector_index.search(&query_vector, 8).len();

    let nodes = ranked
        .iter()
        .take(4)
        .map(|candidate| candidate.node.clone())
        .collect::<Vec<_>>();
    let context_data =
        ContextBuilder::assemble_neighborhood_with_fs(view.as_ref(), Some(&source), &nodes, 4096);
    let graph_revision = view.revision().map_err(|error| error.to_string())?;
    let answer = inference.infer(&context_data);
    let mut context = ResultEnvelope::ok(context_data);
    context.provenance.sources = vec![SourceKind::Graph, SourceKind::WorkingTree];
    context.freshness = Freshness {
        observed_at: None,
        graph_revision: Some(graph_revision),
        graph_state: GraphState::Current,
        lexical_revision: Some(graph_revision),
        lexical_state: LexicalState::Current,
        coverage: CoverageState::Complete,
    };
    if context.data.truncated {
        context.truncation = Some(Truncation {
            truncated: true,
            returned: context.data.snippets.len(),
            available: None,
            reason: TruncationReason::Bytes,
        });
    }
    Ok(RagOutput {
        context,
        answer,
        vector_hits,
    })
}

fn main() -> Result<(), String> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let local_model = arguments
        .windows(2)
        .find(|window| window[0] == "--local-model")
        .map(|window| window[1].clone());
    let allow_download = arguments
        .iter()
        .any(|argument| argument == "--allow-download");

    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    fs::create_dir_all(directory.path().join("src")).map_err(|error| error.to_string())?;
    fs::write(
        directory.path().join("src/lib.rs"),
        "pub fn answer() -> &'static str { \"repin\" }\n",
    )
    .map_err(|error| error.to_string())?;
    fs::write(
        directory.path().join("src/helper.rs"),
        "pub fn helper() { answer(); }\n",
    )
    .map_err(|error| error.to_string())?;

    let output = run_rag(directory.path(), &FakeEmbedder, &FakeInference)?;
    println!("answer: {}", output.answer);
    println!("snippets: {}", output.context.data.snippets.len());
    println!("truncated: {}", output.context.data.truncated);
    println!("vector hits: {}", output.vector_hits);

    if let Some(model_id) = local_model {
        let model = EmbeddedOnnxModel::new(
            directory.path().join("model-cache"),
            model_id,
            Some(32),
            allow_download,
        );
        let vectors = model
            .embed(&["opt-in local model smoke".to_string()])
            .map_err(|error| error.to_string())?;
        println!("local provider: {} dimensions", vectors[0].len());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn fake_rag_flow_is_deterministic_offline() {
        let directory = tempdir().unwrap();
        fs::create_dir_all(directory.path().join("src")).unwrap();
        fs::write(
            directory.path().join("src/lib.rs"),
            "pub fn answer() -> &'static str { \"repin\" }\n",
        )
        .unwrap();

        let first = run_rag(directory.path(), &FakeEmbedder, &FakeInference).unwrap();
        let second = run_rag(directory.path(), &FakeEmbedder, &FakeInference).unwrap();
        assert_eq!(first.context, second.context);
        assert_eq!(first.answer, "answer derived from src/lib.rs");
        assert_eq!(first.vector_hits, second.vector_hits);
        assert_eq!(first.context.provenance.sources.len(), 2);
        assert_eq!(first.context.freshness.coverage, CoverageState::Complete);
        assert!(!first.context.data.truncated);
    }
}
