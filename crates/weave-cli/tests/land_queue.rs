//! `weave land --queue`, end to end, against one hardened bare origin
//! (`receive.denyNonFastForwards`, `receive.denyDeletes`, a pre-receive hook
//! that admits only main, feat/* and refs/weave/*, a post-receive hook that
//! logs every update of main) and many clones submitting at once.
//!
//! What must hold: every submission gets exactly one result; results are in
//! submission order; main only ever fast-forwards, and only to a tree the
//! queue gated and verified; a killed or paused lander's lock is taken over
//! and the paused one cannot publish when it wakes; a verify failure or a
//! conflict refuses that candidate alone.
// These tests drive POSIX shell scripts and Unix permissions; they run on Unix CI.
#![cfg(unix)]

use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

fn git(dir: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .expect("run git")
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

const PRE_RECEIVE: &str = "#!/bin/sh\nwhile read old new ref; do\n  case \"$ref\" in\n    refs/heads/main|refs/heads/feat/*|refs/weave/*) ;;\n    *) echo \"origin: pushes to $ref are not allowed\"; exit 1;;\n  esac\ndone\n";
const POST_RECEIVE: &str = "#!/bin/sh\nwhile read old new ref; do\n  if [ \"$ref\" = refs/heads/main ]; then echo \"$old $new\" >> \"$(dirname \"$0\")/../main-log\"; fi\ndone\n";

struct World {
    root: PathBuf,
}

impl World {
    fn new(name: &str, files: &[(&str, &str)], clones: usize) -> World {
        let root = std::env::temp_dir().join(format!("weave-queue-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let seed = root.join("seed");
        std::fs::create_dir_all(&seed).unwrap();
        git_ok(&seed, &["init", "-q", "-b", "main"]);
        for (p, t) in files {
            std::fs::write(seed.join(p), t).unwrap();
        }
        git_ok(&seed, &["add", "."]);
        git_ok(&seed, &["commit", "-qm", "base"]);
        git_ok(&root, &["clone", "-q", "--bare", "seed", "origin.git"]);
        let o = root.join("origin.git");
        git_ok(&o, &["config", "receive.denyNonFastForwards", "true"]);
        git_ok(&o, &["config", "receive.denyDeletes", "true"]);
        for (n, body) in [("pre-receive", PRE_RECEIVE), ("post-receive", POST_RECEIVE)] {
            let p = o.join("hooks").join(n);
            std::fs::write(&p, body).unwrap();
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        for i in 0..clones {
            git_ok(&root, &["clone", "-q", "origin.git", &format!("c{i}")]);
        }
        World { root }
    }

    fn dir(&self, i: usize) -> PathBuf {
        self.root.join(format!("c{i}"))
    }

    fn origin(&self) -> PathBuf {
        self.root.join("origin.git")
    }

    fn commit(&self, i: usize, files: &[(&str, &str)], msg: &str) {
        let d = self.dir(i);
        for (p, t) in files {
            std::fs::write(d.join(p), t).unwrap();
        }
        git_ok(&d, &["add", "."]);
        git_ok(&d, &["commit", "-qm", msg]);
    }

    fn spawn(&self, i: usize, extra: &[&str], env: &[(&str, &str)]) -> Child {
        let mut c = Command::new(env!("CARGO_BIN_EXE_weave"));
        c.args(["land", "--queue"])
            .args(extra)
            .current_dir(self.dir(i))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@example.invalid")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (k, v) in env {
            c.env(k, v);
        }
        c.spawn().expect("spawn weave land --queue")
    }

    fn land(&self, i: usize, extra: &[&str]) -> (i32, String) {
        finish(self.spawn(i, extra, &[]))
    }

    fn main(&self) -> String {
        git_ok(&self.origin(), &["rev-parse", "main"])
    }

    /// `(old, new)` for every update of main, in order.
    fn main_log(&self) -> Vec<(String, String)> {
        std::fs::read_to_string(self.origin().join("main-log"))
            .unwrap_or_default()
            .lines()
            .map(|l| {
                let (a, b) = l.split_once(' ').unwrap();
                (a.to_string(), b.to_string())
            })
            .collect()
    }

    /// Every result in `done`, oldest first, as JSON.
    fn results(&self) -> Vec<serde_json::Value> {
        let o = self.origin();
        if git(&o, &["rev-parse", "-q", "--verify", "refs/weave/main/done"])
            .stdout
            .is_empty()
        {
            return Vec::new();
        }
        let shas = git_ok(
            &o,
            &[
                "rev-list",
                "--first-parent",
                "--reverse",
                "refs/weave/main/done",
            ],
        );
        shas.lines()
            .map(|s| {
                serde_json::from_str::<serde_json::Value>(&git_ok(
                    &o,
                    &["show", "-s", "--format=%B", s],
                ))
                .unwrap()
            })
            .filter(|v| v["kind"] != "genesis")
            .collect()
    }

    /// Every ticket in `queue`, oldest first.
    fn tickets(&self) -> Vec<serde_json::Value> {
        let o = self.origin();
        let shas = git_ok(
            &o,
            &[
                "rev-list",
                "--first-parent",
                "--reverse",
                "refs/weave/main/queue",
            ],
        );
        shas.lines()
            .map(|s| {
                serde_json::from_str::<serde_json::Value>(&git_ok(
                    &o,
                    &["show", "-s", "--format=%B", s],
                ))
                .unwrap()
            })
            .filter(|v| v["kind"] != "genesis")
            .collect()
    }
}

fn finish(c: Child) -> (i32, String) {
    let out = c.wait_with_output().expect("wait");
    (
        out.status.code().unwrap_or(-1),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

/// The queue's invariants over a finished world: one result per ticket, in
/// ticket order; main's history is exactly the landed results, each a
/// fast-forward of the one before; every landed tree was verified.
fn assert_queue_laws(w: &World, verified_trees: Option<&Path>) {
    let tickets = w.tickets();
    let results = w.results();
    assert_eq!(tickets.len(), results.len(), "every ticket has a result");
    for (i, (t, r)) in tickets.iter().zip(&results).enumerate() {
        assert_eq!(t["ticket"], i as u64);
        assert_eq!(r["ticket"], i as u64, "results in ticket order");
        assert_eq!(
            r["candidate"], t["candidate"],
            "result {i} is for its ticket"
        );
    }
    let landed: Vec<String> = results
        .iter()
        .filter_map(|r| r["landed"].as_str().map(String::from))
        .collect();
    let log = w.main_log();
    let news: Vec<String> = log.iter().map(|(_, n)| n.clone()).collect();
    assert_eq!(
        news, landed,
        "main moved exactly to the landed results, in order"
    );
    for win in log.windows(2) {
        assert_eq!(
            win[0].1, win[1].0,
            "each update of main starts where the last ended"
        );
    }
    for (old, new) in &log {
        let ff = git(&w.origin(), &["merge-base", "--is-ancestor", old, new]);
        assert!(ff.status.success(), "{old} -> {new} is a fast-forward");
    }
    if let Some(vt) = verified_trees {
        let seen: HashSet<String> = std::fs::read_to_string(vt)
            .unwrap_or_default()
            .lines()
            .map(String::from)
            .collect();
        for h in &landed {
            let tree = git_ok(&w.origin(), &["rev-parse", &format!("{h}^{{tree}}")]);
            assert!(
                seen.contains(&tree),
                "landed {h} (tree {tree}) was verified"
            );
        }
    }
}

#[test]
fn one_submitter_lands_through_the_queue() {
    let w = World::new("one", &[("a.txt", "one\n")], 1);
    w.commit(0, &[("a.txt", "one, changed\n")], "change");
    let (code, out) = w.land(0, &["--verify-cmd", "test -f a.txt"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("LANDED"), "{out}");
    assert_eq!(git_ok(&w.origin(), &["show", "main:a.txt"]), "one, changed");
    assert_queue_laws(&w, None);
    // nothing new: refused locally, nothing queued
    let (code, out) = w.land(0, &[]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("nothing to land"), "{out}");
    assert_eq!(w.tickets().len(), 1);
}

#[test]
fn sixteen_parallel_submitters_all_land_in_order_with_no_push_race() {
    let n = 16;
    let mut base = vec![("shared.txt".to_string(), String::new())];
    for i in 0..n {
        base.push((format!("f{i}.txt"), format!("file {i}\n")));
    }
    let base_ref: Vec<(&str, &str)> = base.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let w = World::new("sixteen", &base_ref, n);
    let trees = w.root.join("verified-trees");
    for i in 0..n {
        w.commit(
            i,
            &[(&format!("f{i}.txt"), &format!("file {i}, changed by {i}\n"))],
            "change",
        );
    }
    let verify = format!(
        "git rev-parse HEAD^{{tree}} >> {} && sleep 0.1",
        trees.display()
    );
    let certs = w.root.join("certs");
    let certs_arg = certs.to_str().unwrap().to_string();
    let children: Vec<Child> = (0..n)
        .map(|i| {
            w.spawn(
                i,
                &[
                    "--verify-cmd",
                    &verify,
                    "--lease-ttl",
                    "20",
                    "--certificate-dir",
                    &certs_arg,
                ],
                &[],
            )
        })
        .collect();
    let outs: Vec<(i32, String)> = children.into_iter().map(finish).collect();
    for (i, (code, out)) in outs.iter().enumerate() {
        assert_eq!(*code, 0, "submitter {i}: {out}");
        assert!(out.contains("LANDED"), "submitter {i}: {out}");
    }
    assert_queue_laws(&w, Some(&trees));
    // every landed merge was gated: one merge certificate per landed merge
    // commit (a fast-forward landing has no merge to gate)
    let landed_merges = w
        .results()
        .iter()
        .filter_map(|r| r["landed"].as_str().map(String::from))
        .filter(|h| {
            git_ok(&w.origin(), &["rev-list", "--parents", "-n", "1", h])
                .split_whitespace()
                .count()
                == 3
        })
        .count();
    let merge_certs = std::fs::read_dir(&certs)
        .map(|d| {
            d.filter(|e| {
                e.as_ref()
                    .is_ok_and(|e| e.file_name().to_string_lossy().ends_with("-merge.json"))
            })
            .count()
        })
        .unwrap_or(0);
    assert_eq!(
        merge_certs, landed_merges,
        "one gate certificate per landed merge"
    );
    assert!(landed_merges >= n - 1, "{landed_merges}");
    assert_eq!(w.results().len(), n);
    for i in 0..n {
        assert_eq!(
            git_ok(&w.origin(), &["show", &format!("main:f{i}.txt")]),
            format!("file {i}, changed by {i}")
        );
    }
    // no submitter ever saw a push race
    for (_, out) in &outs {
        assert!(!out.contains("moved while landing"), "{out}");
    }
}

#[test]
fn a_verify_failure_refuses_only_that_candidate() {
    let n = 6;
    let w = World::new(
        "verify",
        &[
            ("a0.txt", "0\n"),
            ("a1.txt", "1\n"),
            ("a2.txt", "2\n"),
            ("a3.txt", "3\n"),
            ("a4.txt", "4\n"),
            ("a5.txt", "5\n"),
        ],
        n,
    );
    for i in 0..n {
        let mut files = vec![(format!("a{i}.txt"), format!("{i} changed\n"))];
        if i == 3 {
            files.push(("BAD".into(), "bad\n".into()));
        }
        let f: Vec<(&str, &str)> = files
            .iter()
            .map(|(a, b)| (a.as_str(), b.as_str()))
            .collect();
        w.commit(i, &f, "change");
    }
    let children: Vec<Child> = (0..n)
        .map(|i| w.spawn(i, &["--verify-cmd", "test ! -f BAD"], &[]))
        .collect();
    let outs: Vec<(i32, String)> = children.into_iter().map(finish).collect();
    for (i, (code, out)) in outs.iter().enumerate() {
        if i == 3 {
            assert_eq!(*code, 1, "{out}");
            assert!(out.contains("verify command failed"), "{out}");
        } else {
            assert_eq!(*code, 0, "submitter {i}: {out}");
        }
    }
    assert_queue_laws(&w, None);
    assert!(git(&w.origin(), &["show", "main:BAD"]).stdout.is_empty());
    let refused: Vec<_> = w
        .results()
        .into_iter()
        .filter(|r| r["landed"].is_null())
        .collect();
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0]["reason"], "verify command failed");
}

#[test]
fn a_conflict_is_refused_with_its_files_and_regions_then_resolved_in_place() {
    let w = World::new("conflict", &[("c.txt", "alpha\nbeta\ngamma\n")], 2);
    w.commit(0, &[("c.txt", "alpha\nbeta from 0\ngamma\n")], "zero");
    w.commit(1, &[("c.txt", "alpha\nbeta from 1\ngamma\n")], "one");
    let (code, out) = w.land(0, &[]);
    assert_eq!(code, 0, "{out}");
    let (code, out) = w.land(1, &[]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("merge refused"), "{out}");
    assert!(out.contains("c.txt"), "{out}");
    assert!(out.contains("c.txt:2-"), "conflict region reported: {out}");
    // the merge is left in progress in the submitter's own tree
    let d = w.dir(1);
    assert!(d.join(".git/MERGE_HEAD").exists(), "{out}");
    let r = w.results();
    assert_eq!(r[1]["files"][0], "c.txt");
    // resolve in place, keep both, and land again
    std::fs::write(d.join("c.txt"), "alpha\nbeta from 0\nbeta from 1\ngamma\n").unwrap();
    let (code, out) = w.land(1, &[]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(
        git_ok(&w.origin(), &["show", "main:c.txt"]),
        "alpha\nbeta from 0\nbeta from 1\ngamma"
    );
    assert_queue_laws(&w, None);
}

fn wait_for(p: &Path, limit: Duration) {
    let t = Instant::now();
    while !p.exists() {
        assert!(t.elapsed() < limit, "{} never appeared", p.display());
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn a_killed_lander_is_taken_over_after_the_lease_and_its_ticket_still_lands() {
    let w = World::new(
        "kill",
        &[("a.txt", "a\n"), ("b.txt", "b\n"), ("c.txt", "c\n")],
        3,
    );
    w.commit(0, &[("a.txt", "a changed\n")], "a");
    w.commit(1, &[("b.txt", "b changed\n")], "b");
    w.commit(2, &[("c.txt", "c changed\n")], "c");
    let started = w.root.join("started");
    let verify = format!(
        "if [ -n \"$SLOW\" ]; then touch {}; sleep 60; fi",
        started.display()
    );
    let mut a = w.spawn(
        0,
        &["--verify-cmd", &verify, "--lease-ttl", "2"],
        &[("SLOW", "1")],
    );
    wait_for(&started, Duration::from_secs(60));
    // A holds the lock and is verifying its own ticket: queue B and C behind it
    let b = w.spawn(1, &["--verify-cmd", &verify, "--lease-ttl", "2"], &[]);
    let c = w.spawn(2, &["--verify-cmd", &verify, "--lease-ttl", "2"], &[]);
    std::thread::sleep(Duration::from_millis(500));
    a.kill().unwrap(); // SIGKILL
    let _ = a.wait();
    let (cb, ob) = finish(b);
    let (cc, oc) = finish(c);
    assert_eq!(cb, 0, "{ob}");
    assert_eq!(cc, 0, "{oc}");
    assert!(
        ob.contains("taking over") || oc.contains("taking over"),
        "{ob}\n{oc}"
    );
    let r = w.results();
    assert_eq!(r.len(), 3, "A's ticket outlived A");
    assert!(r.iter().all(|r| r["landed"].is_string()), "{r:?}");
    for (f, t) in [
        ("a.txt", "a changed"),
        ("b.txt", "b changed"),
        ("c.txt", "c changed"),
    ] {
        assert_eq!(git_ok(&w.origin(), &["show", &format!("main:{f}")]), t);
    }
    assert_queue_laws(&w, None);
}

#[test]
fn a_paused_lander_that_lost_its_lock_cannot_publish_when_it_wakes() {
    let w = World::new("pause", &[("a.txt", "a\n"), ("b.txt", "b\n")], 2);
    w.commit(0, &[("a.txt", "a changed\n")], "a");
    w.commit(1, &[("b.txt", "b changed\n")], "b");
    let started = w.root.join("started");
    let verify = format!(
        "if [ -n \"$SLOW\" ]; then touch {}; sleep 6; fi",
        started.display()
    );
    let a = w.spawn(
        0,
        &["--verify-cmd", &verify, "--lease-ttl", "2"],
        &[("SLOW", "1")],
    );
    wait_for(&started, Duration::from_secs(60));
    let pid = a.id().to_string();
    // freeze A (its heartbeat thread too): B must take over and land both
    assert!(Command::new("kill")
        .args(["-STOP", &pid])
        .status()
        .unwrap()
        .success());
    let b = w.spawn(1, &["--verify-cmd", &verify, "--lease-ttl", "2"], &[]);
    let (cb, ob) = finish(b);
    assert_eq!(cb, 0, "{ob}");
    let main_after_b = w.main();
    // wake A: its verify finishes, its fenced push must fail; it then finds
    // its ticket already landed by B
    assert!(Command::new("kill")
        .args(["-CONT", &pid])
        .status()
        .unwrap()
        .success());
    let (ca, oa) = finish(a);
    assert_eq!(ca, 0, "{oa}");
    assert_eq!(w.main(), main_after_b, "A published nothing after waking");
    let r = w.results();
    assert_eq!(r.len(), 2);
    let landers: BTreeSet<String> = r
        .iter()
        .map(|r| r["lander"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(landers.len(), 1, "B landed both: {r:?}");
    assert_queue_laws(&w, None);
}

#[test]
fn an_origin_that_refuses_queue_refs_fails_the_run_and_publishes_nothing() {
    let w = World::new("closed", &[("a.txt", "a\n")], 1);
    let hook = w.origin().join("hooks").join("pre-receive");
    std::fs::write(&hook, PRE_RECEIVE.replace("|refs/weave/*", "")).unwrap();
    let before = w.main();
    w.commit(0, &[("a.txt", "a changed\n")], "a");
    let (code, out) = w.land(0, &[]);
    assert_eq!(code, 2, "{out}");
    assert_eq!(w.main(), before);
}

/// The lost-work case of land_onto.rs, through the queue: a merge in progress that git
/// never wrote (MERGE_HEAD set, index and tree still HEAD — what git 2.39
/// leaves when it cannot write the index) is not a resolution, and a
/// committed merge whose tree drops files only main changed is not landed.
#[test]
fn a_merge_that_drops_files_only_main_changed_never_lands_through_the_queue() {
    let w = World::new("lost-work", &[("a.txt", "one\n"), ("b.txt", "two\n")], 2);
    w.commit(
        1,
        &[
            ("b.txt", "two, from c1\n"),
            ("f14.txt", "shift operators\n"),
        ],
        "c1: f14",
    );
    let (code, out) = w.land(1, &[]);
    assert_eq!(code, 0, "{out}");
    let main_with_f14 = w.main();
    w.commit(0, &[("a.txt", "one, from c0\n")], "c0");
    let c0 = w.dir(0);

    // the half-applied merge in c0's own tree
    git_ok(&c0, &["fetch", "-q", "origin", "main"]);
    git_ok(
        &c0,
        &["merge", "-q", "--no-commit", "--no-ff", "FETCH_HEAD"],
    );
    git_ok(&c0, &["read-tree", "HEAD"]);
    git_ok(&c0, &["checkout-index", "-f", "-a"]);
    let _ = std::fs::remove_file(c0.join("f14.txt"));
    let (code, out) = w.land(0, &[]);
    assert_ne!(code, 0, "{out}");
    assert!(
        out.contains("does not keep every change") && out.contains("f14.txt"),
        "{out}"
    );
    assert_eq!(w.main(), main_with_f14, "nothing published");

    // committed as is (a merge whose tree is c0's)
    git_ok(&c0, &["commit", "-q", "--no-edit"]);
    let (code, out) = w.land(0, &[]);
    assert_ne!(code, 0, "{out}");
    assert!(
        out.contains("loses changes") && out.contains("f14.txt"),
        "{out}"
    );
    assert_eq!(w.main(), main_with_f14, "nothing published");

    // put back, it lands with both sides
    git_ok(&c0, &["checkout", &main_with_f14, "--", "b.txt", "f14.txt"]);
    git_ok(&c0, &["commit", "-qm", "restore f14"]);
    let (code, out) = w.land(0, &[]);
    assert_eq!(code, 0, "{out}");
    let o = w.origin();
    assert_eq!(git_ok(&o, &["show", "main:f14.txt"]), "shift operators");
    assert_eq!(git_ok(&o, &["show", "main:b.txt"]), "two, from c1");
    assert_eq!(git_ok(&o, &["show", "main:a.txt"]), "one, from c0");
}
