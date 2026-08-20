//! Reusable snapshot-to-graph indexing orchestration.
//!
//! The coordinator consumes core source, language-pack, and store contracts;
//! it does not select concrete adapters or providers.

use repin_core::model::provenance::Revision;
use repin_core::ports::fs::{FileSnapshot, SourceError, SourceFs};
use repin_core::ports::pack::LanguagePack;
use repin_core::ports::store::{Store, StoreError, UpdateSummary, VersionRecords};

pub mod coordinator;

pub use coordinator::{BlastRadius, IndexingCoordinator, InvalidationScope};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct IndexingReport {
    pub files_indexed: usize,
    pub updates: Vec<UpdateSummary>,
}

impl IndexingCoordinator {
    /// Index every selected snapshot from a source using one coordinator.
    pub fn index_source(
        store: &dyn Store,
        source: &dyn SourceFs,
        packs: &[Box<dyn LanguagePack>],
    ) -> Result<IndexingReport, StoreError> {
        Self::index_source_with_records(store, source, packs, None)
    }

    pub fn index_source_with_records(
        store: &dyn Store,
        source: &dyn SourceFs,
        packs: &[Box<dyn LanguagePack>],
        version_records: Option<&VersionRecords>,
    ) -> Result<IndexingReport, StoreError> {
        let mut report = IndexingReport::default();
        let mut callback = |snapshot| {
            let summary =
                Self::apply_snapshot_update_with_records(store, packs, &snapshot, version_records)
                    .map_err(|error| SourceError::Other(error.to_string()))?;
            report.files_indexed += 1;
            report.updates.push(summary);
            Ok(())
        };
        source
            .walk_files(&mut callback)
            .map_err(source_error_to_store)?;
        Ok(report)
    }

    /// Rebuild the authoritative graph from a prepared snapshot set. Source
    /// reads and extraction happen before the write transaction; claim
    /// replacement, revision publication, and version records are atomic.
    pub fn rebuild_source_with_records(
        store: &dyn Store,
        source: &dyn SourceFs,
        packs: &[Box<dyn LanguagePack>],
        version_records: &VersionRecords,
    ) -> Result<IndexingReport, StoreError> {
        let mut snapshots = Vec::<FileSnapshot>::new();
        let mut collect = |snapshot| {
            snapshots.push(snapshot);
            Ok(())
        };
        source
            .walk_files(&mut collect)
            .map_err(source_error_to_store)?;

        let view = store.read_view()?;
        let base_revision = view.revision()?;
        let mut transaction = store.begin_write()?;
        transaction.expect_revision(base_revision)?;
        transaction.clear_graph()?;

        let mut report = IndexingReport::default();
        for snapshot in snapshots {
            let facts = if let Some(pack) = packs
                .iter()
                .find(|pack| pack.can_handle(&snapshot.path, &snapshot.content))
            {
                pack.extract(&snapshot)
                    .map_err(|error| StoreError::Io(error.to_string()))?
            } else {
                Default::default()
            };
            let summary = UpdateSummary {
                revision: Revision::INITIAL,
                files_added: 0,
                files_modified: 0,
                files_deleted: 0,
                nodes_added: facts.nodes.len(),
                nodes_removed: 0,
                edges_added: facts.edges.len(),
                edges_removed: 0,
                unresolved_promoted: 0,
                unresolved_demoted: 0,
            };
            transaction.put_nodes(&facts.nodes)?;
            transaction.put_edges(&facts.edges)?;
            transaction.put_unresolved(&facts.unresolved)?;
            transaction.put_skips(&facts.skips)?;
            transaction.put_diagnostics(&facts.diagnostics)?;
            report.files_indexed += 1;
            report.updates.push(summary);
        }

        let next_revision = base_revision.next();
        transaction.put_version_records(version_records)?;
        transaction.set_revision(next_revision)?;
        transaction.put_update_summary(&UpdateSummary {
            revision: next_revision,
            files_added: report.files_indexed,
            files_modified: 0,
            files_deleted: 0,
            nodes_added: report.updates.iter().map(|s| s.nodes_added).sum(),
            nodes_removed: 0,
            edges_added: report.updates.iter().map(|s| s.edges_added).sum(),
            edges_removed: 0,
            unresolved_promoted: 0,
            unresolved_demoted: 0,
        })?;
        transaction.commit()?;
        Ok(report)
    }
}

