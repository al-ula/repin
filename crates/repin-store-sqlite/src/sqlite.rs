use super::fts5::{Fts5Index, FtsHit};
use super::read_view::SqliteReadView;
use super::schema::SCHEMA_DDL;
use super::transaction::SqliteTransaction;
use super::{STORE_APPLICATION_ID, STORE_SCHEMA_VERSION};
use repin_core::ports::VersionRecords;
use repin_core::ports::store::{ReadView, Store, StoreCapabilities, StoreError, Transaction};
use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use std::path::Path;
use std::sync::{Arc, Mutex};

pub struct SqliteStore {
    conn: Arc<Mutex<Connection>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct StoreInspection {
    pub application_id: u32,
    pub schema_version: u32,
    pub has_user_tables: bool,
}

impl SqliteStore {
    /// Inspect SQLite identity without applying schema DDL or activating graph state.
    pub fn inspect<P: AsRef<Path>>(path: P) -> Result<StoreInspection, StoreError> {
        let conn = Connection::open(path).map_err(|e| StoreError::Io(e.to_string()))?;
        configure_connection(&conn)?;
        Ok(StoreInspection {
            application_id: pragma_u32(&conn, "application_id")?,
            schema_version: pragma_u32(&conn, "user_version")?,
            has_user_tables: has_user_tables(&conn)?,
        })
    }

    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StoreError> {
        let conn = Connection::open(path).map_err(|e| StoreError::Io(e.to_string()))?;

        configure_connection(&conn)?;
        classify_or_initialize(&conn)?;
        validate_version_records(&conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Run an explicitly authorized migration path. The current schema has no
    /// older supported migration yet, so older state is rejected rather than
    /// changed implicitly.
    pub fn migrate<P: AsRef<Path>>(path: P) -> Result<Self, StoreError> {
        let inspection = Self::inspect(&path)?;
        if inspection.application_id != STORE_APPLICATION_ID {
            return Err(StoreError::Corrupt(format!(
                "unrecognized SQLite application_id {:#x}",
                inspection.application_id
            )));
        }
        if inspection.schema_version == STORE_SCHEMA_VERSION {
            return Self::open(path);
        }
        if inspection.schema_version == 1 {
            let conn = Connection::open(&path).map_err(|e| StoreError::Io(e.to_string()))?;
            configure_connection(&conn)?;
            migrate_v1_to_v2(&conn)?;
            return Self::open(path);
        }
        Err(StoreError::SchemaVersionMismatch {
            found: inspection.schema_version,
            supported: STORE_SCHEMA_VERSION,
        })
    }

    pub fn open_in_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory().map_err(|e| StoreError::Io(e.to_string()))?;
        configure_connection(&conn)?;
        classify_or_initialize(&conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn search_fts(&self, query: &str, limit: usize) -> Result<Vec<FtsHit>, StoreError> {
        let conn = self.conn.lock().unwrap();
        Fts5Index::search(&conn, query, limit)
    }

    pub fn checkpoint(&self) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .map_err(|e| StoreError::Io(e.to_string()))?;
        Ok(())
    }

    /// Reconstruct the SQLite FTS projection from authoritative node claims.
    /// The graph tables and their revision remain unchanged.
    pub fn rebuild_lexical(&self) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| StoreError::Io(e.to_string()))?;
        tx.execute("DELETE FROM fts_nodes", [])
            .map_err(|e| StoreError::Io(e.to_string()))?;
        let mut stmt = tx
            .prepare(
                "SELECT nc.node_id, nc.name, COALESCE(nc.qualified_name, ''),
                        sp_path.value, COALESCE(nc.attributes_json, '')
                 FROM node_claims nc
                 JOIN fact_owners fo ON nc.owner_id = fo.id
                 JOIN string_pool sp_path ON fo.path_id = sp_path.id
                 GROUP BY nc.node_id, nc.name, nc.qualified_name,
                          sp_path.value, nc.attributes_json",
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })
            .map_err(|e| StoreError::Io(e.to_string()))?;
        let values = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| StoreError::Io(e.to_string()))?;
        drop(stmt);
        for (node_id, name, qualified_name, path, attributes) in values {
            tx.execute(
                "INSERT INTO fts_nodes (node_id, name, qualified_name, path, attributes)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                (&node_id, &name, &qualified_name, &path, &attributes),
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;
        }
        let revision: i64 = tx
            .query_row(
                "SELECT CAST(COALESCE((SELECT value FROM meta WHERE key = 'revision'), '0') AS INTEGER)",
                [],
                |row| row.get(0),
            )
            .map_err(|e| StoreError::Io(e.to_string()))?;
        tx.execute(
            "INSERT OR REPLACE INTO index_state
             (kind, acknowledged_revision, is_current) VALUES ('lexical', ?1, 1)",
            [revision],
        )
        .map_err(|e| StoreError::Io(e.to_string()))?;
        tx.commit().map_err(|e| StoreError::Io(e.to_string()))
    }

    pub fn raw_connection(&self) -> Arc<Mutex<Connection>> {
        self.conn.clone()
    }
}

fn configure_connection(conn: &Connection) -> Result<(), StoreError> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL; PRAGMA synchronous = FULL; PRAGMA foreign_keys = ON;",
    )
    .map_err(|e| StoreError::Io(e.to_string()))
}

fn pragma_u32(conn: &Connection, name: &str) -> Result<u32, StoreError> {
    conn.query_row(&format!("PRAGMA {name}"), [], |row| row.get::<_, i64>(0))
        .map(|value| value as u32)
        .map_err(|e| StoreError::Io(e.to_string()))
}

