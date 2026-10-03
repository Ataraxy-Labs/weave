//! `weave land` — the argument shell over [`weave_cli::land`].
//!
//! Exit 0: every file git could not merge is PROVEN or VERIFIED. Exit 1: at
//! least one is REFUSED. Exit 2: the run itself failed (git, the resolver
//! command, a write) — nothing may be read into it.

use std::path::Path;
use std::time::Duration;

use weave_cli::land::{self, Resolver};
use weave_core::host::Host;

type R<T> = Result<T, Box<dyn std::error::Error>>;

pub(crate) struct Args<'a> {
    pub base: Option<&'a str>,
    pub ours: Option<&'a str>,
    pub theirs: Option<&'a str>,
    pub resolver: Option<&'a str>,
    pub resolver_timeout: u64,
    pub json: bool,
    pub certificate: Option<&'a str>,
    pub dry_run: bool,
    /// With revisions: judge the files of this revision as the merge's
    /// answer (a merge commit being re-checked).
    pub result: Option<&'a str>,
}

pub(crate) fn run(args: Args<'_>, host: &Host) -> R<()> {
    match land(args, host) {
        Ok(refused) => std::process::exit(if refused { 1 } else { 0 }),
        Err(e) => {
            eprintln!("weave land: {e}. The run did not finish; no label from it stands.");
            std::process::exit(2);
        }
    }
}

/// `weave land --onto`'s arguments.
pub(crate) struct OntoArgs<'a> {
    pub target: &'a str,
    pub resolver: Option<&'a str>,
    pub resolver_timeout: u64,
    pub verify_cmd: Option<&'a str>,
    pub verify_timeout: u64,
    /// `--check` (`sem` | `none`); None: the `.weave/config` default.
    pub check: Option<&'a str>,
    pub checkers: Option<&'a str>,
    pub attempts: usize,
    pub certificate_dir: Option<&'a str>,
}

/// Exit 0: published. 1: refused, nothing published. 2: the run failed.
pub(crate) fn run_onto(args: OntoArgs<'_>, host: &Host) -> R<()> {
    let run = || -> R<weave_cli::onto::Outcome> {
        let (remote, branch) = weave_cli::onto::split_target(args.target)?;
        let opts = weave_cli::onto::Options {
            remote,
            branch,
            resolver: args.resolver.map(|command| Resolver {
                command: command.to_string(),
                timeout: Duration::from_secs(args.resolver_timeout),
            }),
            verify_cmd: args.verify_cmd.map(str::to_string),
            verify_timeout: Duration::from_secs(args.verify_timeout),
            check: weave_cli::semcheck::resolve(args.check, args.checkers, Path::new("."))?,
            attempts: args.attempts,
            certificate_dir: args.certificate_dir.map(std::path::PathBuf::from),
        };
        let mut out = std::io::stdout();
        weave_cli::onto::land_onto(Path::new("."), &opts, host, &mut out)
    };
    match run() {
        Ok(weave_cli::onto::Outcome::Landed(_)) => std::process::exit(0),
        Ok(weave_cli::onto::Outcome::Refused(_)) => std::process::exit(1),
        Err(e) => {
            eprintln!("weave land: {e}. Nothing was published.");
            std::process::exit(2);
        }
    }
}

/// `weave land --queue`. Exit 0: landed. 1: refused (or still queued). 2:
/// the run failed.
pub(crate) fn run_queue(
    args: OntoArgs<'_>,
    lease_ttl: u64,
    queue_timeout: u64,
    host: &Host,
) -> R<()> {
    let run = || -> R<weave_cli::queue::QueueOutcome> {
        let (remote, branch) = weave_cli::onto::split_target(args.target)?;
        let q = weave_cli::queue::QueueOptions {
            onto: weave_cli::onto::Options {
                remote,
                branch,
                resolver: args.resolver.map(|command| Resolver {
                    command: command.to_string(),
                    timeout: Duration::from_secs(args.resolver_timeout),
                }),
                verify_cmd: args.verify_cmd.map(str::to_string),
                verify_timeout: Duration::from_secs(args.verify_timeout),
                check: weave_cli::semcheck::resolve(args.check, args.checkers, Path::new("."))?,
                attempts: args.attempts,
                certificate_dir: args.certificate_dir.map(std::path::PathBuf::from),
            },
            lease_ttl: Duration::from_secs(lease_ttl.max(1)),
            wait_timeout: Duration::from_secs(queue_timeout),
            poll: Duration::from_millis(400),
        };
        let mut out = std::io::stdout();
        weave_cli::queue::submit(Path::new("."), &q, host, &mut out)
    };
    match run() {
        Ok(weave_cli::queue::QueueOutcome::Landed(_)) => std::process::exit(0),
        Ok(weave_cli::queue::QueueOutcome::Refused(_)) => std::process::exit(1),
        Err(e) => {
            eprintln!("weave land --queue: {e}. Nothing was published by this run.");
            std::process::exit(2);
        }
    }
}

/// `Ok(any file refused)`.
fn land(args: Args<'_>, host: &Host) -> R<bool> {
    let dir = Path::new(".");
    let revisions = args.base.is_some() || args.ours.is_some() || args.theirs.is_some();
    let present = match (revisions, args.result) {
        (_, Some(rev)) => Some(land::Present::Rev(rev.to_string())),
        (false, None) => Some(land::Present::WorkingTree { unmerged: false }),
        (true, None) => None,
    };
    let plan = land::plan(dir, args.base, args.ours, args.theirs, present.as_ref())?;
    let resolver = args.resolver.map(|command| Resolver {
        command: command.to_string(),
        timeout: Duration::from_secs(args.resolver_timeout),
    });

    let mut reports = plan.not_text.clone();
    for unit in &plan.units {
        reports.push(land::land_unit(unit, host, resolver.as_ref())?);
    }
    reports.sort_by(|a, b| a.path.cmp(&b.path));

    let mode = if revisions {
        "revisions"
    } else if args.dry_run {
        "dry-run"
    } else {
        "working-tree"
    };
    if mode == "working-tree" {
        land::write_back(dir, &plan, &reports)?;
    }
    // A merge as committed (`--result`): every change only one side made
    // must be in it too — files the gate never reads, since nothing in them
    // conflicts.
    // In the working tree, once nothing is left unmerged: the tree the merge
    // commit would hold, however it came about.
    let judged = match args.result {
        Some(result) => Some(result.to_string()),
        None if mode == "working-tree" => weave_cli::preserve::merge_in_tree(dir)?,
        None => None,
    };
    let lost = match &judged {
        Some(result) => {
            weave_cli::preserve::check(dir, &plan.base, &plan.ours, &plan.theirs, result)?
        }
        None => Vec::new(),
    };
    let mut doc = land::document(&plan, &reports, mode);
    if judged.is_some() {
        doc["one_sided"] = serde_json::to_value(&lost)?;
    }
    if let Some(path) = args.certificate {
        std::fs::write(path, serde_json::to_string_pretty(&doc)? + "\n")
            .map_err(|e| format!("could not write the certificate to {path}: {e}"))?;
    }
    if args.json {
        println!("{}", serde_json::to_string_pretty(&doc)?);
    } else {
        print!("{}", land::render(&plan, &reports));
        if !lost.is_empty() {
            println!(
                "REFUSED: the result loses changes only one side made:\n{}",
                weave_cli::preserve::render(&lost, "ours", "theirs")
            );
        }
        if mode != "working-tree" {
            println!("({mode}: nothing was written to the working tree or the index)");
        }
    }
    Ok(!lost.is_empty() || reports.iter().any(|r| r.status == land::Status::Refused))
}
