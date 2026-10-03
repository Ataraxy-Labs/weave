//! `weave land --onto <remote>/<branch>` — the whole landing: bring the
//! branch's latest commits onto a shared branch, and publish nothing that has
//! not been checked against the exact commit it is published onto.
//!
//! The loop, at most `attempts` times:
//!
//! 1. fetch `<remote>/<branch>`; call its tip `T`;
//! 2. if `T` is not already in the branch, `git merge --no-commit T` (with
//!    whatever merge driver the repository configures) and run the
//!    working-tree gate of [`crate::land`] over the merge in progress —
//!    conflicted files AND files the driver or git merged cleanly. Every
//!    file must come out PROVEN or VERIFIED (DROPPED / UNDELETED included);
//!    then the merge is committed. Otherwise the merge is left in progress
//!    with the refused files conflicted, and nothing is published;
//! 3. every other merge commit between `T` and HEAD not yet checked (one the
//!    agent committed itself) is checked the same way, its own tree as the
//!    answer (revisions mode, `--result`) — and a file it refuses gets a
//!    second hearing as HEAD holds it now, so a later fix commit counts;
//! 4. `--verify-cmd`, when given, runs on the final tree (a build, the
//!    affected tests); a non-zero exit refuses;
//! 5. a fast-forward-only update of `<branch>` on `<remote>`. When it is
//!    rejected, the branch moved: back to 1 — the new merge is gated again,
//!    and the verify command runs again on the new tree.
//!
//! The default is to refuse: no step's failure leads to publishing.
//!
//! Checked merges are remembered by commit id in `<git-dir>/weave-land-verified`
//! (a commit id fixes both parents and the tree, so the verdict cannot go
//! stale), and verify passes by tree and command in
//! `<git-dir>/weave-land-verify-ok`, so a retry does no work twice.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use sha2::{Digest, Sha256};
use weave_core::host::Host;

use crate::gitscan;
use crate::land::{self, Present, Resolver, Status};
use crate::mergestate;
use crate::preserve;

type R<T> = Result<T, Box<dyn std::error::Error>>;

/// What `--onto` was asked to do.
pub struct Options {
    pub remote: String,
    pub branch: String,
    pub resolver: Option<Resolver>,
    pub verify_cmd: Option<String>,
    pub verify_timeout: Duration,
    pub attempts: usize,
    pub certificate_dir: Option<PathBuf>,
}

/// How the run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Published: the branch on the remote is now this commit.
    Landed(String),
    /// Nothing was published, and why (already printed).
    Refused(&'static str),
}

/// `remote/branch` → `(remote, branch)`.
pub fn split_target(target: &str) -> R<(String, String)> {
    match target.split_once('/') {
        Some((r, b)) if !r.is_empty() && !b.is_empty() => Ok((r.to_string(), b.to_string())),
        _ => {
            Err(format!("--onto wants <remote>/<branch> (e.g. origin/main), not `{target}`").into())
        }
    }
}

fn git_run(dir: &Path, args: &[&str], limit: Duration) -> R<(bool, String, String)> {
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(dir);
    let (status, out, err) = gitscan::run_bounded(cmd, &format!("git {}", args[0]), None, limit)?;
    Ok((
        status.success(),
        String::from_utf8_lossy(&out).to_string(),
        String::from_utf8_lossy(&err).to_string(),
    ))
}

fn git(dir: &Path, args: &[&str]) -> R<String> {
    Ok(gitscan::git(dir, args)?.trim().to_string())
}

fn git_ok(dir: &Path, args: &[&str]) -> bool {
    git_run(dir, args, Duration::from_secs(120)).is_ok_and(|(ok, _, _)| ok)
}

fn rev(dir: &Path, r: &str) -> R<String> {
    git(dir, &["rev-parse", "--verify", "-q", r])
}

/// A set of ids kept one per line in a file under the git dir.
struct Ledger(PathBuf);

impl Ledger {
    fn has(&self, id: &str) -> bool {
        std::fs::read_to_string(&self.0).is_ok_and(|t| t.lines().any(|l| l == id))
    }
    fn add(&self, id: &str) -> R<()> {
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.0)?;
        writeln!(f, "{id}")?;
        Ok(())
    }
}

