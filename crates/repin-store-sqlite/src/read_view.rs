use repin_core::line_index::Range;
use repin_core::model::edge::Edge;
use repin_core::model::identity::{EdgeId, NodeId};
use repin_core::model::node::{Attributes, Node, NodeClaim};
use repin_core::model::provenance::{Confidence, Derivation, FactOwner, Provenance, Revision};
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

fn parse_provenance(
    prov_json: Option<&str>,
    root: &str,
    path: &str,
    range: Option<Range>,
    producer: &str,
    producer_version: &str,
) -> Provenance {
    if let Some(j) = prov_json
        && !j.trim().is_empty()
        && let Ok(p) = serde_json::from_str::<Provenance>(j)
    {
        return p;
    }
    Provenance {
        root: root.to_string(),
        path: path.to_string(),
        range,
        extractor: producer.to_string(),
        extractor_version: producer_version.to_string(),
        derivation: Derivation::Extracted,
        confidence: Confidence::EXACT,
        revision: Revision::INITIAL,
    }
}

fn parse_attributes(attr_json: Option<&str>) -> Attributes {
    attr_json
        .and_then(|j| {
            if j.trim().is_empty() {
                None
            } else {
                serde_json::from_str(j).ok()
            }
        })
        .unwrap_or_default()
}

