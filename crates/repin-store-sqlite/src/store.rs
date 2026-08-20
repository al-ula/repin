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
