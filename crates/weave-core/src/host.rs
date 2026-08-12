//! What a merge is allowed to reach outside itself, and who hands it over.
//!
//! A merge is a function of three strings. Two things in this crate were not:
//! it read `WEAVE_MAX_DUPLICATES` out of the environment in the middle of
//! deciding whether a file's identities were usable, and it spawned
//! `git merge-file` — with three temporary files to feed it — whenever the
//! line-level route was taken. Both are real and both earn their keep (see
//! [`git_line_merge`]), but a library that reaches for them itself cannot be
//! asked not to: a caller had no way to say "merge these three strings and
//! touch nothing".
//!
//! So they arrive as a value. [`Host`] is built once, at the program's entry
//! point, and passed down; [`Host::default`] grants nothing, which is what
//! `entity_merge` now runs with. `weave-driver`, `weave` and `weave-mcp` each
//! build one that grants the git route, so what they produce is unchanged.
//! Nothing below the entry point can widen it — a merge can only invoke what
//! it was handed.

use std::io::Write;
use std::process::Command;

/// Which spelling of a line-level merge the caller is being asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineMergeStyle {
    /// Plain: content only, and only when it merges cleanly. Used as a second
    /// opinion after an in-process line merge has already refused.
    Plain,
    /// Labelled: `--diff3` sections labelled `ours`/`base`/`theirs`, whether
    /// the merge was clean or not. Used as the whole answer for a file the
    /// entity model does not describe, where the conflicted bytes themselves
    /// are the product.
    Labelled,
}

/// The answer a line-level merge gave.
#[derive(Debug, Clone)]
pub struct LineMerged {
    /// The merged text — with conflict markers when `clean` is false.
    pub content: String,
    /// Whether it resolved without conflict.
    pub clean: bool,
}

/// A line-level three-way merge the caller has granted.
///
/// `None` means the merge could not be performed at all (the tool is missing,
/// the scratch files could not be written); the caller falls back to the
/// in-process route, exactly as it did when `tempfile` failed before.
pub type LineMerge = fn(LineMergeStyle, &str, &str, &str) -> Option<LineMerged>;

/// The outside world a merge may touch, granted by whoever starts the program.
///
/// Both fields are public and there is no constructor: building one is the
/// whole act of granting, and it should read as plainly at the entry point as
/// it does here.
#[derive(Debug, Clone, Copy)]
pub struct Host {
    /// How often one name may repeat before identity stops meaning anything
    /// in a file, and the entity route is abandoned for a line-level one.
    ///
    /// Read from `WEAVE_MAX_DUPLICATES` by the binaries that support it;
    /// 10 otherwise.
    pub max_duplicates: usize,

    /// The line-level merge this run may use, if the caller granted one.
    pub line_merge: Option<LineMerge>,
}

impl Default for Host {
    /// Grants nothing: no subprocess, no temporary files, no environment.
    /// A merge run against this host is a function of its three inputs.
    fn default() -> Self {
        Host {
            max_duplicates: 10,
            line_merge: None,
        }
    }
}

/// `git merge-file`, passed in explicitly rather than reached for globally.
///
/// This is a genuinely different answer, not a slower spelling of the
/// in-process line merge. The two algorithms disagree often enough, on both
/// the clean/conflict verdict and on the exact bytes of merges they both call
/// clean, that reaching for git after the in-process route refuses is a real
/// second opinion. And for the conflicted case it cannot be replaced at all:
/// git writes `--diff3` sections labelled `ours`/`base`/`theirs`, and no Rust
/// implementation reproduces those bytes (`diffy` has no label support; even
/// `gix-merge`, a port of git's own xdl_merge, is not a drop-in replacement
/// for `git merge-file` itself).
///
/// It is expensive — ~3.2ms of `git` process, irreducible (measured:
/// suppressing git's config discovery does not move it) — which is why it sits
/// behind the in-process route rather than in front of it, and why a caller
/// that would rather not pay it can simply not grant it.
pub fn git_line_merge(
    style: LineMergeStyle,
    base: &str,
    ours: &str,
    theirs: &str,
) -> Option<LineMerged> {
    let scratch = scratch_triple(base, ours, theirs)?;

    let mut cmd = Command::new("git");
    cmd.arg("merge-file").arg("-p"); // print to stdout instead of modifying ours in place
    if style == LineMergeStyle::Labelled {
        cmd.arg("--diff3") // include the ||||||| base section, for jj compatibility
            .arg("-L")
            .arg("ours")
            .arg("-L")
            .arg("base")
            .arg("-L")
            .arg("theirs");
    }
    let output = cmd
        .arg(scratch.ours.path())
        .arg(scratch.base.path())
        .arg(scratch.theirs.path())
        .output()
        .ok()?;

    // Exit 0 = clean; exit >0 = that many conflicts.
    let clean = output.status.success();
    match style {
        // The plain style is only ever consulted for a clean answer, so a
        // conflicted one is the same as no answer.
        LineMergeStyle::Plain if !clean => None,
        LineMergeStyle::Plain => Some(LineMerged {
            content: String::from_utf8(output.stdout).ok()?,
            clean: true,
        }),
        LineMergeStyle::Labelled => Some(LineMerged {
            content: String::from_utf8_lossy(&output.stdout).into_owned(),
            clean,
        }),
    }
}

/// Three temporary files holding `(base, ours, theirs)` for `git merge-file`.
///
/// They are unlinked when this value falls out of scope — unlike a scratch
/// directory hung off a `static`, which would never be dropped.
struct Scratch {
    base: tempfile::NamedTempFile,
    ours: tempfile::NamedTempFile,
    theirs: tempfile::NamedTempFile,
}

/// Materialise the triple, or `None` if the filesystem will not have it —
/// which callers read exactly as they read a missing `git`: use the
/// in-process route.
fn scratch_triple(base: &str, ours: &str, theirs: &str) -> Option<Scratch> {
    fn write_one(content: &str) -> Option<tempfile::NamedTempFile> {
        let mut f = tempfile::NamedTempFile::new().ok()?;
        f.write_all(content.as_bytes()).ok()?;
        f.flush().ok()?;
        Some(f)
    }
    Some(Scratch {
        base: write_one(base)?,
        ours: write_one(ours)?,
        theirs: write_one(theirs)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_host_grants_nothing() {
        let host = Host::default();
        assert!(host.line_merge.is_none());
        assert_eq!(host.max_duplicates, 10);
    }

    #[test]
    fn a_granted_line_merge_answers_and_says_whether_it_was_clean() {
        let base = "a\nb\nc\nd\ne\nf\ng\n";
        let ours = "A\nb\nc\nd\ne\nf\ng\n";
        let theirs = "a\nb\nc\nd\ne\nf\nG\n";
        let Some(merged) = git_line_merge(LineMergeStyle::Labelled, base, ours, theirs) else {
            return; // no git on this machine: the grant is simply unusable
        };
        assert!(merged.clean);
        assert_eq!(merged.content, "A\nb\nc\nd\ne\nf\nG\n");

        let both = "X\nb\nc\nd\ne\nf\ng\n";
        let conflicted = git_line_merge(LineMergeStyle::Labelled, base, ours, both)
            .expect("git answered once already");
        assert!(!conflicted.clean);
        assert!(conflicted.content.contains("<<<<<<< ours"));
        // The plain style has no answer for a conflict, by design.
        assert!(git_line_merge(LineMergeStyle::Plain, base, ours, both).is_none());
    }
}
