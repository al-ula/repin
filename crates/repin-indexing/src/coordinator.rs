use repin_core::model::edge::Edge;
use repin_core::model::identity::EdgeId;
use repin_core::model::node::Node;
use repin_core::model::provenance::{Confidence, Derivation, FactOwner, Provenance, Revision};
use repin_core::model::registries::ArtifactClass;
use repin_core::model::unresolved::UnresolvedKey;
use repin_core::ports::fs::FileSnapshot;
use repin_core::ports::pack::LanguagePack;
use repin_core::ports::store::{
    NodeClassificationUpdate, Store, StoreError, UpdateSummary, VersionRecords,
};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlastRadius {
    Local,
    Dependency,
    Global,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidationScope {
    KindRegistry,
    AttributeRegistry,
    Classification,
    Resolution,
    Pack {
        name: String,
        previous_version: Option<String>,
    },
    Extractor {
        name: String,
        previous_version: Option<String>,
    },
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

    pub fn plan_version_invalidation(
        stored: &VersionRecords,
        current: &VersionRecords,
    ) -> Vec<InvalidationScope> {
        let mut scopes = Vec::new();
        if stored.kind_registry_version != current.kind_registry_version {
            scopes.push(InvalidationScope::KindRegistry);
        }
        if stored.attribute_registry_version != current.attribute_registry_version {
            scopes.push(InvalidationScope::AttributeRegistry);
        }
        if stored.classification_version != current.classification_version {
            scopes.push(InvalidationScope::Classification);
        }
        if stored.resolution_version != current.resolution_version {
            scopes.push(InvalidationScope::Resolution);
        }
        let pack_names: BTreeSet<_> = stored
            .pack_versions
            .keys()
            .chain(current.pack_versions.keys())
            .cloned()
            .collect();
        for name in pack_names {
            if stored.pack_versions.get(&name) != current.pack_versions.get(&name) {
                scopes.push(InvalidationScope::Pack {
                    previous_version: stored.pack_versions.get(&name).cloned(),
                    name,
                });
            }
        }
        let extractor_names: BTreeSet<_> = stored
            .extractor_versions
            .keys()
            .chain(current.extractor_versions.keys())
            .cloned()
            .collect();
        for name in extractor_names {
            if stored.extractor_versions.get(&name) != current.extractor_versions.get(&name) {
                scopes.push(InvalidationScope::Extractor {
                    previous_version: stored.extractor_versions.get(&name).cloned(),
                    name,
                });
            }
        }
        scopes
    }

    pub fn apply_snapshot_update(
        store: &dyn Store,
        packs: &[Box<dyn LanguagePack>],
        snapshot: &FileSnapshot,
    ) -> Result<UpdateSummary, StoreError> {
        Self::apply_snapshot_update_with_records(store, packs, snapshot, None)
    }

    pub fn apply_snapshot_update_with_records(
        store: &dyn Store,
        packs: &[Box<dyn LanguagePack>],
        snapshot: &FileSnapshot,
        version_records: Option<&VersionRecords>,
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

            if let Some(version_records) = version_records {
                transaction.put_version_records(version_records)?;
            }

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

    /// Remove every claim produced by one producer/version without reading source files.
    /// The caller supplies the replacement version record so metadata and removals
    /// become one authoritative revision.
    pub fn invalidate_producer(
        store: &dyn Store,
        producer: &str,
        producer_version: &str,
        replacement_records: &VersionRecords,
    ) -> Result<UpdateSummary, StoreError> {
        let view = store.read_view()?;
        let base_revision = view.revision()?;
        let owners = view.owners_by_producer(producer, Some(producer_version))?;
        let mut transaction = store.begin_write()?;
        transaction.expect_revision(base_revision)?;
        for owner in &owners {
            transaction.remove_claims(owner)?;
        }
        let next_revision = base_revision.next();
        transaction.put_version_records(replacement_records)?;
        transaction.set_revision(next_revision)?;
        let summary = UpdateSummary {
            revision: next_revision,
            files_added: 0,
            files_modified: 0,
            files_deleted: owners.len(),
            nodes_added: 0,
            nodes_removed: 0,
            edges_added: 0,
            edges_removed: 0,
            unresolved_promoted: 0,
            unresolved_demoted: 0,
        };
        transaction.put_update_summary(&summary)?;
        transaction.commit()?;
        Ok(summary)
    }

    pub fn invalidate_all_claims(
        store: &dyn Store,
        replacement_records: &VersionRecords,
    ) -> Result<UpdateSummary, StoreError> {
        let view = store.read_view()?;
        let base_revision = view.revision()?;
        let owners = view.all_owners()?;
        let mut transaction = store.begin_write()?;
        transaction.expect_revision(base_revision)?;
        for owner in &owners {
            transaction.remove_claims(owner)?;
        }
        let next_revision = base_revision.next();
        transaction.put_version_records(replacement_records)?;
        transaction.set_revision(next_revision)?;
        let summary = UpdateSummary {
            revision: next_revision,
            files_added: 0,
            files_modified: 0,
            files_deleted: owners.len(),
            nodes_added: 0,
            nodes_removed: 0,
            edges_added: 0,
            edges_removed: 0,
            unresolved_promoted: 0,
            unresolved_demoted: 0,
        };
        transaction.put_update_summary(&summary)?;
        transaction.commit()?;
        Ok(summary)
    }

    pub fn invalidate_language_pack(
        store: &dyn Store,
        pack_name: &str,
        previous_version: &str,
        replacement_records: &VersionRecords,
    ) -> Result<UpdateSummary, StoreError> {
        Self::invalidate_producer(store, pack_name, previous_version, replacement_records)
    }

    pub fn invalidate_extractor(
        store: &dyn Store,
        extractor_name: &str,
        previous_version: &str,
        replacement_records: &VersionRecords,
    ) -> Result<UpdateSummary, StoreError> {
        Self::invalidate_producer(store, extractor_name, previous_version, replacement_records)
    }

    pub fn invalidate_resolution(
        store: &dyn Store,
        _previous_resolution_version: &str,
        replacement_records: &VersionRecords,
    ) -> Result<UpdateSummary, StoreError> {
        let view = store.read_view()?;
        let base_revision = view.revision()?;
        let owners = view.resolution_owners()?;
        let mut transaction = store.begin_write()?;
        transaction.expect_revision(base_revision)?;
        for owner in &owners {
            transaction.remove_claims(owner)?;
        }
        let next_revision = base_revision.next();
        transaction.put_version_records(replacement_records)?;
        transaction.set_revision(next_revision)?;
        let summary = UpdateSummary {
            revision: next_revision,
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
        transaction.put_update_summary(&summary)?;
        transaction.commit()?;
        Ok(summary)
    }

    /// Re-resolve persisted unresolved references using only the graph
    /// snapshot. No source files are read.
    pub fn resolve_existing(
        store: &dyn Store,
        replacement_records: &VersionRecords,
    ) -> Result<UpdateSummary, StoreError> {
        let view = store.read_view()?;
        let base_revision = view.revision()?;
        let unresolved = view.unresolved_refs()?;
        if unresolved.is_empty() {
            return Ok(UpdateSummary {
                revision: base_revision,
                files_added: 0,
                files_modified: 0,
                files_deleted: 0,
                nodes_added: 0,
                nodes_removed: 0,
                edges_added: 0,
                edges_removed: 0,
                unresolved_promoted: 0,
                unresolved_demoted: 0,
            });
        }
        let mut promoted = Vec::new();
        let mut removed = Vec::new();
        let resolution_version = replacement_records.resolution_version.to_string();
        for reference in unresolved {
            let candidates: Vec<Node> = view
                .nodes_by_name(&reference.seeking, &Default::default())?
                .into_iter()
                .filter(|node| node.name == reference.seeking)
                .collect();
            for target in candidates {
                let edge = Edge {
                    id: EdgeId::new(reference.from, target.id, reference.edge_kind, "resolver"),
                    from: reference.from,
                    to: target.id,
                    kind: reference.edge_kind,
                    provenance: Provenance {
                        root: reference.provenance.root.clone(),
                        path: reference.provenance.path.clone(),
                        range: reference.provenance.range,
                        extractor: "resolver".to_string(),
                        extractor_version: resolution_version.clone(),
                        derivation: Derivation::Resolved,
                        confidence: Confidence::EXACT,
                        revision: base_revision.next(),
                    },
                    attributes: Default::default(),
                };
                promoted.push(repin_core::model::edge::EdgeClaim {
                    edge,
                    owner: FactOwner::new(
                        &reference.provenance.root,
                        &reference.provenance.path,
                        "resolver",
                        &resolution_version,
                    ),
                });
                removed.push(UnresolvedKey {
                    from: reference.from,
                    seeking: reference.seeking.clone(),
                    edge_kind: reference.edge_kind,
                });
            }
        }
        let mut transaction = store.begin_write()?;
        transaction.expect_revision(base_revision)?;
        transaction.put_edges(&promoted)?;
        transaction.remove_unresolved(&removed)?;
        let next_revision = base_revision.next();
        transaction.put_version_records(replacement_records)?;
        transaction.set_revision(next_revision)?;
        let summary = UpdateSummary {
            revision: next_revision,
            files_added: 0,
            files_modified: 0,
            files_deleted: 0,
            nodes_added: 0,
            nodes_removed: 0,
            edges_added: promoted.len(),
            edges_removed: 0,
            unresolved_promoted: removed.len(),
            unresolved_demoted: 0,
        };
        transaction.put_update_summary(&summary)?;
        transaction.commit()?;
        Ok(summary)
    }

    /// Reclassify persisted nodes from bounded stored facts. The classifier is
    /// intentionally source-independent; callers must not parse source here.
    pub fn reclassify_files<F>(
        store: &dyn Store,
        files: &[(String, String)],
        classify: F,
        replacement_records: &VersionRecords,
    ) -> Result<UpdateSummary, StoreError>
    where
        F: Fn(&repin_core::model::node::Node) -> Option<ArtifactClass>,
    {
        let view = store.read_view()?;
        let base_revision = view.revision()?;
        let mut updates = Vec::new();
        for (root, path) in files {
            for claim in view.node_claims_by_file(root, path)? {
                let next = classify(&claim.node);
                if next != claim.node.artifact_class {
                    updates.push(NodeClassificationUpdate {
                        node_id: claim.node.id,
                        owner: claim.owner,
                        artifact_class: next,
                    });
                }
            }
        }
        let mut transaction = store.begin_write()?;
        transaction.expect_revision(base_revision)?;
        transaction.update_node_classifications(&updates)?;
        transaction.put_version_records(replacement_records)?;
        let next_revision = base_revision.next();
        transaction.set_revision(next_revision)?;
        let summary = UpdateSummary {
            revision: next_revision,
            files_added: 0,
            files_modified: files.len(),
            files_deleted: 0,
            nodes_added: 0,
            nodes_removed: 0,
            edges_added: 0,
            edges_removed: 0,
            unresolved_promoted: 0,
            unresolved_demoted: 0,
        };
        transaction.put_update_summary(&summary)?;
        transaction.commit()?;
        Ok(summary)
    }
}
