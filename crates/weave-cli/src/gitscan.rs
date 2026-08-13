//! Reading whole trees out of git, for the repo-scope pass.
//!
//! Deliberately shells out instead of reusing weave-core's git module: the
//! repo-scope pass must keep working while weave-core's internals move, and the
//! only thing it needs from git is "give me every supported file at this rev".

use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::repo_scope::{is_supported, Tree};

type R<T> = Result<T, Box<dyn std::error::Error>>;

fn git(dir: &Path, args: &[&str]) -> R<String> {
    let out = Command::new("git").args(args).current_dir(dir).output()?;
    if !out.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        )
        .into());
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// The blobs at `<rev>:<path>` for a *bounded* set of paths, fetched in ONE
/// `git cat-file --batch` process instead of a `git show` fork per path.
///
/// This is what lets the working-tree check scope to the files a merge touched:
/// `read_rev_tree` reads the WHOLE tree at a rev with a subprocess per file, an
/// O(repo) cost that dominates on a large monorepo. Here the caller names the
/// handful of paths it actually needs and pays one process for all of them.
///
/// A path absent at that rev (git answers `… missing`) is simply not inserted —
/// the same silent skip `read_rev_tree` gives an unreadable blob.
fn read_paths_at_rev(dir: &Path, rev: &str, paths: &[String]) -> R<Tree> {
    let mut tree = Tree::new();
    if paths.is_empty() {
        return Ok(tree);
    }
    let mut child = Command::new("git")
        .args(["cat-file", "--batch"])
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;

    // Feed every request, then close stdin so git flushes and exits. The order
    // of requests is the order of replies, so we replay `paths` to label them.
    {
        let mut stdin = child.stdin.take().ok_or("cat-file stdin")?;
        let mut buf = String::new();
        for p in paths {
            buf.push_str(rev);
            buf.push(':');
            buf.push_str(p);
            buf.push('\n');
        }
        stdin.write_all(buf.as_bytes())?;
        // stdin dropped here → EOF to git.
    }
    let out = child.wait_with_output()?;
    if !out.status.success() {
        return Err("git cat-file --batch failed".into());
    }

    // Reply framing: `<oid> <type> <size>\n<size bytes>\n`, or `<spec> missing\n`.
    let bytes = &out.stdout;
    let mut pos = 0usize;
    for p in paths {
        // Read one header line.
        let Some(nl) = bytes[pos..].iter().position(|&b| b == b'\n') else {
            break;
        };
        let header = String::from_utf8_lossy(&bytes[pos..pos + nl]).to_string();
        pos += nl + 1;
        let fields: Vec<&str> = header.rsplitn(3, ' ').collect();
        // `rsplitn(3)` yields [size, type, oid] for a found object; a `missing`
        // line has no size to parse.
        if fields.len() == 3 {
            if let Ok(size) = fields[0].parse::<usize>() {
                let content = &bytes[pos..pos + size];
                if let Ok(s) = std::str::from_utf8(content) {
                    tree.insert(p.clone(), s.to_string());
                }
                pos += size + 1; // trailing newline after the payload
                continue;
            }
        }
        // `missing` / malformed: nothing consumed past the header, skip the path.
    }
    Ok(tree)
}

pub(crate) fn rev_exists(dir: &Path, rev: &str) -> bool {
    Command::new("git")
        .args(["rev-parse", "--verify", "--quiet", rev])
        .current_dir(dir)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Resolve the merge triple, defaulting to the merge in progress.
///
/// `ours` defaults to `HEAD`; `theirs` defaults to `MERGE_HEAD` (so a bare
/// `weave check` mid-merge does the obvious thing); `base` defaults to the
/// merge base of the two.
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
        None if rev_exists(dir, "MERGE_HEAD") => "MERGE_HEAD".to_string(),
        None => {
            return Err("no --theirs given and no merge in progress (MERGE_HEAD absent)".into())
        }
    };
    let base = match base {
        Some(b) => b.to_string(),
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
    pub scope: String,
}

