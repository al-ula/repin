use repin_core::model::edge::Edge;
use repin_core::model::identity::EdgeId;
use repin_core::model::provenance::{Confidence, Derivation, FactOwner, Provenance, Revision};
use repin_core::model::unresolved::UnresolvedKey;
use repin_core::ports::fs::FileSnapshot;
use repin_core::ports::pack::LanguagePack;
use repin_core::ports::store::{Store, StoreError, UpdateSummary};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlastRadius {
    Local,
    Dependency,
    Global,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct IndexingCoordinator;

impl IndexingCoordinator {
    pub const MAX_COMMIT_ATTEMPTS: usize = 2;

    pub fn classify_blast_radius(path: &str) -> BlastRadius {
        if path.ends_with("Cargo.toml")
            || path.ends_with("package.json")
            || path.ends_with("tsconfig.json")
        {
            BlastRadius::Global
        } else if path.ends_with("mod.rs") || path.ends_with("index.ts") || path.ends_with("lib.rs")
        {
            BlastRadius::Dependency
        } else {
            BlastRadius::Local
        }
    }

    pub fn apply_snapshot_update(
        store: &dyn Store,
        packs: &[Box<dyn LanguagePack>],
        snapshot: &FileSnapshot,
    ) -> Result<UpdateSummary, StoreError> {
        let mut attempts = 0;

        while attempts < Self::MAX_COMMIT_ATTEMPTS {
            attempts += 1;
            let view = store.read_view()?;
            let base_revision = view.revision()?;

            let mut extracted = None;
            for pack in packs {
                if pack.can_handle(&snapshot.path, &snapshot.content)
                    && let Ok(facts) = pack.extract(snapshot)
                {
                    extracted = Some(facts);
                    break;
                }
            }
            let facts = extracted.unwrap_or_default();

            let mut transaction = store.begin_write()?;
            if let Err(error) = transaction.expect_revision(base_revision) {
                let _ = transaction.rollback();
                if attempts < Self::MAX_COMMIT_ATTEMPTS {
                    continue;
                }
                return Err(error);
            }

            transaction.remove_by_file(&snapshot.root, &snapshot.path)?;
            let nodes_added = facts.nodes.len();
            let edges_added = facts.edges.len();
            transaction.put_nodes(&facts.nodes)?;
            transaction.put_edges(&facts.edges)?;
            transaction.put_unresolved(&facts.unresolved)?;
            transaction.put_skips(&facts.skips)?;
            transaction.put_diagnostics(&facts.diagnostics)?;

            let mut unresolved_promoted = 0;
            for claim in &facts.nodes {
                let pending = view.unresolved_seeking(&claim.node.name)?;
                let mut to_remove_keys = Vec::new();
                let mut promoted_edges = Vec::new();

                for unresolved in pending {
                    let edge = Edge {
                        id: EdgeId::new(
                            unresolved.from,
                            claim.node.id,
                            unresolved.edge_kind,
                            "promoter",
                        ),
                        from: unresolved.from,
                        to: claim.node.id,
                        kind: unresolved.edge_kind,
                        provenance: Provenance {
                            root: unresolved.provenance.root.clone(),
                            path: unresolved.provenance.path.clone(),
                            range: None,
                            extractor: "promoter".to_string(),
                            extractor_version: "1.0".to_string(),
                            derivation: Derivation::Resolved,
                            confidence: Confidence::EXACT,
                            revision: base_revision.next(),
                        },
                        attributes: Default::default(),
                    };
                    promoted_edges.push(repin_core::model::edge::EdgeClaim {
                        edge,
                        owner: FactOwner::new(
                            &unresolved.provenance.root,
                            &unresolved.provenance.path,
                            "promoter",
                            "1.0",
                        ),
                    });
                    to_remove_keys.push(UnresolvedKey {
                        from: unresolved.from,
                        seeking: unresolved.seeking,
                        edge_kind: unresolved.edge_kind,
                    });
                }

                if !promoted_edges.is_empty() {
                    unresolved_promoted += promoted_edges.len();
                    transaction.put_edges(&promoted_edges)?;
                    transaction.remove_unresolved(&to_remove_keys)?;
                }
            }

            let next_revision = base_revision.next();
            transaction.set_revision(next_revision)?;
            let summary = UpdateSummary {
                revision: next_revision,
                files_added: 1,
                files_modified: 0,
                files_deleted: 0,
                nodes_added,
                nodes_removed: 0,
                edges_added,
                edges_removed: 0,
                unresolved_promoted,
                unresolved_demoted: 0,
            };
            transaction.put_update_summary(&summary)?;
            transaction.commit()?;
            return Ok(summary);
        }

        Err(StoreError::RevisionConflict {
            expected: Revision::INITIAL,
            actual: Revision::INITIAL,
        })
    }
}
