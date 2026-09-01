//! RED test for the `.weave/` working-tree leak.
//!
//! A 327-run transcript study found that `.weave/state.automerge` — the
//! binary Automerge doc every `EntityStateDoc::save()` writes under the
//! target repo's `.weave/` directory — was neither tracked nor ignored.
//! `git status` reported it as `?? .weave/`, `git add -A` staged the binary
//! blob, and any patch built from the staged diff carried it along,
//! silently invalidating the patch.
//!
//! This test drives the real product path — `EntityStateDoc::open` +
//! `save()` — against a fresh, real git repository (via the actual `git`
//! binary, not a fake), and asserts the two user-visible symptoms are gone:
//! `git status --porcelain` reports nothing under `.weave/`, and `git add -A`
//! followed by `git diff --cached --name-only` stages nothing under it
//! either.

use std::path::Path;
use std::process::Command;

use weave_crdt::EntityStateDoc;

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("git must be runnable for this test");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
fn weave_state_never_shows_up_in_git_status_or_a_staged_diff() {
    let dir = tempfile::tempdir().expect("tempdir");
    let repo = dir.path();

    git(repo, &["init", "-q"]);
    git(repo, &["config", "user.email", "test@example.com"]);
    git(repo, &["config", "user.name", "Test"]);
    // A repo needs at least one commit for `git status`/`git diff --cached`
    // to have a baseline to compare against — an empty repo would pass this
    // test's assertions trivially.
    std::fs::write(repo.join("README.md"), "hello\n").expect("write readme");
    git(repo, &["add", "README.md"]);
    git(repo, &["commit", "-q", "-m", "initial"]);

    // The exact door every weave-cli command and the weave-mcp server open:
    // `<repo_root>/.weave/state.automerge`.
    let state_path = repo.join(".weave").join("state.automerge");
    let mut state = EntityStateDoc::open(&state_path).expect("open a fresh state doc");
    state.save().expect("save creates .weave/ and writes the doc");

    assert!(
        state_path.exists(),
        "the state file should actually have been written to disk"
    );

    let status = git(repo, &["status", "--porcelain"]);
    assert!(
        !status.lines().any(|l| l.contains(".weave")),
        "`git status --porcelain` must not mention .weave/, got:\n{status}"
    );

    git(repo, &["add", "-A"]);
    let staged = git(repo, &["diff", "--cached", "--name-only"]);
    assert!(
        !staged.lines().any(|l| l.starts_with(".weave/")),
        "`git add -A` must not stage anything under .weave/, got:\n{staged}"
    );
}
