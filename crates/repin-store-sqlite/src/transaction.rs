use crate::intern::InternerCache;
use repin_core::line_index::Range;
use repin_core::model::edge::{EdgeClaim, FactClaimKey};
use repin_core::model::node::{Attributes, NodeClaim};
use repin_core::model::provenance::{Confidence, Derivation, FactOwner, Provenance, Revision};
use repin_core::model::unresolved::{UnresolvedKey, UnresolvedRef};
use repin_core::ports::fs::{Diagnostic, Skip};
use repin_core::ports::store::{
    DerivedIndexIntent, IndexKind, StoreError, Transaction, UpdateSummary, VersionRecords,
};
use rusqlite::Connection;
use std::sync::Arc;
use std::sync::Mutex;

fn compact_provenance_json(
    prov: &Provenance,
    owner: &FactOwner,
    node_range: Option<&Range>,
) -> Option<String> {
    if prov.derivation == Derivation::Extracted
        && prov.confidence == Confidence::EXACT
        && prov.revision == Revision::INITIAL
        && prov.extractor == owner.producer
        && prov.extractor_version == owner.producer_version
        && prov.root == owner.root
        && prov.path == owner.path
        && prov.range.as_ref() == node_range
    {
        None
    } else {
        serde_json::to_string(prov).ok()
    }
}

fn compact_attributes_json(attributes: &Attributes) -> Option<String> {
    if attributes.is_empty() {
        None
    } else {
        serde_json::to_string(attributes).ok()
    }
}

pub struct SqliteTransaction {
    conn: Arc<Mutex<Connection>>,
    interner: InternerCache,
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
            interner: InternerCache::new(),
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
            let owner_id = self.interner.get_or_insert_owner(&conn, &claim.owner)?;
            let lang_id = match &node.language {
                Some(lang) => Some(self.interner.get_or_insert_string(&conn, lang)?),
                None => None,
            };
            let range_json = node
                .range
                .as_ref()
                .map(|r| serde_json::to_string(r).unwrap());
            let prov_json =
                compact_provenance_json(&node.provenance, &claim.owner, node.range.as_ref());
            let attr_json = compact_attributes_json(&node.attributes);
            let artifact_class_str = node.artifact_class.as_ref().map(|a| a.as_str().to_string());

