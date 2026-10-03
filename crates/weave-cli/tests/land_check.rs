//! `weave land --check sem`, end to end with a stand-in `sem` (WEAVE_SEM): a
//! script that records how it was called and answers with a chosen sem check
//! document and exit code.
//!
//! What must hold, on both the `--onto` and the `--queue` path: `sem check
//! --base <the exact tip merged onto> --json` runs on the final merged tree
//! before anything is published; exit 0 lands; exit 1 (fail) and exit 2 (could
//! not decide) refuse, publish nothing and hand the diagnostics back; no sem
//! at all refuses (fail closed); the result's certificate is recorded; a
//! `.weave/config` default turns it on and `--check none` off; `--verify-cmd`
//! still runs.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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

const PASS: &str = r#"{"verdict":"pass","exitCode":0,"head":{"tree":"__TREE__"},"notes":[],"checkers":[{"name":"ts","tool":"tsc","toolVersion":"5.9.3","verdict":"pass","mode":"incremental","reasons":["early cutoff: only files a change can affect were rechecked"],"filesRecheckedCount":1,"filesRechecked":["a.ts"],"errors":0,"diagnostics":[]}],"certificate":{"schema":"sem-check-certificate/1","digest":"0123456789abcdef0123456789abcdef01234567","verdict":"pass"}}"#;
const FAIL: &str = r#"{"verdict":"fail","exitCode":1,"head":{"tree":"__TREE__"},"notes":[],"checkers":[{"name":"ts","tool":"tsc","toolVersion":"5.9.3","verdict":"fail","mode":"incremental","reasons":["early cutoff: only files a change can affect were rechecked"],"filesRecheckedCount":2,"filesRechecked":["a.ts","b.ts"],"errors":1,"diagnostics":["b.ts(3,7): error TS2322: Type 'string' is not assignable to type 'number'."]}],"certificate":{"schema":"sem-check-certificate/1","digest":"fedcba9876543210fedcba9876543210fedcba98","verdict":"fail"}}"#;
const UNDECIDED: &str = r#"{"verdict":"undecided","exitCode":2,"notes":[],"checkers":[{"name":"ts","tool":"tsc","toolVersion":null,"verdict":"undecided","mode":"full","reasons":["typescript is not installed in this project"],"filesRecheckedCount":0,"filesRechecked":[],"errors":0,"diagnostics":[]}],"certificate":{"schema":"sem-check-certificate/1","digest":"1111111111111111111111111111111111111111","verdict":"undecided"}}"#;

struct World {
    root: PathBuf,
}