/// Find the merge this repository is in — or has just finished — and read it.
///
/// Two shapes, in order, because they are the two moments an agent asks:
///
/// * **mid-merge**: `MERGE_HEAD` exists. Ours is `HEAD`, theirs is
///   `MERGE_HEAD`. This is the state right after `git merge` exits 1, and it
///   survives `git add` — which is exactly why the index's unmerged list alone
///   is not enough to find the subjects.
/// * **just committed**: `HEAD` has two parents. Ours is `HEAD^1`, theirs is
///   `HEAD^2`. An agent that committed and then wants to know what it did.
///
/// Neither shape present is not an error and must not be reported as one — it
/// is the sentence "there is no merge here", which the caller prints.
pub fn merge_scope(dir: &Path) -> R<Option<MergeScope>> {
    if !rev_exists(dir, "HEAD") {
        return Ok(None);
    }
    let (ours_rev, theirs_rev, moment) = if rev_exists(dir, "MERGE_HEAD") {
        ("HEAD".to_string(), "MERGE_HEAD".to_string(), "in progress")
    } else if rev_exists(dir, "HEAD^2") {
        ("HEAD^1".to_string(), "HEAD^2".to_string(), "just committed")
    } else {
        return Ok(None);
    };
    let base_rev = git(dir, &["merge-base", &ours_rev, &theirs_rev])?
        .trim()
        .to_string();

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
    let mut subjects: BTreeSet<String> = BTreeSet::new();
    for side in [&ours_rev, &theirs_rev] {
        let list = git(dir, &["diff", "--name-only", "-z", &base_rev, side])?;
        subjects.extend(
            list.split('\0')
                .filter(|p| !p.is_empty() && is_supported(p))
                .map(str::to_string),
        );
    }
    // …plus whatever git still calls unmerged, whether or not both sides moved
    // it: git's own verdict about what is unresolved outranks ours.
    if let Ok(list) = git(dir, &["diff", "--name-only", "--diff-filter=U", "-z"]) {
        subjects.extend(
            list.split('\0')
                .filter(|p| !p.is_empty() && is_supported(p))
                .map(str::to_string),
        );
    }
    let subjects: Vec<String> = subjects.into_iter().collect();

    // Read the three merge stages for the SUBJECTS ONLY — one `cat-file --batch`
    // process per rev, not a `git show` per file over the whole tree. The
    // dangling pass proves this is exact: a name is only ever "gone" from a file
    // both a stage and the working tree disagree about, which is a subject; an
    // untouched file's stage and its working-tree copy are identical, so it can
    // neither create a dangling finding nor host the definition that resolves
    // one from stage data. Suppression by an untouched file's *surviving*
    // definition is the working tree's job, and `work` below stays repo-wide for
    // exactly that.
    let base = read_paths_at_rev(dir, &base_rev, &subjects)?;
    let ours = read_paths_at_rev(dir, &ours_rev, &subjects)?;
    let theirs = read_paths_at_rev(dir, &theirs_rev, &subjects)?;
    let work = read_worktree(dir)?;
    let scope = format!(
        "working tree vs the three merge stages of {ours_rev} × {theirs_rev} \
         (base {}, merge {moment}) — {} file(s) either side changed",
        &base_rev[..base_rev.len().min(8)],
        subjects.len()
    );
    Ok(Some(MergeScope {
        base,
        ours,
        theirs,
        work,
        subjects,
        scope,
    }))
}

/// Every tracked, supported file as it exists on disk right now.
pub(crate) fn read_worktree(dir: &Path) -> R<Tree> {
    let listing = git(dir, &["ls-files", "-z"])?;
    let mut tree = Tree::new();
    for rel in listing.split('\0').filter(|p| !p.is_empty()) {
        if !is_supported(rel) {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(dir.join(rel)) {
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
    let stage = |n: u8| -> Option<String> {
        let out = Command::new("git")
            .args(["show", &format!(":{n}:{path}")])
            .current_dir(dir)
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).to_string())
    };
    if let (Some(b), Some(o), Some(t)) = (stage(1), stage(2), stage(3)) {
        return Ok((b, o, t));
    }
    let scope = merge_scope(dir)?.ok_or(
        "no merge in progress and HEAD is not a merge commit — there is no three-way \
                context to explain this file against",
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
    let listing = git(dir, &["ls-tree", "-r", "--name-only", "-z", rev])?;
    let mut tree = Tree::new();
    for path in listing.split('\0').filter(|p| !p.is_empty()) {
        if !is_supported(path) {
            continue;
        }
        let out = Command::new("git")
            .args(["show", &format!("{rev}:{path}")])
            .current_dir(dir)
            .output()?;
        if !out.status.success() {
            continue;
        }
        if let Ok(content) = String::from_utf8(out.stdout) {
            tree.insert(path.to_string(), content);
        }
    }
    Ok(tree)
}