fn source_error_to_store(error: SourceError) -> StoreError {
    StoreError::Io(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use repin_core::ports::pack::{ExtractedFacts, ExtractionError, LanguagePack};
    use repin_fs::CapabilityFs;
    use repin_packs::default_packs;
    use repin_store_sqlite::SqliteStore;
    use std::collections::BTreeMap;
    use std::fs;
    use tempfile::tempdir;

    struct FailingPack;

    impl LanguagePack for FailingPack {
        fn name(&self) -> &'static str {
            "failing"
        }

        fn version(&self) -> &'static str {
            "1"
        }

        fn can_handle(&self, _path: &str, _sample_content: &[u8]) -> bool {
            true
        }

        fn extract(&self, _snapshot: &FileSnapshot) -> Result<ExtractedFacts, ExtractionError> {
            Err(ExtractionError::ParseFailure("injected failure".into()))
        }
    }

    #[test]
    fn index_source_composes_source_and_store_contracts() {
        let directory = tempdir().unwrap();
        fs::create_dir_all(directory.path().join("src")).unwrap();
        fs::write(
            directory.path().join("src/lib.rs"),
            "pub fn embedded_answer() -> u32 { 42 }\n",
        )
        .unwrap();

        let source = CapabilityFs::open("root", directory.path()).unwrap();
        let store = SqliteStore::open_in_memory().unwrap();
        let report = IndexingCoordinator::index_source(&store, &source, &default_packs()).unwrap();

        assert_eq!(report.files_indexed, 1);
        let view = store.read_view().unwrap();
        let nodes = view.nodes_by_file("root", "src/lib.rs").unwrap();
        assert!(nodes.iter().any(|node| node.name == "embedded_answer"));
    }

    #[test]
    fn rebuild_replaces_stale_claims_in_one_authoritative_revision() {
        let directory = tempdir().unwrap();
        fs::write(directory.path().join("lib.rs"), "pub fn old_name() {}\n").unwrap();
        let source = CapabilityFs::open("root", directory.path()).unwrap();
        let store = SqliteStore::open_in_memory().unwrap();
        let packs = default_packs();
        let records = VersionRecords {
            store_schema_version: 2,
            kind_registry_version: 1,
            attribute_registry_version: 1,
            classification_version: 1,
            resolution_version: 1,
            pack_versions: BTreeMap::new(),
            extractor_versions: BTreeMap::new(),
            engine_version: "0.1.0".into(),
            vcs_revision: None,
            observed_dirty_set: None,
        };
        IndexingCoordinator::rebuild_source_with_records(&store, &source, &packs, &records)
            .unwrap();
        fs::write(directory.path().join("lib.rs"), "pub fn new_name() {}\n").unwrap();
        IndexingCoordinator::rebuild_source_with_records(&store, &source, &packs, &records)
            .unwrap();
        let view = store.read_view().unwrap();
        let nodes = view.nodes_by_file("root", "lib.rs").unwrap();
        assert!(nodes.iter().any(|node| node.name == "new_name"));
        assert!(!nodes.iter().any(|node| node.name == "old_name"));
        assert_eq!(view.revision().unwrap().0, 2);
    }

    #[test]
    fn failed_rebuild_rolls_back_to_the_previous_authoritative_graph() {
        let directory = tempdir().unwrap();
        fs::write(directory.path().join("lib.rs"), "pub fn retained() {}\n").unwrap();
        let source = CapabilityFs::open("root", directory.path()).unwrap();
        let store = SqliteStore::open_in_memory().unwrap();
        let packs = default_packs();
        let records = VersionRecords {
            store_schema_version: 2,
            kind_registry_version: 1,
            attribute_registry_version: 1,
            classification_version: 1,
            resolution_version: 1,
            pack_versions: BTreeMap::new(),
            extractor_versions: BTreeMap::new(),
            engine_version: "0.1.0".into(),
            vcs_revision: None,
            observed_dirty_set: None,
        };
        IndexingCoordinator::rebuild_source_with_records(&store, &source, &packs, &records)
            .unwrap();
        let failing: Vec<Box<dyn LanguagePack>> = vec![Box::new(FailingPack)];
        assert!(
            IndexingCoordinator::rebuild_source_with_records(&store, &source, &failing, &records)
                .is_err()
        );
        let view = store.read_view().unwrap();
        assert!(
            view.nodes_by_file("root", "lib.rs")
                .unwrap()
                .iter()
                .any(|node| node.name == "retained")
        );
        assert_eq!(view.revision().unwrap().0, 1);
    }

    #[test]
    fn version_planner_scopes_pack_and_resolution_changes() {
        let stored = VersionRecords {
            store_schema_version: 1,
            kind_registry_version: 1,
            attribute_registry_version: 1,
            classification_version: 1,
            resolution_version: 1,
            pack_versions: BTreeMap::from([(String::from("rust"), String::from("1"))]),
            extractor_versions: BTreeMap::from([(String::from("rust"), String::from("1"))]),
            engine_version: String::from("0.1.0"),
            vcs_revision: None,
            observed_dirty_set: None,
        };
        let mut current = stored.clone();
        current.resolution_version = 2;
        current.pack_versions.insert("rust".into(), "2".into());
        let scopes = IndexingCoordinator::plan_version_invalidation(&stored, &current);
        assert!(scopes.contains(&InvalidationScope::Resolution));
        assert!(scopes.contains(&InvalidationScope::Pack {
            name: "rust".into(),
            previous_version: Some("1".into()),
        }));
    }

    #[test]
    fn producer_invalidation_removes_claims_and_commits_replacement_records() {
        let directory = tempdir().unwrap();
        fs::write(directory.path().join("lib.rs"), "pub fn remove_me() {}").unwrap();
        let source = CapabilityFs::open("root", directory.path()).unwrap();
        let store = SqliteStore::open_in_memory().unwrap();
        let packs = default_packs();
        let records = VersionRecords {
            store_schema_version: 1,
            kind_registry_version: 1,
            attribute_registry_version: 1,
            classification_version: 1,
            resolution_version: 1,
            pack_versions: BTreeMap::new(),
            extractor_versions: BTreeMap::new(),
            engine_version: "0.1.0".into(),
            vcs_revision: None,
            observed_dirty_set: None,
        };
        IndexingCoordinator::index_source_with_records(&store, &source, &packs, Some(&records))
            .unwrap();
        let replacement = VersionRecords {
            engine_version: "0.1.0".into(),
            ..records.clone()
        };
        IndexingCoordinator::invalidate_producer(&store, "rust_pack", "0.2.0", &replacement)
            .unwrap();
        let view = store.read_view().unwrap();
        assert!(view.nodes_by_file("root", "lib.rs").unwrap().is_empty());
        assert_eq!(view.version_records().unwrap(), Some(replacement));
    }

    #[test]
    fn classification_replacement_updates_persisted_claims_without_source_reads() {
        let directory = tempdir().unwrap();
        fs::write(directory.path().join("lib.rs"), "pub fn classify_me() {}").unwrap();
        let source = CapabilityFs::open("root", directory.path()).unwrap();
        let store = SqliteStore::open_in_memory().unwrap();
        let packs = default_packs();
        IndexingCoordinator::index_source(&store, &source, &packs).unwrap();
        let records = VersionRecords {
            store_schema_version: 2,
            kind_registry_version: 1,
            attribute_registry_version: 1,
            classification_version: 2,
            resolution_version: 1,
            pack_versions: BTreeMap::new(),
            extractor_versions: BTreeMap::new(),
            engine_version: "0.1.0".into(),
            vcs_revision: None,
            observed_dirty_set: None,
        };
        IndexingCoordinator::reclassify_files(
            &store,
            &[("root".into(), "lib.rs".into())],
            |_| Some(repin_core::model::registries::ArtifactClass::Docs),
            &records,
        )
        .unwrap();
        let view = store.read_view().unwrap();
        assert!(
            view.nodes_by_file("root", "lib.rs")
                .unwrap()
                .iter()
                .all(|node| node.artifact_class
                    == Some(repin_core::model::registries::ArtifactClass::Docs))
        );
    }
}