/// Land the current branch onto `opts.remote`/`opts.branch`. Progress and
/// every refusal go to `out`, in words meant for the person (or agent) who
/// has to act on them.
pub fn land_onto(dir: &Path, opts: &Options, host: &Host, out: &mut dyn Write) -> R<Outcome> {
    let top = PathBuf::from(git(dir, &["rev-parse", "--show-toplevel"])?);
    let dir = top.as_path();
    let gd = PathBuf::from(git(dir, &["rev-parse", "--absolute-git-dir"])?);
    let target = format!("{}/{}", opts.remote, opts.branch);

    if gd.join("rebase-merge").exists() || gd.join("rebase-apply").exists() {
        writeln!(
            out,
            "land: a rebase is in progress; finish it (git rebase --continue) first. land merges; \
             it does not rebase."
        )?;
        return Ok(Outcome::Refused("rebase in progress"));
    }

    // An index.lock nobody holds is removed (and said); one that is held,
    // or whose holder cannot be told, refuses: land never works around it.
    if let Some(why) = mergestate::clear_stale_lock(&gd, out)? {
        writeln!(
            out,
            "land: REFUSED. {why}. Nothing was published. Wait for that process to finish, then \
             run land again."
        )?;
        return Ok(Outcome::Refused("index.lock held"));
    }

    // A merge in progress: the person's in-place resolution, judged by the
    // gate — unless it is a merge land itself started and never finished,
    // which is nobody's resolution: it is aborted and merged again.
    if gd.join("MERGE_HEAD").exists() {
        if let Some(why) = resume_merge_in_progress(dir, &gd, opts, host, out)? {
            return Ok(Outcome::Refused(why));
        }
    } else {
        mergestate::clear_record(&gd);
    }

    if !git(dir, &["status", "--porcelain", "--untracked-files=no"])?.is_empty() {
        writeln!(
            out,
            "land: you have uncommitted changes to tracked files; commit them first."
        )?;
        return Ok(Outcome::Refused("dirty"));
    }

    for attempt in 1..=opts.attempts.max(1) {
        let (ok, _, err) = git_run(
            dir,
            &["fetch", "-q", &opts.remote, &opts.branch],
            Duration::from_secs(300),
        )?;
        if !ok {
            return Err(format!("git fetch {target} failed: {}", err.trim()).into());
        }
        let tip = rev(dir, "FETCH_HEAD")?;
        match prepare(dir, opts, host, out, &tip, &[])? {
            Prep::Ready(_) => {}
            Prep::Refused(r) => return Ok(Outcome::Refused(r.reason)),
        }

        // 5. publish, fast-forward only
        let head = rev(dir, "HEAD")?;
        let refspec = format!("HEAD:refs/heads/{}", opts.branch);
        let (ok, _, err) = git_run(
            dir,
            &["push", "-q", &opts.remote, &refspec],
            Duration::from_secs(300),
        )?;
        if ok {
            writeln!(
                out,
                "land: LANDED {} on {target}.",
                &head[..head.len().min(12)]
            )?;
            return Ok(Outcome::Landed(head));
        }
        let raced = [
            "rejected",
            "fetch first",
            "non-fast-forward",
            "stale info",
            "failed to update ref",
            "cannot lock ref",
        ]
        .iter()
        .any(|w| err.contains(w));
        if !raced {
            return Err(format!("could not publish to {target}: {}", err.trim()).into());
        }
        writeln!(
            out,
            "land: {target} moved while landing (attempt {attempt}); merging the new tip and \
             checking again."
        )?;
    }
    writeln!(out, "land: {target} kept moving; run land again.")?;
    Ok(Outcome::Refused("target kept moving"))
}

/// A tree ready to publish, or why not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Prep {
    /// HEAD is this commit: gated against the tip, verified; publish it.
    Ready(String),
    /// Nothing may be published. The report was written to `out`.
    Refused(Refusal),
}

/// Why [`prepare`] refused, in a form another process can act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// Short machine reason: "nothing to land", "merge refused",
    /// "a merge on the branch was refused", "verify command failed".
    pub reason: &'static str,
    /// The files the gate refused (empty for a verify failure).
    pub files: Vec<String>,
    /// `path:first-last` line ranges of the conflict blocks left in those
    /// files, where there are markers.
    pub regions: Vec<String>,
}

impl Refusal {
    fn bare(reason: &'static str) -> Refusal {
        Refusal {
            reason,
            files: Vec::new(),
            regions: Vec::new(),
        }
    }
}

