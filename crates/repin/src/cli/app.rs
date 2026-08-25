use crate::cli::client::DaemonClient;
use crate::cli::commands::config::{
    execute_config_init, execute_config_show, execute_config_validate,
};
use crate::cli::commands::context::{execute_context, execute_review_context};
use crate::cli::commands::daemon::{
    execute_daemon_restart, execute_daemon_run, execute_daemon_status, execute_daemon_stop,
};
use crate::cli::commands::eval::execute_eval;
use crate::cli::commands::graph::{
    execute_entity, execute_impact, execute_neighbors, execute_path,
};
use crate::cli::commands::index::{execute_index, execute_init, execute_uninit};
use crate::cli::commands::inspect::{execute_at_position, execute_inspect};
use crate::cli::commands::install::execute_install;
use crate::cli::commands::rebuild::execute_rebuild;
use crate::cli::commands::rerank::execute_rerank;
use crate::cli::commands::search::execute_search;
use crate::cli::commands::status::execute_status;
use crate::cli::commands::sync::execute_sync;
use crate::cli::commands::update::execute_update;
use crate::cli::commands::watch::execute_watch;
use crate::cli::discovery::{discover_project_from, load_effective_config};
use crate::product::ProjectLayout;
use clap::{Parser, Subcommand};
use repin_core::protocol::{ipc::RebuildTarget, PROTOCOL_MAX, PROTOCOL_MIN};
use repin_core::store::SqliteStore;
use repin_core::store::{STORE_FORMAT_ID, STORE_SCHEMA_VERSION};
use repin_core::versions::{
    ATTRIBUTE_REGISTRY_VERSION, CLASSIFICATION_VERSION, KIND_REGISTRY_VERSION, RESOLUTION_VERSION,
};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "repin",
    version = env!("REPIN_DISPLAY_VERSION"),
    about = "Repin — Fast repository intelligence engine"
)]
struct Cli {
    #[arg(
        short,
        long,
        help = "Path to project root or directory containing .repin"
    )]
    project: Option<PathBuf>,

    #[arg(
        short = 'c',
        long = "config",
        help = "Explicit path to configuration file (repin.toml or config.toml)"
    )]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

