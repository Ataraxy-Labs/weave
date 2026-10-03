//! Reading whole trees out of git, for the repo-scope pass.
//!
//! Deliberately shells out instead of reusing weave-core's git module: the
//! repo-scope pass must keep working while weave-core's internals move, and the
//! only thing it needs from git is "give me every supported file at this rev".
//!
//! Every `git` here runs through [`bounded`]: a deadline, both output pipes
//! drained on their own threads, and lazy fetching off. A check that waits on
//! git forever is a check that never says anything, which is worse than one
//! that says "git did not answer".

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::repo_scope::{is_supported, Tree};

type R<T> = Result<T, Box<dyn std::error::Error>>;

/// How long any one `git` may take before the check gives up on it.
const GIT_DEADLINE: Duration = Duration::from_secs(120);

/// Run `git <args>` in `dir`, feeding it `input`, and wait at most `limit`.
///
/// Three things make this unable to hang:
///
/// * stdin is written from its own thread and stdout/stderr are drained on
///   theirs, so no pipe can fill while the other waits — the two-pipe
///   deadlock that once hung `cat-file --batch`;
/// * `GIT_NO_LAZY_FETCH=1`: in a partial (blobless) clone, reading a blob that
///   is not local makes git fetch it from the promisor remote, ONE network
///   round trip per object. A merge with two thousand changed files then sits
///   idle for hours on a `git fetch` grandchild. Missing objects come back as
///   missing instead, and [`read_paths_at_rev`] fetches them in one batch;
/// * the deadline: past it the child is killed and the answer is an error
///   that names the command, never a wait.
fn bounded(dir: &Path, args: &[&str], input: Option<Vec<u8>>, limit: Duration) -> R<Vec<u8>> {
    let mut child = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    if let (Some(bytes), Some(mut stdin)) = (input, child.stdin.take()) {
        // A write error is git having exited early; its status says why.
        std::thread::spawn(move || {
            let _ = stdin.write_all(&bytes);
        });
    }
    let drain = |pipe: Option<Box<dyn Read + Send>>| {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut p) = pipe {
                let _ = p.read_to_end(&mut buf);
            }
            let _ = tx.send(buf);
        });
        rx
    };
    let out = drain(
        child
            .stdout
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );
    let err = drain(
        child
            .stderr
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );

    let deadline = Instant::now() + limit;
    let too_slow = || -> Box<dyn std::error::Error> {
        format!(
            "git {} did not finish within {}s — weave check stopped waiting rather than hang",
            args.join(" "),
            limit.as_secs()
        )
        .into()
    };
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(too_slow());
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    // A grandchild can outlive git and hold a pipe open; the deadline covers
    // that wait too.
    let remaining = || deadline.saturating_duration_since(Instant::now());
    let stdout = out.recv_timeout(remaining()).map_err(|_| too_slow())?;
    let stderr = err.recv_timeout(remaining()).map_err(|_| too_slow())?;
    if !status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&stderr).trim()
        )
        .into());
    }
    Ok(stdout)
}

fn git(dir: &Path, args: &[&str]) -> R<String> {
    let out = bounded(dir, args, None, GIT_DEADLINE)?;
    Ok(String::from_utf8_lossy(&out).to_string())
}

/// One rev's copies of a set of paths.
#[derive(Debug, Default)]
pub struct Stage {
    /// Path -> UTF-8 content, for every requested regular file the rev has.
    pub tree: Tree,
    /// Paths the rev has as a regular file whose bytes could not be read, and
    /// why. Absent is not unreadable: a path the rev does not have is simply
    /// not in `tree`, and that is a fact the checks reason about. An unreadable
    /// one is a fact they cannot reason about, and must not be mistaken for
    /// absence.
    pub unreadable: BTreeMap<String, String>,
    /// Paths the rev has as something other than a regular file — a symlink or
    /// a submodule. Their bytes are not source, and git's guarantees stand.
    pub irregular: BTreeSet<String>,
}

