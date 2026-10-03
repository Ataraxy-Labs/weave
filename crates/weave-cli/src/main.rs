mod commands;

use clap::{Parser, Subcommand};

const QUICKSTART: &str = "\
QUICKSTART
  weave setup                 configure this repo, then `git merge` as normal
  # ... conflict? read the `refused_by:` line inside the markers, then:
  weave explain <file>        per-hunk detail: which lines both sides wrote
  # ... edit the file to resolve it, then:
  weave check                 verify the resolution against the merge stages
  # ... or let a resolver of your choice do it, behind proof and a gate:
  weave land --resolver <cmd> label each file PROVEN / VERIFIED / REFUSED
  weave setup --global        make weave the default driver for every repo";

const LAND_EXAMPLES: &str = "\
EXAMPLES
  git merge agent/feature               # stops on conflicts
  weave land --resolver 'python3 scripts/weave-land-resolver.py' \\
             --certificate land.json    # PROVEN / VERIFIED / REFUSED per file
  git commit                            # refuses while any file is REFUSED

  weave land --ours main --theirs agent/feature --json   # CI: report only";

#[derive(Parser)]
#[command(
    name = "weave",
    about = "Entity-level semantic merge for Git",
    version,
    after_long_help = QUICKSTART
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Configure Git to use weave as merge driver (this repo, or --global for all repos)
    Setup {
        /// Path to weave-driver binary (auto-detected if omitted)
        #[arg(long)]
        driver: Option<String>,
        /// Write merge attributes to .git/info/attributes instead of .gitattributes
        #[arg(long)]
        local: bool,
        /// Configure git globally (~/.gitconfig + global attributes) so weave is
        /// the default driver in every repo. No git repo required.
        #[arg(long)]
        global: bool,
    },
    /// Remove weave merge driver from the current Git repo
    Unsetup,
    /// Materialize entity edits from the CRDT back onto the working files
    Apply {
        /// One or more files to write from the CRDT
        files: Vec<String>,
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
    /// Explain ONE file's conflicts: which guard refused, and the hunks BOTH
    /// sides wrote in. Reads the three merge stages out of the index, so it
    /// describes the conflict git actually produced.
    Explain {
        /// The conflicted file to explain
        file: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Verify your resolution. With no arguments this checks the WORKING TREE
    /// against the three merge stages — markers left behind, lines both sides
    /// kept that went missing, anything stated more often than either side
    /// stated it, references that no longer resolve — and prints one verdict
    /// line per file. With --base/--ours/--theirs (or --*-dir) it runs the
    /// cross-file binding pass between two revisions instead and emits
    /// weave-findings JSON. Exits 1 when there are findings, and 2 when it
    /// could not verify at all — git failed, or the run passed --timeout.
    Check {
        /// Output as JSON (working-tree mode)
        #[arg(long)]
        json: bool,
        /// Merge base revision (default: merge-base of --ours and --theirs)
        #[arg(long)]
        base: Option<String>,
        /// Our side (default: HEAD)
        #[arg(long)]
        ours: Option<String>,
        /// Their side (default: MERGE_HEAD, i.e. the merge in progress)
        #[arg(long)]
        theirs: Option<String>,
        /// No-git mode: directory holding the base tree
        #[arg(long)]
        base_dir: Option<String>,
        /// No-git mode: directory holding our tree
        #[arg(long)]
        ours_dir: Option<String>,
        /// No-git mode: directory holding their tree
        #[arg(long)]
        theirs_dir: Option<String>,
        /// Give up after this many seconds with exit code 2 and a message,
        /// rather than run on. 0 means no limit.
        #[arg(long, default_value_t = 300)]
        timeout: u64,
    },
    /// Land a merge and label every file git could not merge PROVEN, VERIFIED
    /// or REFUSED.
    ///
    /// Run it where `git merge` stopped (or give --base/--ours/--theirs to
    /// read a merge without touching anything). For each file git's line merge
    /// conflicts on:
    ///
    ///   1. weave merges it. A clean result that the independent merge
    ///      certificate proves is the three-way selection is PROVEN. A clean
    ///      element union (entries two sides added to one map, switch, test
    ///      table...) that the certificate's own element check admits, and
    ///      that passes the gate of step 3, is VERIFIED with no resolver call.
    ///
    ///   2. Anything else goes to --resolver <cmd>, any program: it gets the
    ///      file's base/ours/theirs/conflicted text and path as JSON on stdin
    ///      and prints the resolved file, or one line DELETE / KEEP
    ///      (modify/delete only) / CANNOT[: reason].
    ///
    ///   3. The answer must pass the exact gate: no new marker lines; parses
    ///      when both sides parse; `weave check` finds nothing (lost or
    ///      duplicated lines, dangling names, duplicate data keys,
    ///      modify/delete); every line git merged automatically survives;
    ///      inside each conflict block both sides' changes survive (an answer
    ///      that keeps one side of a block drops the other's: DROPPED). One
    ///      retry, with the findings fed back. Pass: VERIFIED. Fail: REFUSED.
    ///
    /// PROVEN and VERIFIED files are written and staged. A REFUSED file keeps
    /// its conflict markers and stays unmerged; a resolver's rejected answer
    /// is never written. Files both sides changed that git merges line-cleanly
    /// are counted, not examined.
    ///
    /// Exit 0: every file PROVEN or VERIFIED. 1: some file REFUSED. 2: the run
    /// failed and nothing was landed.
    #[command(after_long_help = LAND_EXAMPLES)]
    Land {
        /// Resolver command, run with `sh -c` once per attempt (see above)
        #[arg(long, value_name = "CMD")]
        resolver: Option<String>,
        /// Give up on one resolver call after this many seconds
        #[arg(long, default_value_t = 900, value_name = "SECS")]
        resolver_timeout: u64,
        /// Print the report as JSON (the same document --certificate writes)
        #[arg(long)]
        json: bool,
        /// Write the review certificate (JSON: every file's status, rule,
        /// reason and findings, with the merge's three commits) to this file
        #[arg(long, value_name = "FILE")]
        certificate: Option<String>,
        /// Decide and report, but write nothing to the working tree or index
        #[arg(long)]
        dry_run: bool,
        /// Merge base revision (default: merge-base of --ours and --theirs).
        /// Any of --base/--ours/--theirs implies --dry-run.
        #[arg(long)]
        base: Option<String>,
        /// Our side (default: HEAD)
        #[arg(long)]
        ours: Option<String>,
        /// Their side (default: MERGE_HEAD, i.e. the merge in progress)
        #[arg(long)]
        theirs: Option<String>,
    },
    /// Typed entity ops: the write side of the agent contract. Extract the ops
    /// that turn one file into another, and apply them to a file that may have
    /// drifted — as a three-way entity merge, not a fuzzy text patch.
    Patch {
        #[command(subcommand)]
        command: PatchCommands,
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
        /// Save interesting cases (wins, diffs, regressions) to a directory
        #[arg(long)]
        save: Option<String>,
    },
    /// Show lifetime merge statistics
    Stats,
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

#[derive(Subcommand)]
enum PatchCommands {
    /// Emit the typed ops that turn <base-file> into <changed-file>
    Extract {
        /// The file the ops are computed against
        base_file: String,
        /// The file the ops should reproduce
        changed_file: String,
        /// Path used to select the parser (default: the changed file's own path)
        #[arg(long)]
        path: Option<String>,
        /// Inline the base snapshot, making the ops self-contained so `apply`
        /// can do a real three-way merge against a drifted target
        #[arg(long)]
        embed_base: bool,
        /// Write the ops here instead of stdout
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Apply typed ops to a target file, three-way against the ops' base
    Apply {
        /// The ops document produced by `weave patch extract`
        ops_file: String,
        /// The file to apply them to; it may have drifted from the base
        target_file: String,
        /// The base the ops were extracted from, when not embedded in them
        #[arg(long)]
        base: Option<String>,
        /// Write the result here instead of stdout
        #[arg(short, long)]
        output: Option<String>,
        /// Rewrite the target file in place
        #[arg(long)]
        in_place: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    // The one place this binary says what a merge may touch. Every subcommand
    // that merges is handed this; none of them can widen it.
    let host = weave_core::host::Host {
        line_merge: Some(weave_core::host::git_line_merge),
        set_attribute: Some(weave_core::host::git_set_attribute),
        ..Default::default()
    };

    let result = match cli.command {
        Commands::Setup {
            ref driver,
            local,
            global,
        } => commands::setup::run(driver.as_deref(), local, global),
        Commands::Unsetup => commands::setup::unsetup(),
        Commands::Apply { ref files } => commands::apply::run(files),
        Commands::Preview {
            ref branch,
            ref file,
        } => commands::preview::run(branch, file.as_deref(), &host),
        Commands::Status {
            ref file,
            ref agent,
        } => commands::status::run(file.as_deref(), agent.as_deref()),
        Commands::Explain { ref file, json } => commands::explain::run(file, json, &host),
        Commands::Check {
            json,
            ref base,
            ref ours,
            ref theirs,
            ref base_dir,
            ref ours_dir,
            ref theirs_dir,
            timeout,
        } => commands::check::run(commands::check::Args {
            base: base.as_deref(),
            ours: ours.as_deref(),
            theirs: theirs.as_deref(),
            base_dir: base_dir.as_deref(),
            ours_dir: ours_dir.as_deref(),
            theirs_dir: theirs_dir.as_deref(),
            json,
            timeout: std::time::Duration::from_secs(timeout),
        }),
        Commands::Land {
            ref resolver,
            resolver_timeout,
            json,
            ref certificate,
            dry_run,
            ref base,
            ref ours,
            ref theirs,
        } => commands::land::run(
            commands::land::Args {
                base: base.as_deref(),
                ours: ours.as_deref(),
                theirs: theirs.as_deref(),
                resolver: resolver.as_deref(),
                resolver_timeout,
                json,
                certificate: certificate.as_deref(),
                dry_run,
            },
            &host,
        ),
        Commands::Patch { ref command } => match command {
            PatchCommands::Extract {
                ref base_file,
                ref changed_file,
                ref path,
                embed_base,
                ref output,
            } => commands::patch::extract(
                base_file,
                changed_file,
                path.as_deref(),
                *embed_base,
                output.as_deref(),
            ),
            PatchCommands::Apply {
                ref ops_file,
                ref target_file,
                ref base,
                ref output,
                in_place,
            } => commands::patch::apply(
                ops_file,
                target_file,
                base.as_deref(),
                output.as_deref(),
                *in_place,
                &host,
            ),
        },
        Commands::Bench => commands::bench::run(&host),
        Commands::BenchRepo {
            ref repo,
            limit,
            show_diff,
            ref save,
        } => commands::bench_repo::run(repo, limit, show_diff, save.as_deref(), &host),
        Commands::Stats => commands::stats::run(),
        Commands::Summary { ref file, json } => commands::summary::run(file, json),
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
