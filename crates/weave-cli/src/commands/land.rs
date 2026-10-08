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
    let run = land::Run {
        base: args.base,
        ours: args.ours,
        theirs: args.theirs,
        result: args.result,
        resolver: args.resolver.map(|command| Resolver {
            command: command.to_string(),
            timeout: Duration::from_secs(args.resolver_timeout),
        }),
        dry_run: args.dry_run,
    };
    let landing = land::run(Path::new("."), &run, host)?;
    if let Some(path) = args.certificate {
        std::fs::write(path, serde_json::to_string_pretty(&landing.doc)? + "\n")
            .map_err(|e| format!("could not write the certificate to {path}: {e}"))?;
    }
    if args.json {
        println!("{}", serde_json::to_string_pretty(&landing.doc)?);
    } else {
        print!("{}", landing.text);
    }
    Ok(landing.refused)
}
