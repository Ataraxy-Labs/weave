//! `weave check` may fail to verify, but it may not fail to answer.
//!
//! A check that hangs is a check whose silence a script reads as approval, so
//! past `--timeout` it says NOTHING WAS VERIFIED and exits 2 — distinct from 1,
//! which means findings. These drive the real binary against a git that never
//! answers.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

fn git(dir: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run git")
}

fn fixture(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "weave-check-never-hangs-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("mkdir");
    root
}

/// A repository stopped mid-merge on a conflicted `m.py`.
fn mid_merge(name: &str) -> PathBuf {
    let root = fixture(name);
    git(&root, &["init", "-q", "-b", "main"]);
    let write = |text: &str| std::fs::write(root.join("m.py"), text).expect("write");
    write("def a():\n    return 1\n");
    git(&root, &["add", "."]);
    git(&root, &["commit", "-qm", "base"]);
    git(&root, &["checkout", "-qb", "theirs"]);
    write("def a():\n    return 2\n");
    git(&root, &["commit", "-qam", "theirs"]);
    git(&root, &["checkout", "-q", "main"]);
    write("def a():\n    return 3\n");
    git(&root, &["commit", "-qam", "ours"]);
    git(&root, &["merge", "theirs"]);
    root
}

/// Run `weave check` with `args` in `dir`, killing it if it outlives `limit`.
fn weave_check(dir: &Path, args: &[&str], limit: Duration) -> (Option<i32>, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_weave"))
        .arg("check")
        .args(args)
        .current_dir(dir)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run weave");
    let started = Instant::now();
    loop {
        if child.try_wait().expect("wait").is_some() {
            break;
        }
        if started.elapsed() > limit {
            let _ = child.kill();
            panic!("weave check was still running after {limit:?}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let out = child.wait_with_output().expect("output");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

/// Every git that reads the index runs the fsmonitor hook; one that never
/// returns stands in for any git that never answers.
#[cfg(unix)]
#[test]
fn a_git_that_never_answers_ends_in_exit_2_not_a_hang() {
    use std::os::unix::fs::PermissionsExt;
    let root = mid_merge("stall");
    let hook = root.join(".git").join("stall.sh");
    std::fs::write(&hook, "#!/bin/sh\nsleep 60\n").expect("hook");
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    git(
        &root,
        &["config", "core.fsmonitor", hook.to_str().expect("utf8")],
    );

    let (code, stderr) = weave_check(&root, &["--timeout", "2"], Duration::from_secs(30));
    assert_eq!(code, Some(2), "{stderr}");
    assert!(stderr.contains("NOTHING WAS VERIFIED"), "{stderr}");
}

/// An error is not a finding: it exits 2 and says nothing was verified.
#[test]
fn a_check_that_cannot_run_exits_2() {
    let root = fixture("not-a-repo");
    let (code, stderr) = weave_check(
        &root,
        &["--base", "a", "--ours", "b", "--theirs", "c"],
        Duration::from_secs(30),
    );
    assert_eq!(code, Some(2), "{stderr}");
    assert!(stderr.contains("NOTHING WAS VERIFIED"), "{stderr}");
}

/// And the ordinary case still answers: a conflicted file is a finding.
#[test]
fn a_conflicted_merge_is_a_finding_exit_1() {
    let root = mid_merge("conflicted");
    let (code, stderr) = weave_check(&root, &[], Duration::from_secs(60));
    assert_eq!(code, Some(1), "{stderr}");
}
