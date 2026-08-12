//! A public, product-level property: aborting a conflicted merge must give
//! the user back exactly what they had before.
//!
//! This exercises weave the way a user actually meets it — a real git
//! repository, a real branch, `.gitattributes` routing to the real
//! `weave-driver` binary, and a real `git merge` / `git merge --abort`. No
//! internal type or function of weave-core is touched.

use std::fs;
use std::path::Path;
use std::process::Command;

fn git(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git")
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .args(args)
        .output()
        .expect("failed to run git")
}

fn git_ok(dir: &Path, args: &[&str]) {
    let out = git(dir, args);
    assert!(
        out.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn aborting_a_conflicted_merge_restores_the_pre_merge_state() {
    // Whatever weave writes during a conflicted merge, `git merge --abort`
    // must return the working tree to the pre-merge state exactly. The
    // driver only ever sees temporary files; git owns recovery.
    let base = "def f():\n    return 0\n";
    let ours = "def f():\n    return 1\n";
    let theirs = "def f():\n    return 2\n";

    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    let driver_bin = env!("CARGO_BIN_EXE_weave-driver");

    git_ok(dir, &["init", "-q", "-b", "main"]);
    git_ok(dir, &["config", "user.email", "test@example.com"]);
    git_ok(dir, &["config", "user.name", "Test"]);
    git_ok(dir, &["config", "merge.weave.name", "weave entity merge"]);
    git_ok(
        dir,
        &[
            "config",
            "merge.weave.driver",
            &format!("\"{}\" %O %A %B %L %P", driver_bin),
        ],
    );
    fs::write(dir.join(".gitattributes"), "*.py merge=weave\n").unwrap();
    fs::write(dir.join("app.py"), base).unwrap();
    git_ok(dir, &["add", "."]);
    git_ok(dir, &["commit", "-q", "-m", "base"]);
    git_ok(dir, &["checkout", "-q", "-b", "incoming"]);
    fs::write(dir.join("app.py"), theirs).unwrap();
    git_ok(dir, &["commit", "-q", "-am", "theirs"]);
    git_ok(dir, &["checkout", "-q", "main"]);
    fs::write(dir.join("app.py"), ours).unwrap();
    git_ok(dir, &["commit", "-q", "-am", "ours"]);

    let merge = git(dir, &["merge", "--no-edit", "incoming"]);
    assert!(!merge.status.success(), "this merge must conflict");
    let conflicted = fs::read_to_string(dir.join("app.py")).unwrap();
    assert!(
        conflicted.contains("<<<<<<<"),
        "markers visible mid-conflict"
    );

    git_ok(dir, &["merge", "--abort"]);
    let restored = fs::read_to_string(dir.join("app.py")).unwrap();
    assert_eq!(
        restored, ours,
        "after `git merge --abort` the user must be exactly where they \
         started, regardless of what the driver wrote"
    );
}
