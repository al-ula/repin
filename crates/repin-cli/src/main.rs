use clap::{Parser, Subcommand};
use repin_cli::client::DaemonClient;
use repin_cli::commands::inspect::execute_inspect;
use repin_cli::commands::search::execute_search;
use repin_cli::commands::status::execute_status;
use repin_cli::discovery::discover_project_from;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "repin", version, about = "Repository intelligence engine")]
struct Cli {
    #[arg(
        short,
        long,
        help = "Path to project root or directory containing .repin"
    )]
    project: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Search the repository using direct pattern/regex match")]
    Search {
        pattern: String,
        #[arg(short, long, help = "Treat pattern as regular expression")]
        regex: bool,
        #[arg(short, long, default_value = "50", help = "Maximum matches to return")]
        limit: usize,
    },
    #[command(about = "Inspect a file's structural outline and declarations")]
    Inspect { path: String },
    #[command(about = "Display daemon connection and graph index status")]
    Status,
    #[command(about = "Run local daemon server in foreground")]
    Daemon {
        #[arg(long, help = "Custom runtime directory")]
        runtime_dir: Option<PathBuf>,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    if let Commands::Daemon { runtime_dir } = cli.command {
        let rt_dir = runtime_dir.unwrap_or_else(DaemonClient::default_runtime_dir);
        println!("Starting Repin daemon in {}", rt_dir.display());
        let server = repin_daemon::DaemonServer::bind(rt_dir)
            .map_err(|e| format!("Failed to bind daemon: {e}"))?;
        server
            .run_loop()
            .map_err(|e| format!("Daemon error: {e}"))?;
        return Ok(());
    }

    let start_path = cli
        .project
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let discovered = discover_project_from(&start_path).unwrap_or_else(|| {
        let default_repin = start_path.join(".repin");
        let default_db = default_repin.join("graph.sqlite3");
        repin_cli::discovery::DiscoveredProject {
            root_dir: start_path.clone(),
            db_path: default_db,
        }
    });

    let mut client = DaemonClient::connect_or_start(&discovered.db_path)
        .map_err(|e| format!("Failed to connect to daemon: {e}"))?;

    match cli.command {
        Commands::Search {
            pattern,
            regex,
            limit,
        } => execute_search(&mut client, &pattern, regex, limit)
            .map_err(|e| format!("Search error: {e}").into()),
        Commands::Inspect { path } => {
            execute_inspect(&mut client, &path).map_err(|e| format!("Inspect error: {e}").into())
        }
        Commands::Status => {
            execute_status(&mut client).map_err(|e| format!("Status error: {e}").into())
        }
        Commands::Daemon { .. } => unreachable!(),
    }
}
