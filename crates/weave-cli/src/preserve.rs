//! The publish-time invariant: a merge keeps each side's changes.
//!
//! The gate ([`crate::land`]) judges the files BOTH sides changed. A file only
//! one side changed has nothing to resolve, so the gate never reads it — and
//! a merge whose tree is not git's merge (a half-applied merge, a merge git
//! could not write to the index, a hand-made tree) can silently put such a
//! file back to the merge base. This module checks the tree itself, however
//! it was produced, over EVERY path in `base→ours` ∪ `base→theirs`:
//!
//! * a path only **theirs** changed must hold theirs' version, or a version
//!   that still contains theirs' change (every token theirs added is there,
//!   no token theirs deleted is back; the gate's DROPPED / UNDELETED measure,
//!   the whole file read as one block). A deletion by theirs must stay
//!   deleted; a file theirs changed must not be deleted;
//! * a path only **ours** changed may be edited further by the merge (the
//!   merge commit is ours' own work), but must not go back to base: not
//!   restored to the base version, not deleted when ours changed it, not
//!   brought back when ours deleted it.
//!
//! Paths both sides changed are the gate's. Any violation refuses, the file
//! named.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use serde::Serialize;

use crate::gitscan;

type R<T> = Result<T, Box<dyn std::error::Error>>;

/// A path whose change by one side the merge does not keep.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Violation {
    pub path: String,
    /// `"theirs"` or `"ours"`: whose change is lost.
    pub side: &'static str,
    pub detail: String,
}

/// `(mode, oid)` of a path after a change; `None` = deleted.
type Entry = Option<(String, String)>;

/// Every path `git diff --raw from to` reports, with its entry in `to`.
fn changes(dir: &Path, from: &str, to: &str) -> R<BTreeMap<String, Entry>> {
    let raw = gitscan::git(
        dir,
        &[
            "diff",
            "--raw",
            "-z",
            "--no-renames",
            "--no-abbrev",
            "--no-ext-diff",
            "--no-textconv",
            from,
            to,
        ],
    )?;
    let mut out = BTreeMap::new();
    let mut parts = raw.split('\0').filter(|p| !p.is_empty());
    while let Some(meta) = parts.next() {
        let Some(path) = parts.next() else { break };
        // :<old mode> <new mode> <old oid> <new oid> <status>
        let f: Vec<&str> = meta.trim_start_matches(':').split(' ').collect();
        if f.len() < 5 {
            return Err(format!("could not read `git diff --raw` line `{meta}`").into());
        }
        let entry = if f[4].starts_with('D') {
            None
        } else {
            Some((f[1].to_string(), f[3].to_string()))
        };
        out.insert(path.to_string(), entry);
    }
    Ok(out)
}

/// `git cat-file <args>` with weave's deadline: `(exit ok, stdout bytes)`.
fn cat_file(dir: &Path, args: &[&str]) -> (bool, Vec<u8>) {
    let mut cmd = std::process::Command::new("git");
    cmd.arg("cat-file").args(args).current_dir(dir);
    match gitscan::run_bounded(
        cmd,
        "git cat-file",
        None,
        std::time::Duration::from_secs(120),
    ) {
        Ok((status, out, _)) => (status.success(), out),
        Err(_) => (false, Vec::new()),
    }
}

/// The path's text at `rev`; `None` when it is absent or not UTF-8.
fn blob(dir: &Path, rev: &str, path: &str) -> Option<String> {
    match cat_file(dir, &["blob", &format!("{rev}:{path}")]) {
        (true, bytes) => String::from_utf8(bytes).ok(),
        _ => None,
    }
}

