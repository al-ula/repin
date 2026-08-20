use clap::{Parser, Subcommand};
use repin_cli::client::DaemonClient;
use repin_cli::commands::config::{
    execute_config_init, execute_config_show, execute_config_validate,
};
use repin_cli::commands::context::{execute_context, execute_review_context};
use repin_cli::commands::daemon::{
    execute_daemon_restart, execute_daemon_run, execute_daemon_status, execute_daemon_stop,
};
use repin_cli::commands::eval::execute_eval;
use repin_cli::commands::graph::{execute_entity, execute_neighbors};
use repin_cli::commands::index::{execute_index, execute_init, execute_uninit};
use repin_cli::commands::inspect::{execute_at_position, execute_inspect};
use repin_cli::commands::rerank::execute_rerank;
use repin_cli::commands::search::execute_search;
use repin_cli::commands::status::execute_status;
use repin_cli::commands::update::execute_update;
use repin_cli::commands::watch::execute_watch;
use repin_cli::discovery::{discover_project_from, load_effective_config};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "repin",
    version,
    about = "Repin — Fast, deterministic repository intelligence engine"
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

use repin_cli::commands::model::{
    execute_model_download, execute_model_list, execute_model_remove,
};

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Initialize .repin metadata and index repository root")]
    Init {
        #[arg(help = "Optional project directory path (defaults to current directory or --project)")]
        path: Option<PathBuf>,

        #[arg(long, help = "Skip automatic initial repository indexing")]
        no_index: bool,
    },

    #[command(about = "Remove .repin metadata directory and uninitialize workspace")]
    Uninit {
        #[arg(help = "Optional project directory path (defaults to current directory or --project)")]
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

    #[command(about = "Index all repository files deterministically")]
    Index,

    #[command(about = "Incrementally update graph from VCS worktree changes")]
    Update,

    #[command(
        about = "Search the repository using text, regex, graph symbols, or deterministic hybrid fusion"
    )]
    Search {
        pattern: String,
        #[arg(short, long, help = "Direct regular expression worktree search")]
        regex: bool,
        #[arg(short, long, help = "Symbol graph index search")]
        graph: bool,
        #[arg(long, help = "Hybrid multi-channel search (FTS + Symbol graph)")]
        hybrid: bool,
        #[arg(short, long, help = "Maximum matches to return (defaults to config limit)")]
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

    #[command(about = "Construct budgeted context packed for LLM consumption")]
    Context {
        query: String,
        #[arg(
            short,
            long,
            help = "Maximum context budget in bytes (defaults to config token budget)"
        )]
        budget: Option<usize>,
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
pub enum ConfigAction {
    #[command(about = "Initialize starter config.toml (project-level or user global)")]
    Init {
        #[arg(short, long, help = "Initialize user global configuration (~/.config/repin/config.toml)")]
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
    Run,
    #[command(about = "Stop/kill running background daemon")]
    Stop,
    #[command(about = "Alias for stop")]
    Kill,
    #[command(about = "Restart running background daemon")]
    Restart,
    #[command(about = "Check daemon process and socket status")]
    Status,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    let start_path = cli
        .project
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

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
                format!(
                    "Repin workspace is not initialized (PROJECT_NOT_INITIALIZED). Run `repin init` first."
                )
            })?;
            return execute_daemon_restart(runtime_dir.as_deref(), &discovered.db_path)
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
                    format!(
                        "Repin workspace is not initialized (PROJECT_NOT_INITIALIZED). Run `repin init` first."
                    )
                })?;
                return execute_daemon_restart(runtime_dir.as_deref(), &discovered.db_path)
                    .map_err(|e| format!("Restart error: {e}").into());
            }
            if matches!(action, Some(DaemonAction::Status)) {
                return execute_daemon_status(runtime_dir.as_deref())
                    .map_err(|e| format!("Status error: {e}").into());
            }

            return execute_daemon_run(runtime_dir)
                .map_err(|e| format!("Daemon error: {e}").into());
        }
        _ => {}
    }

    if let Commands::Init { path, no_index } = cli.command {
        let target_path = path.unwrap_or(start_path);
        execute_init(&target_path)?;
        if !no_index {
            let discovered = discover_project_from(&target_path).unwrap_or_else(|| {
                let default_repin = target_path.join(".repin");
                let default_db = default_repin.join("graph.sqlite3");
                repin_cli::discovery::DiscoveredProject {
                    root_dir: target_path.clone(),
                    db_path: default_db,
                }
            });
            let mut client = DaemonClient::connect_or_start(&discovered.db_path)
                .map_err(|e| format!("Failed to connect to daemon: {e}"))?;
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
        format!(
            "Repin workspace is not initialized (PROJECT_NOT_INITIALIZED). Run `repin init` first."
        )
    })?;

    let effective_config = load_effective_config(&discovered.root_dir, cli.config.as_deref())
        .unwrap_or_default();

    let mut client = DaemonClient::connect_or_start(&discovered.db_path)
        .map_err(|e| format!("Failed to connect to daemon: {e}"))?;

    match cli.command {
        Commands::Init { .. }
        | Commands::Uninit { .. }
        | Commands::Config { .. }
        | Commands::Model { .. } => unreachable!(),
        Commands::Daemon { .. } | Commands::Stop { .. } | Commands::Restart { .. } => {
            unreachable!()
        }
        Commands::Index => {
            execute_index(&mut client).map_err(|e| format!("Index error: {e}").into())
        }
        Commands::Update => {
            execute_update(&mut client).map_err(|e| format!("Update error: {e}").into())
        }
        Commands::Search {
            pattern,
            mut regex,
            mut graph,
            mut hybrid,
            limit,
        } => {
            if !regex && !graph && !hybrid {
                match effective_config.retrieval.default_mode.as_str() {
                    "graph" => graph = true,
                    "regex" | "direct" => regex = true,
                    _ => hybrid = true,
                }
            }
            let eff_limit = limit.unwrap_or(effective_config.retrieval.default_limit);
            execute_search(&mut client, &pattern, regex, graph, hybrid, eff_limit)
                .map_err(|e| format!("Search error: {e}").into())
        }
        Commands::Rerank {
            query,
            candidates,
            agent_cmd,
        } => {
            let eff_cmd = agent_cmd.unwrap_or_else(|| effective_config.intelligence.rerank.agent_cmd.clone());
            if eff_cmd.is_empty() {
                return Err("Missing agent callback command. Specify --agent-cmd or configure intelligence.rerank.agent_cmd in config.toml".into());
            }
            execute_rerank(&mut client, &query, candidates, &eff_cmd)
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
        Commands::Context { query, budget } => {
            let eff_budget = budget.unwrap_or(effective_config.context.default_token_budget * 4);
            execute_context(&mut client, &query, eff_budget)
                .map_err(|e| format!("Context error: {e}").into())
        }
        Commands::ReviewContext { since, budget } => {
            execute_review_context(&mut client, since, budget)
                .map_err(|e| format!("ReviewContext error: {e}").into())
        }
        Commands::Watch { interval } => {
            let eff_interval = interval.unwrap_or(effective_config.daemon.watch_debounce_ms.max(100));
            execute_watch(&mut client, eff_interval).map_err(|e| format!("Watch error: {e}").into())
        }
        Commands::Eval => execute_eval(&mut client).map_err(|e| format!("Eval error: {e}").into()),
        Commands::Status => {
            execute_status(&mut client).map_err(|e| format!("Status error: {e}").into())
        }
    }
}
