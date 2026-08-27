//! Small embedded RAG proof: indexing and retrieval are Repin capabilities;
//! inference remains owned by the caller.

use repin_context::{AssembledContext, ContextBuilder};
use repin_core::ports::model::{EmbeddingModel, ModelError, ModelIdentity, ModelLocation};
use repin_core::ports::store::Store;
use repin_core::protocol::envelope::ResultEnvelope;
use repin_fs::CapabilityFs;
use repin_indexing::IndexingCoordinator;
use repin_intelligence::EmbeddedOnnxModel;
use repin_packs::default_packs;
use repin_retrieval::{ExactVectorIndex, HybridRetriever, LexicalHit, LexicalSource};
use repin_store_sqlite::SqliteStore;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy)]
struct FakeEmbedder;

impl EmbeddingModel for FakeEmbedder {
    fn identity(&self) -> ModelIdentity {
        ModelIdentity {
            provider: "embedded".to_string(),
            model: "fake-embedder".to_string(),
            version: Some("1.0.0".to_string()),
            location: ModelLocation::Local,
        }
    }

    fn dimensions(&self) -> usize {
        3
    }

    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, ModelError> {
        Ok(texts
            .iter()
            .map(|text| {
                let len = text.len() as f32;
                let hash = text
                    .bytes()
                    .fold(0_u32, |acc, b| acc.wrapping_add(b as u32))
                    as f32;
                vec![len.sin(), (len * 0.5).cos(), (hash % 10.0) / 10.0]
            })
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
        let paths: Vec<&str> = context.snippets.iter().map(|s| s.path.as_str()).collect();
        format!("Answer synthesized from: {}", paths.join(", "))
    }
}

struct StoreLexical<'a> {
    store: &'a SqliteStore,
}

impl LexicalSource for StoreLexical<'_> {
    fn search(&self, query: &str, limit: usize) -> Result<Vec<LexicalHit>, String> {
        let view = self.store.read_view().map_err(|e| e.to_string())?;
        let hits = {
            let conn_lock = self.store.raw_connection();
            let conn = conn_lock.lock().unwrap();
            repin_store_sqlite::Fts5Index::search(&conn, query, limit).map_err(|e| e.to_string())?
        };
        let mut results = Vec::new();
        for hit in hits {
            if view
                .node(&hit.node_id)
                .map_err(|e| e.to_string())?
                .is_some()
            {
                results.push(LexicalHit {
                    node_id: hit.node_id,
                    score: 1.0 / (1.0 + hit.rank.abs()),
                });
            }
        }
        Ok(results)
    }
}

#[derive(Debug)]
struct RagOutput {
    envelope: ResultEnvelope<String>,
}

fn run_rag<E: EmbeddingModel, I: CallerInference>(
    root_path: &Path,
    embedder: &E,
    inference: &I,
) -> Result<RagOutput, String> {
    let fs = CapabilityFs::open("root", root_path).map_err(|e| e.to_string())?;
    let store = SqliteStore::open_in_memory().map_err(|e| e.to_string())?;
    let packs = default_packs();

    IndexingCoordinator::index_source(&store, &fs, &packs).map_err(|e| e.to_string())?;

    let lexical = StoreLexical { store: &store };
    let view = store.read_view().map_err(|e| e.to_string())?;

    let mut vector_index = ExactVectorIndex::new(3);
    let query_vector = embedder
        .embed(&["how to build".to_string()])
        .map_err(|e| e.to_string())?[0]
        .clone();

    let all_nodes = view
        .nodes_by_name("build", &Default::default())
        .map_err(|e| e.to_string())?;
    for node in &all_nodes {
        let node_vec = embedder
            .embed(std::slice::from_ref(&node.name))
            .map_err(|e| e.to_string())?[0]
            .clone();
        vector_index.insert(node.id, node_vec);
    }

    let vector_hits = vector_index.search(&query_vector, 5);

    let results = HybridRetriever::search(
        view.as_ref(),
        Some(&lexical),
        "build",
        5,
        Some(&vector_hits),
        None,
    );

    let nodes: Vec<_> = results.candidates.into_iter().map(|c| c.node).collect();

    let assembled =
        ContextBuilder::assemble_neighborhood_with_fs(view.as_ref(), Some(&fs), &nodes, 4096);
    let answer = inference.infer(&assembled);

    let mut envelope = ResultEnvelope::ok(answer);
    envelope.evidence = assembled
        .snippets
        .into_iter()
        .map(|s| repin_core::protocol::evidence::Evidence::new(s.path).with_preview(s.content))
        .collect();

    Ok(RagOutput { envelope })
}

fn main() -> Result<(), String> {
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let src = dir.path().join("src");
    fs::create_dir_all(&src).map_err(|e| e.to_string())?;
    let rust_file = src.join("lib.rs");
    fs::write(&rust_file, b"pub fn build() { println!(\"building\"); }\n")
        .map_err(|e| e.to_string())?;
    let readme = dir.path().join("README.md");
    fs::write(&readme, b"# Project\nHow to build: run cargo build.\n")
        .map_err(|e| e.to_string())?;

    let embedder = FakeEmbedder;
    let inference = FakeInference;

    let output = run_rag(dir.path(), &embedder, &inference)?;
    println!("RAG Result: {}", output.envelope.data);
    println!("Evidence count: {}", output.envelope.evidence.len());

    let _offline_model = EmbeddedOnnxModel::new(
        std::path::Path::new("/tmp"),
        "fake-model".to_string(),
        Some(384),
        false,
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_rag_pipeline_executes_successfully() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("lib.rs"), b"pub fn build() {}\n").unwrap();
        let readme = dir.path().join("README.md");
        fs::write(&readme, b"# Title\n\nSome build details here.\n").unwrap();

        let embedder = FakeEmbedder;
        let inference = FakeInference;

        let output = run_rag(dir.path(), &embedder, &inference).unwrap();
        assert!(!output.envelope.data.is_empty());
        assert!(!output.envelope.evidence.is_empty());
    }
}