/// Check the merge `result` (any tree-ish) of `ours` and `theirs` over
/// `base`. Empty = every one-sided change is kept.
pub fn check(dir: &Path, base: &str, ours: &str, theirs: &str, result: &str) -> R<Vec<Violation>> {
    let o = changes(dir, base, ours)?;
    let t = changes(dir, base, theirs)?;
    let r = changes(dir, base, result)?;
    let mut out = Vec::new();

    for (path, te) in t.iter().filter(|(p, _)| !o.contains_key(*p)) {
        // `None` here: the result has the path exactly as base has it.
        let re = r.get(path);
        if re == Some(te) {
            continue;
        }
        let detail = match (te, re) {
            (None, _) => Some("theirs deleted the file; the merge still has it".to_string()),
            (Some(_), None) => Some(
                "the merge has the file exactly as the merge base has it: theirs' change to it \
                 is gone"
                    .to_string(),
            ),
            (Some(_), Some(None)) => {
                Some("the merge deletes the file, which theirs changed".to_string())
            }
            (Some(_), Some(Some(_))) => {
                let b = blob(dir, base, path).or_else(|| {
                    // absent at base: theirs added it
                    (!rev_has(dir, base, path)).then(String::new)
                });
                match (b, blob(dir, theirs, path), blob(dir, result, path)) {
                    (Some(b), Some(s), Some(m)) => contains_change(&b, &s, &m),
                    _ => Some(
                        "the merge's version differs from theirs and is not text weave can \
                         compare"
                            .to_string(),
                    ),
                }
            }
        };
        if let Some(detail) = detail {
            out.push(Violation {
                path: path.clone(),
                side: "theirs",
                detail,
            });
        }
    }

    for (path, oe) in o.iter().filter(|(p, _)| !t.contains_key(*p)) {
        let re = r.get(path);
        if re == Some(oe) {
            continue;
        }
        let detail = match (oe, re) {
            (None, None) => Some(
                "ours deleted the file; the merge has it back, as the merge base has it"
                    .to_string(),
            ),
            (Some(_), None) => Some(
                "the merge has the file exactly as the merge base has it: ours' change to it is \
                 gone"
                    .to_string(),
            ),
            (Some(_), Some(None)) => {
                Some("the merge deletes the file, which ours changed".to_string())
            }
            _ => None, // the merge's own further edit of ours' file
        };
        if let Some(detail) = detail {
            out.push(Violation {
                path: path.clone(),
                side: "ours",
                detail,
            });
        }
    }
    Ok(out)
}

fn rev_has(dir: &Path, rev: &str, path: &str) -> bool {
    cat_file(dir, &["-e", &format!("{rev}:{path}")]).0
}

/// The tree the index would commit, read without touching the index or its
/// lock (a scratch copy of the index).
pub fn index_tree(dir: &Path, gd: &Path) -> R<String> {
    let scratch = gd.join(format!("weave-land-index-{}", std::process::id()));
    std::fs::copy(gd.join("index"), &scratch)?;
    let mut cmd = std::process::Command::new("git");
    cmd.args(["write-tree"])
        .current_dir(dir)
        .env("GIT_INDEX_FILE", &scratch);
    let run = gitscan::run_bounded(
        cmd,
        "git write-tree",
        None,
        std::time::Duration::from_secs(120),
    );
    let _ = std::fs::remove_file(&scratch);
    let (status, stdout, stderr) = run?;
    if !status.success() {
        return Err(format!(
            "git write-tree failed: {}",
            String::from_utf8_lossy(&stderr).trim()
        )
        .into());
    }
    Ok(String::from_utf8_lossy(&stdout).trim().to_string())
}

/// The tree the merge in progress in `dir` would commit, once nothing is
/// left unmerged; `None` when no merge is in progress or a path is still
/// unmerged.
pub fn merge_in_tree(dir: &Path) -> R<Option<String>> {
    let gd =
        std::path::PathBuf::from(gitscan::git(dir, &["rev-parse", "--absolute-git-dir"])?.trim());
    if !gd.join("MERGE_HEAD").exists()
        || !gitscan::git(dir, &["diff", "--name-only", "--diff-filter=U"])?
            .trim()
            .is_empty()
    {
        return Ok(None);
    }
    Ok(Some(index_tree(dir, &gd)?))
}

