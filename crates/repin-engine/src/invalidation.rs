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

pub struct InvalidationCoordinator;

impl InvalidationCoordinator {
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
            let base_rev = view.revision()?;

            // 1. Prepare phase outside write transaction
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

            // 2. Begin store transaction
            let mut tx = store.begin_write()?;
            if let Err(e) = tx.expect_revision(base_rev) {
                let _ = tx.rollback();
                if attempts < Self::MAX_COMMIT_ATTEMPTS {
                    continue;
                } else {
                    return Err(e);
                }
            }

            // Remove old facts owned by this file
            tx.remove_by_file(&snapshot.root, &snapshot.path)?;

            // Insert new facts
            let nodes_added = facts.nodes.len();
            let edges_added = facts.edges.len();
            tx.put_nodes(&facts.nodes)?;
            tx.put_edges(&facts.edges)?;
            tx.put_unresolved(&facts.unresolved)?;
            tx.put_skips(&facts.skips)?;
            tx.put_diagnostics(&facts.diagnostics)?;

            // 3. Promote unresolved references
            let mut unresolved_promoted = 0;
            for claim in &facts.nodes {
                let newly_defined_name = &claim.node.name;
                let pending = view.unresolved_seeking(newly_defined_name)?;

                let mut to_remove_keys = Vec::new();
                let mut promoted_edges = Vec::new();

                for u in pending {
                    // Create resolved edge from referencing node to newly defined node
                    let edge_id = EdgeId::new(u.from, claim.node.id, u.edge_kind, "promoter");
                    let edge = Edge {
                        id: edge_id,
                        from: u.from,
                        to: claim.node.id,
                        kind: u.edge_kind,
                        provenance: Provenance {
                            root: u.provenance.root.clone(),
                            path: u.provenance.path.clone(),
                            range: None,
                            extractor: "promoter".to_string(),
                            extractor_version: "1.0".to_string(),
                            derivation: Derivation::Resolved,
                            confidence: Confidence::EXACT,
                            revision: base_rev.next(),
                        },
                        attributes: Default::default(),
                    };

                    promoted_edges.push(repin_core::model::edge::EdgeClaim {
                        edge,
                        owner: FactOwner::new(
                            &u.provenance.root,
                            &u.provenance.path,
                            "promoter",
                            "1.0",
                        ),
                    });

                    to_remove_keys.push(UnresolvedKey {
                        from: u.from,
                        seeking: u.seeking.clone(),
                        edge_kind: u.edge_kind,
                    });
                }

                if !promoted_edges.is_empty() {
                    unresolved_promoted += promoted_edges.len();
                    tx.put_edges(&promoted_edges)?;
                    tx.remove_unresolved(&to_remove_keys)?;
                }
            }

            let next_rev = base_rev.next();
            tx.set_revision(next_rev)?;

            let summary = UpdateSummary {
                revision: next_rev,
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

            tx.put_update_summary(&summary)?;
            tx.commit()?;

            return Ok(summary);
        }

        Err(StoreError::RevisionConflict {
            expected: Revision::INITIAL,
            actual: Revision::INITIAL,
        })
    }
}
