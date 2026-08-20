pub const SCHEMA_DDL: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA synchronous = FULL;
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS string_pool (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    value TEXT UNIQUE NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_string_pool_value ON string_pool(value);

CREATE TABLE IF NOT EXISTS fact_owners (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    root_id INTEGER NOT NULL REFERENCES string_pool(id),
    path_id INTEGER NOT NULL REFERENCES string_pool(id),
    producer_id INTEGER NOT NULL REFERENCES string_pool(id),
    producer_version_id INTEGER NOT NULL REFERENCES string_pool(id),
    UNIQUE(root_id, path_id, producer_id, producer_version_id)
);

CREATE INDEX IF NOT EXISTS idx_fact_owners_path ON fact_owners(path_id);
CREATE INDEX IF NOT EXISTS idx_fact_owners_root_path ON fact_owners(root_id, path_id);

CREATE TABLE IF NOT EXISTS node_claims (
    node_id BLOB NOT NULL,
    owner_id INTEGER NOT NULL REFERENCES fact_owners(id),
    kind TEXT NOT NULL,
    name TEXT NOT NULL,
    qualified_name TEXT,
    range_json TEXT,
    language_id INTEGER REFERENCES string_pool(id),
    artifact_class TEXT,
    provenance_json TEXT,
    attributes_json TEXT,
    PRIMARY KEY (node_id, owner_id)
);

CREATE INDEX IF NOT EXISTS idx_node_claims_name ON node_claims(name);
CREATE INDEX IF NOT EXISTS idx_node_claims_owner ON node_claims(owner_id);

CREATE TABLE IF NOT EXISTS edge_claims (
    edge_id BLOB NOT NULL,
    from_id BLOB NOT NULL,
    to_id BLOB NOT NULL,
    owner_id INTEGER NOT NULL REFERENCES fact_owners(id),
    kind TEXT NOT NULL,
    provenance_json TEXT,
    attributes_json TEXT,
    PRIMARY KEY (edge_id, owner_id)
);

CREATE INDEX IF NOT EXISTS idx_edge_claims_from ON edge_claims(from_id);
CREATE INDEX IF NOT EXISTS idx_edge_claims_to ON edge_claims(to_id);
CREATE INDEX IF NOT EXISTS idx_edge_claims_owner ON edge_claims(owner_id);

CREATE TABLE IF NOT EXISTS unresolved_refs (
    from_id BLOB NOT NULL,
    seeking TEXT NOT NULL,
    scope_hint TEXT,
    edge_kind TEXT NOT NULL,
    owner_id INTEGER NOT NULL REFERENCES fact_owners(id),
    provenance_json TEXT,
    PRIMARY KEY (from_id, seeking, edge_kind)
);

CREATE INDEX IF NOT EXISTS idx_unresolved_seeking ON unresolved_refs(seeking);
CREATE INDEX IF NOT EXISTS idx_unresolved_owner ON unresolved_refs(owner_id);

CREATE TABLE IF NOT EXISTS skips (
    owner_id INTEGER PRIMARY KEY REFERENCES fact_owners(id),
    reason TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS diagnostics (
    owner_id INTEGER NOT NULL REFERENCES fact_owners(id),
    message TEXT NOT NULL,
    span_json TEXT
);

CREATE INDEX IF NOT EXISTS idx_diagnostics_owner ON diagnostics(owner_id);

CREATE TABLE IF NOT EXISTS update_history (
    revision INTEGER PRIMARY KEY,
    summary_json TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS index_state (
    kind TEXT PRIMARY KEY,
    acknowledged_revision INTEGER NOT NULL,
    is_current INTEGER NOT NULL
);

CREATE VIRTUAL TABLE IF NOT EXISTS fts_nodes USING fts5(
    node_id UNINDEXED,
    name,
    qualified_name,
    path,
    attributes,
    tokenize = 'unicode61'
);
"#;
