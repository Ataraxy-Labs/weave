//! The simplified command line: seven visible commands, variations as flags,
//! and every old command name kept as a hidden alias with identical output.
//!
//! Goldens under `tests/fixtures/cli_simplify/` were recorded from the binary
//! before the change, on the merge `mid_merge()` builds. Each old invocation
//! must still print its golden, after the fixture's temporary path becomes
//! `<ROOT>` and benchmark timings (`123us`) become `<T>us`; each new spelling
//! must print the same golden as the command it replaces.
//!
//! Regenerate the goldens with an old binary:
//!   WEAVE_GOLDEN_BIN=/path/to/old/weave WEAVE_UPDATE_GOLDENS=1 cargo test -p weave-cli --test cli_simplify goldens

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn git(dir: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_AUTHOR_DATE", "@1700000000")
        .env("GIT_COMMITTER_DATE", "@1700000000")
        .output()
        .expect("run git")
}

fn root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("weave-cli-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("repo")).unwrap();
    std::fs::create_dir_all(root.join("home")).unwrap();
    root
}

const BASE: &str = "def a():\n    return 1\n\n\ndef b():\n    return 2\n";
const OURS: &str = "def a():\n    return 10\n\n\ndef b():\n    return 2\n";
const THEIRS: &str = "def a():\n    return 11\n\n\ndef b():\n    return 2\n";

/// A repository stopped mid-merge on `app.py`, where both sides changed `a`.
fn mid_merge(name: &str) -> PathBuf {
    let root = root(name);
    let repo = root.join("repo");
    git(&repo, &["init", "-q", "-b", "main"]);
    std::fs::write(repo.join("app.py"), BASE).unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "base"]);
    git(&repo, &["checkout", "-qb", "theirs"]);
    std::fs::write(repo.join("app.py"), THEIRS).unwrap();
    git(&repo, &["commit", "-qam", "theirs"]);
    git(&repo, &["checkout", "-q", "main"]);
    std::fs::write(repo.join("app.py"), OURS).unwrap();
    git(&repo, &["commit", "-qam", "ours"]);
    git(&repo, &["merge", "-q", "theirs"]);
    assert!(repo.join(".git/MERGE_HEAD").exists());
    root
}

fn golden_bin() -> PathBuf {
    std::env::var_os("WEAVE_GOLDEN_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_weave")))
}

