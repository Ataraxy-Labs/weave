mod commands;

use std::io::IsTerminal;

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
  # ... or land onto main: merge, gate, verify, publish:
  weave land --onto origin/main --check sem
  weave setup --global        make weave the default driver for every repo";

/// Whether an old command name's notice is printed: stdout is a terminal and
/// the output is not JSON. `WEAVE_NO_DEPRECATION` silences it everywhere.
fn should_note(stdout_is_terminal: bool, json: bool) -> bool {
    stdout_is_terminal && !json && std::env::var_os("WEAVE_NO_DEPRECATION").is_none()
}

fn notice(old: &str, new: &str) -> String {
    format!("note: `weave {old}` is now `weave {new}` (the old name keeps working)")
}

/// One line on stderr for a person at a terminal; nothing for scripts and
/// agents (piped stdout or JSON output).
fn note(old: &str, new: &str, json: bool) {
    if should_note(std::io::stdout().is_terminal(), json) {
        eprintln!("{}", notice(old, new));
    }
}

const LAND_EXAMPLES: &str = "\
EXAMPLES
  git merge agent/feature               # stops on conflicts
  weave land --resolver 'python3 scripts/weave-land-resolver.py' \\
             --certificate land.json    # PROVEN / VERIFIED / REFUSED per file
  git commit                            # refuses while any file is REFUSED

  weave land --ours main --theirs agent/feature --json   # CI: report only

  # the whole landing: merge origin/main in, gate every file of the merge,
  # build and test the merged tree, publish fast-forward only; retried when
  # main moves, and the new merge is gated and built again
  weave land --onto origin/main --verify-cmd 'go build ./... && go vet ./...'
  weave land --onto origin/main --check sem    # verify with sem check


  # experimental: many agents, one main: queue HEAD and block until it lands or is refused;
  # candidates land one at a time in order, so no push races another
  weave land --queue --verify-cmd 'go build ./... && go vet ./...'

  # verify the merged tree with the project's own compiler, type checker,
  # linter and tests via `sem check` (exact, incremental where provable);
  # `[land] check = sem` in .weave/config makes it the default
  weave land --queue --check sem";

