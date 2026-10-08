//! `weave check` — verify the resolution that is on disk.
//!
//! Three input modes, one command, and the DEFAULT one changed:
//!
//! * **working tree** (no arguments): the four questions of
//!   [`weave_cli::worktree`] against the file as it exists right now, for every
//!   file the merge had to decide. One verdict sentence per file. The old
//!   default compared two *commits* and therefore could not see the edit an
//!   agent had just written — the working tree is the thing that actually
//!   needs verifying.
//! * **repo scope** (`--base/--ours/--theirs`): the cross-file binding pass
//!   between two revisions, emitting weave-findings JSON. Unchanged, and still
//!   the only thing that can answer "did a rename in `a.py` orphan a caller in
//!   `b.py`" for two arbitrary commits.
//! * **directories** (`--base-dir/--ours-dir/--theirs-dir`): the same pass for
//!   callers with no git at all.
//!
//! Exit 0 = nothing found, exit 1 = findings — in every mode, so a script does
//! not have to know which mode it asked for. Exit 2 = nothing could be
//! verified: git failed, or the run outlived `--timeout`. A check that hangs
//! is a check whose silence reads as approval, so it may not: past the limit
//! it says so and stops.

use std::io::Write;
use std::path::Path;
use std::time::Duration;

use weave_cli::{gitscan, repo_scope, worktree};

type R<T> = Result<T, Box<dyn std::error::Error>>;

pub(crate) struct Args<'a> {
    pub base: Option<&'a str>,
    pub ours: Option<&'a str>,
    pub theirs: Option<&'a str>,
    pub base_dir: Option<&'a str>,
    pub ours_dir: Option<&'a str>,
    pub theirs_dir: Option<&'a str>,
    pub json: bool,
    /// Give up past this; zero means never.
    pub timeout: Duration,
}

/// Exit code for "could not verify" — distinct from 1, which means findings.
const UNVERIFIED: i32 = 2;

pub(crate) fn run(args: Args<'_>) -> R<()> {
    if !args.timeout.is_zero() {
        let limit = args.timeout;
        // The last line of defence, and the only one that covers a compute
        // spin: no pipe deadline can interrupt a parse that never returns.
        std::thread::spawn(move || {
            std::thread::sleep(limit);
            eprintln!(
                "weave check: did not finish within {}s and stopped. NOTHING WAS VERIFIED — \
                 this is not a clean bill of health. Re-run with a larger --timeout, or \
                 report the merge that caused it.",
                limit.as_secs()
            );
            std::process::exit(UNVERIFIED);
        });
    }
    if let Err(e) = verify(args) {
        eprintln!("weave check: could not verify: {e}. NOTHING WAS VERIFIED.");
        std::process::exit(UNVERIFIED);
    }
    Ok(())
}

fn verify(args: Args<'_>) -> R<()> {
    let dir_mode = args.base_dir.is_some() || args.ours_dir.is_some() || args.theirs_dir.is_some();
    let rev_mode = args.base.is_some() || args.ours.is_some() || args.theirs.is_some();
    if !dir_mode && !rev_mode {
        return working_tree(args.json);
    }

    let (base, ours, theirs) = if dir_mode {
        match (args.base_dir, args.ours_dir, args.theirs_dir) {
            (Some(b), Some(o), Some(t)) => (
                repo_scope::read_dir_tree(Path::new(b))?,
                repo_scope::read_dir_tree(Path::new(o))?,
                repo_scope::read_dir_tree(Path::new(t))?,
            ),
            _ => {
                return Err(
                    "directory mode needs all three of --base-dir, --ours-dir, --theirs-dir".into(),
                )
            }
        }
    } else {
        gitscan::trees(Path::new("."), args.base, args.ours, args.theirs)?
    };

    let docs = repo_scope::check(&base, &ours, &theirs);
    let n = repo_scope::total_findings(&docs);

    let mut out = std::io::stdout();
    writeln!(out, "{}", serde_json::to_string_pretty(&docs)?)?;
    // A bare `[]` is the one thing this command may never leave a reader with:
    // it reads as approval just as easily as it reads as "weave has no
    // cross-file rule for this language". Say which.
    if docs.is_empty() {
        eprintln!(
            "weave check (repo scope): no cross-file binding findings between these two \
             revisions. That is a statement about DANGLING and SHADOW only — it is not a \
             verification of any file's contents. For that, run `weave check` with no \
             arguments against the working tree."
        );
    }
    out.flush()?;

    if n > 0 {
        eprintln!(
            "weave check: {} finding(s) across {} file(s)",
            n,
            docs.len()
        );
        std::process::exit(1);
    }
    Ok(())
}

/// The default: verify what is on disk.
fn working_tree(json: bool) -> R<()> {
    let Some(report) = worktree::check_in_progress(Path::new("."))? else {
        // Not an error, and emphatically not silence.
        println!("weave check: {}", worktree::NOTHING_TO_CHECK);
        return Ok(());
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&report.to_json())?);
    } else {
        print!("{}", report.render());
    }
    if report.tally().2 > 0 {
        std::process::exit(1);
    }
    Ok(())
}