impl World {
    fn new(name: &str) -> World {
        let root = std::env::temp_dir().join(format!("weave-check-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let seed = root.join("seed");
        std::fs::create_dir_all(&seed).unwrap();
        git_ok(&seed, &["init", "-q", "-b", "main"]);
        std::fs::write(seed.join("a.txt"), "one\n").unwrap();
        std::fs::write(seed.join("b.txt"), "two\n").unwrap();
        git_ok(&seed, &["add", "."]);
        git_ok(&seed, &["commit", "-qm", "base"]);
        git_ok(&root, &["clone", "-q", "--bare", "seed", "origin.git"]);
        for c in ["feat", "other"] {
            git_ok(&root, &["clone", "-q", "origin.git", c]);
        }
        // main moves, so land has a real merge to make and check
        let other = root.join("other");
        std::fs::write(other.join("b.txt"), "two, from other\n").unwrap();
        git_ok(&other, &["commit", "-qam", "other"]);
        git_ok(&other, &["push", "-q", "origin", "HEAD:refs/heads/main"]);
        let feat = root.join("feat");
        std::fs::write(feat.join("a.txt"), "one, from feat\n").unwrap();
        git_ok(&feat, &["commit", "-qam", "feat"]);
        World { root }
    }

    fn main(&self) -> String {
        git_ok(&self.root.join("origin.git"), &["rev-parse", "main"])
    }

    /// A stand-in sem answering `doc` with `code`; it logs its arguments and
    /// cwd, and reports as the checked tree HEAD's tree — or, like sem, another
    /// tree when there are untracked files.
    fn fake_sem(&self, doc: &str, code: i32) -> PathBuf {
        let p = self.root.join("sem");
        let body = self.root.join("sem-answer.json");
        std::fs::write(&body, doc).unwrap();
        std::fs::write(
            &p,
            format!(
                "#!/bin/sh\necho \"$PWD $*\" >> {log}\nt=$(git rev-parse HEAD^{{tree}})\n\
                 if [ -n \"$(git ls-files --others --exclude-standard)\" ]; then t=0000000000000000000000000000000000000000; fi\n\
                 sed \"s/__TREE__/$t/\" {body}\nexit {code}\n",
                log = self.log().display(),
                body = body.display()
            ),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    fn log(&self) -> PathBuf {
        self.root.join("sem-calls.log")
    }

    fn calls(&self) -> Vec<String> {
        std::fs::read_to_string(self.log())
            .unwrap_or_default()
            .lines()
            .map(String::from)
            .collect()
    }

    fn land(&self, args: &[&str], sem: Option<&Path>) -> (i32, String) {
        let mut c = Command::new(env!("CARGO_BIN_EXE_weave"));
        c.arg("land")
            .args(args)
            .current_dir(self.root.join("feat"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@example.invalid");
        match sem {
            Some(p) => c.env("WEAVE_SEM", p),
            // fail-closed case: a WEAVE_SEM that names nothing, and no sem on PATH
            None => c.env("WEAVE_SEM", self.root.join("no-such-sem")),
        };
        let out = c.output().expect("run weave land");
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        (out.status.code().unwrap_or(-1), text)
    }

    fn certificates(&self) -> Vec<serde_json::Value> {
        let d = self.root.join("certs");
        let mut v: Vec<_> = std::fs::read_dir(&d)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.to_string_lossy().ends_with("-check.json"))
            .collect();
        v.sort();
        v.iter()
            .map(|p| serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap())
            .collect()
    }
}

#[test]
fn onto_lands_when_sem_check_passes_and_records_its_certificate() {
    let w = World::new("onto-pass");
    let tip = w.main();
    let sem = w.fake_sem(PASS, 0);
    let certs = w.root.join("certs");
    let (code, out) = w.land(
        &[
            "--onto",
            "origin/main",
            "--check",
            "sem",
            "--certificate-dir",
            certs.to_str().unwrap(),
        ],
        Some(&sem),
    );
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains("sem check PASS: ts pass incremental (1 rechecked)"),
        "{out}"
    );
    assert_ne!(w.main(), tip, "published");
    // sem check ran once, in the repository, against the exact tip merged onto
    let calls = w.calls();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert!(
        calls[0].contains(&format!("check --base {tip} --json")),
        "{calls:?}"
    );
    // and the landed tree is the one it checked
    let c = w.certificates();
    assert_eq!(c.len(), 1);
    assert_eq!(c[0]["kind"], "sem-check");
    assert_eq!(c[0]["verdict"], "pass");
    assert_eq!(c[0]["onto"], tip.as_str());
    assert_eq!(
        c[0]["tree"],
        git_ok(&w.root.join("origin.git"), &["rev-parse", "main^{tree}"]).as_str()
    );
    assert_eq!(
        c[0]["sem"]["certificate"]["digest"],
        "0123456789abcdef0123456789abcdef01234567"
    );
    assert_eq!(c[0]["sem"]["checkers"][0]["mode"], "incremental");
}

#[test]
fn onto_refuses_on_a_failing_check_and_returns_the_diagnostics() {
    let w = World::new("onto-fail");
    let before = w.main();
    let sem = w.fake_sem(FAIL, 1);
    let certs = w.root.join("certs");
    let (code, out) = w.land(
        &[
            "--onto",
            "origin/main",
            "--check",
            "sem",
            "--certificate-dir",
            certs.to_str().unwrap(),
        ],
        Some(&sem),
    );
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("sem check FAILED on the merged tree"), "{out}");
    assert!(
        out.contains("b.ts(3,7): error TS2322: Type 'string' is not assignable to type 'number'."),
        "{out}"
    );
    assert!(out.contains("REFUSED"), "{out}");
    assert_eq!(w.main(), before, "nothing published");
    let c = w.certificates();
    assert_eq!(c[0]["verdict"], "refused");
    assert_eq!(c[0]["reason"], "sem check failed");
}

#[test]
fn onto_refuses_when_sem_check_cannot_decide() {
    let w = World::new("onto-undecided");
    let before = w.main();
    let sem = w.fake_sem(UNDECIDED, 2);
    let (code, out) = w.land(&["--onto", "origin/main", "--check", "sem"], Some(&sem));
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("sem check COULD NOT DECIDE"), "{out}");
    assert!(
        out.contains("typescript is not installed in this project"),
        "{out}"
    );
    assert_eq!(w.main(), before);
    // unreadable output also refuses
    let sem = w.fake_sem("not json", 0);
    let (code, out) = w.land(&["--onto", "origin/main", "--check", "sem"], Some(&sem));
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("without a readable result"), "{out}");
    assert_eq!(w.main(), before);
}

#[test]
fn onto_refuses_a_pass_on_a_tree_with_untracked_files() {
    let w = World::new("onto-untracked");
    let before = w.main();
    let sem = w.fake_sem(PASS, 0);
    std::fs::write(w.root.join("feat/stray.ts"), "export const x = 1;\n").unwrap();
    let (code, out) = w.land(&["--onto", "origin/main", "--check", "sem"], Some(&sem));
    assert_eq!(code, 1, "{out}");
    assert!(
        out.contains("not the one to be published") && out.contains("stray.ts"),
        "{out}"
    );
    assert_eq!(w.main(), before);
}

#[test]
fn onto_fails_closed_without_sem() {
    let w = World::new("onto-nosem");
    let before = w.main();
    let (code, out) = w.land(&["--onto", "origin/main", "--check", "sem"], None);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("`sem` is not installed"), "{out}");
    assert_eq!(w.main(), before, "nothing published");
}

