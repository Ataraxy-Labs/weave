//! The `git` queries weave asks, and the ways one can fail.
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

    /// `git` answered fine, but a filesystem operation this module needed on
    /// top of that answer (writing the local exclude file) failed. Distinct
    /// from [`GitError::NotRunnable`] because the process that failed here is
    /// our own I/O, not git's.
    #[error("could not update {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
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

/// Make sure `entry` (a `.gitignore`-style pattern) is listed in this
/// repository's *local* exclude file, without touching anything the
/// repository itself tracks.
///
/// Weave writes its own coordination state (`.weave/`) into a repo's working
/// tree, and that state must never show up in `git status`, get swept into
/// `git add -A`, or ride along in a generated patch — it is local machine
/// state, not repository content. `.git/info/exclude` is the git-native place
/// for exactly that: it behaves like a `.gitignore` but lives inside `.git/`,
/// so it is never committed, never pushed, and never collides with a
/// `.gitignore` the repository's own maintainers write and version.
///
/// Resolved via `git rev-parse --git-path info/exclude` rather than assuming
/// `<repo_root>/.git/info/exclude`, so this also lands in the right place for
/// a linked worktree (whose private git dir is elsewhere) or a submodule
/// (whose `.git` is a file, not a directory) — anywhere git itself would
/// consider "the local exclude file for this working tree".
///
/// Idempotent: a second call with the same `entry` is a no-op, so this is
/// safe to call on every save rather than only on first creation — which
/// also means a repository whose `.weave/` directory predates this fix gets
/// it retroactively on its very next save.
///
/// A best-effort courtesy, not a requirement: `repo_root` not being inside a
/// git repository, or `git` not being runnable at all, comes back `Ok(())` —
/// silently doing nothing is correct there, since there is no repository
/// tracking to protect against. Only an I/O failure on a `.git` this call
/// *did* manage to identify is reported, and even that is expected to be
/// swallowed by callers for whom this is advisory (see
/// `weave_crdt::EntityStateDoc::save`, which must never fail to write state
/// just because the exclude file couldn't be touched).
pub fn ensure_locally_excluded(repo_root: &Path, entry: &str) -> Result<(), GitError> {
    let args = ["rev-parse", "--git-path", "info/exclude"];
    let output = run(repo_root, &args)?;
    if !output.status.success() {
        // Not inside a git repository (or some other refusal) — nothing to
        // exclude from, and that's fine.
        return Ok(());
    }
    let printed = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if printed.is_empty() {
        return Ok(());
    }
    let exclude_path = {
        let p = PathBuf::from(&printed);
        if p.is_absolute() {
            p
        } else {
            repo_root.join(p)
        }
    };

    let io_err = |source: std::io::Error| GitError::Io {
        path: exclude_path.display().to_string(),
        source,
    };

    if let Some(parent) = exclude_path.parent() {
        std::fs::create_dir_all(parent).map_err(io_err)?;
    }

    let existing = std::fs::read_to_string(&exclude_path).unwrap_or_default();
    if existing.lines().any(|line| line.trim() == entry) {
        return Ok(()); // already excluded
    }

    let mut updated = existing;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(entry);
    updated.push('\n');
    std::fs::write(&exclude_path, updated).map_err(io_err)?;
    Ok(())
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

    fn init_repo(dir: &Path) {
        for args in [
            vec!["init", "-q"],
            vec!["config", "user.email", "test@example.com"],
            vec!["config", "user.name", "Test"],
        ] {
            let status = Command::new("git")
                .arg("-C")
                .arg(dir)
                .args(&args)
                .status()
                .expect("git must be runnable for this test");
            assert!(status.success(), "git {args:?} failed");
        }
    }

    #[test]
    fn ensure_locally_excluded_writes_the_entry_to_info_exclude_not_gitignore() {
        let dir = tempfile::tempdir().expect("tempdir");
        init_repo(dir.path());

        ensure_locally_excluded(dir.path(), ".weave/").expect("ensure_locally_excluded");

        let exclude = std::fs::read_to_string(dir.path().join(".git/info/exclude"))
            .expect("info/exclude should exist");
        assert!(exclude.lines().any(|l| l == ".weave/"));
        assert!(
            !dir.path().join(".gitignore").exists(),
            "must never create or touch the repository's own .gitignore"
        );
    }

    #[test]
    fn ensure_locally_excluded_is_idempotent() {
        let dir = tempfile::tempdir().expect("tempdir");
        init_repo(dir.path());

        ensure_locally_excluded(dir.path(), ".weave/").expect("first call");
        ensure_locally_excluded(dir.path(), ".weave/").expect("second call");
        ensure_locally_excluded(dir.path(), ".weave/").expect("third call");

        let exclude = std::fs::read_to_string(dir.path().join(".git/info/exclude"))
            .expect("info/exclude should exist");
        let hits = exclude.lines().filter(|l| *l == ".weave/").count();
        assert_eq!(hits, 1, "repeated calls must not duplicate the entry");
    }

    #[test]
    fn ensure_locally_excluded_preserves_an_existing_exclude_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        init_repo(dir.path());
        let exclude_path = dir.path().join(".git/info/exclude");
        std::fs::write(&exclude_path, "*.local-scratch\n").expect("seed existing exclude");

        ensure_locally_excluded(dir.path(), ".weave/").expect("ensure_locally_excluded");

        let exclude = std::fs::read_to_string(&exclude_path).expect("read exclude");
        assert!(exclude.lines().any(|l| l == "*.local-scratch"));
        assert!(exclude.lines().any(|l| l == ".weave/"));
    }

    #[test]
    fn ensure_locally_excluded_is_a_silent_no_op_outside_a_repository() {
        let dir = tempfile::tempdir().expect("tempdir (not a git repo)");

        let result = ensure_locally_excluded(dir.path(), ".weave/");

        assert!(result.is_ok(), "must not fail just because there's no repo");
        assert!(
            !dir.path().join(".git").exists(),
            "must not create a .git directory as a side effect"
        );
    }
}