fn owner_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<FactOwner> {
    Ok(FactOwner::new(
        row.get::<_, String>(0)?,
        row.get::<_, String>(1)?,
        row.get::<_, String>(2)?,
        row.get::<_, String>(3)?,
    ))
}

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
                "SELECT
                    nc.node_id, nc.kind, nc.name, nc.qualified_name,
                    sp_root.value, sp_path.value, nc.range_json,
                    sp_lang.value, nc.artifact_class, nc.provenance_json, nc.attributes_json,
                    sp_prod.value, sp_ver.value
                 FROM node_claims nc
                 JOIN fact_owners fo ON nc.owner_id = fo.id
                 JOIN string_pool sp_root ON fo.root_id = sp_root.id
                 JOIN string_pool sp_path ON fo.path_id = sp_path.id
                 JOIN string_pool sp_prod ON fo.producer_id = sp_prod.id
                 JOIN string_pool sp_ver ON fo.producer_version_id = sp_ver.id
                 LEFT JOIN string_pool sp_lang ON nc.language_id = sp_lang.id
                 WHERE nc.node_id = ?1 LIMIT 1",
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
            let prov_json: Option<String> =
                row.get(9).map_err(|e| StoreError::Io(e.to_string()))?;
            let attr_json: Option<String> =
                row.get(10).map_err(|e| StoreError::Io(e.to_string()))?;
            let producer: String = row.get(11).map_err(|e| StoreError::Io(e.to_string()))?;
            let producer_version: String =
                row.get(12).map_err(|e| StoreError::Io(e.to_string()))?;

            let kind = serde_json::from_str(&format!("\"{}\"", kind_str)).unwrap_or(NodeKind::File);
            let range: Option<Range> = range_json.and_then(|j| serde_json::from_str(&j).ok());
            let artifact_class =
                artifact_class_str.and_then(|s| serde_json::from_str(&format!("\"{}\"", s)).ok());
            let provenance = parse_provenance(
                prov_json.as_deref(),
                &root,
                &path,
                range,
                &producer,
                &producer_version,
            );
            let attributes = parse_attributes(attr_json.as_deref());

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
        let pattern = format!("%{}%", name);
        let mut stmt = conn
            .prepare(
                "SELECT
                    nc.node_id, nc.kind, nc.name, nc.qualified_name,
                    sp_root.value, sp_path.value, nc.range_json,
                    sp_lang.value, nc.artifact_class, nc.provenance_json, nc.attributes_json,
                    sp_prod.value, sp_ver.value
                 FROM node_claims nc
                 JOIN fact_owners fo ON nc.owner_id = fo.id
                 JOIN string_pool sp_root ON fo.root_id = sp_root.id
                 JOIN string_pool sp_path ON fo.path_id = sp_path.id
                 JOIN string_pool sp_prod ON fo.producer_id = sp_prod.id
                 JOIN string_pool sp_ver ON fo.producer_version_id = sp_ver.id
                 LEFT JOIN string_pool sp_lang ON nc.language_id = sp_lang.id
                 WHERE nc.name = ?1 COLLATE NOCASE OR nc.name LIKE ?2",
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;

        let rows = stmt
            .query_map((name, &pattern), |row| {
                let node_id_bytes: [u8; 32] = row.get(0)?;
                let kind_str: String = row.get(1)?;
                let name: String = row.get(2)?;
                let qualified_name: Option<String> = row.get(3)?;
                let root: String = row.get(4)?;
                let path: String = row.get(5)?;
                let range_json: Option<String> = row.get(6)?;
                let language: Option<String> = row.get(7)?;
                let artifact_class_str: Option<String> = row.get(8)?;
                let prov_json: Option<String> = row.get(9)?;
                let attr_json: Option<String> = row.get(10)?;
                let producer: String = row.get(11)?;
                let producer_version: String = row.get(12)?;

                let kind =
                    serde_json::from_str(&format!("\"{}\"", kind_str)).unwrap_or(NodeKind::File);
                let range: Option<Range> = range_json.and_then(|j| serde_json::from_str(&j).ok());
                let artifact_class = artifact_class_str
                    .and_then(|s| serde_json::from_str(&format!("\"{}\"", s)).ok());
                let provenance = parse_provenance(
                    prov_json.as_deref(),
                    &root,
                    &path,
                    range,
                    &producer,
                    &producer_version,
                );
                let attributes = parse_attributes(attr_json.as_deref());

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
                "SELECT
                    nc.node_id, nc.kind, nc.name, nc.qualified_name,
                    sp_root.value, sp_path.value, nc.range_json,
                    sp_lang.value, nc.artifact_class, nc.provenance_json, nc.attributes_json,
                    sp_prod.value, sp_ver.value
                 FROM node_claims nc
                 JOIN fact_owners fo ON nc.owner_id = fo.id
                 JOIN string_pool sp_root ON fo.root_id = sp_root.id
                 JOIN string_pool sp_path ON fo.path_id = sp_path.id
                 JOIN string_pool sp_prod ON fo.producer_id = sp_prod.id
                 JOIN string_pool sp_ver ON fo.producer_version_id = sp_ver.id
                 LEFT JOIN string_pool sp_lang ON nc.language_id = sp_lang.id
                 WHERE sp_root.value = ?1 AND sp_path.value = ?2",
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
                let prov_json: Option<String> = row.get(9)?;
                let attr_json: Option<String> = row.get(10)?;
                let producer: String = row.get(11)?;
                let producer_version: String = row.get(12)?;

                let kind =
                    serde_json::from_str(&format!("\"{}\"", kind_str)).unwrap_or(NodeKind::File);
                let range: Option<Range> = range_json.and_then(|j| serde_json::from_str(&j).ok());
                let artifact_class = artifact_class_str
                    .and_then(|s| serde_json::from_str(&format!("\"{}\"", s)).ok());
                let provenance = parse_provenance(
                    prov_json.as_deref(),
                    &root,
                    &path,
                    range,
                    &producer,
                    &producer_version,
                );
                let attributes = parse_attributes(attr_json.as_deref());

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

    fn node_claims_by_file(&self, root: &str, path: &str) -> Result<Vec<NodeClaim>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT
                    nc.node_id, nc.kind, nc.name, nc.qualified_name,
                    sp_root.value, sp_path.value, nc.range_json,
                    sp_lang.value, nc.artifact_class, nc.provenance_json, nc.attributes_json,
                    sp_prod.value, sp_ver.value
                 FROM node_claims nc
                 JOIN fact_owners fo ON nc.owner_id = fo.id
                 JOIN string_pool sp_root ON fo.root_id = sp_root.id
                 JOIN string_pool sp_path ON fo.path_id = sp_path.id
                 JOIN string_pool sp_prod ON fo.producer_id = sp_prod.id
                 JOIN string_pool sp_ver ON fo.producer_version_id = sp_ver.id
                 LEFT JOIN string_pool sp_lang ON nc.language_id = sp_lang.id
                 WHERE sp_root.value = ?1 AND sp_path.value = ?2
                 ORDER BY nc.node_id, sp_prod.value, sp_ver.value",
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;
        let rows = stmt
            .query_map([root, path], |row| {
                let node_id_bytes: [u8; 32] = row.get(0)?;
                let kind_str: String = row.get(1)?;
                let root: String = row.get(4)?;
                let path: String = row.get(5)?;
                let range_json: Option<String> = row.get(6)?;
                let producer: String = row.get(11)?;
                let producer_version: String = row.get(12)?;
                let range: Option<Range> = range_json.and_then(|j| serde_json::from_str(&j).ok());
                let provenance = parse_provenance(
                    row.get::<_, Option<String>>(9)?.as_deref(),
                    &root,
                    &path,
                    range,
                    &producer,
                    &producer_version,
                );
                let node = Node {
                    id: NodeId::from_bytes(node_id_bytes),
                    kind: serde_json::from_str(&format!("\"{}\"", kind_str))
                        .unwrap_or(NodeKind::File),
                    name: row.get(2)?,
                    qualified_name: row.get(3)?,
                    root: root.clone(),
                    path: path.clone(),
                    range,
                    language: row.get(7)?,
                    artifact_class: row
                        .get::<_, Option<String>>(8)?
                        .and_then(|s| serde_json::from_str(&format!("\"{}\"", s)).ok()),
                    provenance,
                    attributes: parse_attributes(row.get::<_, Option<String>>(10)?.as_deref()),
                };
                Ok(NodeClaim {
                    node,
                    owner: FactOwner::new(root, path, producer, producer_version),
                })
            })
            .map_err(|e| StoreError::Io(e.to_string()))?;
        rows.map(|row| row.map_err(|e| StoreError::Io(e.to_string())))
            .collect()
    }

    fn owners_by_producer(
        &self,
        producer: &str,
        producer_version: Option<&str>,
    ) -> Result<Vec<FactOwner>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut sql = String::from(
            "SELECT sp_root.value, sp_path.value, sp_producer.value, sp_version.value
             FROM fact_owners fo
             JOIN string_pool sp_root ON sp_root.id = fo.root_id
             JOIN string_pool sp_path ON sp_path.id = fo.path_id
             JOIN string_pool sp_producer ON sp_producer.id = fo.producer_id
             JOIN string_pool sp_version ON sp_version.id = fo.producer_version_id
             WHERE sp_producer.value = ?1",
        );
        if producer_version.is_some() {
            sql.push_str(" AND sp_version.value = ?2");
        }
        sql.push_str(" ORDER BY sp_root.value, sp_path.value, sp_version.value");

        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| StoreError::Io(e.to_string()))?;
        let rows = if let Some(version) = producer_version {
            stmt.query_map(rusqlite::params![producer, version], owner_from_row)
        } else {
            stmt.query_map(rusqlite::params![producer], owner_from_row)
        }
        .map_err(|e| StoreError::Io(e.to_string()))?;
        rows.map(|row| row.map_err(|e| StoreError::Io(e.to_string())))
            .collect()
    }

    fn resolution_owners(&self) -> Result<Vec<FactOwner>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT DISTINCT
                    sp_root.value, sp_path.value, sp_producer.value, sp_version.value
                 FROM edge_claims ec
                 JOIN fact_owners fo ON ec.owner_id = fo.id
                 JOIN string_pool sp_root ON sp_root.id = fo.root_id
                 JOIN string_pool sp_path ON sp_path.id = fo.path_id
                 JOIN string_pool sp_producer ON sp_producer.id = fo.producer_id
                 JOIN string_pool sp_version ON sp_version.id = fo.producer_version_id
                 WHERE json_extract(ec.provenance_json, '$.derivation')
                    IN ('resolved', 'heuristic')
                 ORDER BY sp_root.value, sp_path.value, sp_producer.value, sp_version.value",
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;
        stmt.query_map([], owner_from_row)
            .map_err(|e| StoreError::Io(e.to_string()))?
            .map(|row| row.map_err(|e| StoreError::Io(e.to_string())))
            .collect()
    }

    fn files(&self) -> Result<Vec<(String, String)>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT DISTINCT sp_root.value, sp_path.value
                 FROM fact_owners fo
                 JOIN string_pool sp_root ON sp_root.id = fo.root_id
                 JOIN string_pool sp_path ON sp_path.id = fo.path_id
                 ORDER BY sp_root.value, sp_path.value",
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| StoreError::Io(e.to_string()))?
            .map(|row| row.map_err(|e| StoreError::Io(e.to_string())))
            .collect()
    }

    fn all_owners(&self) -> Result<Vec<FactOwner>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT root.value, path.value, producer.value, version.value
                 FROM fact_owners fo
                 JOIN string_pool root ON root.id = fo.root_id
                 JOIN string_pool path ON path.id = fo.path_id
                 JOIN string_pool producer ON producer.id = fo.producer_id
                 JOIN string_pool version ON version.id = fo.producer_version_id
                 ORDER BY root.value, path.value, producer.value, version.value",
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;
        stmt.query_map([], owner_from_row)
            .map_err(|e| StoreError::Io(e.to_string()))?
            .map(|row| row.map_err(|e| StoreError::Io(e.to_string())))
            .collect()
    }

    fn edges_from(&self, id: &NodeId, _filters: &EdgeFilters) -> Result<Vec<Edge>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT
                    ec.edge_id, ec.from_id, ec.to_id, ec.kind,
                    sp_root.value, sp_path.value, ec.provenance_json, ec.attributes_json,
                    sp_prod.value, sp_ver.value
                 FROM edge_claims ec
                 JOIN fact_owners fo ON ec.owner_id = fo.id
                 JOIN string_pool sp_root ON fo.root_id = sp_root.id
                 JOIN string_pool sp_path ON fo.path_id = sp_path.id
                 JOIN string_pool sp_prod ON fo.producer_id = sp_prod.id
                 JOIN string_pool sp_ver ON fo.producer_version_id = sp_ver.id
                 WHERE ec.from_id = ?1",
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;

        let rows = stmt
            .query_map([id.as_bytes()], |row| {
                let edge_id_bytes: [u8; 32] = row.get(0)?;
                let from_id_bytes: [u8; 32] = row.get(1)?;
                let to_id_bytes: [u8; 32] = row.get(2)?;
                let kind_str: String = row.get(3)?;
                let root: String = row.get(4)?;
                let path: String = row.get(5)?;
                let prov_json: Option<String> = row.get(6)?;
                let attr_json: Option<String> = row.get(7)?;
                let producer: String = row.get(8)?;
                let producer_version: String = row.get(9)?;

                let kind = serde_json::from_str(&format!("\"{}\"", kind_str))
                    .unwrap_or(EdgeKind::Contains);
                let provenance = parse_provenance(
                    prov_json.as_deref(),
                    &root,
                    &path,
                    None,
                    &producer,
                    &producer_version,
                );
                let attributes = parse_attributes(attr_json.as_deref());

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
                "SELECT
                    ec.edge_id, ec.from_id, ec.to_id, ec.kind,
                    sp_root.value, sp_path.value, ec.provenance_json, ec.attributes_json,
                    sp_prod.value, sp_ver.value
                 FROM edge_claims ec
                 JOIN fact_owners fo ON ec.owner_id = fo.id
                 JOIN string_pool sp_root ON fo.root_id = sp_root.id
                 JOIN string_pool sp_path ON fo.path_id = sp_path.id
                 JOIN string_pool sp_prod ON fo.producer_id = sp_prod.id
                 JOIN string_pool sp_ver ON fo.producer_version_id = sp_ver.id
                 WHERE ec.to_id = ?1",
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;

        let rows = stmt
            .query_map([id.as_bytes()], |row| {
                let edge_id_bytes: [u8; 32] = row.get(0)?;
                let from_id_bytes: [u8; 32] = row.get(1)?;
                let to_id_bytes: [u8; 32] = row.get(2)?;
                let kind_str: String = row.get(3)?;
                let root: String = row.get(4)?;
                let path: String = row.get(5)?;
                let prov_json: Option<String> = row.get(6)?;
                let attr_json: Option<String> = row.get(7)?;
                let producer: String = row.get(8)?;
                let producer_version: String = row.get(9)?;

                let kind = serde_json::from_str(&format!("\"{}\"", kind_str))
                    .unwrap_or(EdgeKind::Contains);
                let provenance = parse_provenance(
                    prov_json.as_deref(),
                    &root,
                    &path,
                    None,
                    &producer,
                    &producer_version,
                );
                let attributes = parse_attributes(attr_json.as_deref());

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

    fn incoming_edge_count(&self, id: &NodeId) -> Result<usize, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT COUNT(*) FROM edge_claims WHERE to_id = ?1")
            .map_err(|e| StoreError::Io(e.to_string()))?;
        let count: i64 = stmt
            .query_row([id.as_bytes()], |row| row.get(0))
            .map_err(|e| StoreError::Io(e.to_string()))?;
        Ok(count as usize)
    }

    fn unresolved_seeking(&self, name: &str) -> Result<Vec<UnresolvedRef>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT
                    ur.from_id, ur.seeking, ur.scope_hint, ur.edge_kind,
                    sp_root.value, sp_path.value, ur.provenance_json,
                    sp_prod.value, sp_ver.value
                 FROM unresolved_refs ur
                 JOIN fact_owners fo ON ur.owner_id = fo.id
                 JOIN string_pool sp_root ON fo.root_id = sp_root.id
                 JOIN string_pool sp_path ON fo.path_id = sp_path.id
                 JOIN string_pool sp_prod ON fo.producer_id = sp_prod.id
                 JOIN string_pool sp_ver ON fo.producer_version_id = sp_ver.id
                 WHERE ur.seeking = ?1",
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;

        let rows = stmt
            .query_map([name], |row| {
                let from_id_bytes: [u8; 32] = row.get(0)?;
                let seeking: String = row.get(1)?;
                let scope_hint: Option<String> = row.get(2)?;
                let edge_kind_str: String = row.get(3)?;
                let root: String = row.get(4)?;
                let path: String = row.get(5)?;
                let prov_json: Option<String> = row.get(6)?;
                let producer: String = row.get(7)?;
                let producer_version: String = row.get(8)?;

                let edge_kind = serde_json::from_str(&format!("\"{}\"", edge_kind_str))
                    .unwrap_or(EdgeKind::References);
                let provenance = parse_provenance(
                    prov_json.as_deref(),
                    &root,
                    &path,
                    None,
                    &producer,
                    &producer_version,
                );

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

    fn unresolved_refs(&self) -> Result<Vec<UnresolvedRef>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT ur.from_id, ur.seeking, ur.scope_hint, ur.edge_kind,
                        sp_root.value, sp_path.value, ur.provenance_json,
                        sp_prod.value, sp_ver.value
                 FROM unresolved_refs ur
                 JOIN fact_owners fo ON ur.owner_id = fo.id
                 JOIN string_pool sp_root ON fo.root_id = sp_root.id
                 JOIN string_pool sp_path ON fo.path_id = sp_path.id
                 JOIN string_pool sp_prod ON fo.producer_id = sp_prod.id
                 JOIN string_pool sp_ver ON fo.producer_version_id = sp_ver.id
                 ORDER BY ur.seeking, ur.from_id, ur.edge_kind",
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                let from_id_bytes: [u8; 32] = row.get(0)?;
                let seeking: String = row.get(1)?;
                let scope_hint: Option<String> = row.get(2)?;
                let edge_kind_str: String = row.get(3)?;
                let root: String = row.get(4)?;
                let path: String = row.get(5)?;
                let provenance_json: Option<String> = row.get(6)?;
                let producer: String = row.get(7)?;
                let version: String = row.get(8)?;
                let edge_kind = serde_json::from_str(&format!("\"{}\"", edge_kind_str))
                    .unwrap_or(EdgeKind::References);
                Ok(UnresolvedRef {
                    from: NodeId::from_bytes(from_id_bytes),
                    seeking,
                    scope_hint,
                    edge_kind,
                    provenance: parse_provenance(
                        provenance_json.as_deref(),
                        &root,
                        &path,
                        None,
                        &producer,
                        &version,
                    ),
                })
            })
            .map_err(|e| StoreError::Io(e.to_string()))?;
        rows.map(|row| row.map_err(|e| StoreError::Io(e.to_string())))
            .collect()
    }

    fn skips(&self, root: Option<&str>, path: Option<&str>) -> Result<Vec<Skip>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT
                    sp_root.value, sp_path.value, s.reason, sp_prod.value, sp_ver.value
                 FROM skips s
                 JOIN fact_owners fo ON s.owner_id = fo.id
                 JOIN string_pool sp_root ON fo.root_id = sp_root.id
                 JOIN string_pool sp_path ON fo.path_id = sp_path.id
                 JOIN string_pool sp_prod ON fo.producer_id = sp_prod.id
                 JOIN string_pool sp_ver ON fo.producer_version_id = sp_ver.id
                 WHERE (?1 IS NULL OR sp_root.value = ?1)
                   AND (?2 IS NULL OR sp_path.value = ?2)",
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;

        let rows = stmt
            .query_map((root, path), |row| {
                let r: String = row.get(0)?;
                let p: String = row.get(1)?;
                let reason: String = row.get(2)?;
                let prod: String = row.get(3)?;
                let ver: String = row.get(4)?;

                Ok(Skip {
                    root: r.clone(),
                    path: p.clone(),
                    reason,
                    owner: FactOwner::new(r, p, prod, ver),
                })
            })
            .map_err(|e| StoreError::Io(e.to_string()))?;

        let mut skips = Vec::new();
        for r in rows {
            skips.push(r.map_err(|e| StoreError::Io(e.to_string()))?);
        }
        Ok(skips)
    }

    fn diagnostics(
        &self,
        root: Option<&str>,
        path: Option<&str>,
    ) -> Result<Vec<Diagnostic>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT
                    sp_root.value, sp_path.value, d.message, d.span_json, sp_prod.value, sp_ver.value
                 FROM diagnostics d
                 JOIN fact_owners fo ON d.owner_id = fo.id
                 JOIN string_pool sp_root ON fo.root_id = sp_root.id
                 JOIN string_pool sp_path ON fo.path_id = sp_path.id
                 JOIN string_pool sp_prod ON fo.producer_id = sp_prod.id
                 JOIN string_pool sp_ver ON fo.producer_version_id = sp_ver.id
                 WHERE (?1 IS NULL OR sp_root.value = ?1)
                   AND (?2 IS NULL OR sp_path.value = ?2)",
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;

        let rows = stmt
            .query_map((root, path), |row| {
                let r: String = row.get(0)?;
                let p: String = row.get(1)?;
                let message: String = row.get(2)?;
                let span_json: Option<String> = row.get(3)?;
                let prod: String = row.get(4)?;
                let ver: String = row.get(5)?;

                let span = span_json.and_then(|j| serde_json::from_str(&j).ok());

                Ok(Diagnostic {
                    root: r.clone(),
                    path: p.clone(),
                    message,
                    span,
                    owner: FactOwner::new(r, p, prod, ver),
                })
            })
            .map_err(|e| StoreError::Io(e.to_string()))?;

        let mut diags = Vec::new();
        for r in rows {
            diags.push(r.map_err(|e| StoreError::Io(e.to_string()))?);
        }
        Ok(diags)
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

    fn node_count(&self) -> Result<usize, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT COUNT(DISTINCT node_id) FROM node_claims")
            .map_err(|e| StoreError::Io(e.to_string()))?;
        let count: i64 = stmt
            .query_row([], |r| r.get(0))
            .map_err(|e| StoreError::Io(e.to_string()))?;
        Ok(count.max(0) as usize)
    }

    fn edge_count(&self) -> Result<usize, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT COUNT(DISTINCT edge_id) FROM edge_claims")
            .map_err(|e| StoreError::Io(e.to_string()))?;
        let count: i64 = stmt
            .query_row([], |r| r.get(0))
            .map_err(|e| StoreError::Io(e.to_string()))?;
        Ok(count.max(0) as usize)
    }
}
