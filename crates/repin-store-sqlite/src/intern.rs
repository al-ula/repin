use repin_core::model::provenance::FactOwner;
use repin_core::ports::store::StoreError;
use rusqlite::Connection;
use std::collections::HashMap;

#[derive(Default, Debug)]
pub struct InternerCache {
    str_to_id: HashMap<String, i64>,
    id_to_str: HashMap<i64, String>,
    owner_to_id: HashMap<FactOwner, i64>,
    id_to_owner: HashMap<i64, FactOwner>,
}

impl InternerCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_or_insert_string(
        &mut self,
        conn: &Connection,
        val: &str,
    ) -> Result<i64, StoreError> {
        if let Some(&id) = self.str_to_id.get(val) {
            return Ok(id);
        }
        conn.execute(
            "INSERT OR IGNORE INTO string_pool (value) VALUES (?1)",
            [val],
        )
        .map_err(|e| StoreError::Io(e.to_string()))?;

        let id: i64 = conn
            .query_row(
                "SELECT id FROM string_pool WHERE value = ?1",
                [val],
                |row| row.get(0),
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;

        self.str_to_id.insert(val.to_string(), id);
        self.id_to_str.insert(id, val.to_string());
        Ok(id)
    }

    pub fn get_or_insert_owner(
        &mut self,
        conn: &Connection,
        owner: &FactOwner,
    ) -> Result<i64, StoreError> {
        if let Some(&id) = self.owner_to_id.get(owner) {
            return Ok(id);
        }
        let root_id = self.get_or_insert_string(conn, &owner.root)?;
        let path_id = self.get_or_insert_string(conn, &owner.path)?;
        let prod_id = self.get_or_insert_string(conn, &owner.producer)?;
        let ver_id = self.get_or_insert_string(conn, &owner.producer_version)?;

        conn.execute(
            "INSERT OR IGNORE INTO fact_owners (root_id, path_id, producer_id, producer_version_id)
             VALUES (?1, ?2, ?3, ?4)",
            (root_id, path_id, prod_id, ver_id),
        )
        .map_err(|e| StoreError::Io(e.to_string()))?;

        let id: i64 = conn
            .query_row(
                "SELECT id FROM fact_owners
                 WHERE root_id = ?1 AND path_id = ?2 AND producer_id = ?3 AND producer_version_id = ?4",
                (root_id, path_id, prod_id, ver_id),
                |row| row.get(0),
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;

        self.owner_to_id.insert(owner.clone(), id);
        self.id_to_owner.insert(id, owner.clone());
        Ok(id)
    }

    pub fn lookup_owner_id(
        &mut self,
        conn: &Connection,
        owner: &FactOwner,
    ) -> Result<Option<i64>, StoreError> {
        if let Some(&id) = self.owner_to_id.get(owner) {
            return Ok(Some(id));
        }
        let mut stmt = conn
            .prepare(
                "SELECT fo.id FROM fact_owners fo
                 JOIN string_pool sp_r ON fo.root_id = sp_r.id
                 JOIN string_pool sp_p ON fo.path_id = sp_p.id
                 JOIN string_pool sp_prod ON fo.producer_id = sp_prod.id
                 JOIN string_pool sp_v ON fo.producer_version_id = sp_v.id
                 WHERE sp_r.value = ?1 AND sp_p.value = ?2 AND sp_prod.value = ?3 AND sp_v.value = ?4",
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;

        let mut rows = stmt
            .query([
                &owner.root,
                &owner.path,
                &owner.producer,
                &owner.producer_version,
            ])
            .map_err(|e| StoreError::Io(e.to_string()))?;

        if let Some(row) = rows.next().map_err(|e| StoreError::Io(e.to_string()))? {
            let id: i64 = row.get(0).map_err(|e| StoreError::Io(e.to_string()))?;
            self.owner_to_id.insert(owner.clone(), id);
            self.id_to_owner.insert(id, owner.clone());
            Ok(Some(id))
        } else {
            Ok(None)
        }
    }

    pub fn lookup_owner_ids_by_file(
        &self,
        conn: &Connection,
        root: &str,
        path: &str,
    ) -> Result<Vec<i64>, StoreError> {
        let mut stmt = conn
            .prepare(
                "SELECT fo.id FROM fact_owners fo
                 JOIN string_pool sp_r ON fo.root_id = sp_r.id
                 JOIN string_pool sp_p ON fo.path_id = sp_p.id
                 WHERE sp_r.value = ?1 AND sp_p.value = ?2",
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;

        let rows = stmt
            .query_map([root, path], |row| row.get(0))
            .map_err(|e| StoreError::Io(e.to_string()))?;

        let mut ids = Vec::new();
        for r in rows {
            ids.push(r.map_err(|e| StoreError::Io(e.to_string()))?);
        }
        Ok(ids)
    }

    pub fn get_owner_by_id(
        &mut self,
        conn: &Connection,
        owner_id: i64,
    ) -> Result<Option<FactOwner>, StoreError> {
        if let Some(owner) = self.id_to_owner.get(&owner_id) {
            return Ok(Some(owner.clone()));
        }
        let mut stmt = conn
            .prepare(
                "SELECT sp_r.value, sp_p.value, sp_prod.value, sp_v.value
                 FROM fact_owners fo
                 JOIN string_pool sp_r ON fo.root_id = sp_r.id
                 JOIN string_pool sp_p ON fo.path_id = sp_p.id
                 JOIN string_pool sp_prod ON fo.producer_id = sp_prod.id
                 JOIN string_pool sp_v ON fo.producer_version_id = sp_v.id
                 WHERE fo.id = ?1",
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;

        let mut rows = stmt
            .query([owner_id])
            .map_err(|e| StoreError::Io(e.to_string()))?;
        if let Some(row) = rows.next().map_err(|e| StoreError::Io(e.to_string()))? {
            let root: String = row.get(0).map_err(|e| StoreError::Io(e.to_string()))?;
            let path: String = row.get(1).map_err(|e| StoreError::Io(e.to_string()))?;
            let prod: String = row.get(2).map_err(|e| StoreError::Io(e.to_string()))?;
            let ver: String = row.get(3).map_err(|e| StoreError::Io(e.to_string()))?;
            let owner = FactOwner::new(root, path, prod, ver);
            self.owner_to_id.insert(owner.clone(), owner_id);
            self.id_to_owner.insert(owner_id, owner.clone());
            Ok(Some(owner))
        } else {
            Ok(None)
        }
    }
}