fn weave_with(bin: &Path, root: &Path, args: &[&str]) -> Output {
    Command::new(bin)
        .args(args)
        .current_dir(root.join("repo"))
        .env("HOME", root.join("home"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env_remove("WEAVE_STATS")
        .env_remove("WEAVE_NO_DEPRECATION")
        .output()
        .expect("run weave")
}

fn weave(root: &Path, args: &[&str]) -> Output {
    weave_with(Path::new(env!("CARGO_BIN_EXE_weave")), root, args)
}

fn normalize(out: &Output, root: &Path) -> String {
    let mut text = format!(
        "exit {:?}\n--- stdout\n{}--- stderr\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let canon = root.canonicalize().unwrap();
    for p in [
        canon.to_string_lossy().to_string(),
        root.to_string_lossy().to_string(),
    ] {
        text = text.replace(&p, "<ROOT>");
    }
    regex_lite_us(&text)
}

/// `1234us` -> `<T>us` (benchmark timings), without a regex dependency.
fn regex_lite_us(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() {
            let start = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            if text[i..].starts_with("us") {
                out.push_str("<T>");
            } else {
                out.push_str(&text[start..i]);
            }
            continue;
        }
        let ch = text[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    // the timing column is padded to a width; collapse runs of spaces before <T>
    out.split('\n')
        .map(|l| {
            let mut s = l.to_string();
            while s.contains(" <T>") && s.contains("  <T>") {
                s = s.replace("  <T>", " <T>");
            }
            s
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// (golden name, old invocation, new spellings that must print the same)
type Case = (
    &'static str,
    &'static [&'static str],
    &'static [&'static [&'static str]],
);

const CASES: &[Case] = &[
    ("explain_json", &["explain", "app.py", "--json"], &[]),
    ("explain_text", &["explain", "app.py"], &[]),
    (
        "summary_json",
        &["summary", "app.py", "--json"],
        &[&["explain", "app.py", "--summary", "--json"]],
    ),
    (
        "summary_text",
        &["summary", "app.py"],
        &[&["explain", "app.py", "--summary"]],
    ),
    ("check_json", &["check", "--json"], &[]),
    ("land_dry_json", &["land", "--dry-run", "--json"], &[]),
    ("stats", &["stats"], &[]),
    ("bench", &["bench"], &[&["stats", "--bench"]]),
];

#[test]
fn goldens_old_invocations_and_their_new_spellings() {
    let update = std::env::var_os("WEAVE_UPDATE_GOLDENS").is_some();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cli_simplify");
    let root = mid_merge("goldens");
    let mut failures = Vec::new();
    for (name, old, news) in CASES {
        let path = dir.join(format!("{name}.out"));
        if update {
            let out = weave_with(&golden_bin(), &root, old);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(&path, normalize(&out, &root)).unwrap();
            continue;
        }
        let golden = std::fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("missing golden {}", path.display()));
        for argv in std::iter::once(*old).chain(news.iter().copied()) {
            let got = normalize(&weave(&root, argv), &root);
            if got != golden {
                failures.push(format!(
                    "weave {} differs from golden {name}:\n--- golden\n{golden}\n--- got\n{got}",
                    argv.join(" ")
                ));
            }
        }
    }
    let _ = std::fs::remove_dir_all(&root);
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// A plain repository with one commit.
fn plain(name: &str) -> PathBuf {
    let root = root(name);
    let repo = root.join("repo");
    git(&repo, &["init", "-q", "-b", "main"]);
    std::fs::write(repo.join("app.py"), BASE).unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "base"]);
    root
}

fn repo_state(root: &Path) -> String {
    let repo = root.join("repo");
    let attrs = std::fs::read_to_string(repo.join(".gitattributes")).unwrap_or_default();
    let config =
        String::from_utf8_lossy(&git(&repo, &["config", "--local", "--list"]).stdout).to_string();
    format!("{attrs}\n{config}")
}

#[test]
fn setup_off_is_unsetup() {
    let (a, b) = (plain("unsetup"), plain("setup-off"));
    for r in [&a, &b] {
        let out = weave(r, &["setup", "--driver", "/bin/true"]);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let old = weave(&a, &["unsetup"]);
    let new = weave(&b, &["setup", "--off"]);
    assert_eq!(normalize(&old, &a), normalize(&new, &b));
    assert_eq!(repo_state(&a), repo_state(&b));
    assert!(
        weave(&b, &["setup", "--off", "--global"]).status.code() == Some(2),
        "--off takes no other flag"
    );
    for r in [a, b] {
        let _ = std::fs::remove_dir_all(r);
    }
}

#[test]
fn the_live_editing_prototype_moved_under_experimental() {
    let (a, b) = (plain("proto-old"), plain("proto-new"));
    let steps: &[(&[&str], &[&str])] = &[
        (
            &["claim", "agent-1", "app.py", "a"],
            &["experimental", "claim", "agent-1", "app.py", "a"],
        ),
        (
            &["status", "--file", "app.py"],
            &["experimental", "status", "--file", "app.py"],
        ),
        (
            &["release", "agent-1", "app.py", "a"],
            &["experimental", "release", "agent-1", "app.py", "a"],
        ),
        (&["apply", "app.py"], &["experimental", "apply", "app.py"]),
    ];
    for (old, new) in steps {
        let o = weave(&a, old);
        let n = weave(&b, new);
        assert_eq!(
            normalize(&o, &a).replace("proto-old", "X"),
            normalize(&n, &b).replace("proto-new", "X"),
            "weave {old:?} vs weave {new:?}"
        );
    }
    for r in [a, b] {
        let _ = std::fs::remove_dir_all(r);
    }
}

#[test]
fn old_names_print_no_notice_when_piped() {
    let root = mid_merge("quiet");
    for argv in [
        &["summary", "app.py"][..],
        &["summary", "app.py", "--json"],
        &["bench"],
        &["status"],
    ] {
        let out = weave(&root, argv);
        assert!(
            !String::from_utf8_lossy(&out.stderr).contains("is now `weave"),
            "weave {argv:?} printed a notice when piped"
        );
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn old_names_note_the_new_spelling_at_a_terminal() {
    let root = mid_merge("tty");
    let line = format!("{} summary app.py", env!("CARGO_BIN_EXE_weave"));
    let mut cmd = Command::new("script");
    if cfg!(target_os = "macos") {
        cmd.args(["-q", "/dev/null", "sh", "-c", &line]);
    } else {
        cmd.args(["-qec", &line, "/dev/null"]);
    }
    let Ok(out) = cmd
        .current_dir(root.join("repo"))
        .env_remove("WEAVE_NO_DEPRECATION")
        .output()
    else {
        return; // no `script` here
    };
    let text = String::from_utf8_lossy(&out.stdout);
    if !text.trim().is_empty() {
        assert!(
            text.contains("note: `weave summary` is now `weave explain --summary`"),
            "{text}"
        );
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn help_lists_the_seven_commands() {
    let out = Command::new(env!("CARGO_BIN_EXE_weave"))
        .arg("--help")
        .output()
        .unwrap();
    let help = String::from_utf8_lossy(&out.stdout);
    let listed: Vec<&str> = help
        .lines()
        .skip_while(|l| !l.starts_with("Commands:"))
        .skip(1)
        .take_while(|l| l.starts_with("  "))
        .filter_map(|l| l.split_whitespace().next())
        .collect();
    assert_eq!(
        listed,
        ["setup", "land", "explain", "check", "preview", "patch", "stats", "help"]
    );
    assert!(help.contains("QUICKSTART"));
}

#[test]
fn land_check_takes_sem_and_needs_a_landing() {
    let root = plain("check-flag");
    let out = weave(&root, &["land", "--check", "sem"]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "--check needs --onto or --queue"
    );
    let out = weave(&root, &["land", "--onto", "origin/main", "--check", "lint"]);
    assert_eq!(out.status.code(), Some(2), "only `sem` is a checker");
    let out = weave(
        &root,
        &[
            "land",
            "--onto",
            "origin/main",
            "--check",
            "sem",
            "--verify-cmd",
            "true",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(2),
        "--check and --verify-cmd are exclusive"
    );
    let _ = std::fs::remove_dir_all(&root);
}
