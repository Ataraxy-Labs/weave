mod commands;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "weave", about = "Entity-level semantic merge for Git")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Configure the current Git repo to use weave as merge driver
    Setup {
        /// Path to weave-driver binary (auto-detected if omitted)
        #[arg(long)]
        driver: Option<String>,
    },
    /// Preview what a merge between branches would look like
    Preview {
        /// The branch to merge into HEAD
        branch: String,
        /// Optional: preview a specific file only
        #[arg(long)]
        file: Option<String>,
    },
    /// Show entity and agent state from CRDT
    Status {
        /// Show entities for a specific file
        #[arg(long)]
        file: Option<String>,
        /// Show status for a specific agent
        #[arg(long)]
        agent: Option<String>,
    },
    /// Claim an entity before editing
    Claim {
        /// Agent identifier
        agent_id: String,
        /// File path containing the entity
        file_path: String,
        /// Entity name to claim
        entity_name: String,
    },
    /// Run merge benchmarks comparing weave vs git line-level merge
    Bench,
    /// Benchmark against real merge commits in an existing repo
    BenchRepo {
        /// Path to a git repository
        repo: String,
        /// Max merge commits to scan
        #[arg(long, default_value_t = 500)]
        limit: usize,
        /// Show line-level diff for weave vs human mismatches
        #[arg(long)]
        show_diff: bool,
    },
    /// Parse weave conflict markers and show a structured summary
    Summary {
        /// Path to a file containing weave conflict markers
        file: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Release a previously claimed entity
    Release {
        /// Agent identifier
        agent_id: String,
        /// File path containing the entity
        file_path: String,
        /// Entity name to release
        entity_name: String,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Setup { ref driver } => {
            commands::setup::run(driver.as_deref())
        }
        Commands::Preview { ref branch, ref file } => {
            commands::preview::run(branch, file.as_deref())
        }
        Commands::Status { ref file, ref agent } => {
            commands::status::run(file.as_deref(), agent.as_deref())
        }
        Commands::Bench => commands::bench::run(),
        Commands::BenchRepo { ref repo, limit, show_diff } => commands::bench_repo::run(repo, limit, show_diff),
        Commands::Summary { ref file, json } => {
            commands::summary::run(file, json)
        }
        Commands::Claim {
            ref agent_id,
            ref file_path,
            ref entity_name,
        } => commands::claim::run(agent_id, file_path, entity_name),
        Commands::Release {
            ref agent_id,
            ref file_path,
            ref entity_name,
        } => commands::release::run(agent_id, file_path, entity_name),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
