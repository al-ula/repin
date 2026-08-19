pub const SCHEMA_DDL: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA synchronous = FULL;
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS node_claims (
    node_id BLOB NOT NULL,
    root TEXT NOT NULL,
    path TEXT NOT NULL,
    producer TEXT NOT NULL,
    producer_version TEXT NOT NULL,
    kind TEXT NOT NULL,
    name TEXT NOT NULL,
    qualified_name TEXT,
    range_json TEXT,
    language TEXT,
    artifact_class TEXT,
    provenance_json TEXT NOT NULL,
    attributes_json TEXT NOT NULL,
    PRIMARY KEY (node_id, root, path, producer, producer_version)
);

CREATE INDEX IF NOT EXISTS idx_node_claims_name ON node_claims(name);
CREATE INDEX IF NOT EXISTS idx_node_claims_file ON node_claims(root, path);

CREATE TABLE IF NOT EXISTS edge_claims (
    edge_id BLOB NOT NULL,
    from_id BLOB NOT NULL,
    to_id BLOB NOT NULL,
    root TEXT NOT NULL,
    path TEXT NOT NULL,
    producer TEXT NOT NULL,
    producer_version TEXT NOT NULL,
    kind TEXT NOT NULL,
    provenance_json TEXT NOT NULL,
    attributes_json TEXT NOT NULL,
    PRIMARY KEY (edge_id, root, path, producer, producer_version)
);

CREATE INDEX IF NOT EXISTS idx_edge_claims_from ON edge_claims(from_id);
CREATE INDEX IF NOT EXISTS idx_edge_claims_to ON edge_claims(to_id);
CREATE INDEX IF NOT EXISTS idx_edge_claims_file ON edge_claims(root, path);

CREATE TABLE IF NOT EXISTS unresolved_refs (
    from_id BLOB NOT NULL,
    seeking TEXT NOT NULL,
    scope_hint TEXT,
    edge_kind TEXT NOT NULL,
    root TEXT NOT NULL,
    path TEXT NOT NULL,
    provenance_json TEXT NOT NULL,
    PRIMARY KEY (from_id, seeking, edge_kind)
);

CREATE INDEX IF NOT EXISTS idx_unresolved_seeking ON unresolved_refs(seeking);

CREATE TABLE IF NOT EXISTS skips (
    root TEXT NOT NULL,
    path TEXT NOT NULL,
    reason TEXT NOT NULL,
    producer TEXT NOT NULL,
    producer_version TEXT NOT NULL,
    PRIMARY KEY (root, path, producer, producer_version)
);

CREATE TABLE IF NOT EXISTS diagnostics (
    root TEXT NOT NULL,
    path TEXT NOT NULL,
    message TEXT NOT NULL,
    span_json TEXT,
    producer TEXT NOT NULL,
    producer_version TEXT NOT NULL
);

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