use crate::cli::commands::model::{
    execute_model_download, execute_model_list, execute_model_remove,
};

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Display package and compatibility version diagnostics")]
    Version {
        #[arg(long, help = "Emit structured JSON diagnostics")]
        json: bool,
    },
    #[command(about = "Inspect or explicitly migrate SQLite project state")]
    Db {
        #[command(subcommand)]
        action: DbAction,
    },
    #[command(about = "Initialize .repin metadata and index repository root")]
    Init {
        #[arg(
            help = "Optional project directory path (defaults to current directory or --project)"
        )]
        path: Option<PathBuf>,

        #[arg(long, help = "Skip automatic initial repository indexing")]
        no_index: bool,
    },

    #[command(about = "Remove .repin metadata directory and uninitialize workspace")]
    Uninit {
        #[arg(
            help = "Optional project directory path (defaults to current directory or --project)"
        )]
        path: Option<PathBuf>,

        #[arg(
            short,
            long,
            short_alias = 'y',
            alias = "yes",
            help = "Force removal without interactive confirmation"
        )]
        force: bool,
    },

    #[command(about = "Manage repository configuration (config.toml)")]
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    #[command(about = "Manage local and downloaded embedding/reranker models")]
    Model {
        #[command(subcommand)]
        action: ModelAction,
    },

    #[command(about = "Index all repository files")]
    Index,

    #[command(about = "Incrementally update graph from VCS worktree changes")]
    Sync,

    #[command(about = "Install Repin and bundled documentation into ~/.local/share/repin")]
    Install {
        #[arg(help = "Optional source directory containing repin binary and bundled assets")]
        source: Option<PathBuf>,
    },

    #[command(about = "Check for or install the latest Repin release from GitHub")]
    Update {
        #[arg(
            long,
            short,
            help = "Only check if an update is available without downloading or installing"
        )]
        check: bool,

        #[arg(
            long,
            short,
            help = "Force reinstall even if already on the latest version"
        )]
        force: bool,
    },

    #[command(about = "Check if an update is available from GitHub")]
    CheckUpdate,

    #[command(about = "Rebuild authoritative graph or a derived index")]
    Rebuild {
        #[arg(value_enum)]
        target: RebuildTargetArg,
    },

    #[command(about = "Search the repository using text, regex, graph symbols, or hybrid fusion")]
    Search {
        pattern: String,
        #[arg(short, long, help = "Direct regular expression worktree search")]
        regex: bool,
        #[arg(short, long, help = "Symbol graph index search")]
        graph: bool,
        #[arg(long, help = "Hybrid multi-channel search (FTS + Symbol graph)")]
        hybrid: bool,
        #[arg(
            long,
            value_enum,
            help = "Search channel: direct, regex, graph, or hybrid (overrides config default_mode)"
        )]
        mode: Option<SearchModeArg>,
        #[arg(
            long,
            help = "Graph degree-centrality boost applied in hybrid ranking (overrides config centrality_boost)"
        )]
        boost: Option<f64>,
        #[arg(
            short,
            long,
            help = "Maximum matches to return (defaults to config limit)"
        )]
        limit: Option<usize>,
    },

    #[command(about = "Rerank candidate symbols using an agent shell callback command")]
    Rerank {
        #[arg(help = "User query or search intent")]
        query: String,
        #[arg(
            long = "agent-cmd",
            short = 'c',
            help = "Shell callback command to invoke for AI reranking (defaults to config agent_cmd)"
        )]
        agent_cmd: Option<String>,
        #[arg(
            long,
            help = "Maximum number of candidates to send/return (overrides config rerank.top_n)"
        )]
        top_n: Option<usize>,
        #[arg(
            long,
            help = "Hard deadline in milliseconds for the agent callback (overrides config rerank.deadline_ms)"
        )]
        deadline_ms: Option<u64>,
        #[arg(
            help = "Optional candidate symbol names or entity IDs. If omitted, top candidates are automatically retrieved from search"
        )]
        candidates: Vec<String>,
    },

    #[command(about = "Inspect a file's structural AST outline and declared symbols")]
    Inspect { path: String },

    #[command(about = "Resolve AST symbol definition at a specific file coordinate")]
    AtPosition {
        path: String,
        line: u32,
        column: u32,
    },

    #[command(about = "Lookup detailed metadata for an entity by name or node ID")]
    Entity { name_or_id: String },

    #[command(about = "Display graph relationship neighbors (callers, callees, definitions)")]
    Neighbors {
        name_or_id: String,
        #[arg(short, long, default_value = "1", help = "Maximum traversal depth")]
        max_depth: usize,
    },

    #[command(
        about = "Analyze downstream/upstream blast radius of modifying a symbol or file (ADR-025)"
    )]
    Impact {
        name_or_id: String,
        #[arg(short, long, default_value = "3", help = "Maximum traversal depth")]
        max_depth: usize,
        #[arg(long, help = "Emit structured JSON envelope")]
        json: bool,
    },

    #[command(about = "Trace shortest dependency or call path connecting two symbols (ADR-025)")]
    Path {
        from: String,
        to: String,
        #[arg(short, long, default_value = "5", help = "Maximum path search depth")]
        max_depth: usize,
        #[arg(long, help = "Emit structured JSON envelope")]
        json: bool,
    },

    #[command(about = "Construct budgeted context packed for LLM consumption")]
    Context {
        query: String,
        #[arg(
            short,
            long,
            help = "Maximum context budget in bytes (defaults to config token budget)"
        )]
        budget: Option<usize>,
        #[arg(
            long,
            help = "Padding lines around each source range (overrides config padding_lines)"
        )]
        padding_lines: Option<usize>,
        #[arg(long, help = "Disable blast-radius neighbor expansion in context")]
        no_blast_radius: bool,
        #[arg(long, help = "Disable verbatim source inclusion in context")]
        no_verbatim_source: bool,
    },

    #[command(about = "Construct review context focused on changed files and impact (ADR-016)")]
    ReviewContext {
        #[arg(long, help = "Base revision number to compute changes since")]
        since: Option<u64>,
        #[arg(
            short,
            long,
            default_value = "65536",
            help = "Maximum review budget in bytes"
        )]
        budget: usize,
    },

    #[command(about = "Continuously watch repository worktree for changes")]
    Watch {
        #[arg(
            short,
            long,
            help = "Polling interval in milliseconds (defaults to config debounce)"
        )]
        interval: Option<u64>,
    },

    #[command(about = "Run Precision-at-N retrieval evaluation suite")]
    Eval,

    #[command(about = "Display daemon connection, graph revision, and index status")]
    Status,

    #[command(about = "Manage background daemon server (run, stop/kill, restart, status)")]
    Daemon {
        #[command(subcommand)]
        action: Option<DaemonAction>,

        #[arg(long, help = "Custom runtime directory")]
        runtime_dir: Option<PathBuf>,

        #[arg(long, help = "Stop/kill running daemon")]
        stop: bool,

        #[arg(long, help = "Restart running daemon")]
        restart: bool,
    },

    #[command(about = "Stop/kill running background daemon")]
    Stop {
        #[arg(long, help = "Custom runtime directory")]
        runtime_dir: Option<PathBuf>,
    },

    #[command(about = "Restart running background daemon")]
    Restart {
        #[arg(long, help = "Custom runtime directory")]
        runtime_dir: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug, Clone)]