#[test]
fn weave_config_default_turns_it_on_and_check_none_off() {
    let w = World::new("config");
    let feat = w.root.join("feat");
    std::fs::create_dir_all(feat.join(".weave")).unwrap();
    std::fs::write(
        feat.join(".weave/config"),
        "[land]\n\tcheck = sem\n\tcheckers = ts,lint\n",
    )
    .unwrap();
    git_ok(&feat, &["add", ".weave/config"]);
    git_ok(&feat, &["commit", "-qm", "weave config"]);
    let sem = w.fake_sem(FAIL, 1);
    let before = w.main();
    let (code, out) = w.land(&["--onto", "origin/main"], Some(&sem));
    assert_eq!(code, 1, "{out}");
    assert!(
        w.calls()[0].contains("--checkers ts,lint"),
        "{:?}",
        w.calls()
    );
    assert_eq!(w.main(), before);
    // --check none: land without it (the verify command still runs)
    let (code, out) = w.land(
        &[
            "--onto",
            "origin/main",
            "--check",
            "none",
            "--verify-cmd",
            "test -f a.txt",
        ],
        Some(&sem),
    );
    assert_eq!(code, 0, "{out}");
    assert_eq!(w.calls().len(), 1, "sem check not run again");
}

#[test]
fn verify_cmd_runs_before_sem_check() {
    let w = World::new("verify-first");
    let sem = w.fake_sem(PASS, 0);
    let before = w.main();
    let (code, out) = w.land(
        &[
            "--onto",
            "origin/main",
            "--check",
            "sem",
            "--verify-cmd",
            "exit 3",
        ],
        Some(&sem),
    );
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("verify command"), "{out}");
    assert!(
        w.calls().is_empty(),
        "sem check never ran: the verify command refused first"
    );
    assert_eq!(w.main(), before);
}

fn done_results(w: &World) -> Vec<serde_json::Value> {
    let o = w.root.join("origin.git");
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
        .filter_map(|s| {
            serde_json::from_str::<serde_json::Value>(&git_ok(&o, &["log", "-1", "--format=%B", s]))
                .ok()
        })
        .filter(|v| v["kind"] == "result")
        .collect()
}

#[test]
fn queue_runs_sem_check_in_the_lander_and_records_it_in_the_result() {
    let w = World::new("queue-pass");
    let tip = w.main();
    let sem = w.fake_sem(PASS, 0);
    let (code, out) = w.land(
        &["--queue", "--check", "sem", "--lease-ttl", "5"],
        Some(&sem),
    );
    assert_eq!(code, 0, "{out}");
    assert_ne!(w.main(), tip);
    let calls = w.calls();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert!(
        calls[0].contains("weave-queue"),
        "ran in the lander's worktree: {calls:?}"
    );
    assert!(calls[0].contains(&format!("--base {tip}")), "{calls:?}");
    let r = done_results(&w);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0]["check"]["verdict"], "pass");
    assert_eq!(
        r[0]["check"]["certificate"]["digest"],
        "0123456789abcdef0123456789abcdef01234567"
    );
}

#[test]
fn queue_refuses_a_failing_check_and_the_submitter_sees_why() {
    let w = World::new("queue-fail");
    let before = w.main();
    let sem = w.fake_sem(FAIL, 1);
    let (code, out) = w.land(
        &["--queue", "--check", "sem", "--lease-ttl", "5"],
        Some(&sem),
    );
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("REFUSED (sem check failed)"), "{out}");
    assert!(out.contains("b.ts(3,7): error TS2322"), "{out}");
    assert_eq!(w.main(), before);
    let r = done_results(&w);
    assert_eq!(r[0]["reason"], "sem check failed");
    assert_eq!(r[0]["check"]["verdict"], "refused");
}

#[test]
fn queue_fails_closed_without_sem() {
    let w = World::new("queue-nosem");
    let before = w.main();
    let (code, out) = w.land(&["--queue", "--check", "sem", "--lease-ttl", "5"], None);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("REFUSED (sem not installed)"), "{out}");
    assert_eq!(w.main(), before);
}
