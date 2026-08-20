use crate::fts5::{Fts5Index, FtsHit};
use crate::read_view::SqliteReadView;
use crate::schema::SCHEMA_DDL;
use crate::transaction::SqliteTransaction;
use repin_core::ports::store::{ReadView, Store, StoreCapabilities, StoreError, Transaction};
use rusqlite::Connection;
use std::path::Path;
use std::sync::{Arc, Mutex};

pub struct SqliteStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteStore {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StoreError> {
        let conn = Connection::open(path).map_err(|e| StoreError::Io(e.to_string()))?;

        // Check if existing schema is from legacy denormalized version
        let has_legacy_schema: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('node_claims') WHERE name = 'root'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|count| count > 0)
            .unwrap_or(false);

        if has_legacy_schema {
            let _ = conn.execute_batch(
                "DROP TABLE IF EXISTS node_claims;
                 DROP TABLE IF EXISTS edge_claims;
                 DROP TABLE IF EXISTS unresolved_refs;
                 DROP TABLE IF EXISTS skips;
                 DROP TABLE IF EXISTS diagnostics;
                 DROP TABLE IF EXISTS fts_nodes;",
            );
        }

        conn.execute_batch(SCHEMA_DDL)
            .map_err(|e| StoreError::Io(e.to_string()))?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn open_in_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory().map_err(|e| StoreError::Io(e.to_string()))?;
        conn.execute_batch(SCHEMA_DDL)
            .map_err(|e| StoreError::Io(e.to_string()))?;

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

    pub fn raw_connection(&self) -> Arc<Mutex<Connection>> {
        self.conn.clone()
    }
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