fn has_user_tables(conn: &Connection) -> Result<bool, StoreError> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%')",
        [],
        |row| row.get::<_, i64>(0),
    )
    .map(|value| value != 0)
    .map_err(|e| StoreError::Io(e.to_string()))
}

fn classify_or_initialize(conn: &Connection) -> Result<(), StoreError> {
    let application_id = pragma_u32(conn, "application_id")?;
    let schema_version = pragma_u32(conn, "user_version")?;
    if application_id == 0 && schema_version == 0 && !has_user_tables(conn)? {
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| StoreError::Io(e.to_string()))?;
        tx.execute_batch(SCHEMA_DDL)
            .map_err(|e| StoreError::Io(e.to_string()))?;
        tx.execute_batch(&format!(
            "PRAGMA application_id = {STORE_APPLICATION_ID}; PRAGMA user_version = {STORE_SCHEMA_VERSION};"
        ))
        .map_err(|e| StoreError::Io(e.to_string()))?;
        tx.commit().map_err(|e| StoreError::Io(e.to_string()))?;
        return Ok(());
    }

    if application_id != STORE_APPLICATION_ID {
        return Err(StoreError::Corrupt(format!(
            "unrecognized SQLite application_id {application_id:#x}"
        )));
    }
    if schema_version > STORE_SCHEMA_VERSION {
        return Err(StoreError::SchemaVersionMismatch {
            found: schema_version,
            supported: STORE_SCHEMA_VERSION,
        });
    }
    if schema_version == 0 {
        return Err(StoreError::Corrupt(
            "existing version-zero database requires explicit migration or rebuild".to_string(),
        ));
    }
    if schema_version < STORE_SCHEMA_VERSION {
        return Err(StoreError::SchemaVersionMismatch {
            found: schema_version,
            supported: STORE_SCHEMA_VERSION,
        });
    }
    Ok(())
}

fn validate_version_records(conn: &Connection) -> Result<(), StoreError> {
    let has_meta = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'meta')",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| StoreError::Io(e.to_string()))?
        != 0;
    if !has_meta {
        return Err(StoreError::Corrupt(
            "current Repin schema is missing the meta table".to_string(),
        ));
    }

    let serialized = conn.query_row(
        "SELECT value FROM meta WHERE key = 'version_records'",
        [],
        |row| row.get::<_, String>(0),
    );
    let Ok(serialized) = serialized else {
        return Ok(());
    };
    let records: VersionRecords = serde_json::from_str(&serialized).map_err(|error| {
        StoreError::Corrupt(format!("invalid serialized version records: {error}"))
    })?;
    let schema_version = pragma_u32(conn, "user_version")?;
    if records.store_schema_version != schema_version {
        return Err(StoreError::Corrupt(format!(
            "PRAGMA user_version {schema_version} disagrees with version-record schema {}",
            records.store_schema_version
        )));
    }
    Ok(())
}

fn migrate_v1_to_v2(conn: &Connection) -> Result<(), StoreError> {
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| StoreError::Io(e.to_string()))?;
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS migration_journal (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             from_version INTEGER NOT NULL,
             to_version INTEGER NOT NULL,
             completed_at INTEGER NOT NULL
         );",
    )
    .map_err(|e| StoreError::Io(e.to_string()))?;

    let records = tx
        .query_row(
            "SELECT value FROM meta WHERE key = 'version_records'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| StoreError::Io(e.to_string()))?;
    if let Some(serialized) = records {
        let mut records: VersionRecords = serde_json::from_str(&serialized).map_err(|error| {
            StoreError::Corrupt(format!("invalid serialized version records: {error}"))
        })?;
        if records.store_schema_version != 1 {
            return Err(StoreError::Corrupt(
                "v1 migration requires version records with schema version 1".to_string(),
            ));
        }
        records.store_schema_version = STORE_SCHEMA_VERSION;
        let serialized =
            serde_json::to_string(&records).map_err(|e| StoreError::Io(e.to_string()))?;
        tx.execute(
            "INSERT OR REPLACE INTO meta(key, value) VALUES ('version_records', ?1)",
            [&serialized],
        )
        .map_err(|e| StoreError::Io(e.to_string()))?;
    }
    let completed_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| StoreError::Io(e.to_string()))?
        .as_secs() as i64;
    tx.execute(
        "INSERT INTO migration_journal(from_version, to_version, completed_at)
         VALUES (1, 2, ?1)",
        [completed_at],
    )
    .map_err(|e| StoreError::Io(e.to_string()))?;
    tx.execute_batch("PRAGMA user_version = 2;")
        .map_err(|e| StoreError::Io(e.to_string()))?;
    tx.commit().map_err(|e| StoreError::Io(e.to_string()))
}

impl Store for SqliteStore {
    fn begin_write(&self) -> Result<Box<dyn Transaction>, StoreError> {
        let tx = SqliteTransaction::new(self.conn.clone())?;
        Ok(Box::new(tx))
    }

    fn read_view(&self) -> Result<Box<dyn ReadView>, StoreError> {
        Ok(Box::new(SqliteReadView::new(self.conn.clone())))
    }

    fn capabilities(&self) -> StoreCapabilities {
        StoreCapabilities {
            transactional_ddl: true,
            concurrent_readers: true,
            vectors_native: false,
            lexical_native: true,
            max_batch_size: Some(10_000),
            supports_savepoints: true,
        }
    }

    fn checkpoint(&self) -> Result<(), StoreError> {
        self.checkpoint()
    }
}
