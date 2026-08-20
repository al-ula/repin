use repin_core::config::{ConfigError, Merge, RepinConfig};
use repin_fs::ExclusionFilter;

#[test]
fn test_conformance_default_configuration() {
    let config = RepinConfig::default();
    assert_eq!(config.schema_version, 1);
    assert_eq!(config.project.roots, vec!["."]);
    assert_eq!(config.retrieval.default_mode, "hybrid");
    assert_eq!(config.retrieval.default_limit, 50);
    assert_eq!(config.retrieval.centrality_boost, 0.15);
    assert_eq!(config.context.default_token_budget, 8192);
    assert_eq!(config.context.padding_lines, 2);
    assert!(config.context.include_blast_radius);
    assert!(config.context.include_verbatim_source);
    assert_eq!(config.storage.wal_checkpoint_mode, "truncate");
    assert_eq!(config.daemon.watch_debounce_ms, 150);
    assert!(config.validate().is_ok());
}

#[test]
fn test_conformance_partial_toml_merging() {
    let mut config = RepinConfig::default();
    let project_toml = r#"
        schema_version = 1

        [indexing]
        exclude_paths = ["build/**", "dist/**"]
        exclude_extensions = ["bundle.js", "snap"]
        max_file_size_bytes = 1048576

        [retrieval]
        default_limit = 25
        centrality_boost = 0.25

        [context]
        padding_lines = 4
    "#;

    config.merge_toml_str(project_toml).expect("merge should succeed");

    assert_eq!(config.indexing.exclude_paths, vec!["build/**", "dist/**"]);
    assert_eq!(config.indexing.exclude_extensions, vec!["bundle.js", "snap"]);
    assert_eq!(config.indexing.max_file_size_bytes, 1048576);
    assert_eq!(config.retrieval.default_limit, 25);
    assert_eq!(config.retrieval.centrality_boost, 0.25);
    assert_eq!(config.context.padding_lines, 4);

    // Unspecified fields retain conservative defaults
    assert_eq!(config.retrieval.default_mode, "hybrid");
    assert_eq!(config.context.default_token_budget, 8192);
    assert_eq!(config.storage.wal_checkpoint_mode, "truncate");
}

#[test]
fn test_conformance_precedence_hierarchy() {
    let mut defaults = RepinConfig::default();
    
    // User config layer
    let mut user_config = RepinConfig::default();
    user_config.retrieval.default_limit = 100;
    user_config.indexing.exclude_paths = vec!["global_ignore/**".to_string()];

    defaults.merge(user_config);
    assert_eq!(defaults.retrieval.default_limit, 100);
    assert_eq!(defaults.indexing.exclude_paths, vec!["global_ignore/**"]);

    // Project config layer (higher precedence)
    let mut project_config = RepinConfig::default();
    project_config.retrieval.default_limit = 25;
    project_config.indexing.exclude_paths = vec!["local_build/**".to_string()];

    defaults.merge(project_config);
    assert_eq!(defaults.retrieval.default_limit, 25);
    // Exclusions merge via union
    assert!(defaults.indexing.exclude_paths.contains(&"global_ignore/**".to_string()));
    assert!(defaults.indexing.exclude_paths.contains(&"local_build/**".to_string()));
}

#[test]
fn test_conformance_immutable_safety_floors() {
    let mut config = RepinConfig::default();
    config.indexing.exclude_paths = vec!["custom_dir/**".to_string()];
    config.indexing.exclude_extensions = vec!["custom_ext".to_string()];

    let filter = ExclusionFilter::with_config(&config.indexing);

    // Hardcoded safety floors cannot be un-excluded
    assert!(filter.is_excluded(".git/HEAD"));
    assert!(filter.is_excluded(".repin/graph.sqlite3"));
    assert!(filter.is_excluded(".env"));
    assert!(filter.is_excluded(".env.production"));
    assert!(filter.is_excluded("keys/id_rsa"));
    assert!(filter.is_excluded("certs/server.key"));
    assert!(filter.is_excluded("certs/tls.pem"));
    assert!(filter.is_excluded("node_modules/pkg/index.js"));
    assert!(filter.is_excluded("target/debug/app"));

    // Custom exclusions also match
    assert!(filter.is_excluded("custom_dir/file.txt"));
    assert!(filter.is_excluded("src/file.custom_ext"));

    // Regular source files are allowed
    assert!(!filter.is_excluded("src/main.rs"));
}

#[test]
fn test_conformance_root_traversal_rejection() {
    let toml_str = r#"
        [project]
        roots = ["../outside"]
    "#;
    let result = RepinConfig::from_toml_str(toml_str);
    assert!(result.is_err());
    match result.unwrap_err() {
        ConfigError::ValidationError(msg) => {
            assert!(msg.contains("escapes the repository boundary"));
        }
        err => panic!("unexpected error: {:?}", err),
    }
}
