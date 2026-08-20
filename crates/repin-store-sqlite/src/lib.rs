pub mod fts5;
pub mod intern;
pub mod read_view;
pub mod schema;
pub mod store;
pub mod transaction;

pub use fts5::{Fts5Index, FtsHit};
pub use intern::InternerCache;
pub use read_view::SqliteReadView;
pub use schema::SCHEMA_DDL;
pub use store::SqliteStore;
pub use transaction::SqliteTransaction;

#[cfg(test)]
mod tests {
    use super::*;
    use repin_core::line_index::{ByteSpan, Position, Range};
    use repin_core::model::edge::{Edge, EdgeClaim, FactClaimKey};
    use repin_core::model::identity::{EdgeId, NodeId};
    use repin_core::model::node::{Attributes, Node, NodeClaim};
    use repin_core::model::provenance::{Confidence, Derivation, FactOwner, Provenance, Revision};
    use repin_core::model::registries::{ArtifactClass, EdgeKind, NodeKind};
    use repin_core::model::unresolved::UnresolvedRef;
    use repin_core::ports::fs::{Diagnostic, Skip};
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

    #[test]
    fn test_string_interning_and_fact_owners_dedup() {
        let store = SqliteStore::open_in_memory().unwrap();
        let mut tx = store.begin_write().unwrap();

        let owner = FactOwner::new("workspace_root", "src/main.rs", "rust_analyzer", "2026-08");

        let mut claims = Vec::new();
        for i in 0..10 {
            let name = format!("fn_{}", i);
            let id =
                NodeId::new(NodeKind::Function, "workspace_root", "src/main.rs", &[], &name, i);
            let node = Node {
                id,
                kind: NodeKind::Function,
                name: name.clone(),
                qualified_name: Some(format!("crate::{}", name)),
                root: "workspace_root".to_string(),
                path: "src/main.rs".to_string(),
                range: Some(Range {
                    span: ByteSpan::new((i * 10) as usize, (i * 10 + 9) as usize),
                    start: Position::new(i as u32, 0),
                    end: Position::new(i as u32, 9),
                }),
                language: Some("rust".to_string()),
                artifact_class: Some(ArtifactClass::Code),
                provenance: Provenance {
                    root: "workspace_root".to_string(),
                    path: "src/main.rs".to_string(),
                    range: Some(Range {
                        span: ByteSpan::new((i * 10) as usize, (i * 10 + 9) as usize),
                        start: Position::new(i as u32, 0),
                        end: Position::new(i as u32, 9),
                    }),
                    extractor: "rust_analyzer".to_string(),
                    extractor_version: "2026-08".to_string(),
                    derivation: Derivation::Extracted,
                    confidence: Confidence::EXACT,
                    revision: Revision::INITIAL,
                },
                attributes: Default::default(),
            };
            claims.push(NodeClaim {
                node,
                owner: owner.clone(),
            });
        }

        tx.put_nodes(&claims).unwrap();
        tx.commit().unwrap();

        let view = store.read_view().unwrap();
        let nodes = view.nodes_by_file("workspace_root", "src/main.rs").unwrap();
        assert_eq!(nodes.len(), 10);
        for (i, node) in nodes.iter().enumerate() {
            assert_eq!(node.name, format!("fn_{}", i));
            assert_eq!(node.root, "workspace_root");
            assert_eq!(node.path, "src/main.rs");
            assert_eq!(node.provenance.extractor, "rust_analyzer");
        }

        // Verify string_pool and fact_owners deduplication via raw SQL check on the store
        let conn_arc = store.raw_connection();
        let conn = conn_arc.lock().unwrap();
        let pool_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM string_pool", [], |r| r.get(0))
            .unwrap();
        let owner_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM fact_owners", [], |r| r.get(0))
            .unwrap();
        let node_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM node_claims", [], |r| r.get(0))
            .unwrap();

        // There are only 5 distinct strings ("workspace_root", "src/main.rs", "rust_analyzer", "2026-08", "rust")
        assert_eq!(pool_count, 5);
        // Exactly 1 fact_owners entry shared across all 10 nodes
        assert_eq!(owner_count, 1);
        assert_eq!(node_count, 10);
    }

