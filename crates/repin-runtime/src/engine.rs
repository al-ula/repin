use crate::agent::AgentReranker;
use crate::context::{AssembledContext, ContextBuilder};
use crate::eval::{BenchmarkHarness, EvalReport};
use crate::inspect::{FileOutline, Inspector};
use crate::invalidation::{IndexingCoordinator, InvalidationCoordinator};
use crate::ranking::{DeterministicRanker, RankReason, RankedCandidate};
use crate::traversal::{GraphTraversal, NeighborsData};
use repin_core::line_index::Position;
use repin_core::model::node::Node;
use repin_core::model::provenance::Revision;
use repin_core::ports::fs::FileSnapshot;
use repin_core::ports::pack::LanguagePack;
use repin_core::ports::store::{Store, StoreError, UpdateSummary};
use repin_core::ports::vcs::Vcs;
use repin_direct_search::{DirectRegex, DirectScanner};
use repin_fs::{CapabilityFs, GitVcs};
use repin_packs::default_packs;
use repin_protocol::envelope::{ResultEnvelope, SourceKind};
use repin_protocol::evidence::Evidence;
use repin_protocol::freshness::{CoverageState, Freshness, GraphState, LexicalState};
use repin_retrieval::{HybridRetriever, LexicalHit, LexicalSource};
use repin_store_sqlite::SqliteStore;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct RuntimeOptions {
    pub root_id: String,
    pub root_path: PathBuf,
    pub db_path: Option<PathBuf>,
}

/// Default concrete composition for embedded and product callers.
pub struct Runtime {
    options: RuntimeOptions,
    fs: CapabilityFs,
    store: Option<SqliteStore>,
    packs: Vec<Box<dyn LanguagePack>>,
}

impl Runtime {
    pub fn open(options: RuntimeOptions) -> Result<Self, String> {
        let fs = CapabilityFs::open(&options.root_id, &options.root_path)
            .map_err(|error| format!("failed to open root filesystem: {error}"))?;
        let store = if let Some(ref db_path) = options.db_path {
            if let Some(parent) = db_path.parent() {
                let _ = std::fs::create_dir_all(parent);
                let gitignore = parent.join(".gitignore");
                if !gitignore.exists() {
                    let _ = std::fs::write(gitignore, "*\n");
                }
            }
            match SqliteStore::open(db_path) {
                Ok(store) => Some(store),
                Err(error) => {
                    tracing::warn!(
                        path = %db_path.display(),
                        error = %error,
                        "sqlite store unavailable; retaining direct retrieval"
                    );
                    None
                }
            }
        } else {
            None
        };
        Ok(Self {
            options,
            fs,
            store,
            packs: default_packs(),
        })
    }

    pub fn open_in_memory(root_path: impl AsRef<Path>) -> Result<Self, String> {
        let root_path = root_path.as_ref().to_path_buf();
        let fs = CapabilityFs::open("root", &root_path)
            .map_err(|error| format!("failed to open filesystem: {error}"))?;
        let store = SqliteStore::open_in_memory()
            .map_err(|error| format!("failed to open in-memory store: {error}"))?;
        Ok(Self {
            options: RuntimeOptions {
                root_id: "root".to_string(),
                root_path,
                db_path: None,
            },
            fs,
            store: Some(store),
            packs: default_packs(),
        })
    }

    pub fn options(&self) -> &RuntimeOptions {
        &self.options
    }

    pub fn store(&self) -> Option<&SqliteStore> {
        self.store.as_ref()
    }