/// The blobs at `<rev>:<path>` for a *bounded* set of paths, in one
/// `ls-tree` and one `cat-file --batch`.
///
/// The tree listing comes first because it is the only way to tell "this rev
/// has no such file" from "this rev has the file but its bytes are not here"
/// — in a partial clone, trees are local and blobs often are not. Asking
/// `cat-file` for `<rev>:<path>` answers `missing` for both, which read a
/// non-local base blob as "base has no such file" and quietly corrupted every
/// line count built on it.
///
/// Blobs missing from a partial clone are then fetched in ONE `git fetch`
/// (the command git itself runs for a lazy fetch, batched), instead of the one
/// fetch per object that `cat-file` would otherwise make; whatever still is
/// not there is reported per path in [`Stage::unreadable`].
fn read_paths_at_rev(dir: &Path, rev: &str, paths: &[String]) -> R<Stage> {
    let mut stage = Stage::default();
    if paths.is_empty() {
        return Ok(stage);
    }
    let wanted: BTreeSet<&str> = paths.iter().map(String::as_str).collect();
    // `<mode> SP <type> SP <oid> TAB <path> NUL`
    let listing = bounded(
        dir,
        &["ls-tree", "-r", "-z", "--full-tree", rev],
        None,
        GIT_DEADLINE,
    )?;
    let mut blobs: Vec<(String, String)> = Vec::new(); // (path, oid)
    for entry in listing.split(|&b| b == 0).filter(|e| !e.is_empty()) {
        let entry = String::from_utf8_lossy(entry);
        let Some((meta, path)) = entry.split_once('\t') else {
            continue;
        };
        if !wanted.contains(path) {
            continue;
        }
        let mut meta = meta.split(' ');
        let (mode, oid) = (meta.next().unwrap_or(""), meta.nth(1).unwrap_or(""));
        if mode == "100644" || mode == "100755" {
            blobs.push((path.to_string(), oid.to_string()));
        } else {
            stage.irregular.insert(path.to_string());
        }
    }

    let oids: Vec<&str> = blobs.iter().map(|(_, oid)| oid.as_str()).collect();
    let mut found = cat_blobs(dir, &oids)?;
    let missing: Vec<&str> = oids
        .iter()
        .copied()
        .filter(|oid| !found.contains_key(*oid))
        .collect();
    let mut why_missing = "the blob is not in the local object store".to_string();
    if !missing.is_empty() {
        match prefetch(dir, &missing) {
            Ok(()) => found.extend(cat_blobs(dir, &missing)?),
            Err(e) => why_missing = e.to_string(),
        }
    }
    for (path, oid) in blobs {
        match found.get(&oid) {
            Some(Some(text)) => {
                stage.tree.insert(path, text.clone());
            }
            // Not UTF-8: a binary file, which no check here reads anyway.
            Some(None) => {}
            None => {
                stage
                    .unreadable
                    .insert(path, format!("{rev}: {why_missing}"));
            }
        }
    }
    Ok(stage)
}

/// `oid -> Some(text)` for every blob the object store has (`None` when it is
/// not UTF-8); an oid it does not have is left out.
fn cat_blobs(dir: &Path, oids: &[&str]) -> R<BTreeMap<String, Option<String>>> {
    let mut out = BTreeMap::new();
    if oids.is_empty() {
        return Ok(out);
    }
    let mut request = String::new();
    for oid in oids {
        request.push_str(oid);
        request.push('\n');
    }
    let bytes = bounded(
        dir,
        &["cat-file", "--batch"],
        Some(request.into_bytes()),
        GIT_DEADLINE,
    )?;
    // Reply framing: `<oid> <type> <size>\n<size bytes>\n`, or `<oid> missing\n`.
    // Requests are bare oids, so every header is space-separated and none can
    // carry a newline.
    let mut pos = 0usize;
    while pos < bytes.len() {
        let Some(nl) = bytes[pos..].iter().position(|&b| b == b'\n') else {
            break;
        };
        let header = String::from_utf8_lossy(&bytes[pos..pos + nl]).to_string();
        pos += nl + 1;
        let fields: Vec<&str> = header.split(' ').collect();
        let [oid, _kind, size] = fields[..] else {
            continue; // `<oid> missing`: no payload follows
        };
        let size: usize = size.parse()?;
        if pos + size > bytes.len() {
            return Err("git cat-file --batch reply was truncated".into());
        }
        let text = std::str::from_utf8(&bytes[pos..pos + size])
            .ok()
            .map(str::to_string);
        out.insert(oid.to_string(), text);
        pos += size + 1; // trailing newline after the payload
    }
    Ok(out)
}