/// [`check`], keeping only the violations that hold at `now` too (the
/// branch as it is now: a later commit that put the change back counts).
pub fn check_now(
    dir: &Path,
    base: &str,
    ours: &str,
    theirs: &str,
    result: &str,
    now: &str,
) -> R<Vec<Violation>> {
    let at_merge = check(dir, base, ours, theirs, result)?;
    if at_merge.is_empty() {
        return Ok(at_merge);
    }
    let at_now: BTreeSet<(String, &'static str)> = check(dir, base, ours, theirs, now)?
        .into_iter()
        .map(|v| (v.path, v.side))
        .collect();
    Ok(at_merge
        .into_iter()
        .filter(|v| at_now.contains(&(v.path.clone(), v.side)))
        .collect())
}

/// Whether `merged` still holds the change `base → side`, in tokens: every
/// token the side added is there at least as often as the side has it, and
/// no token the side deleted is there more often than the side has it.
/// `None` = held; `Some(why)` = not.
pub fn contains_change(base: &str, side: &str, merged: &str) -> Option<String> {
    let (b, s, m) = (counts(base), counts(side), counts(merged));
    let get = |c: &HashMap<&str, usize>, k: &str| c.get(k).copied().unwrap_or(0);
    let mut missing: Vec<&str> = Vec::new();
    let mut back: Vec<&str> = Vec::new();
    for (k, &n) in &s {
        if n > get(&b, k) && get(&m, k) < n {
            missing.push(k);
        }
    }
    for (k, &n) in &b {
        let ns = get(&s, k);
        if ns < n && get(&m, k) > ns {
            back.push(k);
        }
    }
    if missing.is_empty() && back.is_empty() {
        return None;
    }
    let order = |v: &mut Vec<&str>| {
        v.sort_by_key(|k| {
            let word = k
                .chars()
                .next()
                .is_some_and(|c| c.is_alphanumeric() || c == '_');
            (!word, std::cmp::Reverse(k.len()), k.to_string())
        })
    };
    order(&mut missing);
    order(&mut back);
    let names = |v: &[&str]| {
        v.iter()
            .take(5)
            .map(|k| format!("`{k}`"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut parts = Vec::new();
    if !missing.is_empty() {
        parts.push(format!(
            "{} token(s) theirs added are missing, e.g. {}",
            missing.len(),
            names(&missing)
        ));
    }
    if !back.is_empty() {
        parts.push(format!(
            "{} token(s) theirs deleted are back, e.g. {}",
            back.len(),
            names(&back)
        ));
    }
    Some(format!(
        "the merge's version does not contain theirs' change: {}",
        parts.join("; ")
    ))
}

/// A change's tokens: word runs (letters, digits, `_`) and operator runs, as
/// the gate's DROPPED rule reads them.
fn tokens(line: &str) -> Vec<&str> {
    const OPS: &str = "+-*/%=<>!&|^~@?";
    let class = |c: char| {
        if c.is_alphanumeric() || c == '_' {
            1
        } else if OPS.contains(c) {
            2
        } else {
            0
        }
    };
    let mut out = Vec::new();
    let mut start: Option<(usize, u8)> = None;
    for (i, c) in line
        .char_indices()
        .chain(std::iter::once((line.len(), ' ')))
    {
        let k = class(c);
        match start {
            Some((s, sk)) if sk != k => {
                out.push(&line[s..i]);
                start = (k != 0).then_some((i, k));
            }
            None if k != 0 => start = Some((i, k)),
            _ => {}
        }
    }
    out
}

fn counts(text: &str) -> HashMap<&str, usize> {
    let mut m = HashMap::new();
    for l in text.lines() {
        for t in tokens(l) {
            *m.entry(t).or_insert(0) += 1;
        }
    }
    m
}

/// The violations as lines for a person: `  path  (side's change lost: …)`.
pub fn render(v: &[Violation], ours: &str, theirs: &str) -> String {
    let width = v.iter().map(|x| x.path.len()).max().unwrap_or(0);
    v.iter()
        .map(|x| {
            let who = if x.side == "theirs" { theirs } else { ours };
            format!(
                "  {:<width$}  (changed only by {who}; that change is lost: {})",
                x.path, x.detail
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_revert_to_base_does_not_contain_the_change() {
        let base = "package p\n\nfunc A() {}\n";
        let theirs = "package p\n\nfunc A() {}\n\nfunc Shift(x int) int { return x << 1 }\n";
        assert!(contains_change(base, theirs, theirs).is_none());
        let why = contains_change(base, theirs, base).expect("a revert loses the change");
        assert!(why.contains("`Shift`"), "{why}");
        // the change plus an unrelated edit of the merge's own still holds it
        let plus = theirs.replace("func A() {}", "func A() { _ = 1 }");
        assert!(contains_change(base, theirs, &plus).is_none());
        // a deletion by theirs coming back
        assert!(contains_change(theirs, base, theirs).is_some());
    }
}
