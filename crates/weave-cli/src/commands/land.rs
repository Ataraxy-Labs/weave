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

/// `Ok(any file refused)`.
fn land(args: Args<'_>, host: &Host) -> R<bool> {
    let dir = Path::new(".");
    let revisions = args.base.is_some() || args.ours.is_some() || args.theirs.is_some();
    let plan = land::plan(dir, args.base, args.ours, args.theirs)?;
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
    let doc = land::document(&plan, &reports, mode);
    if let Some(path) = args.certificate {
        std::fs::write(path, serde_json::to_string_pretty(&doc)? + "\n")
            .map_err(|e| format!("could not write the certificate to {path}: {e}"))?;
    }
    if args.json {
        println!("{}", serde_json::to_string_pretty(&doc)?);
    } else {
        print!("{}", land::render(&plan, &reports));
        if mode != "working-tree" {
            println!("({mode}: nothing was written to the working tree or the index)");
        }
    }
    Ok(reports.iter().any(|r| r.status == land::Status::Refused))
}