#[derive(Parser)]
#[command(
    name = "weave",
    about = "weave: merge by function and entry, not by line, and land merges onto main behind a fixed gate",
    version,
    after_help = QUICKSTART
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Make `git merge` use weave in this repo (--global: every repo; --off: stop)
    #[command(display_order = 1)]
    Setup {
        /// Path to weave-driver binary (auto-detected if omitted). Example: --driver ~/bin/weave-driver
        #[arg(long)]
        driver: Option<String>,
        /// Write merge attributes to .git/info/attributes instead of .gitattributes.
        /// Example: weave setup --local
        #[arg(long)]
        local: bool,
        /// Configure git globally (~/.gitconfig + global attributes) so weave is
        /// the default driver in every repo. No git repo required. Example: weave setup --global
        #[arg(long)]
        global: bool,
        /// Remove the weave merge driver from this repo. Example: weave setup --off
        #[arg(long, conflicts_with_all = ["driver", "local", "global"])]
        off: bool,
    },
    /// Remove weave merge driver from the current Git repo
    #[command(hide = true)]
    Unsetup,
    /// Materialize entity edits from the CRDT back onto the working files
    #[command(hide = true)]
    Apply {
        /// One or more files to write from the CRDT
        files: Vec<String>,
    },
    /// What would merging this branch into HEAD look like? Nothing is written
    #[command(display_order = 5)]
    Preview {
        /// The branch to merge into HEAD. Example: weave preview agent/feature
        branch: String,
        /// Preview one file only. Example: weave preview agent/feature --file src/app.ts
        #[arg(long)]
        file: Option<String>,
    },
    /// Show entity and agent state from CRDT
    #[command(hide = true)]
    Status {
        /// Show entities for a specific file
        #[arg(long)]
        file: Option<String>,
        /// Show status for a specific agent
        #[arg(long)]
        agent: Option<String>,
    },
    /// Claim an entity before editing
    #[command(hide = true)]
    Claim {
        /// Agent identifier
        agent_id: String,
        /// File path containing the entity
        file_path: String,
        /// Entity name to claim
        entity_name: String,
    },
    /// Why did this file conflict? Which guard refused, and the hunks both sides wrote in
    ///
    /// Explain ONE file's conflicts: which guard refused, and the hunks BOTH
    /// sides wrote in. Reads the three merge stages out of the index, so it
    /// describes the conflict git actually produced. With --summary: parse the
    /// weave conflict markers in any file and summarize them.
    #[command(display_order = 3)]
    Explain {
        /// The conflicted file to explain. Example: weave explain src/app.ts
        file: String,
        /// Summarize the weave conflict markers in the file instead (works on any
        /// file, no merge in progress needed). Example: weave explain src/app.ts --summary
        #[arg(long)]
        summary: bool,
        /// Output as JSON. Example: weave explain src/app.ts --json
        #[arg(long)]
        json: bool,
    },
    /// Is my conflict resolution right? Lost lines, duplicates, leftover markers, dangling names
    ///
    /// Verify your resolution. With no arguments this checks the WORKING TREE
    /// against the three merge stages — markers left behind, lines both sides
    /// kept that went missing, anything stated more often than either side
    /// stated it, references that no longer resolve — and prints one verdict
    /// line per file. With --base/--ours/--theirs (or --*-dir) it runs the
    /// cross-file binding pass between two revisions instead and emits
    /// weave-findings JSON. Exits 1 when there are findings, and 2 when it
    /// could not verify at all — git failed, or the run passed --timeout.
    #[command(display_order = 4)]
    Check {
        /// Output as JSON (working-tree mode). Example: weave check --json
        #[arg(long)]
        json: bool,
        /// Merge base revision (default: merge-base of --ours and --theirs). Example: --base main~3
        #[arg(long)]
        base: Option<String>,
        /// Our side (default: HEAD). Example: --ours main
        #[arg(long)]
        ours: Option<String>,
        /// Their side (default: MERGE_HEAD, i.e. the merge in progress). Example: --theirs agent/feature
        #[arg(long)]
        theirs: Option<String>,
        /// No-git mode: directory holding the base tree. Example: --base-dir /tmp/base
        #[arg(long)]
        base_dir: Option<String>,
        /// No-git mode: directory holding our tree. Example: --ours-dir /tmp/ours
        #[arg(long)]
        ours_dir: Option<String>,
        /// No-git mode: directory holding their tree. Example: --theirs-dir /tmp/theirs
        #[arg(long)]
        theirs_dir: Option<String>,
        /// Give up after this many seconds with exit code 2 and a message,
        /// rather than run on. 0 means no limit. Example: --timeout 60
        #[arg(long, default_value_t = 300)]
        timeout: u64,
    },
    /// Can this merge land? Merge, gate every file (PROVEN / VERIFIED / REFUSED), verify, publish
    ///
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
    /// Before any resolver, a file the merge already answered — one the merge
    /// driver resolved (stage 0 in the index), or the --result revision's
    /// file — is judged as it stands: PROVEN when it is weave's certified
    /// merge, VERIFIED when it passes the gate.
    ///
    /// PROVEN and VERIFIED files are written and staged. A REFUSED file keeps
    /// its conflict markers and stays unmerged; a resolver's rejected answer
    /// is never written. A file both sides changed that git merges
    /// line-cleanly must still pass weave's merge check (nothing lost or
    /// stated twice, no `case` label or map key twice), or it is a unit too.
    ///
    /// --onto <remote>/<branch> runs the whole landing and publishes: fetch,
    /// merge the tip in (--no-commit), gate EVERY file of that merge, commit,
    /// gate any other merge on the branch not yet checked, run --verify-cmd
    /// on the final tree, then update <branch> on <remote> fast-forward only.
    /// If the branch moved meanwhile, the new tip is merged and everything is
    /// checked again. Any refusal publishes nothing (exit 1). The gate is fixed;
    /// what --verify-cmd (or --check sem) runs is yours to choose.
    ///
    /// Exit 0: every file PROVEN or VERIFIED. 1: some file REFUSED. 2: the run
    /// failed and nothing was landed.
    #[command(after_long_help = LAND_EXAMPLES, display_order = 2)]
    #[command(group = clap::ArgGroup::new("publish").args(["onto", "queue"]).multiple(true))]
    Land {
        /// Resolver command, run with `sh -c` once per attempt (see above). Example: --resolver 'python3 scripts/resolve.py'
        #[arg(long, value_name = "CMD")]
        resolver: Option<String>,
        /// Give up on one resolver call after this many seconds. Example: --resolver-timeout 300
        #[arg(long, default_value_t = 900, value_name = "SECS")]
        resolver_timeout: u64,
        /// Print the report as JSON (the same document --certificate writes). Example: weave land --json
        #[arg(long)]
        json: bool,
        /// Write the review certificate (JSON: every file's status, rule,
        /// reason and findings, with the merge's three commits) to this file. Example: --certificate land.json
        #[arg(long, value_name = "FILE")]
        certificate: Option<String>,
        /// Decide and report, but write nothing to the working tree or index. Example: weave land --dry-run
        #[arg(long)]
        dry_run: bool,
        /// Merge base revision (default: merge-base of --ours and --theirs).
        /// Any of --base/--ours/--theirs implies --dry-run. Example: --base main~3
        #[arg(long)]
        base: Option<String>,
        /// Our side (default: HEAD). Example: --ours main
        #[arg(long)]
        ours: Option<String>,
        /// Their side (default: MERGE_HEAD, i.e. the merge in progress). Example: --theirs agent/feature
        #[arg(long)]
        theirs: Option<String>,
        /// With --base/--ours/--theirs: judge this revision's files as the
        /// merge's answer (re-check a merge commit as it was committed). Example: --result HEAD
        #[arg(long, value_name = "REV")]
        result: Option<String>,
        /// Land the current branch onto <remote>/<branch> and publish it there
        /// (see above): nothing is published that has not passed the gate,
        /// and --verify-cmd, against the exact tip it is published onto. Example: weave land --onto origin/main
        #[arg(long, value_name = "REMOTE/BRANCH", conflicts_with_all = ["base", "ours", "theirs", "result", "dry_run"])]
        onto: Option<String>,
        /// With --onto: run this (`sh -c`, in the repository root) on the final
        /// merged tree before publishing; a non-zero exit refuses. At least a
        /// build is recommended (`go build ./... && go vet ./...`,
        /// `npm run build`, `cargo check`), better the affected tests.
        /// Example: --verify-cmd 'cargo check && cargo test'
        #[arg(long, value_name = "CMD", requires = "publish")]
        verify_cmd: Option<String>,
        /// Give up on the verify command (and on sem check) after this many seconds.
        /// Example: --verify-timeout 600
        #[arg(long, default_value_t = 1800, value_name = "SECS")]
        verify_timeout: u64,
        /// With --onto/--queue: `sem` runs `sem check --base <tip>` on the final
        /// merged tree before publishing — the project's own compiler, type
        /// checker, linter and tests, with the verdict of the full run,
        /// rechecking only what the merge can affect. Any verdict but pass
        /// (fail, could not decide, sem not installed) refuses, and its
        /// diagnostics are the report. `none` turns a `.weave/config` default
        /// (`[land] check = sem`) off. Runs after --verify-cmd, if both.
        /// Example: weave land --onto origin/main --check sem
        #[arg(long, value_name = "sem|none", requires = "publish")]
        check: Option<String>,
        /// With --check sem: sem check's --checkers (default: `.weave/config`
        /// land.checkers, else every checker sem detects). Example: --checkers ts,lint
        #[arg(long, value_name = "LIST", requires = "publish")]
        checkers: Option<String>,
        /// With --onto: how many times to merge a moved tip and try again. Example: --attempts 10
        #[arg(long, default_value_t = 5, value_name = "N")]
        attempts: usize,
        /// With --onto: write every gate certificate into this directory. Example: --certificate-dir certs/
        #[arg(long, value_name = "DIR", requires = "publish")]
        certificate_dir: Option<String>,
        /// Experimental. Land through the landing queue kept in the remote (refs/weave/*):
        /// submit HEAD and block until it is landed or refused. Candidates
        /// land one at a time in submission order (merge onto the tip, gate,
        /// --verify-cmd, fast-forward), so no push can race another. A merge
        /// refusal leaves the merge in progress here to resolve in place.
        /// Target: --onto, default origin/main. Example: weave land --queue --check sem
        #[arg(long, conflicts_with_all = ["base", "ours", "theirs", "result", "dry_run"])]
        queue: bool,
        /// With --queue: a lander silent this long is dead; take over its lock. Example: --lease-ttl 60
        #[arg(long, default_value_t = 30, value_name = "SECS")]
        lease_ttl: u64,
        /// With --queue: stop waiting after this long (the ticket stays queued). Example: --queue-timeout 3600
        #[arg(long, default_value_t = 7200, value_name = "SECS")]
        queue_timeout: u64,
    },
    /// Apply an edit to a file that drifted: typed entity ops, merged three-way
    ///
    /// Typed entity ops: the write side of the agent contract. Extract the ops
    /// that turn one file into another, and apply them to a file that may have
    /// drifted — as a three-way entity merge, not a fuzzy text patch.
    #[command(display_order = 6)]
    Patch {
        #[command(subcommand)]
        command: PatchCommands,
    },
    /// Run merge benchmarks comparing weave vs git line-level merge
    #[command(hide = true)]
    Bench,
    /// Benchmark against real merge commits in an existing repo
    #[command(hide = true)]
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
    /// How has weave done? Lifetime merge statistics; --bench and --repo compare it with git
    #[command(display_order = 7)]
    Stats {
        /// Run the built-in merge scenarios, weave against git's line merge. Example: weave stats --bench
        #[arg(long)]
        bench: bool,
        /// Replay the real merge commits of this repository, weave against what was
        /// committed. Example: weave stats --repo ../project --limit 200
        #[arg(long, value_name = "PATH", conflicts_with = "bench")]
        repo: Option<String>,
        /// With --repo: max merge commits to scan. Example: weave stats --repo ../project --limit 200
        #[arg(long, default_value_t = 500, requires = "repo")]
        limit: usize,
        /// With --repo: show a line diff for each weave vs human mismatch. Example: weave stats --repo ../project --show-diff
        #[arg(long, requires = "repo")]
        show_diff: bool,
        /// With --repo: save interesting cases to this directory. Example: weave stats --repo ../project --save cases/
        #[arg(long, value_name = "DIR", requires = "repo")]
        save: Option<String>,
    },
    /// The live-editing prototype: claim, release, status, apply (experimental)
    #[command(hide = true)]
    Experimental {
        #[command(subcommand)]
        command: ExperimentalCommands,
    },
    /// Parse weave conflict markers and show a structured summary
    #[command(hide = true)]
    Summary {
        /// Path to a file containing weave conflict markers
        file: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Release a previously claimed entity
    #[command(hide = true)]
    Release {
        /// Agent identifier
        agent_id: String,
        /// File path containing the entity
        file_path: String,
        /// Entity name to release
        entity_name: String,
    },
}

