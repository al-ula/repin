use super::agent::AgentReranker;
use super::eval::{BenchmarkHarness, EvalReport};
use super::inspect::{FileOutline, Inspector};
use crate::config::RepinConfig;
use crate::context::{AssembledContext, ContextBuilder};
use crate::direct_search::{DirectRegex, DirectScanner};
use crate::fs::{CapabilityFs, GitVcs};
use crate::indexing::{IndexingCoordinator, InvalidationCoordinator, InvalidationScope};
use crate::line_index::Position;
use crate::model::node::Node;
use crate::model::provenance::Revision;
use crate::packs::default_packs;
use crate::ports::fs::FileSnapshot;
use crate::ports::pack::LanguagePack;
use crate::ports::store::VersionRecords;
use crate::ports::store::{Store, StoreError, UpdateSummary};
use crate::ports::vcs::Vcs;
use crate::protocol::envelope::{ResultEnvelope, SourceKind};
use crate::protocol::evidence::Evidence;
use crate::protocol::freshness::{CoverageState, Freshness, GraphState, LexicalState};
use crate::retrieval::ranking::{DeterministicRanker, RankReason, RankedCandidate};
use crate::retrieval::traversal::{GraphTraversal, ImpactData, NeighborsData, PathTraceData};
use crate::retrieval::{HybridRetriever, LexicalHit, LexicalSource};
use crate::store::SqliteStore;
use std::collections::{BTreeMap, BTreeSet, HashMap};
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
    store_error: Option<StoreError>,
    packs: Vec<Box<dyn LanguagePack>>,
    config: RepinConfig,
}

#[derive(Default)]
struct VersionInvalidationResult {
    targeted_files: BTreeSet<(String, String)>,
    full_rebuild: bool,
    had_scopes: bool,
}

impl Runtime {
    pub fn open(options: RuntimeOptions) -> Result<Self, String> {
        Self::open_with_config_and_exclusions(options, RepinConfig::default(), &[])
    }

    pub fn open_with_config(options: RuntimeOptions, config: RepinConfig) -> Result<Self, String> {
        Self::open_with_config_and_exclusions(options, config, &[])
    }

    pub fn open_with_config_and_exclusions(
        options: RuntimeOptions,
        config: RepinConfig,
        additional_exclusions: &[String],
    ) -> Result<Self, String> {
        let filter = crate::fs::ExclusionFilter::with_config_and_exclusions(
            &config.indexing,
            additional_exclusions,
        );

        let fs = CapabilityFs::open_with_filter(&options.root_id, &options.root_path, filter)
            .map_err(|error| format!("failed to open root filesystem: {error}"))?;
        let (store, store_error) = if let Some(ref db_path) = options.db_path {
            match SqliteStore::open(db_path) {
                Ok(store) => (Some(store), None),
                Err(error) => {
                    tracing::warn!(
                        path = %db_path.display(),
                        error = %error,
                        "sqlite store unavailable; retaining direct retrieval"
                    );
                    (None, Some(error))
                }
            }
        } else {
            (None, None)
        };
        Ok(Self {
            options,
            fs,
            store,
            store_error,
            packs: default_packs(),
            config,
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
            store_error: None,
            packs: default_packs(),
            config: RepinConfig::default(),
        })
    }

    pub fn options(&self) -> &RuntimeOptions {
        &self.options
    }

    pub fn store(&self) -> Option<&SqliteStore> {
        self.store.as_ref()
    }

    pub fn store_error(&self) -> Option<&StoreError> {
        self.store_error.as_ref()
    }

    pub fn config(&self) -> &RepinConfig {
        &self.config
    }

