use crate::model::identity::NodeId;
use crate::model::node::{Node, NodeClaim};
use crate::model::provenance::{Confidence, Derivation, FactOwner, Provenance, Revision};
use crate::model::registries::NodeKind;
use crate::ports::store::{Store, StoreError};

pub fn test_store_commit_atomicity(store: &dyn Store) -> Result<(), StoreError> {
    let mut tx = store.begin_write()?;
    let id = NodeId::new(NodeKind::Function, "root", "src/test.rs", &[], "foo", 0);
    let node = Node {
        id,
        kind: NodeKind::Function,
        name: "foo".to_string(),
        qualified_name: None,
        root: "root".to_string(),
        path: "src/test.rs".to_string(),
        range: None,
        language: Some("rust".to_string()),
        artifact_class: None,
        provenance: Provenance {
            root: "root".to_string(),
            path: "src/test.rs".to_string(),
            range: None,
            extractor: "test".to_string(),
            extractor_version: "1.0".to_string(),
            derivation: Derivation::Extracted,
            confidence: Confidence::EXACT,
            revision: Revision::INITIAL,
        },
        attributes: Default::default(),
    };

    tx.put_nodes(&[NodeClaim {
        node,
        owner: FactOwner::new("root", "src/test.rs", "test", "1.0"),
    }])?;
    tx.set_revision(Revision(1))?;
    tx.commit()?;

    let view = store.read_view()?;
    assert_eq!(view.revision()?, Revision(1));
    assert!(view.node(&id)?.is_some());
    Ok(())
}

pub fn test_store_rollback_safety(store: &dyn Store) -> Result<(), StoreError> {
    let mut tx = store.begin_write()?;
    let id = NodeId::new(NodeKind::Function, "root", "src/test.rs", &[], "bar", 0);
    let node = Node {
        id,
        kind: NodeKind::Function,
        name: "bar".to_string(),
        qualified_name: None,
        root: "root".to_string(),
        path: "src/test.rs".to_string(),
        range: None,
        language: Some("rust".to_string()),
        artifact_class: None,
        provenance: Provenance {
            root: "root".to_string(),
            path: "src/test.rs".to_string(),
            range: None,
            extractor: "test".to_string(),
            extractor_version: "1.0".to_string(),
            derivation: Derivation::Extracted,
            confidence: Confidence::EXACT,
            revision: Revision::INITIAL,
        },
        attributes: Default::default(),
    };

    tx.put_nodes(&[NodeClaim {
        node,
        owner: FactOwner::new("root", "src/test.rs", "test", "1.0"),
    }])?;
    tx.rollback()?;

    let view = store.read_view()?;
    assert!(view.node(&id)?.is_none());
    Ok(())
}
