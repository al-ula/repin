use repin_core::model::edge::{EdgeClaim, FactClaimKey};
use repin_core::model::node::NodeClaim;
use repin_core::model::provenance::{FactOwner, Revision};
use repin_core::model::unresolved::{UnresolvedKey, UnresolvedRef};
use repin_core::ports::fs::{Diagnostic, Skip};
use repin_core::ports::store::{
    DerivedIndexIntent, IndexKind, StoreError, Transaction, UpdateSummary, VersionRecords,
};
use rusqlite::Connection;
use std::sync::Arc;
use std::sync::Mutex;

pub struct SqliteTransaction {
    conn: Arc<Mutex<Connection>>,
    committed: bool,
}

impl SqliteTransaction {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Result<Self, StoreError> {
        let conn_guard = conn.lock().unwrap();
        conn_guard
            .execute_batch("BEGIN IMMEDIATE TRANSACTION;")
            .map_err(|e| StoreError::Io(e.to_string()))?;
        drop(conn_guard);

        Ok(Self {
            conn,
            committed: false,
        })
    }
}

impl Transaction for SqliteTransaction {
    fn expect_revision(&mut self, base: Revision) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT value FROM meta WHERE key = 'revision'")
            .map_err(|e| StoreError::Io(e.to_string()))?;

        let mut rows = stmt.query([]).map_err(|e| StoreError::Io(e.to_string()))?;
        let current_rev =
            if let Some(row) = rows.next().map_err(|e| StoreError::Io(e.to_string()))? {
                let s: String = row.get(0).map_err(|e| StoreError::Io(e.to_string()))?;
                Revision(s.parse().unwrap_or(0))
            } else {
                Revision::INITIAL
            };

