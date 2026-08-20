//! Reusable snapshot-to-graph indexing orchestration.
//!
//! The coordinator consumes core source, language-pack, and store contracts;
//! it does not select concrete adapters or providers.

use repin_core::ports::fs::{SourceError, SourceFs};
use repin_core::ports::pack::LanguagePack;
use repin_core::ports::store::{Store, StoreError, UpdateSummary};

pub mod coordinator;

pub use coordinator::{BlastRadius, IndexingCoordinator};

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
        let mut report = IndexingReport::default();
        let mut callback = |snapshot| {
            let summary = Self::apply_snapshot_update(store, packs, &snapshot)
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
}

fn source_error_to_store(error: SourceError) -> StoreError {
    StoreError::Io(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use repin_fs::CapabilityFs;
    use repin_packs::default_packs;
    use repin_store_sqlite::SqliteStore;
    use std::fs;
    use tempfile::tempdir;

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
}
