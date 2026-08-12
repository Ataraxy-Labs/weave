//! The `git` queries weave asks, and the four ways one can fail.
//!
//! Every function here shells out to `git`, and every one of them used to
//! answer failure with `Box<dyn Error>` built from a `format!` string — so
//! "git is not installed", "this directory is not a repository", "the two
//! branches share no history" and "git printed something we could not read"
//! arrived at the caller as the same type, distinguishable only by matching on
//! prose. [`GitError`] names them instead: a caller that wants to fall back
//! when a repository is missing, but report when the binary is missing, can
//! now write that.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Why a `git` query did not produce an answer.
///
/// Four variants, one per thing that can actually go wrong, each carrying the
/// operands the caller would otherwise have to re-derive from a message: the
/// argv that was run, the directory it was asked about, the two refs that
/// share no history.
#[derive(Debug, thiserror::Error)]
pub enum GitError {
    /// `git` itself could not be started — not installed, not on `PATH`, or
    /// the process could not be spawned. Distinct from [`GitError::Refused`]
    /// because no repository state can fix it.
    #[error("could not run `git {argv}`: {source}")]
    NotRunnable {
        argv: String,
        #[source]
        source: std::io::Error,
    },

    /// The directory is not inside a git repository. Carries the directory
    /// asked about, which for the CWD-relative query is the one operand the
    /// caller did not supply.
    #[error("not inside a git repository: {dir}")]
    NotARepository { dir: String },

    /// Two refs with no common ancestor. Both operands travel with it, so a
    /// caller can say which pair it was without re-reading its own arguments.
    #[error("no merge base between '{head}' and '{branch}'")]
    NoMergeBase { head: String, branch: String },

    /// `git` ran and declined. The exit status and whatever it put on stderr
    /// are the whole of what it told us, so both travel.
    #[error("`git {argv}` exited with {status}: {stderr}")]
    Refused {
        argv: String,
        status: i32,
        stderr: String,
    },
}

/// Run `git -C <dir> <args>` and hand back its output, or say which way it
/// failed.
///
/// The directory is an explicit parameter, not the process working directory:
/// a query reads the repository it was handed and no other. Without it, every
/// `git` here would act on whichever repository the process happens to stand
/// in — a repository the caller never named, so a caller that discovered
/// repository R could be answered from repository D.
fn run(dir: &Path, args: &[&str]) -> Result<std::process::Output, GitError> {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|source| GitError::NotRunnable {
            argv: format!("-C {} {}", dir.display(), args.join(" ")),
            source,
        })
}

/// The stderr `git` produced, trimmed, for a run that failed.
fn refusal(args: &[&str], output: &std::process::Output) -> GitError {
    GitError::Refused {
        argv: args.join(" "),
        status: output.status.code().unwrap_or(-1),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
    }
}

/// Find the root of the git repository that contains `dir`.
///
/// Pass the directory to discover from (`Path::new(".")` for the working
/// directory). The repository is named, not ambient — see [`run`].
pub fn find_repo_root(dir: &Path) -> Result<PathBuf, GitError> {
    let args = ["rev-parse", "--show-toplevel"];
    let output = run(dir, &args)?;
    if !output.status.success() {
        return Err(GitError::NotARepository {
            dir: dir.display().to_string(),
        });
    }
    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(PathBuf::from(root))
}

/// Find the root of the git repository that contains the given path.
/// Uses `git -C <dir> rev-parse --show-toplevel` so it works regardless of CWD.
pub fn find_repo_root_from_path(path: &Path) -> Result<PathBuf, GitError> {
    let dir = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent()
            .map(|p| {
                if p.as_os_str().is_empty() {
                    Path::new(".")
                } else {
                    p
                }
            })
            .unwrap_or(Path::new("."))
            .to_path_buf()
    };
    let args = ["rev-parse", "--show-toplevel"];
    let output = run(&dir, &args)?;
    if !output.status.success() {
        return Err(GitError::NotARepository {
            dir: dir.display().to_string(),
        });
    }
    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(PathBuf::from(root))
}