            conn.execute(
                "INSERT OR REPLACE INTO node_claims (
                    node_id, owner_id, kind, name, qualified_name, range_json,
                    language_id, artifact_class, provenance_json, attributes_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                (
                    node.id.as_bytes(),
                    owner_id,
                    node.kind.as_str(),
                    &node.name,
                    &node.qualified_name,
                    &range_json,
                    lang_id,
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
                    node.qualified_name.as_deref().unwrap_or(""),
                    &node.path,
                    attr_json.as_deref().unwrap_or(""),
                ),
            );
        }
        Ok(())
    }

    fn put_edges(&mut self, claims: &[EdgeClaim]) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        for claim in claims {
            let edge = &claim.edge;
            let owner_id = self.interner.get_or_insert_owner(&conn, &claim.owner)?;
            let prov_json = compact_provenance_json(&edge.provenance, &claim.owner, None);
            let attr_json = compact_attributes_json(&edge.attributes);

            conn.execute(
                "INSERT OR REPLACE INTO edge_claims (
                    edge_id, from_id, to_id, owner_id, kind,
                    provenance_json, attributes_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                (
                    edge.id.as_bytes(),
                    edge.from.as_bytes(),
                    edge.to.as_bytes(),
                    owner_id,
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
            if let Some(owner_id) = self.interner.lookup_owner_id(&conn, &key.owner)? {
                conn.execute(
                    "DELETE FROM node_claims WHERE node_id = ?1 AND owner_id = ?2",
                    (&key.fact_id, owner_id),
                )
                .map_err(|e| StoreError::Io(e.to_string()))?;

                let _ = conn.execute("DELETE FROM fts_nodes WHERE node_id = ?1", [&key.fact_id]);
            }
        }
        Ok(())
    }

    fn remove_edge_claims(&mut self, keys: &[FactClaimKey]) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        for key in keys {
            if let Some(owner_id) = self.interner.lookup_owner_id(&conn, &key.owner)? {
                conn.execute(
                    "DELETE FROM edge_claims WHERE edge_id = ?1 AND owner_id = ?2",
                    (&key.fact_id, owner_id),
                )
                .map_err(|e| StoreError::Io(e.to_string()))?;
            }
        }
        Ok(())
    }

    fn remove_claims(&mut self, owner: &FactOwner) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        if let Some(owner_id) = self.interner.lookup_owner_id(&conn, owner)? {
            conn.execute("DELETE FROM node_claims WHERE owner_id = ?1", [owner_id])
                .map_err(|e| StoreError::Io(e.to_string()))?;
            conn.execute("DELETE FROM edge_claims WHERE owner_id = ?1", [owner_id])
                .map_err(|e| StoreError::Io(e.to_string()))?;
            conn.execute("DELETE FROM skips WHERE owner_id = ?1", [owner_id])
                .map_err(|e| StoreError::Io(e.to_string()))?;
            conn.execute("DELETE FROM diagnostics WHERE owner_id = ?1", [owner_id])
                .map_err(|e| StoreError::Io(e.to_string()))?;
        }
        Ok(())
    }

    fn remove_by_file(&mut self, root: &str, path: &str) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        let owner_ids = self.interner.lookup_owner_ids_by_file(&conn, root, path)?;
        for owner_id in owner_ids {
            conn.execute("DELETE FROM node_claims WHERE owner_id = ?1", [owner_id])
                .map_err(|e| StoreError::Io(e.to_string()))?;
            conn.execute("DELETE FROM edge_claims WHERE owner_id = ?1", [owner_id])
                .map_err(|e| StoreError::Io(e.to_string()))?;
            conn.execute(
                "DELETE FROM unresolved_refs WHERE owner_id = ?1",
                [owner_id],
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;
            conn.execute("DELETE FROM skips WHERE owner_id = ?1", [owner_id])
                .map_err(|e| StoreError::Io(e.to_string()))?;
            conn.execute("DELETE FROM diagnostics WHERE owner_id = ?1", [owner_id])
                .map_err(|e| StoreError::Io(e.to_string()))?;
        }
        conn.execute("DELETE FROM fts_nodes WHERE path = ?1", [path])
            .map_err(|e| StoreError::Io(e.to_string()))?;
        Ok(())
    }

    fn put_unresolved(&mut self, refs: &[UnresolvedRef]) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        for u in refs {
            let owner = FactOwner::new(
                &u.provenance.root,
                &u.provenance.path,
                &u.provenance.extractor,
                &u.provenance.extractor_version,
            );
            let owner_id = self.interner.get_or_insert_owner(&conn, &owner)?;
            let prov_json =
                compact_provenance_json(&u.provenance, &owner, u.provenance.range.as_ref());
            conn.execute(
                "INSERT OR REPLACE INTO unresolved_refs (
                    from_id, seeking, scope_hint, edge_kind, owner_id, provenance_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                (
                    u.from.as_bytes(),
                    &u.seeking,
                    &u.scope_hint,
                    u.edge_kind.as_str(),
                    owner_id,
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
            let owner_id = self.interner.get_or_insert_owner(&conn, &s.owner)?;
            conn.execute(
                "INSERT OR REPLACE INTO skips (owner_id, reason)
                 VALUES (?1, ?2)",
                (owner_id, &s.reason),
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;
        }
        Ok(())
    }

    fn put_diagnostics(&mut self, diagnostics: &[Diagnostic]) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        for d in diagnostics {
            let owner_id = self.interner.get_or_insert_owner(&conn, &d.owner)?;
            let span_json = d.span.as_ref().map(|s| serde_json::to_string(s).unwrap());
            conn.execute(
                "INSERT INTO diagnostics (owner_id, message, span_json)
                 VALUES (?1, ?2, ?3)",
                (owner_id, &d.message, &span_json),
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