/// The live-editing prototype, kept for its existing users.
#[derive(Subcommand)]
enum ExperimentalCommands {
    /// Claim an entity before editing
    Claim {
        /// Agent identifier
        agent_id: String,
        /// File path containing the entity
        file_path: String,
        /// Entity name to claim
        entity_name: String,
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
    /// Show entity and agent state from CRDT
    Status {
        /// Show entities for a specific file
        #[arg(long)]
        file: Option<String>,
        /// Show status for a specific agent
        #[arg(long)]
        agent: Option<String>,
    },
    /// Materialize entity edits from the CRDT back onto the working files
    Apply {
        /// One or more files to write from the CRDT
        files: Vec<String>,
    },
}

#[derive(Subcommand)]
enum PatchCommands {
    /// Emit the typed ops that turn <base-file> into <changed-file>
    Extract {
        /// The file the ops are computed against. Example: weave patch extract old.ts new.ts
        base_file: String,
        /// The file the ops should reproduce. Example: weave patch extract old.ts new.ts
        changed_file: String,
        /// Path used to select the parser (default: the changed file's own path). Example: --path src/app.ts
        #[arg(long)]
        path: Option<String>,
        /// Inline the base snapshot, making the ops self-contained so `apply`
        /// can do a real three-way merge against a drifted target. Example: weave patch extract old.ts new.ts --embed-base
        #[arg(long)]
        embed_base: bool,
        /// Write the ops here instead of stdout. Example: -o ops.json
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Apply typed ops to a target file, three-way against the ops' base
    Apply {
        /// The ops document produced by `weave patch extract`. Example: weave patch apply ops.json src/app.ts
        ops_file: String,
        /// The file to apply them to; it may have drifted from the base. Example: weave patch apply ops.json src/app.ts
        target_file: String,
        /// The base the ops were extracted from, when not embedded in them. Example: --base old.ts
        #[arg(long)]
        base: Option<String>,
        /// Write the result here instead of stdout. Example: -o merged.ts
        #[arg(short, long)]
        output: Option<String>,
        /// Rewrite the target file in place. Example: weave patch apply ops.json src/app.ts --in-place
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
            off,
        } => {
            if off {
                commands::setup::unsetup()
            } else {
                commands::setup::run(driver.as_deref(), local, global)
            }
        }
        Commands::Unsetup => {
            note("unsetup", "setup --off", false);
            commands::setup::unsetup()
        }
        Commands::Apply { ref files } => {
            note("apply", "experimental apply", false);
            commands::apply::run(files)
        }
        Commands::Preview {
            ref branch,
            ref file,
        } => commands::preview::run(branch, file.as_deref(), &host),
        Commands::Status {
            ref file,
            ref agent,
        } => {
            note("status", "experimental status", false);
            commands::status::run(file.as_deref(), agent.as_deref())
        }
        Commands::Explain {
            ref file,
            summary,
            json,
        } => {
            if summary {
                commands::summary::run(file, json)
            } else {
                commands::explain::run(file, json, &host)
            }
        }
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
            ref result,
            ref onto,
            ref verify_cmd,
            verify_timeout,
            ref check,
            ref checkers,
            attempts,
            ref certificate_dir,
            queue,
            lease_ttl,
            queue_timeout,
        } => match onto {
            _ if queue => commands::land::run_queue(
                commands::land::OntoArgs {
                    target: onto.as_deref().unwrap_or("origin/main"),
                    resolver: resolver.as_deref(),
                    resolver_timeout,
                    verify_cmd: verify_cmd.as_deref(),
                    verify_timeout,
                    check: check.as_deref(),
                    checkers: checkers.as_deref(),
                    attempts,
                    certificate_dir: certificate_dir.as_deref(),
                },
                lease_ttl,
                queue_timeout,
                &host,
            ),
            Some(target) => commands::land::run_onto(
                commands::land::OntoArgs {
                    target,
                    resolver: resolver.as_deref(),
                    resolver_timeout,
                    verify_cmd: verify_cmd.as_deref(),
                    verify_timeout,
                    check: check.as_deref(),
                    checkers: checkers.as_deref(),
                    attempts,
                    certificate_dir: certificate_dir.as_deref(),
                },
                &host,
            ),
            None => commands::land::run(
                commands::land::Args {
                    base: base.as_deref(),
                    ours: ours.as_deref(),
                    theirs: theirs.as_deref(),
                    resolver: resolver.as_deref(),
                    resolver_timeout,
                    json,
                    certificate: certificate.as_deref(),
                    dry_run,
                    result: result.as_deref(),
                },
                &host,
            ),
        },
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
        Commands::Bench => {
            note("bench", "stats --bench", false);
            commands::bench::run(&host)
        }
        Commands::BenchRepo {
            ref repo,
            limit,
            show_diff,
            ref save,
        } => {
            note("bench-repo", "stats --repo", false);
            commands::bench_repo::run(repo, limit, show_diff, save.as_deref(), &host)
        }
        Commands::Stats {
            bench,
            ref repo,
            limit,
            show_diff,
            ref save,
        } => match repo {
            Some(repo) => commands::bench_repo::run(repo, limit, show_diff, save.as_deref(), &host),
            None if bench => commands::bench::run(&host),
            None => commands::stats::run(),
        },
        Commands::Summary { ref file, json } => {
            note("summary", "explain --summary", json);
            commands::summary::run(file, json)
        }
        Commands::Claim {
            ref agent_id,
            ref file_path,
            ref entity_name,
        } => {
            note("claim", "experimental claim", false);
            commands::claim::run(agent_id, file_path, entity_name)
        }
        Commands::Release {
            ref agent_id,
            ref file_path,
            ref entity_name,
        } => {
            note("release", "experimental release", false);
            commands::release::run(agent_id, file_path, entity_name)
        }
        Commands::Experimental { ref command } => match command {
            ExperimentalCommands::Claim {
                agent_id,
                file_path,
                entity_name,
            } => commands::claim::run(agent_id, file_path, entity_name),
            ExperimentalCommands::Release {
                agent_id,
                file_path,
                entity_name,
            } => commands::release::run(agent_id, file_path, entity_name),
            ExperimentalCommands::Status { file, agent } => {
                commands::status::run(file.as_deref(), agent.as_deref())
            }
            ExperimentalCommands::Apply { files } => commands::apply::run(files),
        },
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notice_only_for_a_terminal_and_never_in_json_mode() {
        if std::env::var_os("WEAVE_NO_DEPRECATION").is_some() {
            return;
        }
        assert!(should_note(true, false));
        assert!(!should_note(true, true));
        assert!(!should_note(false, false));
        assert!(!should_note(false, true));
    }

    #[test]
    fn notice_is_one_line_naming_both_spellings() {
        assert_eq!(
            notice("summary", "explain --summary"),
            "note: `weave summary` is now `weave explain --summary` (the old name keeps working)"
        );
    }
}