/// Steps 2–4 of the loop against one fetched `tip`, on the branch checked out
/// in `dir`: merge `tip` in and gate every file of the merge, gate every other
/// unchecked merge on the branch, run the verify command on the final tree.
/// `Ready(HEAD)` when all of it passed — the caller publishes HEAD with a
/// lease on `tip`. `env` is extra environment for the verify command.
///
/// A merge refusal leaves the merge in progress in `dir`, the refused files
/// conflicted, so the person can resolve them in place.
pub fn prepare(
    dir: &Path,
    opts: &Options,
    host: &Host,
    out: &mut dyn Write,
    tip: &str,
    env: &[(&str, &str)],
) -> R<Prep> {
    let gd = PathBuf::from(git(dir, &["rev-parse", "--absolute-git-dir"])?);
    let checked = Ledger(gd.join("weave-land-verified"));
    let verify_ok = Ledger(gd.join("weave-land-verify-ok"));
    let target = format!("{}/{}", opts.remote, opts.branch);

    if git(dir, &["rev-list", &format!("{tip}..HEAD")])?.is_empty() {
        writeln!(
            out,
            "land: nothing to land: your branch has no commits that are not already on {target}."
        )?;
        return Ok(Prep::Refused(Refusal::bare("nothing to land")));
    }

    // 2. the merge with the tip, gated before it is committed. land's own
    // merge whose commit failed is aborted cleanly; it is merged again once.
    if !git_ok(dir, &["merge-base", "--is-ancestor", tip, "HEAD"]) {
        let short = &tip[..tip.len().min(12)];
        let mut committed = false;
        for _ in 0..2 {
            if let Err(why) = start_merge(dir, &gd, &target, tip, out)? {
                return Ok(Prep::Refused(Refusal::bare(why)));
            }
            let conflicted = git(dir, &["diff", "--name-only", "--diff-filter=U"])?;
            if conflicted.is_empty() {
                writeln!(out, "land: merged {target} ({short}); checking the merge …")?;
            } else {
                writeln!(
                    out,
                    "land: merging {target} ({short}) conflicts in:\n{}",
                    indent(&conflicted)
                )?;
            }
            match gate_merge_in_progress(dir, opts, host, out, tip, true)? {
                Gated::Committed => {
                    committed = true;
                    break;
                }
                Gated::Refused(refused) => {
                    let regions = conflict_regions(dir, &refused);
                    if !regions.is_empty() {
                        writeln!(
                            out,
                            "land: conflict regions:\n{}",
                            indent(&regions.join("\n"))
                        )?;
                    }
                    return Ok(Prep::Refused(Refusal {
                        reason: "merge refused",
                        files: refused,
                        regions,
                    }));
                }
                Gated::Aborted => {
                    writeln!(
                        out,
                        "land: land's own merge could not be committed and was aborted; merging \
                         again."
                    )?;
                }
            }
        }
        if !committed {
            writeln!(
                out,
                "\nland: REFUSED. land's own merge could not be committed twice; nothing was \
                 published. Run land again once no other git command is running here."
            )?;
            return Ok(Prep::Refused(Refusal::bare("merge could not be committed")));
        }
    }
    // 3. every other merge on the branch not yet checked
    let refused = gate_branch_merges(dir, opts, host, out, &checked, tip)?;
    if !refused.is_empty() {
        return Ok(Prep::Refused(Refusal {
            reason: "a merge on the branch was refused",
            files: refused,
            regions: Vec::new(),
        }));
    }

    // 4. the verify command on the exact tree to be published
    if let Some(cmd) = &opts.verify_cmd {
        let tree = rev(dir, "HEAD^{tree}")?;
        let key = format!("{tree} {}", hex(&Sha256::digest(cmd.as_bytes())));
        if !verify_ok.has(&key) {
            writeln!(
                out,
                "land: running the verify command on the merged tree: {cmd}"
            )?;
            match run_verify(dir, cmd, tip, opts.verify_timeout, env)? {
                Ok(()) => verify_ok.add(&key)?,
                Err(report) => {
                    writeln!(out, "{report}")?;
                    writeln!(
                        out,
                        "\nland: REFUSED. The verify command failed on the merged tree \
                         (your branch + {target}); nothing was published. Fix the failure \
                         (it may come from combining your change with what is on {target}), \
                         commit the fix, then run land again."
                    )?;
                    return Ok(Prep::Refused(Refusal::bare("verify command failed")));
                }
            }
        }
    }
    Ok(Prep::Ready(rev(dir, "HEAD")?))
}

