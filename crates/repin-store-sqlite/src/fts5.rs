use repin_core::model::identity::NodeId;
use repin_core::ports::store::StoreError;
use rusqlite::Connection;

#[derive(Debug, Clone, PartialEq)]
pub struct FtsHit {
    pub node_id: NodeId,
    pub name: String,
    pub path: String,
    pub rank: f64,
}

pub struct Fts5Index;

impl Fts5Index {
    pub fn search(conn: &Connection, query: &str, limit: usize) -> Result<Vec<FtsHit>, StoreError> {
        let mut stmt = conn
            .prepare(
                "SELECT node_id, name, path, rank
                 FROM fts_nodes
                 WHERE fts_nodes MATCH ?1
                 ORDER BY rank
                 LIMIT ?2",
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;

        let rows = stmt
            .query_map((query, limit as i64), |row| {
                let node_id_bytes: [u8; 32] = row.get(0)?;
                let name: String = row.get(1)?;
                let path: String = row.get(2)?;
                let rank: f64 = row.get(3)?;

                Ok(FtsHit {
                    node_id: NodeId::from_bytes(node_id_bytes),
                    name,
                    path,
                    rank,
                })
            })
            .map_err(|e| StoreError::Io(e.to_string()))?;

        let mut hits = Vec::new();
        for r in rows {
            hits.push(r.map_err(|e| StoreError::Io(e.to_string()))?);
        }
        Ok(hits)
    }
}