enum DbAction {
    #[command(about = "Inspect SQLite identity and schema without graph activation")]
    Inspect {
        #[arg(help = "SQLite database path (defaults to .repin/graph.sqlite3)")]
        path: Option<PathBuf>,
        #[arg(long, help = "Emit inspection as JSON")]
        json: bool,
    },
    #[command(about = "Run an explicitly authorized SQLite migration")]
    Migrate {
        #[arg(help = "SQLite database path (defaults to .repin/graph.sqlite3)")]
        path: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum ConfigAction {
    #[command(about = "Initialize starter config.toml (project-level or user global)")]
    Init {
        #[arg(
            short,
            long,
            help = "Initialize user global configuration (~/.config/repin/config.toml)"
        )]
        global: bool,
        #[arg(short, long, help = "Overwrite existing configuration if present")]
        force: bool,
    },
    #[command(about = "Display effective merged configuration in TOML format")]
    Show,
    #[command(about = "Validate syntax and schema of configuration")]
    Validate,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ModelAction {
    #[command(about = "Download model weights from Hugging Face Hub into local cache")]
    Download {
        #[arg(help = "Hugging Face model ID (e.g. Alibaba-NLP/gte-modernbert-base)")]
        model: String,
    },
    #[command(about = "List all models stored in local cache (~/.cache/repin/models/)")]
    List,
    #[command(about = "Remove a downloaded model from local cache")]
    Remove {
        #[arg(help = "Hugging Face model ID (e.g. Alibaba-NLP/gte-modernbert-base)")]
        model: String,
    },
}

#[derive(Subcommand, Debug, Clone)]
enum DaemonAction {
    #[command(about = "Run daemon server in foreground")]
    Run {
        #[arg(
            long,
            help = "Idle timeout in seconds before a detached context is reaped (overrides config idle_timeout_secs)"
        )]
        idle_timeout: Option<u64>,
    },
    #[command(about = "Stop/kill running background daemon")]
    Stop,
    #[command(about = "Alias for stop")]
    Kill,
    #[command(about = "Restart running background daemon")]
    Restart,
    #[command(about = "Check daemon process and socket status")]
    Status,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum RebuildTargetArg {
    Graph,
    Lexical,
    Vector,
    All,
}