    #[test]
    fn test_empty_and_custom_attributes_and_provenance() {
        let store = SqliteStore::open_in_memory().unwrap();
        let mut tx = store.begin_write().unwrap();

        let id1 = NodeId::new(NodeKind::Function, "root", "src/lib.rs", &[], "empty_attr_fn", 0);
        let node1 = Node {
            id: id1,
            kind: NodeKind::Function,
            name: "empty_attr_fn".to_string(),
            qualified_name: None,
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
            attributes: Attributes::new(),
        };

        let mut custom_attrs = Attributes::new();
        custom_attrs.insert("doc".to_string(), serde_json::json!("This is a complex doc comment"));
        custom_attrs.insert("visibility".to_string(), serde_json::json!("pub(crate)"));
        custom_attrs.insert("is_async".to_string(), serde_json::json!(true));

        let id2 = NodeId::new(NodeKind::Function, "root", "src/lib.rs", &[], "custom_attr_fn", 1);
        let node2 = Node {
            id: id2,
            kind: NodeKind::Function,
            name: "custom_attr_fn".to_string(),
            qualified_name: Some("crate::custom_attr_fn".to_string()),
            root: "root".to_string(),
            path: "src/lib.rs".to_string(),
            range: Some(Range {
                span: ByteSpan::new(100, 200),
                start: Position::new(10, 0),
                end: Position::new(20, 0),
            }),
            language: Some("rust".to_string()),
            artifact_class: Some(ArtifactClass::Code),
            provenance: Provenance {
                root: "root".to_string(),
                path: "src/lib.rs".to_string(),
                range: Some(Range {
                    span: ByteSpan::new(100, 200),
                    start: Position::new(10, 0),
                    end: Position::new(20, 0),
                }),
                extractor: "custom_infer".to_string(),
                extractor_version: "2.0".to_string(),
                derivation: Derivation::Inferred,
                confidence: Confidence::new(0.85),
                revision: Revision(42),
            },
            attributes: custom_attrs.clone(),
        };

        let owner = FactOwner::new("root", "src/lib.rs", "rust_pack", "1.0");
        tx.put_nodes(&[
            NodeClaim {
                node: node1,
                owner: owner.clone(),
            },
            NodeClaim {
                node: node2,
                owner: owner.clone(),
            },
        ])
        .unwrap();
        tx.commit().unwrap();

        let view = store.read_view().unwrap();
        let res1 = view.node(&id1).unwrap().unwrap();
        assert_eq!(res1.name, "empty_attr_fn");
        assert!(res1.attributes.is_empty());
        assert_eq!(res1.provenance.derivation, Derivation::Extracted);
        assert_eq!(res1.provenance.confidence, Confidence::EXACT);

        let res2 = view.node(&id2).unwrap().unwrap();
        assert_eq!(res2.name, "custom_attr_fn");
        assert_eq!(
            res2.attributes.get("visibility").unwrap(),
            &serde_json::json!("pub(crate)")
        );
        assert_eq!(
            res2.attributes.get("is_async").unwrap(),
            &serde_json::json!(true)
        );
        assert_eq!(res2.provenance.derivation, Derivation::Inferred);
        assert_eq!(res2.provenance.extractor, "custom_infer");
        assert_eq!(res2.provenance.revision, Revision(42));
    }

