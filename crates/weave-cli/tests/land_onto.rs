//! `weave land --onto`, end to end: a bare "origin", a feature clone that
//! lands onto origin/main, and a second clone that moves main underneath it.
//!
//! The rule under test: nothing is published that has not passed the gate,
//! and the verify command, against the exact tip of main it lands on — and a
//! main that moves during land is merged and checked again.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn git(dir: &Path, args: &[&str]) -> Output {
    let out = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .expect("run git");
    out
}

fn git_ok(dir: &Path, args: &[&str]) -> String {
    let out = git(dir, args);
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

struct World {
    root: PathBuf,
}

impl World {
    /// origin.git with `files` on main, and two clones: `feat` (the lander)
    /// and `other` (who moves main).
    fn new(name: &str, files: &[(&str, &str)]) -> World {
        let root = std::env::temp_dir().join(format!("weave-onto-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        git_ok(&root, &["init", "-q", "--bare", "-b", "main", "origin.git"]);
        let seed = root.join("seed");
        git_ok(&root, &["clone", "-q", "origin.git", "seed"]);
        for (p, t) in files {
            std::fs::write(seed.join(p), t).unwrap();
        }
        git_ok(&seed, &["add", "."]);
        git_ok(&seed, &["commit", "-qm", "base"]);
        git_ok(&seed, &["push", "-q", "origin", "HEAD:refs/heads/main"]);
        for c in ["feat", "other"] {
            git_ok(&root, &["clone", "-q", "origin.git", c]);
        }
        World { root }
    }

    fn dir(&self, c: &str) -> PathBuf {
        self.root.join(c)
    }

    fn commit(&self, c: &str, files: &[(&str, &str)], msg: &str) {
        let d = self.dir(c);
        for (p, t) in files {
            std::fs::write(d.join(p), t).unwrap();
        }
        git_ok(&d, &["add", "."]);
        git_ok(&d, &["commit", "-qm", msg]);
    }

    /// `other` publishes its commits straight to main.
    fn other_publishes(&self) {
        git_ok(
            &self.dir("other"),
            &["push", "-q", "origin", "HEAD:refs/heads/main"],
        );
    }

    fn main(&self) -> String {
        git_ok(&self.root.join("origin.git"), &["rev-parse", "main"])
    }

    fn show_main(&self, path: &str) -> String {
        git_ok(
            &self.root.join("origin.git"),
            &["show", &format!("main:{path}")],
        )
    }

    fn land(&self, extra: &[&str]) -> (i32, String) {
        self.land_env(extra, &[])
    }

    fn land_env(&self, extra: &[&str], env: &[(&str, &str)]) -> (i32, String) {
        let out = Command::new(env!("CARGO_BIN_EXE_weave"))
            .args(["land", "--onto", "origin/main"])
            .args(extra)
            .envs(env.iter().copied())
            .current_dir(self.dir("feat"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@example.invalid")
            .output()
            .expect("run weave land --onto");
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        (out.status.code().unwrap_or(-1), text)
    }
}

const SWITCH_BASE: &str = "package p\n\nfunc F(x int) int {\n\tswitch x {\n\tcase 1:\n\t\treturn 10\n\tcase 2:\n\t\treturn 20\n\t}\n\treturn 0\n}\n";
// Each side adds `case 3:` to the one switch, at a different place: git
// merges it line-cleanly, and the result does not compile.
const SWITCH_OURS: &str = "package p\n\nfunc F(x int) int {\n\tswitch x {\n\tcase 1:\n\t\treturn 10\n\tcase 3:\n\t\treturn 31\n\tcase 2:\n\t\treturn 20\n\t}\n\treturn 0\n}\n";
const SWITCH_THEIRS: &str = "package p\n\nfunc F(x int) int {\n\tswitch x {\n\tcase 1:\n\t\treturn 10\n\tcase 2:\n\t\treturn 20\n\tcase 3:\n\t\treturn 32\n\t}\n\treturn 0\n}\n";

#[test]
fn lands_a_clean_merge_after_the_gate_and_the_verify_command() {
    let w = World::new("clean", &[("a.txt", "one\n"), ("b.txt", "two\n")]);
    w.commit("other", &[("b.txt", "two, from other\n")], "other");
    w.other_publishes();
    w.commit("feat", &[("a.txt", "one, from feat\n")], "feat");
    let (code, out) = w.land(&[
        "--verify-cmd",
        "test -f a.txt && test -f b.txt && echo verified >&2",
    ]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("LANDED"), "{out}");
    assert!(out.contains("running the verify command"), "{out}");
    assert_eq!(w.show_main("a.txt"), "one, from feat");
    assert_eq!(w.show_main("b.txt"), "two, from other");
}

#[test]
fn a_failing_verify_command_publishes_nothing() {
    let w = World::new("verify-fails", &[("a.txt", "one\n")]);
    w.commit("feat", &[("a.txt", "one, from feat\n")], "feat");
    let before = w.main();
    let (code, out) = w.land(&["--verify-cmd", "echo build broke; exit 3"]);
    assert_eq!(code, 1, "{out}");
    assert!(
        out.contains("build broke") && out.contains("REFUSED"),
        "{out}"
    );
    assert_eq!(w.main(), before, "nothing published");
}

#[test]
fn a_line_clean_duplicate_case_is_refused_and_nothing_is_published() {
    let w = World::new("dup-case", &[("p.go", SWITCH_BASE)]);
    w.commit("other", &[("p.go", SWITCH_THEIRS)], "other: case 3");
    w.other_publishes();
    let before = w.main();
    w.commit("feat", &[("p.go", SWITCH_OURS)], "feat: case 3");
    let (code, out) = w.land(&[]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("REFUSED") && out.contains("p.go"), "{out}");
    assert!(out.contains("case `3`"), "the refusal names the key: {out}");
    assert_eq!(w.main(), before, "nothing published");
    // the merge is left in progress, the file conflicted, for the agent
    let feat = w.dir("feat");
    assert!(feat.join(".git/MERGE_HEAD").exists());
    assert!(!git_ok(&feat, &["ls-files", "-u", "p.go"]).is_empty());

    // The agent resolves it in place (theirs keeps 3, ours moves to 4) and
    // runs land again: the resolution is gated, committed and published.
    let resolved = "package p\n\nfunc F(x int) int {\n\tswitch x {\n\tcase 1:\n\t\treturn 10\n\tcase 4:\n\t\treturn 31\n\tcase 2:\n\t\treturn 20\n\tcase 3:\n\t\treturn 32\n\t}\n\treturn 0\n}\n";
    std::fs::write(feat.join("p.go"), resolved).unwrap();
    let (code, out) = w.land(&[]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(w.show_main("p.go"), resolved.trim_end());
}

#[test]
fn main_moving_during_land_is_merged_and_checked_again() {
    let w = World::new(
        "race",
        &[("a.txt", "one\n"), ("b.txt", "two\n"), ("c.txt", "three\n")],
    );
    w.commit("feat", &[("a.txt", "one, from feat\n")], "feat");
    w.commit("other", &[("b.txt", "two, from other\n")], "other");
    // The first verify run moves main (another lander wins the race); the
    // count of verify runs is kept in a file outside the repo.
    let count = w.root.join("verify-count");
    let other = w.dir("other");
    let script = w.root.join("verify.sh");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\nn=$(cat {c} 2>/dev/null || echo 0); n=$((n+1)); echo $n > {c}\n\
             if [ $n -eq 1 ]; then git -C {o} -c user.name=t -c user.email=t@x push -q origin HEAD:refs/heads/main; fi\n\
             test -f a.txt\n",
            c = count.display(),
            o = other.display()
        ),
    )
    .unwrap();
    let cmd = format!("sh {}", script.display());
    let (code, out) = w.land(&["--verify-cmd", &cmd]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("moved while landing"), "{out}");
    assert_eq!(
        std::fs::read_to_string(&count).unwrap().trim(),
        "2",
        "verified again on the new merge"
    );
    assert_eq!(w.show_main("a.txt"), "one, from feat");
    assert_eq!(w.show_main("b.txt"), "two, from other");
    // what was published contains the moved tip
    let tip = git_ok(&other, &["rev-parse", "HEAD"]);
    git_ok(
        &w.root.join("origin.git"),
        &["merge-base", "--is-ancestor", &tip, "main"],
    );
}

#[test]
fn a_merge_the_agent_committed_itself_is_gated_too() {
    let w = World::new("own-merge", &[("p.go", SWITCH_BASE)]);
    w.commit("other", &[("p.go", SWITCH_THEIRS)], "other: case 3");
    w.other_publishes();
    w.commit("feat", &[("p.go", SWITCH_OURS)], "feat: case 3");
    let feat = w.dir("feat");
    git_ok(&feat, &["fetch", "-q", "origin", "main"]);
    // plain git merges it line-cleanly; the agent commits it as is
    git_ok(&feat, &["merge", "-q", "--no-edit", "FETCH_HEAD"]);
    let before = w.main();
    let (code, out) = w.land(&[]);
    assert_eq!(code, 1, "{out}");
    assert!(
        out.contains("does not pass weave's gate") && out.contains("case `3`"),
        "{out}"
    );
    assert_eq!(w.main(), before, "nothing published");

    // A later commit that repairs the file counts: the merge is heard again
    // as the branch holds the file now.
    let fixed = "package p\n\nfunc F(x int) int {\n\tswitch x {\n\tcase 1:\n\t\treturn 10\n\tcase 4:\n\t\treturn 31\n\tcase 2:\n\t\treturn 20\n\tcase 3:\n\t\treturn 32\n\t}\n\treturn 0\n}\n";
    w.commit("feat", &[("p.go", fixed)], "feat: case 4");
    let (code, out) = w.land(&[]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(w.show_main("p.go"), fixed.trim_end());
}

// ------------------------------------------------------------------------
// A lost-work case: weave's own `git commit` of a checked merge failed
// on an `index.lock` a killed git process left behind; the agent removed the
// lock and ran land again; land took the merge in progress — one git 2.39
// had never written (MERGE_HEAD set, index and working tree still HEAD) —
// as "your resolution", the gate (which reads only files both sides
// changed) had nothing to read, and the published merge's tree was ours:
// every file only main had changed went back to the merge base.

/// main gains a feature in files only `other` touches (one edited, one new);
/// `feat` changes a file of its own.
fn lost_work_world(name: &str) -> World {
    let w = World::new(name, &[("a.txt", "one\n"), ("b.txt", "two\n")]);
    w.commit(
        "other",
        &[
            ("b.txt", "two, from other\n"),
            ("f14.txt", "shift operators\n"),
        ],
        "other: f14",
    );
    w.other_publishes();
    w.commit("feat", &[("a.txt", "one, from feat\n")], "feat");
    w
}

/// Leave the merge in progress in `feat` the way git 2.39 leaves a
/// `git merge --no-commit` that could not write the index: MERGE_HEAD
/// written, the index and the working tree still at HEAD.
fn half_apply(feat: &Path) {
    git_ok(feat, &["read-tree", "HEAD"]);
    git_ok(feat, &["checkout-index", "-f", "-a"]);
    let _ = std::fs::remove_file(feat.join("f14.txt"));
    assert!(feat.join(".git/MERGE_HEAD").exists());
    assert!(git_ok(feat, &["status", "--porcelain", "--untracked-files=no"]).is_empty());
}

fn assert_both_sides_on_main(w: &World) {
    assert_eq!(w.show_main("a.txt"), "one, from feat");
    assert_eq!(w.show_main("b.txt"), "two, from other");
    assert_eq!(w.show_main("f14.txt"), "shift operators");
}

/// A running process with `.git/index.lock` open in `dir`; killed on drop
/// (the lock file stays: a killed git leaves it so).
struct Holder(std::process::Child);

impl Holder {
    fn start(dir: &Path) -> Holder {
        let lock = dir.join(".git/index.lock");
        std::fs::write(&lock, "").unwrap();
        let child = Command::new("sleep")
            .arg("60")
            .stdin(std::fs::File::open(&lock).unwrap())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        Holder(child)
    }
}

impl Drop for Holder {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn a_commit_that_fails_on_a_lock_then_a_removed_lock_never_publishes_a_merge_that_drops_main() {
    let w = lost_work_world("lock-commit");
    let before = w.main();
    let feat = w.dir("feat");
    // Another git process takes index.lock just before land's own commit.
    let pidfile = w.root.join("holder.pid");
    let fault = format!(
        ": > .git/index.lock; sleep 60 < .git/index.lock > /dev/null 2>&1 & echo $! > {}",
        pidfile.display()
    );
    let (code, out) = w.land_env(&[], &[("WEAVE_LAND_FAULT_BEFORE_COMMIT", &fault)]);
    let pid = std::fs::read_to_string(&pidfile).expect("the fault ran");
    assert_eq!(code, 1, "{out}");
    assert!(
        out.contains("git commit of the checked merge failed"),
        "{out}"
    );
    assert!(out.contains("held by a running process"), "{out}");
    assert_eq!(w.main(), before, "nothing published");
    assert!(
        feat.join(".git/weave-land-merge").exists(),
        "land's unfinished merge is recorded: {out}"
    );

    // What the agent did: remove the lock by hand and run land
    // again — with the merge in progress the one git 2.39 leaves.
    Command::new("kill").arg(pid.trim()).status().unwrap();
    std::fs::remove_file(feat.join(".git/index.lock")).unwrap();
    half_apply(&feat);
    let (code, out) = w.land(&[]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("never finished"), "{out}");
    assert!(!out.contains("checking your resolution"), "{out}");
    assert_both_sides_on_main(&w);
}

#[test]
fn a_merge_git_never_wrote_is_not_a_resolution() {
    let w = lost_work_world("half-applied");
    let before = w.main();
    let feat = w.dir("feat");
    // Not land's merge (nothing recorded): the agent's own, or one whose
    // record is gone. The gate has no file both sides changed to read.
    git_ok(&feat, &["fetch", "-q", "origin", "main"]);
    git_ok(
        &feat,
        &["merge", "-q", "--no-commit", "--no-ff", "FETCH_HEAD"],
    );
    half_apply(&feat);
    let (code, out) = w.land(&[]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("checking your resolution"), "{out}");
    assert!(
        out.contains("does not keep every change")
            && out.contains("b.txt")
            && out.contains("f14.txt"),
        "the refusal names the files: {out}"
    );
    assert!(!out.contains("a.txt "), "a.txt keeps feat's change: {out}");
    assert_eq!(w.main(), before, "nothing published");
    assert!(feat.join(".git/MERGE_HEAD").exists(), "left as it is");

    git_ok(&feat, &["merge", "--abort"]);
    let (code, out) = w.land(&[]);
    assert_eq!(code, 0, "{out}");
    assert_both_sides_on_main(&w);
}

#[test]
fn a_committed_merge_that_drops_files_only_main_changed_is_refused() {
    let w = lost_work_world("ours-merge");
    let before = w.main();
    let feat = w.dir("feat");
    git_ok(&feat, &["fetch", "-q", "origin", "main"]);
    // a merge commit whose tree is ours
    git_ok(
        &feat,
        &["merge", "-q", "-s", "ours", "--no-edit", "FETCH_HEAD"],
    );
    let (code, out) = w.land(&[]);
    assert_eq!(code, 1, "{out}");
    assert!(
        out.contains("loses changes") && out.contains("b.txt") && out.contains("f14.txt"),
        "{out}"
    );
    assert_eq!(w.main(), before, "nothing published");

    // Putting main's change back in a later commit counts.
    git_ok(&feat, &["checkout", &before, "--", "b.txt", "f14.txt"]);
    git_ok(&feat, &["commit", "-qm", "restore main's f14"]);
    let (code, out) = w.land(&[]);
    assert_eq!(code, 0, "{out}");
    assert_both_sides_on_main(&w);
}

#[test]
fn a_lock_a_running_process_holds_refuses_and_a_stale_one_is_cleared_out_loud() {
    let w = lost_work_world("stale-lock");
    let before = w.main();
    let feat = w.dir("feat");
    let holder = Holder::start(&feat);
    let (code, out) = w.land(&[]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("held by a running process"), "{out}");
    assert_eq!(w.main(), before, "nothing published");

    drop(holder);
    assert!(feat.join(".git/index.lock").exists());
    let (code, out) = w.land(&[]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("removed a stale"), "{out}");
    assert_both_sides_on_main(&w);
}

#[test]
fn the_invariant_refuses_a_tree_that_reverts_a_file_only_theirs_changed() {
    use weave_cli::preserve::check;
    let w = lost_work_world("invariant");
    let feat = w.dir("feat");
    git_ok(&feat, &["fetch", "-q", "origin", "main"]);
    let ours = git_ok(&feat, &["rev-parse", "HEAD"]);
    let theirs = git_ok(&feat, &["rev-parse", "FETCH_HEAD"]);
    let base = git_ok(&feat, &["merge-base", &ours, &theirs]);
    git_ok(
        &feat,
        &["merge", "-q", "--no-commit", "--no-ff", "FETCH_HEAD"],
    );
    let tree = |files: &[(&str, Option<&str>)]| {
        for (p, t) in files {
            match t {
                Some(t) => {
                    std::fs::write(feat.join(p), t).unwrap();
                    git_ok(&feat, &["add", p]);
                }
                None => {
                    git_ok(&feat, &["rm", "-q", "-f", p]);
                }
            }
        }
        let t = git_ok(&feat, &["write-tree"]);
        git_ok(&feat, &["reset", "-q", "--hard", "HEAD"]);
        git_ok(
            &feat,
            &["merge", "-q", "--no-commit", "--no-ff", "FETCH_HEAD"],
        );
        t
    };
    let lost = |t: &str| -> Vec<(String, &'static str)> {
        check(&feat, &base, &ours, &theirs, t)
            .unwrap()
            .into_iter()
            .map(|v| (v.path, v.side))
            .collect()
    };

    // git's merge keeps both sides
    assert_eq!(lost(&tree(&[])), vec![]);
    // ours' tree as the merge (bcdf0fd's shape): both of main's files back at base
    assert_eq!(
        lost(&format!("{ours}^{{tree}}")),
        vec![("b.txt".into(), "theirs"), ("f14.txt".into(), "theirs")]
    );
    // one theirs-only file reverted
    assert_eq!(
        lost(&tree(&[("b.txt", Some("two\n"))])),
        vec![("b.txt".into(), "theirs")]
    );
    // a file main added, deleted by the merge
    assert_eq!(
        lost(&tree(&[("f14.txt", None)])),
        vec![("f14.txt".into(), "theirs")]
    );
    // main's change kept, with an edit of the merge's own beside it: kept
    assert_eq!(
        lost(&tree(&[(
            "b.txt",
            Some("two, from other\nand a line of the merge's own\n")
        )])),
        vec![]
    );
    // part of main's change dropped
    assert_eq!(
        lost(&tree(&[("b.txt", Some("two, from\n"))])),
        vec![("b.txt".into(), "theirs")]
    );
    // ours' own change reverted to base; ours' file edited further is the merge's to do
    assert_eq!(
        lost(&tree(&[("a.txt", Some("one\n"))])),
        vec![("a.txt".into(), "ours")]
    );
    assert_eq!(
        lost(&tree(&[("a.txt", Some("one, from feat, adapted\n"))])),
        vec![]
    );
}

/// A `sem` on PATH that records its arguments and exits with `code`.
#[cfg(unix)]
fn fake_sem(w: &World, code: i32) -> String {
    let bin = w.root.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let log = w.root.join("sem.args");
    std::fs::write(
        bin.join("sem"),
        format!(
            "#!/bin/sh\necho \"$@\" >> '{}'\necho sem says {code} >&2\nexit {code}\n",
            log.display()
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(bin.join("sem"), std::fs::Permissions::from_mode(0o755)).unwrap();
    format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    )
}

#[cfg(unix)]
#[test]
fn check_sem_verifies_with_sem_check_and_a_failing_verdict_publishes_nothing() {
    let w = World::new("check-sem-fails", &[("a.txt", "one\n")]);
    w.commit("feat", &[("a.txt", "one, from feat\n")], "feat");
    let before = w.main();
    let path = fake_sem(&w, 1);
    let (code, out) = w.land_env(&["--check", "sem"], &[("PATH", &path)]);
    assert_eq!(code, 1, "{out}");
    assert!(
        out.contains("sem says 1") && out.contains("REFUSED"),
        "{out}"
    );
    assert_eq!(w.main(), before, "nothing published");
    let args = std::fs::read_to_string(w.root.join("sem.args")).unwrap();
    assert_eq!(args.trim(), "check");
}

#[cfg(unix)]
#[test]
fn check_sem_lands_when_sem_check_passes() {
    let w = World::new("check-sem-passes", &[("a.txt", "one\n")]);
    w.commit("feat", &[("a.txt", "one, from feat\n")], "feat");
    let path = fake_sem(&w, 0);
    let (code, out) = w.land_env(&["--check", "sem"], &[("PATH", &path)]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("LANDED"), "{out}");
    assert_eq!(w.show_main("a.txt"), "one, from feat");
}
