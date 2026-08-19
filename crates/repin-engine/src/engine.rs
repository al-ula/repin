use crate::context::{AssembledContext, ContextBuilder};
use crate::inspect::{FileOutline, Inspector};
use crate::invalidation::InvalidationCoordinator;
use crate::ranking::{DeterministicRanker, RankedCandidate};
use repin_core::line_index::Position;
use repin_core::model::node::Node;
use repin_core::model::provenance::Revision;
use repin_core::ports::fs::FileSnapshot;
use repin_core::ports::pack::LanguagePack;
use repin_core::ports::store::{Store, StoreError, UpdateSummary};
use repin_direct_search::{DirectRegex, DirectScanner};
use repin_fs::CapabilityFs;
use repin_packs::default_packs;
use repin_protocol::envelope::{ResultEnvelope, SourceKind};
use repin_protocol::evidence::Evidence;
use repin_protocol::freshness::{CoverageState, Freshness, GraphState, LexicalState};
use repin_store_sqlite::SqliteStore;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct EngineOptions {
    pub root_id: String,
    pub root_path: PathBuf,
    pub db_path: Option<PathBuf>,
}

pub struct Engine {
    options: EngineOptions,
    fs: CapabilityFs,
    store: Option<SqliteStore>,
    packs: Vec<Box<dyn LanguagePack>>,
}

impl Engine {
    pub fn open(options: EngineOptions) -> Result<Self, String> {
        let fs = CapabilityFs::open(&options.root_id, &options.root_path)
            .map_err(|e| format!("failed to open root filesystem: {e}"))?;

        let store = if let Some(ref db_p) = options.db_path {
            if let Some(parent) = db_p.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            Some(SqliteStore::open(db_p).map_err(|e| format!("failed to open sqlite store: {e}"))?)
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
        let root_path_buf = root_path.as_ref().to_path_buf();
        let fs = CapabilityFs::open("root", &root_path_buf)
            .map_err(|e| format!("failed to open filesystem: {e}"))?;
        let store = SqliteStore::open_in_memory()
            .map_err(|e| format!("failed to open in-memory store: {e}"))?;

        Ok(Self {
            options: EngineOptions {
                root_id: "root".to_string(),
                root_path: root_path_buf,
                db_path: None,
            },
            fs,
            store: Some(store),
            packs: default_packs(),
        })
    }

    pub fn options(&self) -> &EngineOptions {
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
            Ok(r) => r,
            Err(e) => {
                let mut env = ResultEnvelope::not_found(Vec::new());
                env.status = repin_protocol::envelope::Status::Invalid;
                env.warnings.push(repin_protocol::envelope::Warning {
                    code: repin_protocol::errors::ErrorCode::InvalidQuery,
                    message: e.to_string(),
                    detail: None,
                });
                return env;
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

        let mut env = ResultEnvelope::ok(all_evidence.clone());
        env.evidence = all_evidence;
        env.provenance.sources.push(SourceKind::WorkingTree);
        env.freshness = Freshness {
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
        env
    }

    pub fn search_graph(
        &self,
        query: &str,
        max_results: usize,
    ) -> ResultEnvelope<Vec<RankedCandidate>> {
        let Some(ref store) = self.store else {
            let mut env = ResultEnvelope::not_found(Vec::new());
            env.status = repin_protocol::envelope::Status::Unavailable;
            env.warnings.push(repin_protocol::envelope::Warning {
                code: repin_protocol::errors::ErrorCode::CapabilityUnavailable,
                message: "graph store not configured or available".to_string(),
                detail: None,
            });
            return env;
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

        let mut ranked = DeterministicRanker::rank(query, candidate_nodes);
        ranked.truncate(max_results);

        let mut env = ResultEnvelope::ok(ranked);
        env.provenance.sources.push(SourceKind::Graph);
        env.freshness = Freshness {
            observed_at: None,
            graph_revision: view.revision().ok(),
            graph_state: GraphState::Current,
            lexical_revision: view.revision().ok(),
            lexical_state: LexicalState::Current,
            coverage: CoverageState::Complete,
        };
        env
    }

    pub fn inspect_file(&self, relative_path: &str) -> ResultEnvelope<FileOutline> {
        let Some(ref store) = self.store else {
            let mut env = ResultEnvelope::not_found(FileOutline {
                root: self.options.root_id.clone(),
                path: relative_path.to_string(),
                symbols: Vec::new(),
            });
            env.status = repin_protocol::envelope::Status::Unavailable;
            return env;
        };

        let Ok(view) = store.read_view() else {
            return ResultEnvelope::not_found(FileOutline {
                root: self.options.root_id.clone(),
                path: relative_path.to_string(),
                symbols: Vec::new(),
            });
        };

        let outline = Inspector::inspect_file(&*view, &self.options.root_id, relative_path);
        let mut env = ResultEnvelope::ok(outline);
        env.provenance.sources.push(SourceKind::Graph);
        env
    }

    pub fn at_position(&self, relative_path: &str, pos: Position) -> ResultEnvelope<Option<Node>> {
        let Some(ref store) = self.store else {
            let mut env = ResultEnvelope::not_found(None);
            env.status = repin_protocol::envelope::Status::Unavailable;
            return env;
        };

        let Ok(view) = store.read_view() else {
            return ResultEnvelope::not_found(None);
        };

        let node = Inspector::at_position(&*view, &self.options.root_id, relative_path, pos);
        let mut env = ResultEnvelope::ok(node);
        env.provenance.sources.push(SourceKind::Graph);
        env
    }

    pub fn review_context(
        &self,
        _changed_since: Option<Revision>,
        budget_bytes: usize,
    ) -> ResultEnvelope<AssembledContext> {
        let Some(ref store) = self.store else {
            let mut env = ResultEnvelope::not_found(AssembledContext {
                snippets: Vec::new(),
                total_bytes: 0,
                truncated: false,
            });
            env.status = repin_protocol::envelope::Status::Unavailable;
            return env;
        };

        let Ok(view) = store.read_view() else {
            return ResultEnvelope::not_found(AssembledContext {
                snippets: Vec::new(),
                total_bytes: 0,
                truncated: false,
            });
        };

        let sample_nodes = view
            .nodes_by_name("main", &Default::default())
            .unwrap_or_default();
        let assembled = ContextBuilder::assemble_neighborhood(&*view, &sample_nodes, budget_bytes);

        let mut env = ResultEnvelope::ok(assembled);
        env.provenance.sources.push(SourceKind::Graph);
        env
    }

    pub fn update_snapshot(&self, snapshot: &FileSnapshot) -> Result<UpdateSummary, StoreError> {
        let Some(ref store) = self.store else {
            return Err(StoreError::Io("store not available".to_string()));
        };
        InvalidationCoordinator::apply_snapshot_update(store, &self.packs, snapshot)
    }

    pub fn index_all_worktree(&self) -> Result<usize, String> {
        let mut count = 0;
        self.fs
            .walk_files(|snapshot| {
                if self.update_snapshot(&snapshot).is_ok() {
                    count += 1;
                }
                Ok(())
            })
            .map_err(|e| format!("indexing error: {e}"))?;
        Ok(count)
    }
}