    pub fn search_direct(
        &self,
        pattern: &str,
        is_regex: bool,
        max_results: usize,
    ) -> ResultEnvelope<Vec<Evidence>> {
        let regex = match DirectRegex::compile(pattern, is_regex) {
            Ok(regex) => regex,
            Err(error) => {
                let mut envelope = ResultEnvelope::not_found(Vec::new());
                envelope.status = repin_protocol::envelope::Status::Invalid;
                envelope.warnings.push(repin_protocol::envelope::Warning {
                    code: repin_protocol::errors::ErrorCode::InvalidQuery,
                    message: error.to_string(),
                    detail: None,
                });
                return envelope;
            }
        };

        let mut all_evidence = Vec::new();
        let _ = self.fs.walk_files(|snapshot| {
            if all_evidence.len() < max_results
                && let Ok(matches) = DirectScanner::scan_snapshot(
                    &regex,
                    &snapshot,
                    max_results - all_evidence.len(),
                )
            {
                all_evidence.extend(matches);
            }
            Ok(())
        });

        let mut envelope = ResultEnvelope::ok(all_evidence.clone());
        envelope.evidence = all_evidence;
        envelope.provenance.sources.push(SourceKind::WorkingTree);
        envelope.freshness = Freshness {
            observed_at: Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            ),
            graph_revision: None,
            graph_state: GraphState::Unknown,
            lexical_revision: None,
            lexical_state: LexicalState::Disabled,
            coverage: CoverageState::Complete,
        };
        envelope
    }

    pub fn search_graph(
        &self,
        query: &str,
        max_results: usize,
    ) -> ResultEnvelope<Vec<RankedCandidate>> {
        let Some(store) = self.store.as_ref() else {
            return unavailable_graph();
        };
        let Ok(fts_hits) = store.search_fts(query, max_results * 2) else {
            return ResultEnvelope::not_found(Vec::new());
        };
        let Ok(view) = store.read_view() else {
            return ResultEnvelope::not_found(Vec::new());
        };
        let mut candidate_nodes = Vec::new();
        for hit in fts_hits {
            if let Ok(Some(node)) = view.node(&hit.node_id) {
                candidate_nodes.push(node);
            }
        }
        let mut in_degrees = HashMap::new();
        for node in &candidate_nodes {
            if let Ok(count) = view.incoming_edge_count(&node.id) {
                in_degrees.insert(node.id, count);
            }
        }
        let mut ranked =
            DeterministicRanker::rank_fusion(query, candidate_nodes, &HashMap::new(), &in_degrees);
        ranked.truncate(max_results);
        let mut envelope = ResultEnvelope::ok(ranked);
        envelope.provenance.sources.push(SourceKind::Graph);
        envelope.freshness = graph_freshness(&*view);
        envelope
    }

    pub fn inspect_file(&self, relative_path: &str) -> ResultEnvelope<FileOutline> {
        let Some(store) = self.store.as_ref() else {
            return unavailable_outline(&self.options.root_id, relative_path);
        };
        let Ok(view) = store.read_view() else {
            return ResultEnvelope::not_found(FileOutline {
                root: self.options.root_id.clone(),
                path: relative_path.to_string(),
                symbols: Vec::new(),
            });
        };
        let outline = Inspector::inspect_file(&*view, &self.options.root_id, relative_path);
        let mut envelope = ResultEnvelope::ok(outline);
        envelope.provenance.sources.push(SourceKind::Graph);
        envelope
    }

    pub fn at_position(
        &self,
        relative_path: &str,
        position: Position,
    ) -> ResultEnvelope<Option<Node>> {
        let Some(store) = self.store.as_ref() else {
            let mut envelope = ResultEnvelope::not_found(None);
            envelope.status = repin_protocol::envelope::Status::Unavailable;
            return envelope;
        };
        let Ok(view) = store.read_view() else {
            return ResultEnvelope::not_found(None);
        };
        let node = Inspector::at_position(&*view, &self.options.root_id, relative_path, position);
        let mut envelope = ResultEnvelope::ok(node);
        envelope.provenance.sources.push(SourceKind::Graph);
        envelope
    }

    pub fn review_context(
        &self,
        _changed_since: Option<Revision>,
        budget_bytes: usize,
    ) -> ResultEnvelope<AssembledContext> {
        let Some(store) = self.store.as_ref() else {
            return unavailable_context();
        };
        let Ok(view) = store.read_view() else {
            return ResultEnvelope::not_found(empty_context());
        };
        let sample_nodes = view
            .nodes_by_name("main", &Default::default())
            .unwrap_or_default();
        let assembled = ContextBuilder::assemble_neighborhood_with_fs(
            &*view,
            Some(&self.fs),
            &sample_nodes,
            budget_bytes,
        );
        let mut envelope = ResultEnvelope::ok(assembled);
        envelope.provenance.sources.push(SourceKind::Graph);
        envelope
    }

    pub fn update_snapshot(&self, snapshot: &FileSnapshot) -> Result<UpdateSummary, StoreError> {
        let Some(store) = self.store.as_ref() else {
            return Err(StoreError::Io("store not available".to_string()));
        };
        InvalidationCoordinator::apply_snapshot_update(store, &self.packs, snapshot)
    }

    pub fn search_hybrid(
        &self,
        query: &str,
        max_results: usize,
    ) -> ResultEnvelope<Vec<RankedCandidate>> {
        let Some(store) = self.store.as_ref() else {
            return unavailable_graph();
        };
        let Ok(view) = store.read_view() else {
            return ResultEnvelope::not_found(Vec::new());
        };
        let lexical = SqliteLexical { store };
        let ranked =
            HybridRetriever::search(&*view, Some(&lexical), query, max_results, None).candidates;
        let mut envelope = ResultEnvelope::ok(ranked);
        envelope.provenance.sources.push(SourceKind::Graph);
        envelope.freshness = graph_freshness(&*view);
        envelope
    }

    pub fn rerank_candidates(
        &self,
        query: &str,
        candidates: &[String],
        agent_cmd: &str,
    ) -> ResultEnvelope<Vec<RankedCandidate>> {
        let Some(store) = self.store.as_ref() else {
            let mut envelope = ResultEnvelope::not_found(Vec::new());
            envelope.status = repin_protocol::envelope::Status::Unavailable;
            return envelope;
        };
        let Ok(view) = store.read_view() else {
            return ResultEnvelope::not_found(Vec::new());
        };
        let ranked = if candidates.is_empty() {
            self.search_hybrid(query, 20).data
        } else {
            let mut nodes = candidates
                .iter()
                .filter_map(|item| GraphTraversal::lookup_entity(&*view, item))
                .collect::<Vec<_>>();
            nodes.sort_by_key(|node| node.id);
            DeterministicRanker::rank(query, nodes)
        };
        if ranked.is_empty() {
            let mut envelope = ResultEnvelope::ok(Vec::new());
            envelope.provenance.sources.push(SourceKind::Graph);
            return envelope;
        }
        let final_ranked =
            match AgentReranker::rerank_with_shell_callback(query, ranked.clone(), agent_cmd) {
                Ok(reordered) => reordered,
                Err(error) => {
                    let mut envelope = ResultEnvelope::ok(ranked);
                    envelope.provenance.sources.push(SourceKind::Graph);
                    envelope.warnings.push(repin_protocol::envelope::Warning {
                        code: repin_protocol::errors::ErrorCode::CapabilityUnavailable,
                        message: format!("Agent reranker callback failed: {error}"),
                        detail: None,
                    });
                    return envelope;
                }
            };
        let mut envelope = ResultEnvelope::ok(final_ranked);
        envelope.provenance.sources.push(SourceKind::Graph);
        envelope
    }

    pub fn rerank_with_model(
        &self,
        query: &str,
        candidates: &[String],
        reranker: &dyn repin_core::ports::model::Reranker,
    ) -> ResultEnvelope<Vec<RankedCandidate>> {
        let Some(store) = self.store.as_ref() else {
            let mut envelope = ResultEnvelope::not_found(Vec::new());
            envelope.status = repin_protocol::envelope::Status::Unavailable;
            return envelope;
        };
        let Ok(view) = store.read_view() else {
            return ResultEnvelope::not_found(Vec::new());
        };
        let ranked = if candidates.is_empty() {
            self.search_hybrid(query, 20).data
        } else {
            let nodes = candidates
                .iter()
                .filter_map(|item| GraphTraversal::lookup_entity(&*view, item))
                .collect::<Vec<_>>();
            DeterministicRanker::rank(query, nodes)
        };
        if ranked.is_empty() {
            let mut envelope = ResultEnvelope::ok(Vec::new());
            envelope.provenance.sources.push(SourceKind::Graph);
            return envelope;
        }
        let model_candidates = ranked
            .iter()
            .map(|candidate| {
                let doc_summary = candidate
                    .node
                    .attributes
                    .get("docSummary")
                    .and_then(|value| value.as_str())
                    .unwrap_or("");
                repin_core::ports::model::RerankCandidate {
                    id: candidate.node.id.to_string(),
                    content: format!(
                        "{} ({}) in {} {}",
                        candidate.node.name,
                        candidate.node.kind.as_str(),
                        candidate.node.path,
                        doc_summary
                    ),
                }
            })
            .collect::<Vec<_>>();
        let model_hits = match reranker.rerank(query, &model_candidates) {
            Ok(hits) => hits,
            Err(error) => {
                let mut envelope = ResultEnvelope::ok(ranked);
                envelope.provenance.sources.push(SourceKind::Graph);
                envelope.warnings.push(repin_protocol::envelope::Warning {
                    code: repin_protocol::errors::ErrorCode::CapabilityUnavailable,
                    message: format!("Model reranker failed: {error}"),
                    detail: None,
                });
                return envelope;
            }
        };
        let hit_map = model_hits
            .into_iter()
            .map(|hit| (hit.id, (hit.rank, hit.score)))
            .collect::<HashMap<_, _>>();
        let mut final_ranked = ranked;
        for candidate in &mut final_ranked {
            let id = candidate.node.id.to_string();
            if let Some((_rank, score)) = hit_map.get(&id) {
                candidate.explanation.reasons.push(RankReason {
                    signal: format!("model_rerank:{}", reranker.identity().provider),
                    score: f64::from(*score),
                    detail: Some(format!("model: {}", reranker.identity().model)),
                });
                candidate.explanation.total_score += f64::from(*score);
            }
        }
        final_ranked.sort_by(|left, right| {
            right
                .explanation
                .total_score
                .partial_cmp(&left.explanation.total_score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.node.id.cmp(&right.node.id))
        });
        let mut envelope = ResultEnvelope::ok(final_ranked);
        envelope.provenance.sources.push(SourceKind::Graph);
        envelope
    }

    pub fn lookup_entity(&self, name_or_id: &str) -> ResultEnvelope<Option<Node>> {
        let Some(store) = self.store.as_ref() else {
            let mut envelope = ResultEnvelope::not_found(None);
            envelope.status = repin_protocol::envelope::Status::Unavailable;
            return envelope;
        };
        let Ok(view) = store.read_view() else {
            return ResultEnvelope::not_found(None);
        };
        let node = GraphTraversal::lookup_entity(&*view, name_or_id);
        let mut envelope = ResultEnvelope::ok(node);
        envelope.provenance.sources.push(SourceKind::Graph);
        envelope
    }

    pub fn lookup_neighbors(
        &self,
        name_or_id: &str,
        max_depth: usize,
    ) -> ResultEnvelope<Option<NeighborsData>> {
        let Some(store) = self.store.as_ref() else {
            let mut envelope = ResultEnvelope::not_found(None);
            envelope.status = repin_protocol::envelope::Status::Unavailable;
            return envelope;
        };
        let Ok(view) = store.read_view() else {
            return ResultEnvelope::not_found(None);
        };
        let data = GraphTraversal::lookup_neighbors(&*view, name_or_id, max_depth);
        let mut envelope = ResultEnvelope::ok(data);
        envelope.provenance.sources.push(SourceKind::Graph);
        envelope
    }

    pub fn assemble_context(
        &self,
        query: &str,
        budget_bytes: usize,
    ) -> ResultEnvelope<AssembledContext> {
        let Some(store) = self.store.as_ref() else {
            return unavailable_context();
        };
        let Ok(view) = store.read_view() else {
            return ResultEnvelope::not_found(empty_context());
        };
        let search_result = self.search_graph(query, 5);
        let primary_nodes = search_result
            .data
            .into_iter()
            .map(|candidate| candidate.node)
            .collect::<Vec<_>>();
        let assembled = ContextBuilder::assemble_neighborhood_with_fs(
            &*view,
            Some(&self.fs),
            &primary_nodes,
            budget_bytes,
        );
        let mut envelope = ResultEnvelope::ok(assembled);
        envelope.provenance.sources.push(SourceKind::Graph);
        envelope
    }

    pub fn sync_vcs(&self) -> Result<UpdateSummary, String> {
        let git = GitVcs::new();
        let root = self.options.root_path.to_string_lossy();
        let change_set = git
            .status(&root)
            .map_err(|error| format!("git status failed: {error}"))?;
        let mut total = UpdateSummary {
            revision: Revision::INITIAL,
            files_added: 0,
            files_modified: 0,
            files_deleted: 0,
            nodes_added: 0,
            nodes_removed: 0,
            edges_added: 0,
            edges_removed: 0,
            unresolved_promoted: 0,
            unresolved_demoted: 0,
        };
        let mut changed_files = change_set.modified_files;
        changed_files.extend(change_set.added_files);
        for path in &changed_files {
            if let Ok(snapshot) = self.fs.read_snapshot(path)
                && let Ok(summary) = self.update_snapshot(&snapshot)
            {
                total.files_modified += 1;
                total.nodes_added += summary.nodes_added;
                total.edges_added += summary.edges_added;
                total.unresolved_promoted += summary.unresolved_promoted;
                total.revision = summary.revision;
            }
        }
        if let Some(store) = self.store.as_ref() {
            let _ = store.checkpoint();
        }
        Ok(total)
    }

    pub fn evaluate_precision(&self) -> ResultEnvelope<EvalReport> {
        let report = BenchmarkHarness::evaluate_engine(self);
        let mut envelope = ResultEnvelope::ok(report);
        envelope.provenance.sources.push(SourceKind::Graph);
        envelope
    }

    pub fn index_all_worktree(&self) -> Result<usize, String> {
        let store = self
            .store
            .as_ref()
            .ok_or_else(|| "indexing error: store not available".to_string())?;
        let report = IndexingCoordinator::index_source(store, &self.fs, &self.packs)
            .map_err(|error| format!("indexing error: {error}"))?;
        if let Some(store) = self.store.as_ref() {
            let _ = store.checkpoint();
        }
        Ok(report.files_indexed)
    }
}