/// Find the merge base between two refs, in the named repository.
pub fn find_merge_base(repo_root: &Path, head: &str, branch: &str) -> Result<String, GitError> {
    let args = ["merge-base", head, branch];
    let output = run(repo_root, &args)?;
    if !output.status.success() {
        return Err(GitError::NoMergeBase {
            head: head.to_string(),
            branch: branch.to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Show file content at a given revision, in the named repository.
pub fn git_show(repo_root: &Path, rev: &str, file: &str) -> Result<String, GitError> {
    let spec = format!("{}:{}", rev, file);
    let args = ["show", &spec];
    let output = run(repo_root, &args)?;
    if !output.status.success() {
        return Err(refusal(&args, &output));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Show file content at a revision, telling "absent at this revision" apart
/// from "git could not answer".
///
/// `Ok(None)` is a *fact*: the revision resolves and the path is simply not in
/// its tree — which is exactly the base content of a file one side added, where
/// `""` is the correct operand for a three-way merge. `Err` is the honest
/// answer to a bad rev (a typo, an unfetched or pruned ref) or a damaged object
/// that cannot be inflated. Collapsing the two into a fabricated `""` — what
/// `git_show(...).unwrap_or_default()` did — funds a merge with a value weave
/// never read — fail-stop instead of fabricating one.
///
/// The tree is consulted with `ls-tree`, which reads the tree object rather
/// than the blob, so a present-but-corrupt object still lists here and is
/// caught by the `show` below instead of being mistaken for an absent file. No
/// stderr prose is matched: the rev's validity and the path's presence are each
/// asked of git directly.
pub fn git_show_optional(
    repo_root: &Path,
    rev: &str,
    file: &str,
) -> Result<Option<String>, GitError> {
    let ls_args = ["ls-tree", rev, "--", file];
    let ls = run(repo_root, &ls_args)?;
    if !ls.status.success() {
        // git declined to resolve the revision at all: a bad ref, not an
        // absent file.
        return Err(refusal(&ls_args, &ls));
    }
    if String::from_utf8_lossy(&ls.stdout).trim().is_empty() {
        // The revision is good and the path is genuinely not in it.
        return Ok(None);
    }
    // Present in the tree — the content must be readable, or the object is
    // damaged and no merge may be computed against it.
    Ok(Some(git_show(repo_root, rev, file)?))
}

/// Get files changed in both branches relative to their merge base.
pub fn get_changed_files(
    repo_root: &Path,
    merge_base: &str,
    head: &str,
    branch: &str,
) -> Result<Vec<String>, GitError> {
    let ours_args = ["diff", "--name-only", merge_base, head];
    let ours_output = run(repo_root, &ours_args)?;
    if !ours_output.status.success() {
        return Err(refusal(&ours_args, &ours_output));
    }
    let ours_files: HashSet<String> = String::from_utf8_lossy(&ours_output.stdout)
        .lines()
        .map(|s| s.to_string())
        .collect();

    let theirs_args = ["diff", "--name-only", merge_base, branch];
    let theirs_output = run(repo_root, &theirs_args)?;
    if !theirs_output.status.success() {
        return Err(refusal(&theirs_args, &theirs_output));
    }
    let theirs_files: HashSet<String> = String::from_utf8_lossy(&theirs_output.stdout)
        .lines()
        .map(|s| s.to_string())
        .collect();

    let mut both: Vec<String> = ours_files.intersection(&theirs_files).cloned().collect();
    both.sort();
    Ok(both)
}

/// Get files changed between two refs.
pub fn diff_files(
    repo_root: &Path,
    base_ref: &str,
    target_ref: &str,
) -> Result<Vec<String>, GitError> {
    let args = ["diff", "--name-only", base_ref, target_ref];
    let output = run(repo_root, &args)?;
    if !output.status.success() {
        return Err(refusal(&args, &output));
    }
    let files: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_repository_is_its_own_variant_not_a_message() {
        let outside = std::env::temp_dir();
        match find_repo_root_from_path(&outside) {
            Err(GitError::NotARepository { dir }) => {
                assert!(dir.contains(&outside.display().to_string()));
            }
            // A temp dir inside a repository is unusual but not impossible;
            // the point of the test is that failure is matchable, not that
            // this particular machine fails.
            Ok(_) => {}
            Err(other) => panic!("expected NotARepository, got {other:?}"),
        }
    }

    #[test]
    fn a_refusal_carries_gits_own_status_and_argv() {
        match git_show(Path::new("."), "weave-no-such-rev", "no/such/file") {
            Err(GitError::Refused {
                argv,
                status,
                stderr,
            }) => {
                assert!(argv.starts_with("show weave-no-such-rev:"));
                assert_ne!(status, 0);
                assert!(!stderr.is_empty());
            }
            // No git on this machine is the other legal answer, and it is a
            // different variant precisely so a caller can tell them apart.
            Err(GitError::NotRunnable { .. }) => {}
            other => panic!("expected Refused, got {other:?}"),
        }
    }

    #[test]
    fn two_unrelated_refs_report_both_operands() {
        match find_merge_base(Path::new("."), "weave-no-such-ref-a", "weave-no-such-ref-b") {
            Err(GitError::NoMergeBase { head, branch }) => {
                assert_eq!(head, "weave-no-such-ref-a");
                assert_eq!(branch, "weave-no-such-ref-b");
            }
            Err(GitError::NotRunnable { .. }) => {}
            other => panic!("expected NoMergeBase, got {other:?}"),
        }
    }
}