        if current_rev != base {
            return Err(StoreError::RevisionConflict {
                expected: base,
                actual: current_rev,
            });
        }
        Ok(())
    }

    fn put_nodes(&mut self, claims: &[NodeClaim]) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        for claim in claims {
            let node = &claim.node;
            let range_json = node
                .range
                .as_ref()
                .map(|r| serde_json::to_string(r).unwrap());
            let prov_json = serde_json::to_string(&node.provenance).unwrap_or_default();
            let attr_json = serde_json::to_string(&node.attributes).unwrap_or_default();
            let artifact_class_str = node.artifact_class.as_ref().map(|a| a.as_str().to_string());

            conn.execute(
                "INSERT OR REPLACE INTO node_claims (
                    node_id, root, path, producer, producer_version,
                    kind, name, qualified_name, range_json, language,
                    artifact_class, provenance_json, attributes_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                (
                    node.id.as_bytes(),
                    &node.root,
                    &node.path,
                    &claim.owner.producer,
                    &claim.owner.producer_version,
                    node.kind.as_str(),
                    &node.name,
                    &node.qualified_name,
                    &range_json,
                    &node.language,
                    &artifact_class_str,
                    &prov_json,
                    &attr_json,
                ),
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;

            // Index in FTS5
            let _ = conn.execute(
                "INSERT INTO fts_nodes (node_id, name, qualified_name, path, attributes)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                (
                    node.id.as_bytes(),
                    &node.name,
                    &node.qualified_name.as_deref().unwrap_or(""),
                    &node.path,
                    &attr_json,
                ),
            );
        }
        Ok(())
    }

    fn put_edges(&mut self, claims: &[EdgeClaim]) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        for claim in claims {
            let edge = &claim.edge;
            let prov_json = serde_json::to_string(&edge.provenance).unwrap_or_default();
            let attr_json = serde_json::to_string(&edge.attributes).unwrap_or_default();

            conn.execute(
                "INSERT OR REPLACE INTO edge_claims (
                    edge_id, from_id, to_id, root, path, producer, producer_version,
                    kind, provenance_json, attributes_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                (
                    edge.id.as_bytes(),
                    edge.from.as_bytes(),
                    edge.to.as_bytes(),
                    &claim.owner.root,
                    &claim.owner.path,
                    &claim.owner.producer,
                    &claim.owner.producer_version,
                    edge.kind.as_str(),
                    &prov_json,
                    &attr_json,
                ),
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;
        }
        Ok(())
    }

    fn remove_node_claims(&mut self, keys: &[FactClaimKey]) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        for key in keys {
            conn.execute(
                "DELETE FROM node_claims
                 WHERE node_id = ?1 AND root = ?2 AND path = ?3 AND producer = ?4 AND producer_version = ?5",
                (
                    &key.fact_id,
                    &key.owner.root,
                    &key.owner.path,
                    &key.owner.producer,
                    &key.owner.producer_version,
                ),
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;

            let _ = conn.execute("DELETE FROM fts_nodes WHERE node_id = ?1", [&key.fact_id]);
        }
        Ok(())
    }

    fn remove_edge_claims(&mut self, keys: &[FactClaimKey]) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        for key in keys {
            conn.execute(
                "DELETE FROM edge_claims
                 WHERE edge_id = ?1 AND root = ?2 AND path = ?3 AND producer = ?4 AND producer_version = ?5",
                (
                    &key.fact_id,
                    &key.owner.root,
                    &key.owner.path,
                    &key.owner.producer,
                    &key.owner.producer_version,
                ),
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;
        }
        Ok(())
    }

    fn remove_claims(&mut self, owner: &FactOwner) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM node_claims WHERE root = ?1 AND path = ?2 AND producer = ?3 AND producer_version = ?4",
            (&owner.root, &owner.path, &owner.producer, &owner.producer_version),
        )
        .map_err(|e| StoreError::Io(e.to_string()))?;

        conn.execute(
            "DELETE FROM edge_claims WHERE root = ?1 AND path = ?2 AND producer = ?3 AND producer_version = ?4",
            (&owner.root, &owner.path, &owner.producer, &owner.producer_version),
        )
        .map_err(|e| StoreError::Io(e.to_string()))?;

        conn.execute(
            "DELETE FROM skips WHERE root = ?1 AND path = ?2 AND producer = ?3 AND producer_version = ?4",
            (&owner.root, &owner.path, &owner.producer, &owner.producer_version),
        )
        .map_err(|e| StoreError::Io(e.to_string()))?;

        conn.execute(
            "DELETE FROM diagnostics WHERE root = ?1 AND path = ?2 AND producer = ?3 AND producer_version = ?4",
            (&owner.root, &owner.path, &owner.producer, &owner.producer_version),
        )
        .map_err(|e| StoreError::Io(e.to_string()))?;

        Ok(())
    }

    fn remove_by_file(&mut self, root: &str, path: &str) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM node_claims WHERE root = ?1 AND path = ?2",
            (root, path),
        )
        .map_err(|e| StoreError::Io(e.to_string()))?;
        conn.execute(
            "DELETE FROM edge_claims WHERE root = ?1 AND path = ?2",
            (root, path),
        )
        .map_err(|e| StoreError::Io(e.to_string()))?;
        conn.execute(
            "DELETE FROM unresolved_refs WHERE root = ?1 AND path = ?2",
            (root, path),
        )
        .map_err(|e| StoreError::Io(e.to_string()))?;
        conn.execute(
            "DELETE FROM skips WHERE root = ?1 AND path = ?2",
            (root, path),
        )
        .map_err(|e| StoreError::Io(e.to_string()))?;
        conn.execute(
            "DELETE FROM diagnostics WHERE root = ?1 AND path = ?2",
            (root, path),
        )
        .map_err(|e| StoreError::Io(e.to_string()))?;
        conn.execute("DELETE FROM fts_nodes WHERE path = ?1", [path])
            .map_err(|e| StoreError::Io(e.to_string()))?;
        Ok(())
    }

    fn put_unresolved(&mut self, refs: &[UnresolvedRef]) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        for u in refs {
            let prov_json = serde_json::to_string(&u.provenance).unwrap_or_default();
            conn.execute(
                "INSERT OR REPLACE INTO unresolved_refs (
                    from_id, seeking, scope_hint, edge_kind, root, path, provenance_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                (
                    u.from.as_bytes(),
                    &u.seeking,
                    &u.scope_hint,
                    u.edge_kind.as_str(),
                    &u.provenance.root,
                    &u.provenance.path,
                    &prov_json,
                ),
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;
        }
        Ok(())
    }

    fn remove_unresolved(&mut self, keys: &[UnresolvedKey]) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        for k in keys {
            conn.execute(
                "DELETE FROM unresolved_refs WHERE from_id = ?1 AND seeking = ?2 AND edge_kind = ?3",
                (k.from.as_bytes(), &k.seeking, k.edge_kind.as_str()),
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;
        }
        Ok(())
    }

    fn put_skips(&mut self, skips: &[Skip]) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        for s in skips {
            conn.execute(
                "INSERT OR REPLACE INTO skips (root, path, reason, producer, producer_version)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                (
                    &s.root,
                    &s.path,
                    &s.reason,
                    &s.owner.producer,
                    &s.owner.producer_version,
                ),
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;
        }
        Ok(())
    }

    fn put_diagnostics(&mut self, diagnostics: &[Diagnostic]) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        for d in diagnostics {
            let span_json = d.span.as_ref().map(|s| serde_json::to_string(s).unwrap());
            conn.execute(
                "INSERT INTO diagnostics (root, path, message, span_json, producer, producer_version)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                (&d.root, &d.path, &d.message, &span_json, &d.owner.producer, &d.owner.producer_version),
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;
        }
        Ok(())
    }

    fn put_update_summary(&mut self, summary: &UpdateSummary) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        let j = serde_json::to_string(summary).map_err(|e| StoreError::Io(e.to_string()))?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        conn.execute(
            "INSERT OR REPLACE INTO update_history (revision, summary_json, created_at)
             VALUES (?1, ?2, ?3)",
            (summary.revision.0 as i64, &j, now),
        )
        .map_err(|e| StoreError::Io(e.to_string()))?;
        Ok(())
    }

    fn put_version_records(&mut self, records: &VersionRecords) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        let j = serde_json::to_string(records).map_err(|e| StoreError::Io(e.to_string()))?;
        conn.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('version_records', ?1)",
            [&j],
        )
        .map_err(|e| StoreError::Io(e.to_string()))?;
        Ok(())
    }

    fn put_index_intent(&mut self, intent: &DerivedIndexIntent) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        let kind_str = match intent.kind {
            IndexKind::Lexical => "lexical",
            IndexKind::Vector => "vector",
        };
        conn.execute(
            "INSERT OR REPLACE INTO index_state (kind, acknowledged_revision, is_current)
             VALUES (?1, ?2, 0)",
            (kind_str, intent.revision.0 as i64),
        )
        .map_err(|e| StoreError::Io(e.to_string()))?;
        Ok(())
    }

    fn acknowledge_index(&mut self, kind: IndexKind, revision: Revision) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        let kind_str = match kind {
            IndexKind::Lexical => "lexical",
            IndexKind::Vector => "vector",
        };
        conn.execute(
            "INSERT OR REPLACE INTO index_state (kind, acknowledged_revision, is_current)
             VALUES (?1, ?2, 1)",
            (kind_str, revision.0 as i64),
        )
        .map_err(|e| StoreError::Io(e.to_string()))?;
        Ok(())
    }

    fn set_revision(&mut self, revision: Revision) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('revision', ?1)",
            [revision.0.to_string()],
        )
        .map_err(|e| StoreError::Io(e.to_string()))?;
        Ok(())
    }

    fn commit(mut self: Box<Self>) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch("COMMIT;")
            .map_err(|e| StoreError::Io(e.to_string()))?;
        self.committed = true;
        Ok(())
    }

    fn rollback(mut self: Box<Self>) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch("ROLLBACK;")
            .map_err(|e| StoreError::Io(e.to_string()))?;
        self.committed = true;
        Ok(())
    }
}

impl Drop for SqliteTransaction {
    fn drop(&mut self) {
        if !self.committed
            && let Ok(conn) = self.conn.lock()
        {
            let _ = conn.execute_batch("ROLLBACK;");
        }
    }
}