/// The remote a partial clone lazily fetches from: `remote.<name>.promisor`
/// (what `git clone --filter` writes), or the older `extensions.partialClone`.
fn promisor_remote(dir: &Path) -> Option<String> {
    let flagged = git(dir, &["config", "--get-regexp", r"^remote\..*\.promisor$"]).ok();
    flagged
        .iter()
        .flat_map(|s| s.lines())
        .filter_map(|l| l.split_once(' '))
        .filter(|(_, v)| v.trim() == "true")
        .find_map(|(k, _)| {
            k.strip_prefix("remote.")?
                .strip_suffix(".promisor")
                .map(str::to_string)
        })
        .or_else(|| {
            git(dir, &["config", "--get", "extensions.partialClone"])
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
}

/// Fetch `oids` from the partial clone's promisor remote in ONE request.
///
/// This is the fetch git runs for a single lazily-read object, given every
/// object at once. It is an error — never a wait past the deadline — when the
/// repository is not a partial clone or the remote does not answer.
fn prefetch(dir: &Path, oids: &[&str]) -> R<()> {
    let remote = promisor_remote(dir)
        .ok_or("the blob is not in the local object store and this is not a partial clone")?;
    let mut request = String::new();
    for oid in oids {
        request.push_str(oid);
        request.push('\n');
    }
    bounded(
        dir,
        &[
            "-c",
            "fetch.negotiationAlgorithm=noop",
            "fetch",
            &remote,
            "--no-tags",
            "--no-write-fetch-head",
            "--recurse-submodules=no",
            "--filter=blob:none",
            "--stdin",
        ],
        Some(request.into_bytes()),
        GIT_DEADLINE,
    )
    .map_err(|e| {
        format!(
            "{} blob(s) are not local and could not be fetched: {e}",
            oids.len()
        )
    })?;
    Ok(())
}

pub(crate) fn rev_exists(dir: &Path, rev: &str) -> bool {
    git(dir, &["rev-parse", "--verify", "--quiet", rev]).is_ok()
}

/// Git's canonical empty-tree object id — the tree with no entries.
const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

/// Parent of a replayed commit (`REBASE_HEAD` / `CHERRY_PICK_HEAD`), or the
/// empty tree when the replayed commit is a root commit.
fn replay_base(dir: &Path, theirs_rev: &str) -> R<String> {
    let parent = format!("{theirs_rev}^");
    if rev_exists(dir, &parent) {
        Ok(git(dir, &["rev-parse", &parent])?.trim().to_string())
    } else {
        Ok(EMPTY_TREE.to_string())
    }
}

/// Resolve the merge triple, defaulting to the operation in progress.
///
/// `ours` defaults to `HEAD`; `theirs` defaults to the commit of whatever
/// three-way operation is underway. A plain merge records it as `MERGE_HEAD`,
/// but a rebase, cherry-pick or revert conflict has no `MERGE_HEAD` — git
/// records the commit being applied as `REBASE_HEAD` / `CHERRY_PICK_HEAD` /
/// `REVERT_HEAD` instead, so a bare `weave check` can still verify a resolution
/// against it (issue #157). `base` defaults to the merge base of the two for a
/// merge or revert, and to the replayed commit's parent for a rebase or
/// cherry-pick: a replay applies one commit as a patch, and the merge base
/// would smear the whole branch divergence into the context.
pub(crate) fn resolve_revs(
    dir: &Path,
    base: Option<&str>,
    ours: Option<&str>,
    theirs: Option<&str>,
) -> R<(String, String, String)> {
    if !rev_exists(dir, "HEAD") {
        return Err(
            "not a git repository (or no commits yet) — use the directory mode instead".into(),
        );
    }
    let ours = ours.unwrap_or("HEAD").to_string();
    let theirs = match theirs {
        Some(t) => t.to_string(),
        None => [
            "MERGE_HEAD",
            "REBASE_HEAD",
            "CHERRY_PICK_HEAD",
            "REVERT_HEAD",
        ]
        .into_iter()
        .find(|r| rev_exists(dir, r))
        .map(str::to_string)
        .ok_or(
            "no --theirs given and no merge, rebase, cherry-pick or revert in progress \
                 (no MERGE_HEAD / REBASE_HEAD / CHERRY_PICK_HEAD / REVERT_HEAD)",
        )?,
    };
    let base = match base {
        Some(b) => b.to_string(),
        None if theirs == "REBASE_HEAD" || theirs == "CHERRY_PICK_HEAD" => {
            replay_base(dir, &theirs)?
        }
        None => git(dir, &["merge-base", &ours, &theirs])?
            .trim()
            .to_string(),
    };
    Ok((base, ours, theirs))
}

pub fn trees(
    dir: &Path,
    base: Option<&str>,
    ours: Option<&str>,
    theirs: Option<&str>,
) -> R<(Tree, Tree, Tree)> {
    let (base, ours, theirs) = resolve_revs(dir, base, ours, theirs)?;
    Ok((
        read_rev_tree(dir, &base)?,
        read_rev_tree(dir, &ours)?,
        read_rev_tree(dir, &theirs)?,
    ))
}

/// Everything the working-tree check needs: the three merge stages, the bytes
/// on disk, the files the merge actually had to decide, and a sentence naming
/// what was compared.
pub struct MergeScope {
    pub base: Tree,
    pub ours: Tree,
    pub theirs: Tree,
    /// The working tree, as it is right now.
    pub work: Tree,
    /// Files BOTH sides changed, plus anything git still has unmerged.
    pub subjects: Vec<String>,
    /// Subjects a merge stage has but whose bytes could not be read, and why.
    /// They are not in `subjects`: a file checked against a stage it could not
    /// see would be checked against a guess. Each still gets a verdict.
    pub unreadable: BTreeMap<String, String>,
    /// Subjects a merge stage has as a symlink or a submodule: not source, and
    /// not in `subjects`.
    pub irregular: BTreeSet<String>,
    pub scope: String,
}

/// Find the merge/rebase this repository is in — or has just finished — and
/// read it.
///
/// Three shapes, in order, because they are the moments an agent asks:
///
/// * **mid-merge**: `MERGE_HEAD` exists. Ours is `HEAD`, theirs is
///   `MERGE_HEAD`. This is the state right after `git merge` exits 1, and it
///   survives `git add` — which is exactly why the index's unmerged list alone
///   is not enough to find the subjects.
/// * **mid-rebase / cherry-pick / revert**: there is no `MERGE_HEAD`, but git
///   records the commit being applied as `REBASE_HEAD` / `CHERRY_PICK_HEAD` /
///   `REVERT_HEAD`. Ours is `HEAD` (the side built so far), theirs is that
///   commit. Without this, `weave check` fell through to "nothing was checked"
///   during every rebase, while still suggesting itself (issue #157). For a
///   rebase or cherry-pick the base is that commit's parent (`REBASE_HEAD^`),
///   not `merge-base(ours, theirs)`: a replay applies one commit as a patch,
///   and the merge base would smear the whole branch's divergence into it.
/// * **just committed**: `HEAD` has two parents. Ours is `HEAD^1`, theirs is
///   `HEAD^2`. An agent that committed and then wants to know what it did.
///
/// No shape present is not an error and must not be reported as one — it is
/// the sentence "there is no merge here", which the caller prints.
pub fn merge_scope(dir: &Path) -> R<Option<MergeScope>> {
    if !rev_exists(dir, "HEAD") {
        return Ok(None);
    }
    let (ours_rev, theirs_rev, moment, use_replay_base) = if rev_exists(dir, "MERGE_HEAD") {
        (
            "HEAD".to_string(),
            "MERGE_HEAD".to_string(),
            "merge in progress",
            false,
        )
    } else if let Some(op_head) = ["REBASE_HEAD", "CHERRY_PICK_HEAD", "REVERT_HEAD"]
        .into_iter()
        .find(|r| rev_exists(dir, r))
    {
        // A rebase, cherry-pick or revert conflict has no MERGE_HEAD; git
        // records the commit being applied as *_HEAD, and the unmerged index
        // stages are the real three-way. HEAD is the side built so far, so
        // `weave check` can verify a resolution here too (issue #157). A
        // replayed commit's base is its own parent; a revert keeps the merge
        // base, as before.
        let moment = match op_head {
            "REBASE_HEAD" => "rebase in progress",
            "CHERRY_PICK_HEAD" => "cherry-pick in progress",
            _ => "revert in progress",
        };
        (
            "HEAD".to_string(),
            op_head.to_string(),
            moment,
            op_head != "REVERT_HEAD",
        )
    } else if rev_exists(dir, "HEAD^2") {
        (
            "HEAD^1".to_string(),
            "HEAD^2".to_string(),
            "merge just committed",
            false,
        )
    } else {
        return Ok(None);
    };
    let base_rev = if use_replay_base {
        replay_base(dir, &theirs_rev)?
    } else {
        git(dir, &["merge-base", &ours_rev, &theirs_rev])?
            .trim()
            .to_string()
    };

    // The subjects are every file this merge PRODUCED: whatever either side
    // moved. Restricting it to files both sides moved was the obvious-looking
    // choice and it was wrong — the merge's most dangerous breakage is the
    // cross-file one, where one side renames a definition in `a.py` and the
    // other adds a caller in `b.py`, and neither file is contested. A checker
    // that only looks where git had to choose cannot see it.
    //
    // This is asked of git by NAME — a name-only diff of each side against the
    // base — and never by reading the two whole trees and comparing them in
    // process. On a large monorepo that whole-tree read is the check's dominant
    // cost, and it buys nothing: the answer is exactly the set git already has
    // as the merge's changed paths.
    //
    // `--no-renames`: rename detection reads blobs, which in a partial clone
    // means fetching them, and a rename's OLD path is a path the merge
    // produced too.
    let mut subjects: BTreeSet<String> = BTreeSet::new();
    for side in [&ours_rev, &theirs_rev] {
        let list = git(
            dir,
            &["diff", "--name-only", "--no-renames", "-z", &base_rev, side],
        )?;
        subjects.extend(
            list.split('\0')
                .filter(|p| !p.is_empty() && is_supported(p))
                .map(str::to_string),
        );
    }
    // …plus whatever git still calls unmerged, whether or not both sides moved
    // it: git's own verdict about what is unresolved outranks ours.
    if let Ok(list) = git(
        dir,
        &[
            "diff",
            "--name-only",
            "--no-renames",
            "--diff-filter=U",
            "-z",
        ],
    ) {
        subjects.extend(
            list.split('\0')
                .filter(|p| !p.is_empty() && is_supported(p))
                .map(str::to_string),
        );
    }
    let subjects: Vec<String> = subjects.into_iter().collect();

    // Read the three merge stages for the SUBJECTS ONLY — one `ls-tree` and one
    // `cat-file --batch` per rev, not a `git show` per file over the whole tree. The
    // dangling pass proves this is exact: a name is only ever "gone" from a file
    // both a stage and the working tree disagree about, which is a subject; an
    // untouched file's stage and its working-tree copy are identical, so it can
    // neither create a dangling finding nor host the definition that resolves
    // one from stage data. Suppression by an untouched file's *surviving*
    // definition is the working tree's job, and `work` below stays repo-wide for
    // exactly that.
    let stages = [
        read_paths_at_rev(dir, &base_rev, &subjects)?,
        read_paths_at_rev(dir, &ours_rev, &subjects)?,
        read_paths_at_rev(dir, &theirs_rev, &subjects)?,
    ];
    let work = read_worktree(dir)?;
    let scope = format!(
        "working tree vs the three merge stages of {ours_rev} × {theirs_rev} \
         (base {}, {moment}) — {} file(s) either side changed",
        &base_rev[..base_rev.len().min(8)],
        subjects.len()
    );
    // A symlink or submodule is not source: git's guarantees stand for it, as
    // for any file weave has no grammar for.
    let mut unreadable: BTreeMap<String, String> = BTreeMap::new();
    let mut irregular: BTreeSet<String> = BTreeSet::new();
    for stage in &stages {
        for (path, why) in &stage.unreadable {
            unreadable
                .entry(path.clone())
                .or_insert_with(|| why.clone());
        }
        irregular.extend(stage.irregular.iter().cloned());
    }
    let subjects = subjects
        .into_iter()
        .filter(|p| !irregular.contains(p) && !unreadable.contains_key(p))
        .collect();
    irregular.retain(|p| !unreadable.contains_key(p));
    let [base, ours, theirs] = stages.map(|s| s.tree);
    Ok(Some(MergeScope {
        base,
        ours,
        theirs,
        work,
        subjects,
        unreadable,
        irregular,
        scope,
    }))
}

/// Every tracked, supported, regular file as it exists on disk right now.
///
/// Regular only: reading through a tracked symlink reads whatever it points
/// at, and a link to a FIFO or a device never returns.
pub(crate) fn read_worktree(dir: &Path) -> R<Tree> {
    let listing = git(dir, &["ls-files", "-z"])?;
    let mut tree = Tree::new();
    for rel in listing.split('\0').filter(|p| !p.is_empty()) {
        if !is_supported(rel) {
            continue;
        }
        let path = dir.join(rel);
        if !std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_file()) {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(path) {
            tree.insert(rel.to_string(), content);
        }
    }
    Ok(tree)
}

/// The three merge stages of ONE path, straight out of the index
/// (`:1:`/`:2:`/`:3:`) while the file is unmerged, falling back to the merge
/// revisions once it has been staged.
///
/// `weave explain` needs the triple for a single file and must keep working
/// after `git add` — the index stages disappear at that moment, and an
/// explanation that stops being available the instant you stage is an
/// explanation nobody can use twice.
pub fn file_stages(dir: &Path, path: &str) -> R<(String, String, String)> {
    let stage = |n: u8| git(dir, &["show", &format!(":{n}:{path}")]).ok();
    if let (Some(b), Some(o), Some(t)) = (stage(1), stage(2), stage(3)) {
        return Ok((b, o, t));
    }
    let scope = merge_scope(dir)?.ok_or(
        "no merge, rebase, cherry-pick or revert in progress and HEAD is not a merge \
                commit — there is no three-way context to explain this file against",
    )?;
    let get = |t: &Tree| t.get(path).cloned().unwrap_or_default();
    if !scope.ours.contains_key(path) && !scope.theirs.contains_key(path) {
        return Err(format!("`{path}` is in neither side of this merge").into());
    }
    Ok((get(&scope.base), get(&scope.ours), get(&scope.theirs)))
}

/// The WHOLE tree at `rev`, restricted to files weave has a grammar for.
/// Unsupported / binary / oversize files inherit git's guarantees wholesale
/// and are not scanned.
pub(crate) fn read_rev_tree(dir: &Path, rev: &str) -> R<Tree> {
    let listing = git(
        dir,
        &["ls-tree", "-r", "--name-only", "-z", "--full-tree", rev],
    )?;
    let paths: Vec<String> = listing
        .split('\0')
        .filter(|p| !p.is_empty() && is_supported(p))
        .map(str::to_string)
        .collect();
    // An unreadable blob is skipped here, as an unreadable `git show` always
    // was: this pass reports cross-file findings, not per-file verdicts.
    Ok(read_paths_at_rev(dir, rev, &paths)?.tree)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::atomic::{AtomicU32, Ordering};

    static FIXTURE_COUNTER: AtomicU32 = AtomicU32::new(0);

    fn git_ok(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .expect("run git");
        assert!(
            out.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
        String::from_utf8_lossy(&out.stdout).to_string()
    }

    fn git_fails(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .expect("run git");
        assert!(
            !status.success(),
            "git {} unexpectedly succeeded",
            args.join(" ")
        );
    }

    fn git_fixture(name: &str) -> PathBuf {
        let n = FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "weave-gitscan-test-{}-{}-{}",
            std::process::id(),
            name,
            n
        ));
        fs::create_dir_all(&root).expect("create fixture dir");
        git_ok(&root, &["init", "-q"]);
        git_ok(&root, &["checkout", "-b", "main"]);
        root
    }

    /// The batch read must return even when git's replies are far larger than
    /// one pipe buffer. Writing every request before reading any reply
    /// deadlocked here: git blocked on a full stdout, stopped reading stdin,
    /// and the request write never returned. It runs on a watchdog thread so a
    /// regression fails the test instead of hanging the suite.
    #[test]
    fn batch_read_of_large_blobs_does_not_deadlock() {
        // Both pipes have to overflow for the deadlock: enough requests that
        // they outgrow the stdin buffer, and enough reply bytes to fill stdout
        // before git has read them all.
        let root = git_fixture("cat-file-large");
        let deep = "a_directory_name_long_enough_to_fatten_every_request/and_one_more_level";
        fs::create_dir_all(root.join(deep)).expect("mkdir");
        let mut paths = Vec::new();
        for i in 0..3000 {
            let name = format!("{deep}/module_number_{i:05}.py");
            fs::write(root.join(&name), format!("value_{i} = {i}\n").repeat(8)).expect("write");
            paths.push(name);
        }
        paths.push("absent.py".to_string());
        git_ok(&root, &["add", "."]);
        git_ok(&root, &["commit", "-m", "big"]);

        let (tx, rx) = std::sync::mpsc::channel();
        let dir = root.clone();
        let asked = paths.clone();
        std::thread::spawn(move || {
            let _ = tx.send(
                read_paths_at_rev(&dir, "HEAD", &asked)
                    .map(|s| s.tree)
                    .map_err(|e| e.to_string()),
            );
        });
        let tree = rx
            .recv_timeout(std::time::Duration::from_secs(60))
            .expect("read_paths_at_rev hung on a large batch")
            .expect("read_paths_at_rev failed");
        assert_eq!(
            tree.len(),
            3000,
            "every present path read, the absent one skipped"
        );
        assert_eq!(
            tree[&format!("{deep}/module_number_02999.py")],
            "value_2999 = 2999\n".repeat(8)
        );
    }

    /// Rebasing a diverged branch onto main leaves REBASE_HEAD at the replayed
    /// commit; merge_scope must recognize that shape so `weave check` works.
    #[test]
    fn merge_scope_during_rebase_conflict() {
        let root = git_fixture("rebase-conflict");
        let base_content = "def a():\n    return 1\n\ndef keep():\n    return 'stable'\n";

        fs::write(root.join("m.py"), base_content).expect("write base");
        git_ok(&root, &["add", "m.py"]);
        git_ok(&root, &["commit", "-m", "initial"]);

        git_ok(&root, &["checkout", "-b", "feature"]);
        let feature_content = base_content.replace("return 1", "return 2");
        fs::write(root.join("m.py"), &feature_content).expect("write feature");
        git_ok(&root, &["add", "m.py"]);
        git_ok(&root, &["commit", "-m", "feature"]);
        let replayed_commit = git_ok(&root, &["rev-parse", "HEAD"]).trim().to_string();

        git_ok(&root, &["checkout", "main"]);
        let main_content = base_content.replace("return 1", "return 3");
        fs::write(root.join("m.py"), &main_content).expect("write main");
        git_ok(&root, &["add", "m.py"]);
        git_ok(&root, &["commit", "-m", "main"]);

        git_ok(&root, &["checkout", "feature"]);
        git_fails(&root, &["rebase", "main"]);

        let scope = merge_scope(&root)
            .expect("merge_scope should not error")
            .expect("merge_scope should recognize mid-rebase state");

        assert!(
            scope.scope.contains("rebase in progress"),
            "scope should name the rebase moment: {}",
            scope.scope
        );

        let rebase_head = git_ok(&root, &["rev-parse", "REBASE_HEAD"])
            .trim()
            .to_string();
        assert_eq!(
            rebase_head, replayed_commit,
            "REBASE_HEAD should be the commit being replayed"
        );

        let theirs_rev = git_ok(&root, &["rev-parse", "REBASE_HEAD"])
            .trim()
            .to_string();
        assert_eq!(
            scope.theirs.get("m.py").map(String::as_str),
            Some(feature_content.as_str()),
            "theirs stage should come from REBASE_HEAD"
        );
        assert!(
            scope.subjects.iter().any(|p| p == "m.py"),
            "subjects should include the conflicted file: {:?}",
            scope.subjects
        );

        // Sanity: ours is HEAD (main's tip), not the replayed commit.
        let head_rev = git_ok(&root, &["rev-parse", "HEAD"]).trim().to_string();
        assert_ne!(head_rev, replayed_commit);
        assert_eq!(
            scope.ours.get("m.py").map(String::as_str),
            Some(main_content.as_str()),
            "ours stage should come from HEAD"
        );
        assert_eq!(theirs_rev, replayed_commit);
    }

    /// A blobless clone whose blobs are not local: the source, and a bare
    /// partial clone of it that has every tree and no blob.
    fn partial_clone(name: &str, files: &[(&str, &str)]) -> (PathBuf, PathBuf) {
        let src = git_fixture(name);
        git_ok(&src, &["config", "uploadpack.allowFilter", "true"]);
        git_ok(&src, &["config", "uploadpack.allowAnySHA1InWant", "true"]);
        for (path, text) in files {
            if let Some(dir) = Path::new(path).parent() {
                fs::create_dir_all(src.join(dir)).expect("mkdir");
            }
            fs::write(src.join(path), text).expect("write");
        }
        git_ok(&src, &["add", "."]);
        git_ok(
            &src,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "commit",
                "-qm",
                "c",
            ],
        );
        let clone = src.with_extension("partial.git");
        let url = format!("file://{}", src.display());
        let parent = clone.parent().expect("parent");
        git_ok(
            parent,
            &[
                "clone",
                "-q",
                "--bare",
                "--filter=blob:none",
                &url,
                clone.to_str().expect("utf8"),
            ],
        );
        (src, clone)
    }

    /// Runs `f` on a watchdog thread, so a hang fails the test instead of the
    /// suite.
    fn within<T: Send + 'static>(secs: u64, f: impl FnOnce() -> T + Send + 'static) -> T {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(f());
        });
        rx.recv_timeout(std::time::Duration::from_secs(secs))
            .expect("hung past its deadline")
    }

    /// In a partial clone, every blob the stage read needs arrives in ONE
    /// fetch — not one `git fetch` per object, which is what `cat-file --batch`
    /// does on its own, and which left `weave check` idle for hours on a merge
    /// of a few thousand files.
    #[test]
    fn a_partial_clone_fetches_missing_stage_blobs_in_one_batch() {
        let files: Vec<(String, String)> = (0..40)
            .map(|i| {
                (
                    format!("pkg/m{i}.py"),
                    format!("def f{i}():\n    return {i}\n"),
                )
            })
            .collect();
        let refs: Vec<(&str, &str)> = files
            .iter()
            .map(|(p, t)| (p.as_str(), t.as_str()))
            .collect();
        let (_src, clone) = partial_clone("partial-fetch", &refs);
        let paths: Vec<String> = files.iter().map(|(p, _)| p.clone()).collect();
        let packs = |dir: &Path| {
            fs::read_dir(dir.join("objects/pack"))
                .expect("pack dir")
                .filter(|e| {
                    e.as_ref()
                        .is_ok_and(|e| e.path().extension().is_some_and(|x| x == "pack"))
                })
                .count()
        };
        assert_eq!(packs(&clone), 1, "the clone's own pack: trees, no blobs");
        let dir = clone.clone();
        let stage = within(60, move || {
            read_paths_at_rev(&dir, "HEAD", &paths).map_err(|e| e.to_string())
        })
        .expect("read");
        assert!(stage.unreadable.is_empty(), "{:?}", stage.unreadable);
        assert_eq!(stage.tree.len(), 40);
        assert_eq!(stage.tree["pkg/m7.py"], "def f7():\n    return 7\n");
        // One fetch writes one pack. A fetch per object would write forty.
        assert_eq!(packs(&clone), 2, "all forty blobs arrived in one fetch");
    }

    /// When the promisor remote cannot answer, a blob that is not local is
    /// reported UNREADABLE — promptly, per path — and never confused with a
    /// path the rev does not have.
    #[test]
    fn an_unfetchable_blob_is_unreadable_not_absent_and_not_a_hang() {
        let (_src, clone) = partial_clone("partial-dead", &[("a.py", "def a():\n    return 1\n")]);
        git_ok(
            &clone,
            &["remote", "set-url", "origin", "/nonexistent/weave-remote"],
        );
        let asked = vec!["a.py".to_string(), "never_existed.py".to_string()];
        let stage = within(60, move || {
            read_paths_at_rev(&clone, "HEAD", &asked).map_err(|e| e.to_string())
        })
        .expect("a dead remote is a per-path answer, not an error");
        assert!(stage.tree.is_empty());
        assert!(
            stage.unreadable.contains_key("a.py"),
            "{:?}",
            stage.unreadable
        );
        assert!(
            !stage.unreadable.contains_key("never_existed.py"),
            "absent is not unreadable"
        );
    }

    /// A symlink or a submodule is not a source file: it is set aside, not read
    /// as one (a symlink's blob is its target path) and not read through.
    #[cfg(unix)]
    #[test]
    fn a_symlink_is_irregular_not_source() {
        let root = git_fixture("symlink");
        fs::write(root.join("real.py"), "x = 1\n").expect("write");
        std::os::unix::fs::symlink("real.py", root.join("link.py")).expect("symlink");
        git_ok(&root, &["add", "."]);
        git_ok(
            &root,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "commit",
                "-qm",
                "c",
            ],
        );
        let stage =
            read_paths_at_rev(&root, "HEAD", &["real.py".into(), "link.py".into()]).expect("read");
        assert_eq!(stage.tree.keys().collect::<Vec<_>>(), vec!["real.py"]);
        assert!(stage.irregular.contains("link.py"));
        let work = read_worktree(&root).expect("worktree");
        assert!(
            !work.contains_key("link.py"),
            "a symlink is never read through"
        );
    }

    /// Past its deadline a git child is killed and the answer is an error that
    /// names it — even when a grandchild still holds the output pipe open.
    #[test]
    fn a_git_that_never_answers_is_an_error_not_a_wait() {
        let root = git_fixture("slow");
        let started = std::time::Instant::now();
        let err = within(30, move || {
            bounded(
                &root,
                &["-c", "alias.stall=!sleep 20", "stall"],
                None,
                std::time::Duration::from_secs(1),
            )
            .map_err(|e| e.to_string())
        })
        .expect_err("a stalled git must not succeed");
        assert!(err.contains("did not finish within 1s"), "{err}");
        assert!(started.elapsed() < std::time::Duration::from_secs(10));
    }
}
