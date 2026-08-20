pub mod fts5;
pub mod read_view;
pub mod schema;
pub mod store;
pub mod transaction;

pub use fts5::{Fts5Index, FtsHit};
pub use read_view::SqliteReadView;
pub use schema::SCHEMA_DDL;
pub use store::SqliteStore;
pub use transaction::SqliteTransaction;

#[cfg(test)]
mod tests {
    use super::*;
    use repin_core::model::identity::NodeId;
    use repin_core::model::node::{Node, NodeClaim};
    use repin_core::model::provenance::{Confidence, Derivation, FactOwner, Provenance, Revision};
    use repin_core::model::registries::NodeKind;
    use repin_core::ports::store::Store;

    #[test]
    fn test_sqlite_store_roundtrip() {
        let store = SqliteStore::open_in_memory().unwrap();
        let mut tx = store.begin_write().unwrap();

        let id = NodeId::new(NodeKind::Function, "root", "src/lib.rs", &[], "test_fn", 0);
        let node = Node {
            id,
            kind: NodeKind::Function,
            name: "test_fn".to_string(),
            qualified_name: Some("crate::test_fn".to_string()),
            root: "root".to_string(),
            path: "src/lib.rs".to_string(),
            range: None,
            language: Some("rust".to_string()),
            artifact_class: None,
            provenance: Provenance {
                root: "root".to_string(),
                path: "src/lib.rs".to_string(),
                range: None,
                extractor: "rust_pack".to_string(),
                extractor_version: "1.0".to_string(),
                derivation: Derivation::Extracted,
                confidence: Confidence::EXACT,
                revision: Revision::INITIAL,
            },
            attributes: Default::default(),
        };

        let claim = NodeClaim {
            node: node.clone(),
            owner: FactOwner::new("root", "src/lib.rs", "rust_pack", "1.0"),
        };

        tx.put_nodes(&[claim]).unwrap();
        tx.set_revision(Revision(1)).unwrap();
        tx.commit().unwrap();

        let view = store.read_view().unwrap();
        assert_eq!(view.revision().unwrap(), Revision(1));

        let retrieved = view.node(&id).unwrap().expect("node should exist");
        assert_eq!(retrieved.name, "test_fn");

        let fts_hits = store.search_fts("test_fn", 10).unwrap();
        assert_eq!(fts_hits.len(), 1);
        assert_eq!(fts_hits[0].node_id, id);
    }

    #[test]
    fn test_checkpoint_and_incoming_edge_count() {
        use repin_core::model::edge::{Edge, EdgeClaim};
        use repin_core::model::identity::EdgeId;
        use repin_core::model::registries::EdgeKind;

        let store = SqliteStore::open_in_memory().unwrap();
        let mut tx = store.begin_write().unwrap();

        let id_target = NodeId::new(NodeKind::Struct, "root", "src/lib.rs", &[], "Target", 0);
        let id_caller = NodeId::new(NodeKind::Function, "root", "src/caller.rs", &[], "caller", 0);

        let edge = Edge {
            id: EdgeId::new(id_caller, id_target, EdgeKind::Calls, "rust_pack"),
            from: id_caller,
            to: id_target,
            kind: EdgeKind::Calls,
            provenance: Provenance {
                root: "root".to_string(),
                path: "src/caller.rs".to_string(),
                range: None,
                extractor: "rust_pack".to_string(),
                extractor_version: "1.0".to_string(),
                derivation: Derivation::Extracted,
                confidence: Confidence::EXACT,
                revision: Revision::INITIAL,
            },
            attributes: Default::default(),
        };

        let edge_claim = EdgeClaim {
            edge,
            owner: FactOwner::new("root", "src/caller.rs", "rust_pack", "1.0"),
        };

        tx.put_edges(&[edge_claim]).unwrap();
        tx.commit().unwrap();

        // Check incoming edge count
        let view = store.read_view().unwrap();
        assert_eq!(view.incoming_edge_count(&id_target).unwrap(), 1);
        assert_eq!(view.incoming_edge_count(&id_caller).unwrap(), 0);

        // Checkpoint execution
        assert!(store.checkpoint().is_ok());
    }
}