fn graph_freshness(view: &dyn repin_core::ports::store::ReadView) -> Freshness {
    let revision = view.revision().ok();
    Freshness {
        observed_at: None,
        graph_revision: revision,
        graph_state: GraphState::Current,
        lexical_revision: revision,
        lexical_state: LexicalState::Current,
        coverage: CoverageState::Complete,
    }
}

fn unavailable_graph() -> ResultEnvelope<Vec<RankedCandidate>> {
    let mut envelope = ResultEnvelope::not_found(Vec::new());
    envelope.status = repin_protocol::envelope::Status::Unavailable;
    envelope.warnings.push(repin_protocol::envelope::Warning {
        code: repin_protocol::errors::ErrorCode::CapabilityUnavailable,
        message: "graph store not configured or available".to_string(),
        detail: None,
    });
    envelope
}

fn empty_context() -> AssembledContext {
    AssembledContext {
        snippets: Vec::new(),
        total_bytes: 0,
        truncated: false,
    }
}

fn unavailable_context() -> ResultEnvelope<AssembledContext> {
    let mut envelope = ResultEnvelope::not_found(empty_context());
    envelope.status = repin_protocol::envelope::Status::Unavailable;
    envelope
}

fn unavailable_outline(root: &str, path: &str) -> ResultEnvelope<FileOutline> {
    let mut envelope = ResultEnvelope::not_found(FileOutline {
        root: root.to_string(),
        path: path.to_string(),
        symbols: Vec::new(),
    });
    envelope.status = repin_protocol::envelope::Status::Unavailable;
    envelope
}

struct SqliteLexical<'a> {
    store: &'a SqliteStore,
}

impl LexicalSource for SqliteLexical<'_> {
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