impl From<RebuildTargetArg> for RebuildTarget {
    fn from(value: RebuildTargetArg) -> Self {
        match value {
            RebuildTargetArg::Graph => Self::Graph,
            RebuildTargetArg::Lexical => Self::Lexical,
            RebuildTargetArg::Vector => Self::Vector,
            RebuildTargetArg::All => Self::All,
        }
    }
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum SearchModeArg {
    Direct,
    Regex,
    Graph,
    Hybrid,
}

/// Resolve the three search-channel booleans from an explicit `--mode`, the
/// legacy channel flags, or the configured `retrieval.default_mode`.
fn resolve_search_mode(
    mode: Option<SearchModeArg>,
    regex: bool,
    graph: bool,
    hybrid: bool,
    default_mode: &str,
) -> (bool, bool, bool) {
    if regex {
        return (true, false, false);
    }
    if graph && !hybrid {
        return (false, true, false);
    }
    if hybrid {
        return (false, false, true);
    }
    match mode {
        Some(SearchModeArg::Direct) | Some(SearchModeArg::Regex) => (true, false, false),
        Some(SearchModeArg::Graph) => (false, true, false),
        Some(SearchModeArg::Hybrid) => (false, false, true),
        None => match default_mode {
            "graph" => (false, true, false),
            "regex" | "direct" => (true, false, false),
            _ => (false, false, true),
        },
    }
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let start_path = cli
        .project
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    if let Commands::Version { json } = cli.command {
        print_version(json)?;
        return Ok(());
    }

    if let Commands::Db { ref action } = cli.command {
        let default_db = ProjectLayout::at_root(&start_path).db_path;
        match action {
            DbAction::Inspect { path, json } => {
                let db_path = path.clone().unwrap_or(default_db);
                let inspection = SqliteStore::inspect(&db_path)
                    .map_err(|error| format!("db inspect failed: {error}"))?;
                if *json {
                    println!("{}", serde_json::to_string_pretty(&inspection)?);
                } else {
                    println!("path: {}", db_path.display());
                    println!("application_id: {:#x}", inspection.application_id);
                    println!("schema_version: {}", inspection.schema_version);
                    println!("has_user_tables: {}", inspection.has_user_tables);
                }
                return Ok(());
            }
            DbAction::Migrate { path } => {
                let db_path = path.clone().unwrap_or(default_db);
                SqliteStore::migrate(&db_path)
                    .map_err(|error| format!("db migrate failed: {error}"))?;
                println!("SQLite state is at the current supported schema.");
                return Ok(());
            }
        }
    }

    // Config management commands
    if let Commands::Config { ref action } = cli.command {
        match action {
            ConfigAction::Init { global, force } => {
                execute_config_init(cli.project, *global, *force)?;
                return Ok(());
            }
            ConfigAction::Show => {
                execute_config_show(cli.project, cli.config)?;
                return Ok(());
            }
            ConfigAction::Validate => {
                execute_config_validate(cli.project, cli.config)?;
                return Ok(());
            }
        }
    }

    // Model management commands
    if let Commands::Model { ref action } = cli.command {
        match action {
            ModelAction::Download { model } => {
                execute_model_download(model)?;
                return Ok(());
            }
            ModelAction::List => {
                execute_model_list()?;
                return Ok(());
            }
            ModelAction::Remove { model } => {
                execute_model_remove(model)?;
                return Ok(());
            }
        }
    }

    // Daemon lifecycle commands
    match cli.command {
        Commands::Stop { runtime_dir } => {
            return execute_daemon_stop(runtime_dir.as_deref())
                .map_err(|e| format!("Stop error: {e}").into());
        }
        Commands::Restart { runtime_dir } => {
            let discovered = discover_project_from(&start_path).ok_or_else(|| {
                "Repin workspace is not initialized (PROJECT_NOT_INITIALIZED). Run `repin init` first."
                    .to_string()
            })?;
            let resolved_config =
                load_effective_config(&discovered.root_dir, cli.config.as_deref())
                    .unwrap_or_default();
            return execute_daemon_restart(
                runtime_dir.as_deref(),
                &discovered.db_path,
                &resolved_config,
            )
            .map_err(|e| format!("Restart error: {e}").into());
        }
        Commands::Daemon {
            action,
            runtime_dir,
            stop,
            restart,
        } => {
            if stop || matches!(action, Some(DaemonAction::Stop | DaemonAction::Kill)) {
                return execute_daemon_stop(runtime_dir.as_deref())
                    .map_err(|e| format!("Stop error: {e}").into());
            }
            if restart || matches!(action, Some(DaemonAction::Restart)) {
                let discovered = discover_project_from(&start_path).ok_or_else(|| {
                    "Repin workspace is not initialized (PROJECT_NOT_INITIALIZED). Run `repin init` first."
                        .to_string()
                })?;
                let resolved_config =
                    load_effective_config(&discovered.root_dir, cli.config.as_deref())
                        .unwrap_or_default();
                return execute_daemon_restart(
                    runtime_dir.as_deref(),
                    &discovered.db_path,
                    &resolved_config,
                )
                .map_err(|e| format!("Restart error: {e}").into());
            }
            if matches!(action, Some(DaemonAction::Status)) {
                return execute_daemon_status(runtime_dir.as_deref())
                    .map_err(|e| format!("Status error: {e}").into());
            }

            let idle_timeout = match &action {
                Some(DaemonAction::Run { idle_timeout }) => *idle_timeout,
                _ => None,
            };
            return execute_daemon_run(runtime_dir, idle_timeout)
                .map_err(|e| format!("Daemon error: {e}").into());
        }
        _ => {}
    }

    // Installation and updater commands (do not require project initialization)
    if let Commands::Install { source } = cli.command {
        execute_install(source).map_err(|e| format!("Install error: {e}"))?;
        return Ok(());
    }

    if let Commands::CheckUpdate = cli.command {
        execute_update(true, false).map_err(|e| format!("Update error: {e}"))?;
        return Ok(());
    }

    if let Commands::Update { check, force } = cli.command {
        execute_update(check, force).map_err(|e| format!("Update error: {e}"))?;
        return Ok(());
    }

    if let Commands::Init { path, no_index } = cli.command {
        let target_path = path.unwrap_or(start_path);
        let resolved_config =
            load_effective_config(&target_path, cli.config.as_deref()).unwrap_or_default();
        let mut client = execute_init(&target_path, Some(resolved_config))?;
        if !no_index {
            execute_index(&mut client).map_err(|e| format!("Index error: {e}"))?;
        }
        return Ok(());
    }

    if let Commands::Uninit { path, force } = cli.command {
        let target_path = path.unwrap_or(start_path);
        execute_uninit(&target_path, force)?;
        return Ok(());
    }

    let discovered = discover_project_from(&start_path).ok_or_else(|| {
        "Repin workspace is not initialized (PROJECT_NOT_INITIALIZED). Run `repin init` first."
            .to_string()
    })?;

    let effective_config =
        load_effective_config(&discovered.root_dir, cli.config.as_deref()).unwrap_or_default();

    let mut client = DaemonClient::connect_or_start(&discovered.db_path, &effective_config)
        .map_err(|e| format!("Failed to connect to daemon: {e}"))?;

    match cli.command {
        Commands::Version { .. }
        | Commands::Db { .. }
        | Commands::Install { .. }
        | Commands::Update { .. }
        | Commands::CheckUpdate
        | Commands::Init { .. }
        | Commands::Uninit { .. }
        | Commands::Config { .. }
        | Commands::Model { .. } => unreachable!(),
        Commands::Daemon { .. } | Commands::Stop { .. } | Commands::Restart { .. } => {
            unreachable!()
        }
        Commands::Index => {
            execute_index(&mut client).map_err(|e| format!("Index error: {e}").into())
        }
        Commands::Sync => execute_sync(&mut client).map_err(|e| format!("Sync error: {e}").into()),
        Commands::Rebuild { target } => execute_rebuild(&mut client, target.into())
            .map_err(|e| format!("Rebuild error: {e}").into()),
        Commands::Search {
            pattern,
            regex,
            graph,
            hybrid,
            mode,
            boost,
            limit,
        } => {
            let (is_regex, use_graph, use_hybrid) = resolve_search_mode(
                mode,
                regex,
                graph,
                hybrid,
                &effective_config.retrieval.default_mode,
            );
            let eff_limit = limit.unwrap_or(effective_config.retrieval.default_limit);
            execute_search(
                &mut client,
                &pattern,
                is_regex,
                use_graph,
                use_hybrid,
                eff_limit,
                boost,
            )
            .map_err(|e| format!("Search error: {e}").into())
        }
        Commands::Rerank {
            query,
            candidates,
            agent_cmd,
            top_n,
            deadline_ms,
        } => {
            let eff_cmd =
                agent_cmd.unwrap_or_else(|| effective_config.intelligence.rerank.agent_cmd.clone());
            if eff_cmd.is_empty() {
                return Err("Missing agent callback command. Specify --agent-cmd or configure intelligence.rerank.agent_cmd in config.toml".into());
            }
            execute_rerank(
                &mut client,
                &query,
                candidates,
                &eff_cmd,
                top_n,
                deadline_ms,
            )
            .map_err(|e| format!("Rerank error: {e}").into())
        }
        Commands::Inspect { path } => {
            execute_inspect(&mut client, &path).map_err(|e| format!("Inspect error: {e}").into())
        }
        Commands::AtPosition { path, line, column } => {
            execute_at_position(&mut client, &path, line, column)
                .map_err(|e| format!("AtPosition error: {e}").into())
        }
        Commands::Entity { name_or_id } => execute_entity(&mut client, &name_or_id)
            .map_err(|e| format!("Entity error: {e}").into()),
        Commands::Neighbors {
            name_or_id,
            max_depth,
        } => execute_neighbors(&mut client, &name_or_id, max_depth)
            .map_err(|e| format!("Neighbors error: {e}").into()),
        Commands::Impact {
            name_or_id,
            max_depth,
            json,
        } => execute_impact(&mut client, &name_or_id, max_depth, json)
            .map_err(|e| format!("Impact error: {e}").into()),
        Commands::Path {
            from,
            to,
            max_depth,
            json,
        } => execute_path(&mut client, &from, &to, max_depth, json)
            .map_err(|e| format!("Path error: {e}").into()),
        Commands::Context {
            query,
            budget,
            padding_lines,
            no_blast_radius,
            no_verbatim_source,
        } => {
            let eff_budget = budget.unwrap_or(effective_config.context.default_token_budget * 4);
            let ctx_override = if padding_lines.is_some() || no_blast_radius || no_verbatim_source {
                Some(repin_core::config::ContextConfig {
                    default_token_budget: eff_budget / 4,
                    padding_lines: padding_lines.unwrap_or(0),
                    include_blast_radius: !no_blast_radius,
                    include_verbatim_source: !no_verbatim_source,
                })
            } else {
                None
            };
            execute_context(&mut client, &query, eff_budget, ctx_override)
                .map_err(|e| format!("Context error: {e}").into())
        }
        Commands::ReviewContext { since, budget } => {
            execute_review_context(&mut client, since, budget)
                .map_err(|e| format!("ReviewContext error: {e}").into())
        }
        Commands::Watch { interval } => {
            let eff_interval =
                interval.unwrap_or(effective_config.daemon.watch_debounce_ms.max(100));
            execute_watch(&mut client, eff_interval).map_err(|e| format!("Watch error: {e}").into())
        }
        Commands::Eval => execute_eval(&mut client).map_err(|e| format!("Eval error: {e}").into()),
        Commands::Status => {
            execute_status(&mut client).map_err(|e| format!("Status error: {e}").into())
        }
    }
}

#[derive(Debug, Serialize)]
struct VersionDiagnostics {
    version: &'static str,
    package_version: &'static str,
    commit: Option<&'static str>,
    build_id: Option<&'static str>,
    target: String,
    protocol_min: u32,
    protocol_max: u32,
    store_format: &'static str,
    store_schema_version: u32,
    kind_registry_version: u32,
    attribute_registry_version: u32,
    classification_version: u32,
    resolution_version: u32,
}

fn print_version(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    if !json {
        println!("repin {}", env!("REPIN_DISPLAY_VERSION"));
        return Ok(());
    }
    let diagnostics = VersionDiagnostics {
        version: env!("REPIN_DISPLAY_VERSION"),
        package_version: env!("CARGO_PKG_VERSION"),
        commit: option_env!("REPIN_GIT_COMMIT"),
        build_id: option_env!("REPIN_BUILD_ID"),
        target: env!("REPIN_TARGET").to_owned(),
        protocol_min: PROTOCOL_MIN,
        protocol_max: PROTOCOL_MAX,
        store_format: STORE_FORMAT_ID,
        store_schema_version: STORE_SCHEMA_VERSION,
        kind_registry_version: KIND_REGISTRY_VERSION,
        attribute_registry_version: ATTRIBUTE_REGISTRY_VERSION,
        classification_version: CLASSIFICATION_VERSION,
        resolution_version: RESOLUTION_VERSION,
    };
    println!("{}", serde_json::to_string_pretty(&diagnostics)?);
    Ok(())
}
