use repin_core::model::edge::Edge;
use repin_core::model::identity::{EdgeId, NodeId};
use repin_core::model::node::Node;
use repin_core::model::provenance::{Confidence, Derivation, Provenance, Revision};
use repin_core::model::registries::{EdgeKind, NodeKind};
use repin_core::model::unresolved::UnresolvedRef;
use repin_core::ports::fs::{Diagnostic, Skip};
use repin_core::ports::store::{
    DerivedIndexState, EdgeFilters, IndexKind, NodeFilters, ReadView, StoreError, UpdateSummary,
    VersionRecords,
};
use rusqlite::Connection;
use std::sync::Arc;
use std::sync::Mutex;

pub struct SqliteReadView {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteReadView {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }
}

impl ReadView for SqliteReadView {
    fn node(&self, id: &NodeId) -> Result<Option<Node>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT node_id, kind, name, qualified_name, root, path, range_json, language, artifact_class, provenance_json, attributes_json
                 FROM node_claims WHERE node_id = ?1 LIMIT 1",
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;

        let mut rows = stmt
            .query([id.as_bytes()])
            .map_err(|e| StoreError::Io(e.to_string()))?;

        if let Some(row) = rows.next().map_err(|e| StoreError::Io(e.to_string()))? {
            let node_id_bytes: [u8; 32] = row.get(0).map_err(|e| StoreError::Io(e.to_string()))?;
            let kind_str: String = row.get(1).map_err(|e| StoreError::Io(e.to_string()))?;
            let name: String = row.get(2).map_err(|e| StoreError::Io(e.to_string()))?;
            let qualified_name: Option<String> =
                row.get(3).map_err(|e| StoreError::Io(e.to_string()))?;
            let root: String = row.get(4).map_err(|e| StoreError::Io(e.to_string()))?;
            let path: String = row.get(5).map_err(|e| StoreError::Io(e.to_string()))?;
            let range_json: Option<String> =
                row.get(6).map_err(|e| StoreError::Io(e.to_string()))?;
            let language: Option<String> = row.get(7).map_err(|e| StoreError::Io(e.to_string()))?;
            let artifact_class_str: Option<String> =
                row.get(8).map_err(|e| StoreError::Io(e.to_string()))?;
            let prov_json: String = row.get(9).map_err(|e| StoreError::Io(e.to_string()))?;
            let attr_json: String = row.get(10).map_err(|e| StoreError::Io(e.to_string()))?;

            let kind = serde_json::from_str(&format!("\"{}\"", kind_str)).unwrap_or(NodeKind::File);
            let range = range_json.and_then(|j| serde_json::from_str(&j).ok());
            let artifact_class =
                artifact_class_str.and_then(|s| serde_json::from_str(&format!("\"{}\"", s)).ok());
            let provenance: Provenance =
                serde_json::from_str(&prov_json).unwrap_or_else(|_| Provenance {
                    root: root.clone(),
                    path: path.clone(),
                    range: None,
                    extractor: "unknown".to_string(),
                    extractor_version: "0".to_string(),
                    derivation: Derivation::Extracted,
                    confidence: Confidence::EXACT,
                    revision: Revision::INITIAL,
                });
            let attributes = serde_json::from_str(&attr_json).unwrap_or_default();

            Ok(Some(Node {
                id: NodeId::from_bytes(node_id_bytes),
                kind,
                name,
                qualified_name,
                root,
                path,
                range,
                language,
                artifact_class,
                provenance,
                attributes,
            }))
        } else {
            Ok(None)
        }
    }

    fn nodes_by_name(&self, name: &str, _filters: &NodeFilters) -> Result<Vec<Node>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT node_id, kind, name, qualified_name, root, path, range_json, language, artifact_class, provenance_json, attributes_json
                 FROM node_claims WHERE name = ?1",
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;

        let rows = stmt
            .query_map([name], |row| {
                let node_id_bytes: [u8; 32] = row.get(0)?;
                let kind_str: String = row.get(1)?;
                let name: String = row.get(2)?;
                let qualified_name: Option<String> = row.get(3)?;
                let root: String = row.get(4)?;
                let path: String = row.get(5)?;
                let range_json: Option<String> = row.get(6)?;
                let language: Option<String> = row.get(7)?;
                let artifact_class_str: Option<String> = row.get(8)?;
                let prov_json: String = row.get(9)?;
                let attr_json: String = row.get(10)?;

                let kind =
                    serde_json::from_str(&format!("\"{}\"", kind_str)).unwrap_or(NodeKind::File);
                let range = range_json.and_then(|j| serde_json::from_str(&j).ok());
                let artifact_class = artifact_class_str
                    .and_then(|s| serde_json::from_str(&format!("\"{}\"", s)).ok());
                let provenance: Provenance =
                    serde_json::from_str(&prov_json).unwrap_or_else(|_| Provenance {
                        root: root.clone(),
                        path: path.clone(),
                        range: None,
                        extractor: "unknown".to_string(),
                        extractor_version: "0".to_string(),
                        derivation: Derivation::Extracted,
                        confidence: Confidence::EXACT,
                        revision: Revision::INITIAL,
                    });
                let attributes = serde_json::from_str(&attr_json).unwrap_or_default();

                Ok(Node {
                    id: NodeId::from_bytes(node_id_bytes),
                    kind,
                    name,
                    qualified_name,
                    root,
                    path,
                    range,
                    language,
                    artifact_class,
                    provenance,
                    attributes,
                })
            })
            .map_err(|e| StoreError::Io(e.to_string()))?;

        let mut nodes = Vec::new();
        for r in rows {
            nodes.push(r.map_err(|e| StoreError::Io(e.to_string()))?);
        }
        Ok(nodes)
    }

    fn nodes_by_file(&self, root: &str, path: &str) -> Result<Vec<Node>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT node_id, kind, name, qualified_name, root, path, range_json, language, artifact_class, provenance_json, attributes_json
                 FROM node_claims WHERE root = ?1 AND path = ?2",
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;

        let rows = stmt
            .query_map([root, path], |row| {
                let node_id_bytes: [u8; 32] = row.get(0)?;
                let kind_str: String = row.get(1)?;
                let name: String = row.get(2)?;
                let qualified_name: Option<String> = row.get(3)?;
                let root: String = row.get(4)?;
                let path: String = row.get(5)?;
                let range_json: Option<String> = row.get(6)?;
                let language: Option<String> = row.get(7)?;
                let artifact_class_str: Option<String> = row.get(8)?;
                let prov_json: String = row.get(9)?;
                let attr_json: String = row.get(10)?;

                let kind =
                    serde_json::from_str(&format!("\"{}\"", kind_str)).unwrap_or(NodeKind::File);
                let range = range_json.and_then(|j| serde_json::from_str(&j).ok());
                let artifact_class = artifact_class_str
                    .and_then(|s| serde_json::from_str(&format!("\"{}\"", s)).ok());
                let provenance: Provenance =
                    serde_json::from_str(&prov_json).unwrap_or_else(|_| Provenance {
                        root: root.clone(),
                        path: path.clone(),
                        range: None,
                        extractor: "unknown".to_string(),
                        extractor_version: "0".to_string(),
                        derivation: Derivation::Extracted,
                        confidence: Confidence::EXACT,
                        revision: Revision::INITIAL,
                    });
                let attributes = serde_json::from_str(&attr_json).unwrap_or_default();

                Ok(Node {
                    id: NodeId::from_bytes(node_id_bytes),
                    kind,
                    name,
                    qualified_name,
                    root,
                    path,
                    range,
                    language,
                    artifact_class,
                    provenance,
                    attributes,
                })
            })
            .map_err(|e| StoreError::Io(e.to_string()))?;

        let mut nodes = Vec::new();
        for r in rows {
            nodes.push(r.map_err(|e| StoreError::Io(e.to_string()))?);
        }
        Ok(nodes)
    }

    fn edges_from(&self, id: &NodeId, _filters: &EdgeFilters) -> Result<Vec<Edge>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT edge_id, from_id, to_id, kind, provenance_json, attributes_json
                 FROM edge_claims WHERE from_id = ?1",
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;

        let rows = stmt
            .query_map([id.as_bytes()], |row| {
                let edge_id_bytes: [u8; 32] = row.get(0)?;
                let from_id_bytes: [u8; 32] = row.get(1)?;
                let to_id_bytes: [u8; 32] = row.get(2)?;
                let kind_str: String = row.get(3)?;
                let prov_json: String = row.get(4)?;
                let attr_json: String = row.get(5)?;

                let kind = serde_json::from_str(&format!("\"{}\"", kind_str))
                    .unwrap_or(EdgeKind::Contains);
                let provenance: Provenance =
                    serde_json::from_str(&prov_json).unwrap_or_else(|_| Provenance {
                        root: "root".to_string(),
                        path: "path".to_string(),
                        range: None,
                        extractor: "unknown".to_string(),
                        extractor_version: "0".to_string(),
                        derivation: Derivation::Extracted,
                        confidence: Confidence::EXACT,
                        revision: Revision::INITIAL,
                    });
                let attributes = serde_json::from_str(&attr_json).unwrap_or_default();

                Ok(Edge {
                    id: EdgeId::from_bytes(edge_id_bytes),
                    from: NodeId::from_bytes(from_id_bytes),
                    to: NodeId::from_bytes(to_id_bytes),
                    kind,
                    provenance,
                    attributes,
                })
            })
            .map_err(|e| StoreError::Io(e.to_string()))?;

        let mut edges = Vec::new();
        for r in rows {
            edges.push(r.map_err(|e| StoreError::Io(e.to_string()))?);
        }
        Ok(edges)
    }

    fn edges_to(&self, id: &NodeId, _filters: &EdgeFilters) -> Result<Vec<Edge>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT edge_id, from_id, to_id, kind, provenance_json, attributes_json
                 FROM edge_claims WHERE to_id = ?1",
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;

        let rows = stmt
            .query_map([id.as_bytes()], |row| {
                let edge_id_bytes: [u8; 32] = row.get(0)?;
                let from_id_bytes: [u8; 32] = row.get(1)?;
                let to_id_bytes: [u8; 32] = row.get(2)?;
                let kind_str: String = row.get(3)?;
                let prov_json: String = row.get(4)?;
                let attr_json: String = row.get(5)?;

                let kind = serde_json::from_str(&format!("\"{}\"", kind_str))
                    .unwrap_or(EdgeKind::Contains);
                let provenance: Provenance =
                    serde_json::from_str(&prov_json).unwrap_or_else(|_| Provenance {
                        root: "root".to_string(),
                        path: "path".to_string(),
                        range: None,
                        extractor: "unknown".to_string(),
                        extractor_version: "0".to_string(),
                        derivation: Derivation::Extracted,
                        confidence: Confidence::EXACT,
                        revision: Revision::INITIAL,
                    });
                let attributes = serde_json::from_str(&attr_json).unwrap_or_default();

                Ok(Edge {
                    id: EdgeId::from_bytes(edge_id_bytes),
                    from: NodeId::from_bytes(from_id_bytes),
                    to: NodeId::from_bytes(to_id_bytes),
                    kind,
                    provenance,
                    attributes,
                })
            })
            .map_err(|e| StoreError::Io(e.to_string()))?;

        let mut edges = Vec::new();
        for r in rows {
            edges.push(r.map_err(|e| StoreError::Io(e.to_string()))?);
        }
        Ok(edges)
    }

    fn unresolved_seeking(&self, name: &str) -> Result<Vec<UnresolvedRef>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT from_id, seeking, scope_hint, edge_kind, provenance_json
                 FROM unresolved_refs WHERE seeking = ?1",
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;

        let rows = stmt
            .query_map([name], |row| {
                let from_id_bytes: [u8; 32] = row.get(0)?;
                let seeking: String = row.get(1)?;
                let scope_hint: Option<String> = row.get(2)?;
                let edge_kind_str: String = row.get(3)?;
                let prov_json: String = row.get(4)?;

                let edge_kind = serde_json::from_str(&format!("\"{}\"", edge_kind_str))
                    .unwrap_or(EdgeKind::References);
                let provenance: Provenance =
                    serde_json::from_str(&prov_json).unwrap_or_else(|_| Provenance {
                        root: "root".to_string(),
                        path: "path".to_string(),
                        range: None,
                        extractor: "unknown".to_string(),
                        extractor_version: "0".to_string(),
                        derivation: Derivation::Extracted,
                        confidence: Confidence::EXACT,
                        revision: Revision::INITIAL,
                    });

                Ok(UnresolvedRef {
                    from: NodeId::from_bytes(from_id_bytes),
                    seeking,
                    scope_hint,
                    edge_kind,
                    provenance,
                })
            })
            .map_err(|e| StoreError::Io(e.to_string()))?;

        let mut refs = Vec::new();
        for r in rows {
            refs.push(r.map_err(|e| StoreError::Io(e.to_string()))?);
        }
        Ok(refs)
    }

    fn skips(&self, _root: Option<&str>, _path: Option<&str>) -> Result<Vec<Skip>, StoreError> {
        Ok(Vec::new())
    }

    fn diagnostics(
        &self,
        _root: Option<&str>,
        _path: Option<&str>,
    ) -> Result<Vec<Diagnostic>, StoreError> {
        Ok(Vec::new())
    }

    fn changes_since(&self, revision: Revision) -> Result<Vec<UpdateSummary>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT summary_json FROM update_history WHERE revision > ?1 ORDER BY revision ASC",
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;

        let rows = stmt
            .query_map([revision.0 as i64], |row| {
                let j: String = row.get(0)?;
                Ok(serde_json::from_str(&j).unwrap_or(UpdateSummary {
                    revision,
                    files_added: 0,
                    files_modified: 0,
                    files_deleted: 0,
                    nodes_added: 0,
                    nodes_removed: 0,
                    edges_added: 0,
                    edges_removed: 0,
                    unresolved_promoted: 0,
                    unresolved_demoted: 0,
                }))
            })
            .map_err(|e| StoreError::Io(e.to_string()))?;

        let mut summaries = Vec::new();
        for r in rows {
            summaries.push(r.map_err(|e| StoreError::Io(e.to_string()))?);
        }
        Ok(summaries)
    }

    fn version_records(&self) -> Result<Option<VersionRecords>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT value FROM meta WHERE key = 'version_records'")
            .map_err(|e| StoreError::Io(e.to_string()))?;

        let mut rows = stmt.query([]).map_err(|e| StoreError::Io(e.to_string()))?;
        if let Some(row) = rows.next().map_err(|e| StoreError::Io(e.to_string()))? {
            let j: String = row.get(0).map_err(|e| StoreError::Io(e.to_string()))?;
            Ok(serde_json::from_str(&j).ok())
        } else {
            Ok(None)
        }
    }

    fn index_states(&self) -> Result<Vec<DerivedIndexState>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT kind, acknowledged_revision, is_current FROM index_state")
            .map_err(|e| StoreError::Io(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                let kind_str: String = row.get(0)?;
                let rev: i64 = row.get(1)?;
                let is_cur: i64 = row.get(2)?;

                let kind = if kind_str == "vector" {
                    IndexKind::Vector
                } else {
                    IndexKind::Lexical
                };

                Ok(DerivedIndexState {
                    kind,
                    acknowledged_revision: Revision(rev as u64),
                    is_current: is_cur != 0,
                })
            })
            .map_err(|e| StoreError::Io(e.to_string()))?;

        let mut states = Vec::new();
        for r in rows {
            states.push(r.map_err(|e| StoreError::Io(e.to_string()))?);
        }
        Ok(states)
    }

    fn revision(&self) -> Result<Revision, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT value FROM meta WHERE key = 'revision'")
            .map_err(|e| StoreError::Io(e.to_string()))?;

        let mut rows = stmt.query([]).map_err(|e| StoreError::Io(e.to_string()))?;
        if let Some(row) = rows.next().map_err(|e| StoreError::Io(e.to_string()))? {
            let val_str: String = row.get(0).map_err(|e| StoreError::Io(e.to_string()))?;
            let num: u64 = val_str.parse().unwrap_or(0);
            Ok(Revision(num))
        } else {
            Ok(Revision::INITIAL)
        }
    }
}
