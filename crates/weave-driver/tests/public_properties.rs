//! Public product-level properties: abort restores the original file, and
//! successful merges/rebases preserve the spacing of untouched declarations.
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
        .env("WEAVE_STATS", "0")
        .env("WEAVE_AUDIT", "0")
        .env("WEAVE_FINDINGS", "0")
        .env("WEAVE_EVENT", "1")
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

const SPACING_BASE: &str = "\
const PARAMS_VAR = js`params`
const SNIPPETS_VAR = js`snippets`
type Option = string
type Query = number

function first() { return 1 }

function second() { return 2 }

function third() { return 3 }

function fourth() { return 4 }
";

fn spacing_repo() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    git_ok(dir, &["init", "-q", "-b", "main", "--template="]);
    git_ok(dir, &["config", "user.email", "test@example.com"]);
    git_ok(dir, &["config", "user.name", "Test"]);
    git_ok(
        dir,
        &[
            "config",
            "merge.weave.driver",
            &format!("\"{}\" %O %A %B %L %P", env!("CARGO_BIN_EXE_weave-driver")),
        ],
    );
    fs::write(dir.join(".gitattributes"), "*.ts merge=weave\n").unwrap();
    fs::write(dir.join("app.ts"), SPACING_BASE).unwrap();
    git_ok(dir, &["add", "."]);
    git_ok(dir, &["commit", "-q", "-m", "base"]);
    tmp
}

fn assert_entity_merges(output: &std::process::Output, count: usize) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "git operation failed: {stderr}");
    assert_eq!(
        stderr.matches("\"used_fallback\":false").count(),
        count,
        "exercise the actual entity merge driver on every replay: {stderr}"
    );
}

#[test]
fn merging_unrelated_edits_does_not_add_blank_lines_issue169() {
    let tmp = spacing_repo();
    let dir = tmp.path();
    let ours = SPACING_BASE.replace("return 1 }", "return 10 }");
    let theirs = SPACING_BASE.replace("return 4 }", "return 40 }");
    git_ok(dir, &["checkout", "-q", "-b", "incoming"]);
    fs::write(dir.join("app.ts"), &theirs).unwrap();
    git_ok(dir, &["commit", "-q", "-am", "theirs"]);
    git_ok(dir, &["checkout", "-q", "main"]);
    fs::write(dir.join("app.ts"), &ours).unwrap();
    git_ok(dir, &["commit", "-q", "-am", "ours"]);

    let merge = git(dir, &["merge", "--no-edit", "incoming"]);
    assert_entity_merges(&merge, 1);
    assert_eq!(
        fs::read_to_string(dir.join("app.ts")).unwrap(),
        ours.replace("return 4 }", "return 40 }")
    );
    assert!(git(dir, &["status", "--porcelain"]).stdout.is_empty());
}

#[test]
fn rebasing_several_commits_does_not_add_blank_lines_issue169() {
    let tmp = spacing_repo();
    let dir = tmp.path();
    git_ok(dir, &["checkout", "-q", "-b", "topic"]);
    for value in 10..14 {
        fs::write(
            dir.join("app.ts"),
            SPACING_BASE.replace("return 1 }", &format!("return {value} }}")),
        )
        .unwrap();
        git_ok(dir, &["commit", "-q", "-am", &format!("topic {value}")]);
    }
    git_ok(dir, &["checkout", "-q", "main"]);
    let main = SPACING_BASE.replace("return 4 }", "return 40 }");
    fs::write(dir.join("app.ts"), &main).unwrap();
    git_ok(dir, &["commit", "-q", "-am", "main"]);
    git_ok(dir, &["checkout", "-q", "topic"]);

    let rebase = git(dir, &["rebase", "--merge", "main"]);
    assert_entity_merges(&rebase, 4);
    assert_eq!(
        fs::read_to_string(dir.join("app.ts")).unwrap(),
        main.replace("return 1 }", "return 13 }")
    );
    assert!(git(dir, &["status", "--porcelain"]).stdout.is_empty());
}