/// `path:first-last` (1-based) for every conflict block still in `files`.
pub fn conflict_regions(dir: &Path, files: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for f in files {
        let Ok(text) = std::fs::read_to_string(dir.join(f)) else {
            continue;
        };
        let mut open = None;
        for (i, line) in text.lines().enumerate() {
            if line.starts_with("<<<<<<<") {
                open = Some(i + 1);
            } else if line.starts_with(">>>>>>>") {
                if let Some(a) = open.take() {
                    out.push(format!("{f}:{a}-{}", i + 1));
                }
            }
        }
    }
    out
}

/// The merge in progress in `dir`: the person's in-place resolution, judged
/// with the working-tree gate and the both-sides invariant and committed when
/// it passes — or land's own unfinished merge, aborted (never judged as a
/// resolution). `true`: go on (committed, or aborted cleanly).
pub fn check_resolution(dir: &Path, opts: &Options, host: &Host, out: &mut dyn Write) -> R<bool> {
    let gd = PathBuf::from(git(dir, &["rev-parse", "--absolute-git-dir"])?);
    if let Some(why) = mergestate::clear_stale_lock(&gd, out)? {
        writeln!(
            out,
            "land: REFUSED. {why}. Nothing was published. Wait for that process to finish, then \
             run land again."
        )?;
        return Ok(false);
    }
    Ok(resume_merge_in_progress(dir, &gd, opts, host, out)?.is_none())
}

fn indent(lines: &str) -> String {
    lines
        .lines()
        .map(|l| format!("  {l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn write_certificate(opts: &Options, tag: &str, doc: &serde_json::Value) -> R<()> {
    if let Some(d) = &opts.certificate_dir {
        std::fs::create_dir_all(d)?;
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::fs::write(
            d.join(format!("{stamp}-{tag}.json")),
            serde_json::to_string_pretty(doc)? + "\n",
        )?;
    }
    Ok(())
}

/// How [`gate_merge_in_progress`] ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Gated {
    /// Every file passed, the tree keeps both sides' changes, and the merge
    /// is committed (HEAD).
    Committed,
    /// Nothing committed; these files are why (the report is printed).
    Refused(Vec<String>),
    /// land's own merge could not be committed and was aborted cleanly: the
    /// branch is back where it was before the merge. Merge again.
    Aborted,
}

/// Start land's own merge of `tip` (`git merge --no-commit --no-ff`),
/// recorded in `<git-dir>/weave-land-merge` as in-flight first. `Ok(Err)`
/// (the reason; the report printed) when git did not complete the merge —
/// it could not write the index, say — in which case whatever it left is
/// aborted: a merge git did not finish is never judged as a merge.
pub fn start_merge(
    dir: &Path,
    gd: &Path,
    target: &str,
    tip: &str,
    out: &mut dyn Write,
) -> R<Result<(), &'static str>> {
    let short = &tip[..tip.len().min(12)];
    if let Some(why) = mergestate::clear_stale_lock(gd, out)? {
        writeln!(
            out,
            "land: REFUSED. {why}. Nothing was published. Wait for that process to finish, then \
             run land again."
        )?;
        return Ok(Err("index.lock held"));
    }
    let pre = rev(dir, "HEAD")?;
    mergestate::write_record(gd, "in-flight", &pre, tip)?;
    let msg = format!("Merge {target} ({short}) into the branch");
    let (ok, mout, merr) = git_run(
        dir,
        &["merge", "--no-commit", "--no-ff", "-m", &msg, tip],
        Duration::from_secs(300),
    )?;
    let started = gd.join("MERGE_HEAD").exists();
    let conflicted = if started {
        git(dir, &["diff", "--name-only", "--diff-filter=U"])?
    } else {
        String::new()
    };
    // git merge exits 1 for a merge with conflicts; anything else that is not
    // a success — or any word that git could not write — is not a merge.
    let written =
        started && !mergestate::merge_not_written(&mout, &merr) && (ok || !conflicted.is_empty());
    if written {
        return Ok(Ok(()));
    }
    writeln!(
        out,
        "land: git did not complete the merge of {target} ({short}):\n{}",
        indent(format!("{}\n{}", mout.trim(), merr.trim()).trim())
    )?;
    let clean = if started {
        abort_own_merge(dir, gd, &pre, out)?
    } else {
        let back = rev(dir, "HEAD")? == pre
            && git(dir, &["status", "--porcelain", "--untracked-files=no"])?.is_empty();
        if back {
            mergestate::clear_record(gd);
        }
        back
    };
    writeln!(
        out,
        "\nland: REFUSED. Nothing was published, and nothing from that merge was taken as a \
         merge. {}",
        if clean {
            "Your branch is as it was. Run land again (if git could not write the index, wait \
             until no other git command is running in this repository)."
        } else {
            "land could not put your branch back as it was; its unfinished merge is recorded — \
             run land again: land aborts its own unfinished merge before anything else. Do not \
             commit that merge yourself."
        }
    )?;
    Ok(Err("merge did not complete"))
}