    pub fn pending_version_invalidations(&self) -> Vec<InvalidationScope> {
        let Some(store) = self.store.as_ref() else {
            return Vec::new();
        };
        let Ok(view) = store.read_view() else {
            return Vec::new();
        };
        let Some(stored) = view.version_records().ok().flatten() else {
            return Vec::new();
        };
        let current = self.current_version_records();
        IndexingCoordinator::plan_version_invalidation(&stored, &current)
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
                envelope.status = crate::protocol::envelope::Status::Invalid;
                envelope.warnings.push(crate::protocol::envelope::Warning {
                    code: crate::protocol::errors::ErrorCode::InvalidQuery,
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
            return unavailable_graph(self.store_error.as_ref());
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
            envelope.status = crate::protocol::envelope::Status::Unavailable;
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
        let records = self.current_version_records();
        InvalidationCoordinator::apply_snapshot_update_with_records(
            store,
            &self.packs,
            snapshot,
            Some(&records),
        )
    }

    pub fn remove_file(&self, path: &str) -> Result<UpdateSummary, StoreError> {
        let Some(store) = self.store.as_ref() else {
            return Err(StoreError::Io("store not available".to_string()));
        };
        let records = self.current_version_records();
        let summary = InvalidationCoordinator::apply_file_removal(
            store,
            &self.options.root_id,
            path,
            Some(&records),
        )?;
        let _ = store.checkpoint();
        Ok(summary)
    }

    pub fn update_file(&self, path: &str) -> Result<Option<UpdateSummary>, StoreError> {
        let Some(store) = self.store.as_ref() else {
            return Err(StoreError::Io("store not available".to_string()));
        };
        match self.fs.read_snapshot(path) {
            Ok(snapshot) => {
                let summary = self.update_snapshot(&snapshot)?;
                let _ = store.checkpoint();
                Ok(Some(summary))
            }
            Err(_) => {
                let summary = self.remove_file(path)?;
                Ok(Some(summary))
            }
        }
    }

    pub fn search_hybrid(
        &self,
        query: &str,
        max_results: usize,
        centrality_boost: Option<f64>,
    ) -> ResultEnvelope<Vec<RankedCandidate>> {
        let Some(store) = self.store.as_ref() else {
            return unavailable_graph(self.store_error.as_ref());
        };
        let Ok(view) = store.read_view() else {
            return ResultEnvelope::not_found(Vec::new());
        };
        let lexical = SqliteLexical { store };
        let ranked = HybridRetriever::search(
            &*view,
            Some(&lexical),
            query,
            max_results,
            None,
            centrality_boost,
        )
        .candidates;
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
        top_n: Option<usize>,
        deadline_ms: Option<u64>,
    ) -> ResultEnvelope<Vec<RankedCandidate>> {
        let Some(store) = self.store.as_ref() else {
            let mut envelope = ResultEnvelope::not_found(Vec::new());
            envelope.status = crate::protocol::envelope::Status::Unavailable;
            return envelope;
        };
        let Ok(view) = store.read_view() else {
            return ResultEnvelope::not_found(Vec::new());
        };
        let seed_limit = top_n.unwrap_or(20);
        let ranked = if candidates.is_empty() {
            self.search_hybrid(query, seed_limit, None).data
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
        let final_ranked = match AgentReranker::rerank_with_shell_callback(
            query,
            ranked.clone(),
            agent_cmd,
            deadline_ms,
        ) {
            Ok(reordered) => reordered,
            Err(error) => {
                let mut envelope = ResultEnvelope::ok(ranked);
                envelope.provenance.sources.push(SourceKind::Graph);
                envelope.warnings.push(crate::protocol::envelope::Warning {
                    code: crate::protocol::errors::ErrorCode::CapabilityUnavailable,
                    message: format!("Agent reranker callback failed: {error}"),
                    detail: None,
                });
                return envelope;
            }
        };
        let truncated = if let Some(top_n) = top_n {
            final_ranked[..final_ranked.len().min(top_n)].to_vec()
        } else {
            final_ranked
        };
        let mut envelope = ResultEnvelope::ok(truncated);
        envelope.provenance.sources.push(SourceKind::Graph);
        envelope
    }

    pub fn rerank_with_model(
        &self,
        query: &str,
        candidates: &[String],
        reranker: &dyn crate::ports::model::Reranker,
    ) -> ResultEnvelope<Vec<RankedCandidate>> {
        let Some(store) = self.store.as_ref() else {
            let mut envelope = ResultEnvelope::not_found(Vec::new());
            envelope.status = crate::protocol::envelope::Status::Unavailable;
            return envelope;
        };
        let Ok(view) = store.read_view() else {
            return ResultEnvelope::not_found(Vec::new());
        };
        let ranked = if candidates.is_empty() {
            self.search_hybrid(query, 20, None).data
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
                crate::ports::model::RerankCandidate {
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
                envelope.warnings.push(crate::protocol::envelope::Warning {
                    code: crate::protocol::errors::ErrorCode::CapabilityUnavailable,
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
            envelope.status = crate::protocol::envelope::Status::Unavailable;
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
            envelope.status = crate::protocol::envelope::Status::Unavailable;
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

    pub fn lookup_impact(
        &self,
        name_or_id: &str,
        max_depth: usize,
    ) -> ResultEnvelope<Option<ImpactData>> {
        let Some(store) = self.store.as_ref() else {
            let mut envelope = ResultEnvelope::not_found(None);
            envelope.status = crate::protocol::envelope::Status::Unavailable;
            return envelope;
        };
        let Ok(view) = store.read_view() else {
            return ResultEnvelope::not_found(None);
        };
        let data = GraphTraversal::lookup_impact(&*view, name_or_id, max_depth);
        let mut envelope = ResultEnvelope::ok(data);
        envelope.provenance.sources.push(SourceKind::Graph);
        envelope
    }

    pub fn trace_paths(
        &self,
        from: &str,
        to: &str,
        max_depth: usize,
    ) -> ResultEnvelope<Option<PathTraceData>> {
        let Some(store) = self.store.as_ref() else {
            let mut envelope = ResultEnvelope::not_found(None);
            envelope.status = crate::protocol::envelope::Status::Unavailable;
            return envelope;
        };
        let Ok(view) = store.read_view() else {
            return ResultEnvelope::not_found(None);
        };
        let data = GraphTraversal::lookup_paths(&*view, from, to, max_depth);
        let mut envelope = ResultEnvelope::ok(data);
        envelope.provenance.sources.push(SourceKind::Graph);
        envelope
    }

    pub fn assemble_context(
        &self,
        query: &str,
        budget_bytes: usize,
        context_override: Option<crate::config::ContextConfig>,
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
        let config = context_override.unwrap_or(crate::config::ContextConfig {
            default_token_budget: budget_bytes / 4,
            padding_lines: 0,
            include_blast_radius: true,
            include_verbatim_source: true,
        });
        let assembled = ContextBuilder::assemble_neighborhood_with_config(
            &*view,
            Some(&self.fs),
            &primary_nodes,
            &config,
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
        let records = self.current_version_records();
        let invalidation = self.apply_pending_version_invalidations(store, &records)?;
        if invalidation.full_rebuild {
            let report = IndexingCoordinator::rebuild_source_with_records(
                store,
                &self.fs,
                &self.packs,
                &records,
            )
            .map_err(|error| format!("rebuild error: {error}"))?;
            IndexingCoordinator::resolve_existing(store, &records)
                .map_err(|error| format!("resolution error: {error}"))?;
            return Ok(report.files_indexed);
        }
        if invalidation.had_scopes {
            let mut files_indexed = 0;
            for (root, path) in invalidation.targeted_files {
                if root == self.options.root_id
                    && let Ok(snapshot) = self.fs.read_snapshot(&path)
                {
                    IndexingCoordinator::apply_snapshot_update_with_records(
                        store,
                        &self.packs,
                        &snapshot,
                        Some(&records),
                    )
                    .map_err(|error| format!("indexing error: {error}"))?;
                    files_indexed += 1;
                }
            }
            IndexingCoordinator::resolve_existing(store, &records)
                .map_err(|error| format!("resolution error: {error}"))?;
            let _ = store.checkpoint();
            return Ok(files_indexed);
        }
        let report = IndexingCoordinator::index_source_with_records(
            store,
            &self.fs,
            &self.packs,
            Some(&records),
        )
        .map_err(|error| format!("indexing error: {error}"))?;
        IndexingCoordinator::resolve_existing(store, &records)
            .map_err(|error| format!("resolution error: {error}"))?;
        if let Some(store) = self.store.as_ref() {
            let _ = store.checkpoint();
        }
        Ok(report.files_indexed)
    }

    fn apply_pending_version_invalidations(
        &self,
        store: &SqliteStore,
        current: &VersionRecords,
    ) -> Result<VersionInvalidationResult, String> {
        let view = store.read_view().map_err(|e| e.to_string())?;
        let Some(stored) = view.version_records().map_err(|e| e.to_string())? else {
            return Ok(VersionInvalidationResult::default());
        };
        let scopes = IndexingCoordinator::plan_version_invalidation(&stored, current);
        let mut result = VersionInvalidationResult {
            had_scopes: !scopes.is_empty(),
            ..VersionInvalidationResult::default()
        };
        for scope in scopes {
            match scope {
                InvalidationScope::Classification => {
                    let files = view.files().map_err(|e| e.to_string())?;
                    IndexingCoordinator::reclassify_files(
                        store,
                        &files,
                        |node| Some(crate::fs::classify_artifact(&node.path)),
                        current,
                    )
                    .map_err(|e| e.to_string())?;
                }
                InvalidationScope::Resolution => {
                    IndexingCoordinator::invalidate_resolution(
                        store,
                        &stored.resolution_version.to_string(),
                        current,
                    )
                    .map_err(|e| e.to_string())?;
                }
                InvalidationScope::Pack {
                    name,
                    previous_version: Some(prev_ver),
                } => {
                    for owner in view
                        .owners_by_producer(&name, Some(&prev_ver))
                        .map_err(|e| e.to_string())?
                    {
                        result
                            .targeted_files
                            .insert((owner.root.clone(), owner.path.clone()));
                    }
                    IndexingCoordinator::invalidate_language_pack(store, &name, &prev_ver, current)
                        .map_err(|e| e.to_string())?;
                }
                InvalidationScope::Extractor {
                    name,
                    previous_version: Some(prev_ver),
                } => {
                    for owner in view
                        .owners_by_producer(&name, Some(&prev_ver))
                        .map_err(|e| e.to_string())?
                    {
                        result
                            .targeted_files
                            .insert((owner.root.clone(), owner.path.clone()));
                    }
                    IndexingCoordinator::invalidate_extractor(store, &name, &prev_ver, current)
                        .map_err(|e| e.to_string())?;
                }
                InvalidationScope::KindRegistry | InvalidationScope::AttributeRegistry => {
                    result.full_rebuild = true;
                    IndexingCoordinator::invalidate_all_claims(store, current)
                        .map_err(|e| e.to_string())?;
                }
                InvalidationScope::Pack { .. } | InvalidationScope::Extractor { .. } => {}
            }
        }
        Ok(result)
    }

    /// Execute the public recovery target. Graph/all use the idempotent,
    /// version-aware source coordinator; derived-index targets remain
    /// explicit until their concrete adapters are available.
    pub fn rebuild(&self, target: crate::protocol::ipc::RebuildTarget) -> Result<usize, String> {
        match target {
            crate::protocol::ipc::RebuildTarget::Graph
            | crate::protocol::ipc::RebuildTarget::All => {
                let store = self
                    .store
                    .as_ref()
                    .ok_or_else(|| "rebuild error: store not available".to_string())?;
                let records = self.current_version_records();
                let report = IndexingCoordinator::rebuild_source_with_records(
                    store,
                    &self.fs,
                    &self.packs,
                    &records,
                )
                .map_err(|e| format!("rebuild error: {e}"))?;
                IndexingCoordinator::resolve_existing(store, &records)
                    .map_err(|e| format!("resolution error: {e}"))?;
                Ok(report.files_indexed)
            }
            crate::protocol::ipc::RebuildTarget::Lexical => self
                .store
                .as_ref()
                .ok_or_else(|| "lexical rebuild error: store not available".to_string())?
                .rebuild_lexical()
                .map(|_| 0)
                .map_err(|e| format!("lexical rebuild error: {e}")),
            crate::protocol::ipc::RebuildTarget::Vector => {
                Err("vector rebuild is unavailable: no vector adapter is configured".to_string())
            }
        }
    }

    fn current_version_records(&self) -> VersionRecords {
        let mut pack_versions = BTreeMap::new();
        let mut extractor_versions = BTreeMap::new();
        for pack in &self.packs {
            pack_versions.insert(pack.name().to_string(), pack.version().to_string());
            extractor_versions.insert(pack.name().to_string(), pack.version().to_string());
        }
        VersionRecords {
            store_schema_version: crate::store::STORE_SCHEMA_VERSION,
            kind_registry_version: crate::versions::KIND_REGISTRY_VERSION,
            attribute_registry_version: crate::versions::ATTRIBUTE_REGISTRY_VERSION,
            classification_version: crate::versions::CLASSIFICATION_VERSION,
            resolution_version: crate::versions::RESOLUTION_VERSION,
            pack_versions,
            extractor_versions,
            engine_version: env!("CARGO_PKG_VERSION").to_string(),
            vcs_revision: option_env!("REPIN_GIT_COMMIT").map(str::to_owned),
            observed_dirty_set: None,
        }
    }
}

fn graph_freshness(view: &dyn crate::ports::store::ReadView) -> Freshness {
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

fn unavailable_graph(error: Option<&StoreError>) -> ResultEnvelope<Vec<RankedCandidate>> {
    let mut envelope = ResultEnvelope::not_found(Vec::new());
    envelope.status = crate::protocol::envelope::Status::Unavailable;
    let code = match error {
        Some(StoreError::SchemaVersionMismatch { found, supported }) if found > supported => {
            crate::protocol::errors::ErrorCode::ProjectStateNewer
        }
        Some(_) => crate::protocol::errors::ErrorCode::ProjectStateInvalid,
        None => crate::protocol::errors::ErrorCode::CapabilityUnavailable,
    };
    let message = error
        .map(ToString::to_string)
        .unwrap_or_else(|| "graph store not configured or available".to_string());
    envelope.warnings.push(crate::protocol::envelope::Warning {
        code,
        message,
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
    envelope.status = crate::protocol::envelope::Status::Unavailable;
    envelope
}

fn unavailable_outline(root: &str, path: &str) -> ResultEnvelope<FileOutline> {
    let mut envelope = ResultEnvelope::not_found(FileOutline {
        root: root.to_string(),
        path: path.to_string(),
        symbols: Vec::new(),
    });
    envelope.status = crate::protocol::envelope::Status::Unavailable;
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