/// Strip every conflict box from `text`, keeping only the frame: the lines
/// that survive whichever way a reader resolves the conflict.
fn frame_lines(text: &str) -> Vec<&str> {
    let mut inside = false;
    let mut out = Vec::new();
    for line in text.lines() {
        if line.starts_with("<<<<<<<") {
            inside = true;
        } else if line.starts_with(">>>>>>>") {
            inside = false;
        } else if !inside {
            out.push(line);
        }
    }
    out
}

/// A resolution that takes one side of every box, markers removed.
fn take_side(text: &str, ours: bool) -> String {
    #[derive(PartialEq)]
    enum Z {
        Frame,
        Ours,
        Base,
        Theirs,
    }
    let mut z = Z::Frame;
    let mut out = String::new();
    for line in text.lines() {
        if line.starts_with("<<<<<<<") {
            z = Z::Ours;
            continue;
        }
        if line.starts_with("|||||||") && z != Z::Frame {
            z = Z::Base;
            continue;
        }
        if line.starts_with("=======") && z != Z::Frame {
            z = Z::Theirs;
            continue;
        }
        if line.starts_with(">>>>>>>") {
            z = Z::Frame;
            continue;
        }
        // The enhanced box's `refused_by:` line opens the ours side and is
        // part of the box's furniture; weave's own two-button resolution drops
        // it, so this one does too.
        if z == Z::Ours && line.contains("refused_by: ") {
            continue;
        }
        let keep = match z {
            Z::Frame => true,
            Z::Ours => ours,
            Z::Theirs => !ours,
            Z::Base => false,
        };
        if keep {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// The driver's `weave explain` hint must never land OUTSIDE a conflict
/// region. It used to be a trailing comment line after the last box, so
/// resolving the markers left it behind and it was committed as source.
#[test]
fn the_explain_hint_never_lands_outside_the_conflict_markers() {
    let base = "function alpha() {\n  return 0\n}\n\nfunction beta() {\n  return 10\n}\n";
    let ours = "function alpha() {\n  return 1\n}\n\nfunction beta() {\n  return 11\n}\n";
    let theirs = "function alpha() {\n  return 2\n}\n\nfunction beta() {\n  return 12\n}\n";
    let driver_bin = env!("CARGO_BIN_EXE_weave-driver");

    for standard in [false, true] {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();
        fs::write(dir.join("base.ts"), base).unwrap();
        fs::write(dir.join("ours.ts"), ours).unwrap();
        fs::write(dir.join("theirs.ts"), theirs).unwrap();
        let mut cmd = Command::new(driver_bin);
        cmd.current_dir(dir)
            .env("WEAVE_STATS", "0")
            .args(["base.ts", "ours.ts", "theirs.ts"]);
        if standard {
            cmd.args(["-l", "7", "-p", "src/widget.ts"]);
        } else {
            cmd.args(["7", "src/widget.ts"]);
        }
        let out = cmd.output().expect("driver runs");
        assert_eq!(out.status.code(), Some(1), "this merge must conflict");
        let merged = fs::read_to_string(dir.join("ours.ts")).unwrap();
        assert!(merged.contains("<<<<<<<"), "{merged}");

        // Every frame line is a line one of the inputs wrote.
        for line in frame_lines(&merged) {
            assert!(
                !line.contains("weave explain") && !line.contains("weave: run"),
                "hint text outside the markers (standard={standard}):\n{merged}"
            );
            assert!(
                base.lines()
                    .chain(ours.lines())
                    .chain(theirs.lines())
                    .any(|l| l == line),
                "frame line `{line}` is in no input (standard={standard}):\n{merged}"
            );
        }
        // Either one-sided resolution is exactly that side: nothing weave wrote
        // survives the markers.
        assert_eq!(take_side(&merged, true), ours, "standard={standard}");
        assert_eq!(take_side(&merged, false), theirs, "standard={standard}");
        // The hint is still delivered: on a marker line, and on stderr.
        assert!(
            merged
                .lines()
                .any(|l| l.starts_with(">>>>>>>") && l.contains("weave explain src/widget.ts")),
            "{merged}"
        );
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("weave explain src/widget.ts"),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