/// Abort the merge in progress that land itself started from `pre`, and
/// check that the branch is back at `pre` with a clean index and working
/// tree. `false` (said in `out`) when it could not be done; the record of the
/// unfinished merge stays, so the next run tries again.
fn abort_own_merge(dir: &Path, gd: &Path, pre: &str, out: &mut dyn Write) -> R<bool> {
    let mut tries = 0;
    loop {
        tries += 1;
        let (ok, _, err) = git_run(dir, &["merge", "--abort"], Duration::from_secs(120))?;
        if ok || !gd.join("MERGE_HEAD").exists() {
            break;
        }
        if tries >= 2 {
            writeln!(out, "land: git merge --abort failed: {}", err.trim())?;
            return Ok(false);
        }
        // Most likely the lock again: clear it if nobody holds it, else stop.
        if let Some(why) = mergestate::clear_stale_lock(gd, out)? {
            writeln!(out, "land: could not abort land's own merge: {why}.")?;
            return Ok(false);
        }
    }
    let back = rev(dir, "HEAD")? == pre
        && !gd.join("MERGE_HEAD").exists()
        && git(dir, &["status", "--porcelain", "--untracked-files=no"])?.is_empty();
    if back {
        mergestate::clear_record(gd);
    } else {
        writeln!(
            out,
            "land: after aborting its own merge the branch is not back at {} with a clean tree.",
            &pre[..pre.len().min(12)]
        )?;
    }
    Ok(back)
}

/// A merge is in progress when land starts. land's own unfinished merge
/// (recorded in-flight, same HEAD, same tip) is aborted — never taken as a
/// resolution. Anything else is the person's resolution, judged by the gate
/// and the both-sides invariant. `Some(reason)` = refused.
fn resume_merge_in_progress(
    dir: &Path,
    gd: &Path,
    opts: &Options,
    host: &Host,
    out: &mut dyn Write,
) -> R<Option<&'static str>> {
    let theirs = rev(dir, "MERGE_HEAD")?;
    let head = rev(dir, "HEAD")?;
    match mergestate::read_record(gd) {
        Some(r) if r.state == "in-flight" && r.pre == head && r.tip == theirs => {
            writeln!(
                out,
                "land: a merge of {} that land started was never finished (its commit failed, or \
                 land was stopped); it is nobody's resolution. Aborting it and merging again …",
                &theirs[..theirs.len().min(12)]
            )?;
            if abort_own_merge(dir, gd, &head, out)? {
                Ok(None)
            } else {
                writeln!(
                    out,
                    "\nland: REFUSED. Nothing was published. Run land again once no other git \
                     command is running here; do not commit that merge yourself."
                )?;
                Ok(Some("unfinished merge could not be aborted"))
            }
        }
        _ => {
            writeln!(
                out,
                "land: checking your resolution of the merge in progress …"
            )?;
            match gate_merge_in_progress(dir, opts, host, out, &theirs, false)? {
                Gated::Committed => Ok(None),
                _ => Ok(Some("resolution refused")),
            }
        }
    }
}