    #[test]
    fn test_removal_operations_and_cascade() {
        let store = SqliteStore::open_in_memory().unwrap();
        let mut tx = store.begin_write().unwrap();

        let owner1 = FactOwner::new("root", "src/foo.rs", "rust_pack", "1.0");
        let owner2 = FactOwner::new("root", "src/bar.rs", "rust_pack", "1.0");

        let id1 = NodeId::new(NodeKind::Function, "root", "src/foo.rs", &[], "foo_fn", 0);
        let id2 = NodeId::new(NodeKind::Function, "root", "src/bar.rs", &[], "bar_fn", 0);

        let node1 = Node {
            id: id1,
            kind: NodeKind::Function,
            name: "foo_fn".to_string(),
            qualified_name: None,
            root: "root".to_string(),
            path: "src/foo.rs".to_string(),
            range: None,
            language: Some("rust".to_string()),
            artifact_class: None,
            provenance: Provenance {
                root: "root".to_string(),
                path: "src/foo.rs".to_string(),
                range: None,
                extractor: "rust_pack".to_string(),
                extractor_version: "1.0".to_string(),
                derivation: Derivation::Extracted,
                confidence: Confidence::EXACT,
                revision: Revision::INITIAL,
            },
            attributes: Default::default(),
        };

        let node2 = Node {
            id: id2,
            kind: NodeKind::Function,
            name: "bar_fn".to_string(),
            qualified_name: None,
            root: "root".to_string(),
            path: "src/bar.rs".to_string(),
            range: None,
            language: Some("rust".to_string()),
            artifact_class: None,
            provenance: Provenance {
                root: "root".to_string(),
                path: "src/bar.rs".to_string(),
                range: None,
                extractor: "rust_pack".to_string(),
                extractor_version: "1.0".to_string(),
                derivation: Derivation::Extracted,
                confidence: Confidence::EXACT,
                revision: Revision::INITIAL,
            },
            attributes: Default::default(),
        };

        tx.put_nodes(&[
            NodeClaim {
                node: node1,
                owner: owner1.clone(),
            },
            NodeClaim {
                node: node2,
                owner: owner2.clone(),
            },
        ])
        .unwrap();
        tx.commit().unwrap();

        let view = store.read_view().unwrap();
        assert_eq!(view.node_count().unwrap(), 2);

        // Remove by specific fact key
        let mut tx2 = store.begin_write().unwrap();
        tx2.remove_node_claims(&[FactClaimKey {
            fact_id: *id1.as_bytes(),
            owner: owner1.clone(),
        }])
        .unwrap();
        tx2.commit().unwrap();

        let view2 = store.read_view().unwrap();
        assert_eq!(view2.node_count().unwrap(), 1);
        assert!(view2.node(&id1).unwrap().is_none());
        assert!(view2.node(&id2).unwrap().is_some());

        // Remove by file
        let mut tx3 = store.begin_write().unwrap();
        tx3.remove_by_file("root", "src/bar.rs").unwrap();
        tx3.commit().unwrap();

        let view3 = store.read_view().unwrap();
        assert_eq!(view3.node_count().unwrap(), 0);
    }

    #[test]
    fn test_skips_diagnostics_and_unresolved() {
        let store = SqliteStore::open_in_memory().unwrap();
        let mut tx = store.begin_write().unwrap();

        let owner = FactOwner::new("root", "src/skipped.rs", "rust_pack", "1.0");
        let skip = Skip {
            root: "root".to_string(),
            path: "src/skipped.rs".to_string(),
            reason: "binary file".to_string(),
            owner: owner.clone(),
        };

        let diag = Diagnostic {
            root: "root".to_string(),
            path: "src/skipped.rs".to_string(),
            message: "syntax error on line 4".to_string(),
            span: None,
            owner: owner.clone(),
        };

        let unres_id = NodeId::new(NodeKind::Function, "root", "src/skipped.rs", &[], "caller", 0);
        let unres = UnresolvedRef {
            from: unres_id,
            seeking: "missing_target".to_string(),
            scope_hint: Some("crate::util".to_string()),
            edge_kind: EdgeKind::Calls,
            provenance: Provenance {
                root: "root".to_string(),
                path: "src/skipped.rs".to_string(),
                range: None,
                extractor: "rust_pack".to_string(),
                extractor_version: "1.0".to_string(),
                derivation: Derivation::Extracted,
                confidence: Confidence::EXACT,
                revision: Revision::INITIAL,
            },
        };

        tx.put_skips(&[skip]).unwrap();
        tx.put_diagnostics(&[diag]).unwrap();
        tx.put_unresolved(&[unres]).unwrap();
        tx.commit().unwrap();

        let view = store.read_view().unwrap();
        let skips = view.skips(Some("root"), Some("src/skipped.rs")).unwrap();
        assert_eq!(skips.len(), 1);
        assert_eq!(skips[0].reason, "binary file");

        let diags = view
            .diagnostics(Some("root"), Some("src/skipped.rs"))
            .unwrap();
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "syntax error on line 4");

        let unres_list = view.unresolved_seeking("missing_target").unwrap();
        assert_eq!(unres_list.len(), 1);
        assert_eq!(unres_list[0].seeking, "missing_target");
        assert_eq!(unres_list[0].scope_hint.as_deref(), Some("crate::util"));
    }
}