/// The working-tree gate over the merge in progress (theirs = `theirs`):
/// conflicted files answered by the file on disk once it has no markers
/// left (else by `--resolver`), cleanly merged files by what the merge wrote.
/// All PROVEN / VERIFIED, and the tree to be committed keeps every change
/// only one side made ([`preserve`]): the merge is committed and remembered
/// as checked. Otherwise the refused files are left conflicted.
///
/// `own`: land started this merge (it is recorded in-flight). A refusal
/// hands it to the person (recorded `refused`); a commit that fails aborts
/// it ([`Gated::Aborted`]) — land never leaves its own unfinished merge for
/// anyone to take as a resolution. Not `own`, it is the person's resolution:
/// whatever happens, it is left exactly as it is.
fn gate_merge_in_progress(
    dir: &Path,
    opts: &Options,
    host: &Host,
    out: &mut dyn Write,
    theirs: &str,
    own: bool,
) -> R<Gated> {
    let gd = PathBuf::from(git(dir, &["rev-parse", "--absolute-git-dir"])?);
    let plan = land::plan(
        dir,
        None,
        None,
        Some(theirs),
        Some(&Present::WorkingTree { unmerged: true }),
    )?;
    let mut reports = plan.not_text.clone();
    for unit in &plan.units {
        reports.push(land::land_unit(unit, host, opts.resolver.as_ref())?);
    }
    reports.sort_by(|a, b| a.path.cmp(&b.path));
    land::write_back(dir, &plan, &reports)?;
    write_certificate(
        opts,
        "merge",
        &land::document(&plan, &reports, "working-tree"),
    )?;
    let refused: Vec<&str> = reports
        .iter()
        .filter(|r| r.status == Status::Refused)
        .map(|r| r.path.as_str())
        .collect();
    let still_unmerged = git(dir, &["diff", "--name-only", "--diff-filter=U"])?;
    if !reports.is_empty() {
        write!(out, "{}", land::render(&plan, &reports))?;
    }
    let pre = rev(dir, "HEAD")?;
    if !refused.is_empty() || !still_unmerged.is_empty() {
        let mut names: BTreeSet<String> = refused.iter().map(|s| s.to_string()).collect();
        names.extend(still_unmerged.lines().map(str::to_string));
        let names: Vec<String> = names.into_iter().collect();
        writeln!(
            out,
            "\nland: REFUSED. These files are not merged in a way weave can prove or verify:\n{}\n\
             Edit them in place to resolve the conflicts (keep both your changes and the changes \
             from the other side; remove every conflict marker), re-run the tests, then run land \
             again. land checks your resolution and commits the merge for you. (Or abort the \
             merge with `git merge --abort`, change your own commits so they no longer collide — \
             a different case value or key, say — commit, and run land again.)",
            indent(&names.join("\n"))
        )?;
        if own {
            mergestate::write_record(&gd, "refused", &pre, theirs)?;
        }
        return Ok(Gated::Refused(names.into_iter().collect()));
    }

    // What the gate read from disk must be what is committed.
    let judged: Vec<&str> = plan
        .units
        .iter()
        .map(|u| u.path.as_str())
        .chain(plan.not_examined.iter().map(String::as_str))
        .collect();
    if !judged.is_empty() {
        let mut args = vec!["diff", "--name-only", "--"];
        args.extend(judged.iter().copied());
        let unstaged = git(dir, &args)?;
        if !unstaged.is_empty() {
            writeln!(
                out,
                "\nland: REFUSED. These files on disk are not what the merge commit would hold \
                 (the index has another version):\n{}\nStage what you mean to commit (git add), \
                 then run land again.",
                indent(&unstaged)
            )?;
            return Ok(Gated::Refused(
                unstaged.lines().map(str::to_string).collect(),
            ));
        }
    }

    // The tree to be committed, however it came about, keeps every change
    // only one side made — the gate above judged only files both changed.
    let tree = preserve::index_tree(dir, &gd)?;
    let lost = preserve::check(dir, &plan.base, &plan.ours, &plan.theirs, &tree)?;
    if !lost.is_empty() {
        let target = format!("{}/{}", opts.remote, opts.branch);
        write_certificate(
            opts,
            "preserve",
            &serde_json::json!({
                "schema": land::SCHEMA,
                "merge": {"base": plan.base, "ours": plan.ours, "theirs": plan.theirs, "tree": tree},
                "one_sided": lost,
            }),
        )?;
        writeln!(
            out,
            "\nland: REFUSED. The merge in progress does not keep every change: files only one \
             side changed have lost that change:\n{}",
            preserve::render(&lost, "your branch", &target)
        )?;
        let names: Vec<String> = lost.iter().map(|v| v.path.clone()).collect();
        if own {
            let clean = abort_own_merge(dir, &gd, &pre, out)?;
            writeln!(
                out,
                "land: {} Nothing was published. Run land again.",
                if clean {
                    "land aborted its merge."
                } else {
                    "land could not abort its merge; it stays recorded as unfinished and the next \
                     run aborts it."
                }
            )?;
        } else {
            writeln!(
                out,
                "This is not a resolution of the merge with {target}: a file only one side \
                 changed must keep that change (a merge git did not finish writing leaves such \
                 files as they were). Nothing was published. Run `git merge --abort`, then run \
                 land again — land merges again from scratch."
            )?;
        }
        return Ok(Gated::Refused(names));
    }

    // Fault injection for the tests (debug builds only): run a command just
    // before land's own commit — another git process taking index.lock, say.
    #[cfg(debug_assertions)]
    if let Ok(cmd) = std::env::var("WEAVE_LAND_FAULT_BEFORE_COMMIT") {
        let _ = Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .current_dir(dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
    let (ok, _, err) = git_run(
        dir,
        &["commit", "-q", "--no-edit"],
        Duration::from_secs(120),
    )?;
    if !ok {
        writeln!(
            out,
            "land: git commit of the checked merge failed: {}",
            err.trim()
        )?;
        if own && abort_own_merge(dir, &gd, &pre, out)? {
            return Ok(Gated::Aborted);
        }
        writeln!(
            out,
            "\nland: REFUSED. Nothing was published. {}",
            if own {
                "land's own merge could not be committed or aborted; it stays recorded as \
                 unfinished, and the next run aborts it and merges again. Run land again once no \
                 other git command is running here; do not commit that merge yourself."
            } else {
                "Your resolution is left as it is. Run land again once no other git command is \
                 running here."
            }
        )?;
        return Ok(Gated::Refused(Vec::new()));
    }
    let head = rev(dir, "HEAD")?;
    let committed = rev(dir, "HEAD^{tree}")?;
    if committed != tree {
        writeln!(
            out,
            "\nland: REFUSED. The merge commit {} holds tree {}, not the tree weave checked \
             ({}); nothing was published. Run land again: the commit is checked as a merge on \
             your branch.",
            &head[..head.len().min(12)],
            &committed[..committed.len().min(12)],
            &tree[..tree.len().min(12)]
        )?;
        mergestate::clear_record(&gd);
        return Ok(Gated::Refused(Vec::new()));
    }
    Ledger(gd.join("weave-land-verified")).add(&head)?;
    Ledger(gd.join("weave-land-preserved")).add(&head)?;
    mergestate::clear_record(&gd);
    writeln!(
        out,
        "land: the merge passed weave's gate; committed {}.",
        &head[..head.len().min(12)]
    )?;
    Ok(Gated::Committed)
}

/// Every merge commit in `tip..HEAD` not yet checked, judged in revisions
/// mode with its own tree as the answer. `false` (and the report printed)
/// when one is refused.
fn gate_branch_merges(
    dir: &Path,
    opts: &Options,
    host: &Host,
    out: &mut dyn Write,
    checked: &Ledger,
    tip: &str,
) -> R<Vec<String>> {
    let merges = git(dir, &["rev-list", "--merges", &format!("{tip}..HEAD")])?;
    let gd = PathBuf::from(git(dir, &["rev-parse", "--absolute-git-dir"])?);
    let preserved = Ledger(gd.join("weave-land-preserved"));
    for m in merges.lines().filter(|m| !m.is_empty()) {
        // Every merge on the branch, gated before or not, keeps every change
        // only one side made — as committed, or as the branch holds it now.
        if !preserved.has(m) {
            let parents = git(dir, &["rev-list", "--parents", "-n", "1", m])?;
            let parents: Vec<&str> = parents.split_whitespace().skip(1).collect();
            if parents.len() == 2 {
                let base = git(dir, &["merge-base", parents[0], parents[1]])?;
                let lost = preserve::check_now(dir, &base, parents[0], parents[1], m, "HEAD")?;
                if !lost.is_empty() {
                    let target = format!("{}/{}", opts.remote, opts.branch);
                    write_certificate(
                        opts,
                        &format!("{}-preserve", &m[..m.len().min(12)]),
                        &serde_json::json!({
                            "schema": land::SCHEMA,
                            "merge": {"base": base, "ours": parents[0], "theirs": parents[1], "result": m},
                            "one_sided": lost,
                        }),
                    )?;
                    writeln!(
                        out,
                        "\nland: REFUSED. The merge {} on your branch loses changes that only one \
                         side made (and the branch has not put them back since):\n{}\nPut each \
                         change back (for a file only the other side changed: `git checkout {} -- \
                         <file>`), commit, re-run the tests, then run land again.",
                        &m[..m.len().min(12)],
                        preserve::render(&lost, "your branch", &target),
                        &parents[1][..parents[1].len().min(12)]
                    )?;
                    return Ok(lost.into_iter().map(|v| v.path).collect());
                }
            }
            preserved.add(m)?;
        }
        if checked.has(m) {
            continue;
        }
        let parents = git(dir, &["rev-list", "--parents", "-n", "1", m])?;
        let parents: Vec<&str> = parents.split_whitespace().skip(1).collect();
        if parents.len() != 2 {
            writeln!(
                out,
                "land: REFUSED. {m} is a merge of {} parents; land checks two-parent merges only.",
                parents.len()
            )?;
            return Ok(vec![m.to_string()]);
        }
        let base = git(dir, &["merge-base", parents[0], parents[1]])?;
        let plan = land::plan(
            dir,
            Some(&base),
            Some(parents[0]),
            Some(parents[1]),
            Some(&Present::Rev(m.to_string())),
        )?;
        let mut reports = plan.not_text.clone();
        for unit in &plan.units {
            let mut r = land::land_unit(unit, host, None)?;
            // A merge refused as committed may have been repaired by a later
            // commit: what the branch holds now is the answer that counts,
            // and it is held to the same gate against the same three stages.
            if r.status == Status::Refused {
                let now = match git_run(
                    dir,
                    &["show", &format!("HEAD:{}", unit.path)],
                    Duration::from_secs(120),
                )? {
                    (true, text, _) => land::Candidate::File(text),
                    (false, _, _) => land::Candidate::Delete,
                };
                if unit.present.as_ref() != Some(&now) {
                    let later = land::Unit {
                        present: Some(now),
                        ..unit.clone()
                    };
                    let r2 = land::land_unit(&later, host, None)?;
                    if r2.status != Status::Refused {
                        r = r2;
                        r.reason = format!("as the branch holds it now: {}", r.reason);
                    }
                }
            }
            reports.push(r);
        }
        reports.sort_by(|a, b| a.path.cmp(&b.path));
        write_certificate(
            opts,
            &m[..m.len().min(12)],
            &land::document(&plan, &reports, "revisions"),
        )?;
        if reports.iter().any(|r| r.status == Status::Refused) {
            write!(out, "{}", land::render(&plan, &reports))?;
            writeln!(
                out,
                "\nland: REFUSED. The merge {} on your branch does not pass weave's gate (see \
                 above). Fix the files it names (keep both sides' changes), commit the fix, re-run \
                 the tests, then run land again.",
                &m[..m.len().min(12)]
            )?;
            return Ok(reports
                .iter()
                .filter(|r| r.status == Status::Refused)
                .map(|r| r.path.clone())
                .collect());
        }
        checked.add(m)?;
    }
    Ok(Vec::new())
}

/// Run the verify command in the repository root on a clean checkout of
/// HEAD. `Ok(Err(report))` when it fails or leaves tracked files changed.
fn run_verify(
    dir: &Path,
    cmd: &str,
    tip: &str,
    limit: Duration,
    env: &[(&str, &str)],
) -> R<Result<(), String>> {
    let mut c = Command::new("sh");
    c.arg("-c")
        .arg(cmd)
        .current_dir(dir)
        .env("WEAVE_LAND_ONTO", tip)
        .env("WEAVE_LAND_HEAD", rev(dir, "HEAD")?);
    for (k, v) in env {
        c.env(k, v);
    }
    let run = gitscan::run_bounded(c, "the verify command", None, limit);
    let (status, stdout, stderr) = match run {
        Ok(r) => r,
        Err(e) => return Ok(Err(format!("land: the verify command did not finish: {e}"))),
    };
    let tail = |b: &[u8]| {
        let t = String::from_utf8_lossy(b);
        let lines: Vec<&str> = t.lines().collect();
        lines[lines.len().saturating_sub(40)..].join("\n")
    };
    if !status.success() {
        return Ok(Err(format!(
            "land: the verify command exited {}:\n{}\n{}",
            status
                .code()
                .map_or("on a signal".into(), |c| c.to_string()),
            tail(&stdout),
            tail(&stderr)
        )));
    }
    let dirty = git(dir, &["status", "--porcelain", "--untracked-files=no"])?;
    if !dirty.is_empty() {
        return Ok(Err(format!(
            "land: the verify command changed tracked files; the tree it checked is not the tree \
             that would be published:\n{}",
            indent(&dirty)
        )));
    }
    Ok(Ok(()))
}
