//! `weave land`, end to end: the real binary on synthetic merges, with a fake
//! resolver that replays canned answers and records what it was asked.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

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

fn fixture(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("weave-land-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("repo")).expect("mkdir");
    root
}

/// A repository stopped mid-merge. Each entry is `(path, base, ours, theirs)`;
/// `None` = the file does not exist on that side.
type Entry<'a> = (&'a str, Option<&'a str>, Option<&'a str>, Option<&'a str>);

fn mid_merge(name: &str, files: &[Entry]) -> PathBuf {
    let root = fixture(name);
    let repo = root.join("repo");
    git(&repo, &["init", "-q", "-b", "main"]);
    let put = |pick: &dyn Fn(&Entry) -> Option<String>| {
        for f in files {
            let p = repo.join(f.0);
            match pick(f) {
                Some(text) => {
                    std::fs::create_dir_all(p.parent().unwrap()).expect("mkdir");
                    std::fs::write(&p, text).expect("write")
                }
                None => {
                    let _ = std::fs::remove_file(&p);
                }
            }
        }
        git(&repo, &["add", "-A"]);
    };
    std::fs::write(repo.join("README"), "fixture\n").unwrap();
    put(&|f| f.1.map(str::to_string));
    git(&repo, &["commit", "-qm", "base"]);
    git(&repo, &["checkout", "-qb", "theirs"]);
    put(&|f| f.3.map(str::to_string));
    git(&repo, &["commit", "-qm", "theirs"]);
    git(&repo, &["checkout", "-q", "main"]);
    put(&|f| f.2.map(str::to_string));
    git(&repo, &["commit", "-qm", "ours"]);
    git(&repo, &["merge", "-q", "theirs"]);
    assert!(
        repo.join(".git/MERGE_HEAD").exists(),
        "fixture must stop mid-merge"
    );
    root
}

/// A resolver that answers its n-th call with `answers[n-1]` and keeps the
/// n-th request as `in.<n>`.
fn fake_resolver(root: &Path, answers: &[&str]) -> String {
    for (i, a) in answers.iter().enumerate() {
        std::fs::write(root.join(format!("answer.{}", i + 1)), a).unwrap();
    }
    let script = root.join("resolver.sh");
    std::fs::write(
        &script,
        "#!/bin/sh\nd=\"$(dirname \"$0\")\"\nn=$(ls \"$d\" | grep -c '^in\\.')\nn=$((n+1))\n\
         cat > \"$d/in.$n\"\ncat \"$d/answer.$n\"\n",
    )
    .unwrap();
    format!("sh {}", script.display())
}

fn request(root: &Path, n: usize) -> Value {
    serde_json::from_str(&std::fs::read_to_string(root.join(format!("in.{n}"))).unwrap()).unwrap()
}

fn land(root: &Path, args: &[&str]) -> (i32, Value) {
    let out = Command::new(env!("CARGO_BIN_EXE_weave"))
        .arg("land")
        .arg("--json")
        .args(args)
        .current_dir(root.join("repo"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .expect("run weave land");
    let code = out.status.code().expect("exit code");
    let json = serde_json::from_slice(&out.stdout).unwrap_or(Value::Null);
    if json.is_null() {
        eprintln!("stderr: {}", String::from_utf8_lossy(&out.stderr));
    }
    (code, json)
}

fn file<'a>(doc: &'a Value, path: &str) -> &'a Value {
    doc["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["path"] == path)
        .unwrap_or_else(|| panic!("{path} not in report: {doc:#}"))
}

fn unmerged(root: &Path) -> String {
    String::from_utf8(git(&root.join("repo"), &["ls-files", "-u"]).stdout).unwrap()
}

fn read(root: &Path, path: &str) -> String {
    std::fs::read_to_string(root.join("repo").join(path)).unwrap()
}

// Both sides edit a different method of one class, on adjacent lines: git
// conflicts, weave merges, and the certificate proves it by nesting.
const CLASS: (&str, &str, &str) = (
    "class K:\n    def a(self): return 1\n    def b(self): return 2\n",
    "class K:\n    def a(self): return 10\n    def b(self): return 2\n",
    "class K:\n    def a(self): return 1\n    def b(self): return 20\n",
);

// Both sides edit the same line: weave conflicts too.
const SAME_LINE: (&str, &str, &str) = (
    "import os\n\ndef f():\n    return os.sep\n",
    "import os\n\ndef f():\n    return os.sep + 'a'\n",
    "import os\n\ndef f():\n    return os.sep + 'b'\n",
);
const SAME_LINE_GOOD: &str = "import os\n\ndef f():\n    return os.sep + 'a' + 'b'\n";
// Drops `import os`, a line both sides kept.
const SAME_LINE_LOSSY: &str = "\ndef f():\n    return os.sep + 'a' + 'b'\n";

#[test]
fn proven_by_the_certificate() {
    let root = mid_merge(
        "proven",
        &[("m.py", Some(CLASS.0), Some(CLASS.1), Some(CLASS.2))],
    );
    let (code, doc) = land(&root, &[]);
    assert_eq!(code, 0, "{doc:#}");
    let f = file(&doc, "m.py");
    assert_eq!(f["status"], "PROVEN");
    assert_eq!(f["rule"], "certificate");
    assert_eq!(f["weave"], "clean");
    assert_eq!(f["attempts"], 0);
    assert_eq!(
        read(&root, "m.py"),
        "class K:\n    def a(self): return 10\n    def b(self): return 20\n"
    );
    assert_eq!(unmerged(&root), "", "a proven file is staged");
}

// Rule set v2 (`imp_used`): a header both sides changed, where one side's added
// alias imports are orphaned by the other side's body rewrite, is not PROVEN.
// v1's `imp_strict` admitted it. The control keeps the aliases in use.
const CS_BASE: &str = "using OldFramework;\nusing Shop.Core;\n\nnamespace Shop.Tests;\n\n[Fixture]\npublic class CartTests\n{\n    [Check]\n    public void empty_cart_has_no_total()\n    {\n        Assert.Equal(0, new Cart().Total);\n    }\n}\n";

fn cs_theirs() -> String {
    CS_BASE.replace(
        "using OldFramework;\n",
        "using OldFramework;\nusing FixtureAttribute = Other.Runner.ClassAttribute;\nusing CheckAttribute = Other.Runner.MethodAttribute;\n",
    )
}

#[test]
fn orphaned_alias_imports_are_not_proven() {
    let theirs = cs_theirs();
    let body_new = "[Serializable]\npublic class CartTests\n{\n    [Fact]\n    public void empty_cart_has_no_total()\n    {\n        new Cart().Total.ShouldBe(0);\n    }\n}\n";
    let ours = format!("using Shop.Core;\n\nnamespace Shop.Tests;\n\n{body_new}");
    let root = mid_merge(
        "orphaned-alias",
        &[("t.cs", Some(CS_BASE), Some(&ours), Some(&theirs))],
    );
    let (code, doc) = land(&root, &[]);
    assert_eq!(code, 1, "{doc:#}");
    let f = file(&doc, "t.cs");
    assert_eq!(f["status"], "REFUSED", "{f:#}");
    if f["weave"] == "clean" {
        assert!(
            f["reason"].as_str().unwrap().contains("not certified"),
            "{f:#}"
        );
    }
    assert_eq!(doc["proof_allowances"][0], "imp_used");
}

#[test]
fn alias_imports_still_used_are_proven() {
    let theirs = cs_theirs();
    let edit = |t: &str| {
        t.replace("using OldFramework;\n", "").replace(
            "Assert.Equal(0, new Cart().Total);",
            "Assert.Equal(0m, new Cart().Total);",
        )
    };
    let ours = edit(CS_BASE);
    let root = mid_merge(
        "used-alias",
        &[("t.cs", Some(CS_BASE), Some(&ours), Some(&theirs))],
    );
    let (code, doc) = land(&root, &[]);
    let f = file(&doc, "t.cs");
    assert_eq!(code, 0, "{doc:#}");
    assert_eq!(f["status"], "PROVEN", "{f:#}");
    assert_eq!(read(&root, "t.cs"), edit(&theirs));
}

#[test]
fn verified_by_the_gate() {
    let root = mid_merge(
        "verified",
        &[(
            "m.py",
            Some(SAME_LINE.0),
            Some(SAME_LINE.1),
            Some(SAME_LINE.2),
        )],
    );
    // No trailing newline: the answer follows the sides' convention.
    let resolver = fake_resolver(&root, &[SAME_LINE_GOOD.trim_end()]);
    let (code, doc) = land(&root, &["--resolver", &resolver]);
    assert_eq!(code, 0, "{doc:#}");
    let f = file(&doc, "m.py");
    assert_eq!(f["status"], "VERIFIED");
    assert_eq!(f["rule"], "gate");
    assert_eq!(f["weave"], "conflicted");
    assert_eq!(f["attempts"], 1);
    assert_eq!(read(&root, "m.py"), SAME_LINE_GOOD);
    assert_eq!(unmerged(&root), "");
    let req = request(&root, 1);
    assert_eq!(req["path"], "m.py");
    assert_eq!(req["kind"], "content");
    assert_eq!(req["base"], SAME_LINE.0);
    assert_eq!(req["ours"], SAME_LINE.1);
    assert_eq!(req["theirs"], SAME_LINE.2);
    assert!(req["conflicted"].as_str().unwrap().contains("<<<<<<< ours"));
    assert_eq!(req["attempt"], 1);
    assert_eq!(req["previous"], Value::Null);
}

#[test]
fn a_clean_but_unproven_weave_merge_goes_to_the_resolver() {
    // Each side appends a different class at the end: weave merges it, but
    // the certificate does not admit weave's placement of the gap after them,
    // and a class is not a declaration whose order `elem_union` may ignore
    // (its body runs where it stands).
    let base = "class A:\n    x = 1\n\n\nclass B:\n    x = 2\n";
    let ours = "class A:\n    x = 1\n\n\nclass B:\n    x = 2\n\n\nclass D:\n    x = 4\n";
    let theirs = "class A:\n    x = 1\n\n\nclass B:\n    x = 2\n\n\nclass C:\n    x = 3\n";
    let root = mid_merge(
        "unproven",
        &[("m.py", Some(base), Some(ours), Some(theirs))],
    );
    let good = "class A:\n    x = 1\n\n\nclass B:\n    x = 2\n\n\nclass D:\n    x = 4\n\n\nclass C:\n    x = 3\n";
    let resolver = fake_resolver(&root, &[good]);
    let (code, doc) = land(&root, &["--resolver", &resolver]);
    assert_eq!(code, 0, "{doc:#}");
    let f = file(&doc, "m.py");
    assert_eq!(f["weave"], "clean");
    assert_eq!(f["status"], "VERIFIED");
    assert_eq!(f["rule"], "gate");
    assert!(
        f["reason"].as_str().unwrap().contains("not certified"),
        "{f:#}"
    );
    assert_eq!(read(&root, "m.py"), good);
}

#[test]
fn functions_both_sides_appended_land_as_a_whole_file_union() {
    // The same with functions: the file read as one statement list of
    // declarations is a union, so no resolver is asked.
    let base = "def a():\n    return 1\n\n\ndef b():\n    return 2\n";
    let ours = "def a():\n    return 1\n\n\ndef b():\n    return 2\n\n\ndef d():\n    return 4\n";
    let theirs = "def a():\n    return 1\n\n\ndef b():\n    return 2\n\n\ndef c():\n    return 3\n";
    let root = mid_merge(
        "whole-file-union",
        &[("m.py", Some(base), Some(ours), Some(theirs))],
    );
    let resolver = fake_resolver(&root, &[ours]);
    let (code, doc) = land(&root, &["--resolver", &resolver]);
    assert_eq!(code, 0, "{doc:#}");
    let f = file(&doc, "m.py");
    assert_eq!((f["status"].as_str(), f["rule"].as_str(), f["attempts"].as_u64()), (Some("VERIFIED"), Some("elem_union"), Some(0)), "{f:#}");
    assert!(f["reason"].as_str().unwrap().contains("whole file"), "{f:#}");
    assert!(!root.join("in.1").exists(), "the resolver was not called");
    let landed = read(&root, "m.py");
    assert!(landed.contains("def c():") && landed.contains("def d():"), "{landed}");
}

#[test]
fn refused_after_one_retry_keeps_the_markers() {
    let root = mid_merge(
        "refused",
        &[(
            "m.py",
            Some(SAME_LINE.0),
            Some(SAME_LINE.1),
            Some(SAME_LINE.2),
        )],
    );
    // What a merge driver does with a file it resolved: stage it. A refusal
    // must put it back to unmerged all the same.
    git(&root.join("repo"), &["add", "m.py"]);
    assert_eq!(unmerged(&root), "");
    let resolver = fake_resolver(&root, &[SAME_LINE_LOSSY, SAME_LINE_LOSSY]);
    let (code, doc) = land(&root, &["--resolver", &resolver]);
    assert_eq!(code, 1, "{doc:#}");
    let f = file(&doc, "m.py");
    assert_eq!(f["status"], "REFUSED");
    assert_eq!(f["rule"], "gate");
    assert_eq!(f["attempts"], 2);
    assert_eq!(f["reasons"], serde_json::json!(["WEAVE"]));
    assert_eq!(f["findings"][0]["class"], "LOSS");
    assert!(f["findings"][0]["detail"]
        .as_str()
        .unwrap()
        .contains("import os"));

    // The retry was told why, and shown its own rejected answer.
    let retry = request(&root, 2);
    assert_eq!(retry["attempt"], 2);
    assert_eq!(retry["previous"], SAME_LINE_LOSSY);
    let told = retry["findings"].as_array().unwrap();
    assert!(
        told.iter()
            .any(|f| f.as_str().unwrap().starts_with("LOSS: ")),
        "{told:?}"
    );

    // Never written; the conflict stays.
    let on_disk = read(&root, "m.py");
    assert!(on_disk.contains("<<<<<<<"), "{on_disk}");
    assert_ne!(on_disk, SAME_LINE_LOSSY);
    assert_eq!(
        unmerged(&root).lines().count(),
        3,
        "stages 1, 2 and 3 are back"
    );
    let commit = git(&root.join("repo"), &["commit", "-qm", "merge"]);
    assert!(
        !commit.status.success(),
        "git must refuse to commit a refused file"
    );
}

#[test]
fn a_retry_that_fixes_the_findings_is_verified() {
    let root = mid_merge(
        "retry",
        &[(
            "m.py",
            Some(SAME_LINE.0),
            Some(SAME_LINE.1),
            Some(SAME_LINE.2),
        )],
    );
    let resolver = fake_resolver(&root, &[SAME_LINE_LOSSY, SAME_LINE_GOOD]);
    let (code, doc) = land(&root, &["--resolver", &resolver]);
    assert_eq!(code, 0, "{doc:#}");
    let f = file(&doc, "m.py");
    assert_eq!(f["status"], "VERIFIED");
    assert_eq!(f["attempts"], 2);
    assert_eq!(f["findings"], serde_json::json!([]));
    assert_eq!(read(&root, "m.py"), SAME_LINE_GOOD);
}

#[test]
fn markers_and_parse_errors_are_refused() {
    let root = mid_merge(
        "markers",
        &[(
            "m.py",
            Some(SAME_LINE.0),
            Some(SAME_LINE.1),
            Some(SAME_LINE.2),
        )],
    );
    let marked = "import os\n\ndef f():\n<<<<<<< ours\n    return os.sep\n";
    let broken = "import os\n\ndef f(:\n    return os.sep + 'a' + 'b'\n";
    let resolver = fake_resolver(&root, &[marked, broken]);
    let (code, doc) = land(&root, &["--resolver", &resolver]);
    assert_eq!(code, 1);
    let f = file(&doc, "m.py");
    assert!(
        f["reasons"].as_array().unwrap().contains(&"PARSE".into()),
        "{f:#}"
    );
    let first = request(&root, 2);
    let told: Vec<&str> = first["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(told.iter().any(|t| t.starts_with("MARKERS: ")), "{told:?}");
}

#[test]
fn automerged_lines_must_survive() {
    // git merges the new import automatically; an answer that loses it fails.
    let base = "import os\n\n\ndef f():\n    return 1\n";
    let ours = "import os\nimport sys\n\n\ndef f():\n    return 2\n";
    let theirs = "import os\n\n\ndef f():\n    return 3\n";
    let root = mid_merge(
        "automerged",
        &[("m.py", Some(base), Some(ours), Some(theirs))],
    );
    let dropped = "import os\n\n\ndef f():\n    return 5\n";
    let resolver = fake_resolver(&root, &[dropped, dropped]);
    let (code, doc) = land(&root, &["--resolver", &resolver]);
    assert_eq!(code, 1, "{doc:#}");
    let f = file(&doc, "m.py");
    assert!(
        f["reasons"]
            .as_array()
            .unwrap()
            .contains(&"AUTOMERGED".into()),
        "{f:#}"
    );
}

#[test]
fn a_duplicate_data_key_is_refused_through_weave_check() {
    let base = "{\n  \"a\": 1\n}\n";
    let ours = "{\n  \"a\": 1,\n  \"b\": 2\n}\n";
    let theirs = "{\n  \"a\": 1,\n  \"b\": 5\n}\n";
    let root = mid_merge(
        "dupkey",
        &[("conf.json", Some(base), Some(ours), Some(theirs))],
    );
    let twice = "{\n  \"a\": 1,\n  \"b\": 2,\n  \"b\": 5\n}\n";
    let resolver = fake_resolver(&root, &[twice, twice]);
    let (code, doc) = land(&root, &["--resolver", &resolver]);
    assert_eq!(code, 1, "{doc:#}");
    let f = file(&doc, "conf.json");
    assert_eq!(f["reasons"], serde_json::json!(["WEAVE"]), "{f:#}");
    assert!(f["findings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|x| x["class"] == "DUP" && x["detail"].as_str().unwrap().contains("key `b`")));
}

#[test]
fn a_file_weave_has_no_grammar_for_gets_the_line_rules() {
    let base = "alpha line one\nbravo line two\ncharlie three\n";
    let ours = "alpha line one\nbravo line TWO\ncharlie three\n";
    let theirs = "alpha line one\nbravo line 2\ncharlie three\n";
    let root = mid_merge(
        "lines",
        &[("notes.unknownext", Some(base), Some(ours), Some(theirs))],
    );
    let lossy = "bravo line 2\ncharlie three\n";
    let resolver = fake_resolver(&root, &[lossy, lossy]);
    let (code, doc) = land(&root, &["--resolver", &resolver]);
    assert_eq!(code, 1, "{doc:#}");
    let f = file(&doc, "notes.unknownext");
    // `lossy` is also theirs verbatim, so ours' `TWO` is dropped as well.
    assert_eq!(
        f["reasons"],
        serde_json::json!(["LINES", "DROPPED"]),
        "{f:#}"
    );
    assert_eq!(f["findings"][0]["class"], "LOSS");
    assert_eq!(f["findings"][1]["class"], "DROPPED");
    assert!(
        f["findings"][1]["detail"]
            .as_str()
            .unwrap()
            .contains("`TWO`"),
        "{f:#}"
    );
}

#[test]
fn modify_delete_keep_and_delete() {
    let base = "def keep():\n    return 1\n";
    let modified = "def keep():\n    return 2\n";
    let root = mid_merge(
        "moddel",
        &[
            ("a.py", Some(base), Some(modified), None),
            ("b.py", Some(base), None, Some(modified)),
        ],
    );
    // a.py: keep the modified side. b.py: delete it, twice — the other side's
    // edit exists nowhere else, so weave check refuses the deletion.
    let resolver = fake_resolver(&root, &["KEEP\n", "DELETE\n", "DELETE\n"]);
    let (code, doc) = land(&root, &["--resolver", &resolver]);
    assert_eq!(code, 1, "{doc:#}");
    let a = file(&doc, "a.py");
    assert_eq!(a["kind"], "modify/delete");
    assert_eq!(a["weave"], "not-run");
    assert_eq!(a["status"], "VERIFIED", "{a:#}");
    assert_eq!(read(&root, "a.py"), modified);
    let b = file(&doc, "b.py");
    assert_eq!(b["status"], "REFUSED", "{b:#}");
    assert_eq!(b["attempts"], 2);
    assert!(b["findings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["class"] == "MODDEL"));
    assert_eq!(request(&root, 3)["previous"], "DELETE");
    assert!(unmerged(&root).contains("b.py"));
}

#[test]
fn a_verified_delete_removes_the_file() {
    // Theirs only re-laid the file out; ours deleted it. Deleting is what
    // weave check's modify/delete rule accepts.
    let base = "def keep():\n    return 1\n";
    let relaid = "def keep():\n\n    return 1\n";
    let root = mid_merge("delete", &[("gone.py", Some(base), None, Some(relaid))]);
    let resolver = fake_resolver(&root, &["DELETE\n"]);
    let (code, doc) = land(&root, &["--resolver", &resolver]);
    assert_eq!(code, 0, "{doc:#}");
    let f = file(&doc, "gone.py");
    assert_eq!(f["status"], "VERIFIED", "{f:#}");
    assert_eq!(f["sha256"], Value::Null);
    assert!(!root.join("repo/gone.py").exists());
    assert_eq!(unmerged(&root), "");
    let listed = git(&root.join("repo"), &["ls-files", "--", "gone.py"]).stdout;
    assert!(listed.is_empty(), "the deletion is staged");
}

#[test]
fn cannot_and_no_resolver_are_refused() {
    let root = mid_merge(
        "cannot",
        &[(
            "m.py",
            Some(SAME_LINE.0),
            Some(SAME_LINE.1),
            Some(SAME_LINE.2),
        )],
    );
    let (code, doc) = land(&root, &["--dry-run"]);
    assert_eq!(code, 1);
    assert_eq!(file(&doc, "m.py")["rule"], "no-resolver");
    let resolver = fake_resolver(&root, &["CANNOT: the intent is unclear\n"]);
    let (code, doc) = land(&root, &["--resolver", &resolver]);
    assert_eq!(code, 1);
    let f = file(&doc, "m.py");
    assert_eq!(f["rule"], "resolver");
    assert_eq!(f["attempts"], 1, "a declined answer is not retried");
    assert!(f["reason"]
        .as_str()
        .unwrap()
        .contains("the intent is unclear"));
}

#[test]
fn binary_files_are_not_attempted() {
    let root = fixture("binary");
    let repo = root.join("repo");
    git(&repo, &["init", "-q", "-b", "main"]);
    std::fs::write(repo.join("blob.bin"), b"\x00\xff\x01base").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "base"]);
    git(&repo, &["checkout", "-qb", "theirs"]);
    std::fs::write(repo.join("blob.bin"), b"\x00\xff\x01theirs").unwrap();
    git(&repo, &["commit", "-qam", "theirs"]);
    git(&repo, &["checkout", "-q", "main"]);
    std::fs::write(repo.join("blob.bin"), b"\x00\xff\x01ours").unwrap();
    git(&repo, &["commit", "-qam", "ours"]);
    git(&repo, &["merge", "-q", "theirs"]);
    let (code, doc) = land(&root, &[]);
    assert_eq!(code, 1);
    assert_eq!(file(&doc, "blob.bin")["rule"], "not-text");
}

#[test]
fn revisions_mode_touches_nothing() {
    let root = mid_merge(
        "revisions",
        &[("m.py", Some(CLASS.0), Some(CLASS.1), Some(CLASS.2))],
    );
    let before = read(&root, "m.py");
    let (code, doc) = land(&root, &["--ours", "HEAD", "--theirs", "theirs"]);
    assert_eq!(code, 0);
    assert_eq!(doc["merge"]["mode"], "revisions");
    assert_eq!(file(&doc, "m.py")["status"], "PROVEN");
    assert_eq!(read(&root, "m.py"), before);
    assert_eq!(unmerged(&root).lines().count(), 3);
}

#[test]
fn a_resolver_that_cannot_be_run_is_an_error() {
    let root = mid_merge(
        "noresolver",
        &[(
            "m.py",
            Some(SAME_LINE.0),
            Some(SAME_LINE.1),
            Some(SAME_LINE.2),
        )],
    );
    let (code, _) = land(
        &root,
        &["--resolver", "definitely-not-a-command-weave-land"],
    );
    assert_eq!(code, 2);
    assert_eq!(unmerged(&root).lines().count(), 3, "nothing landed");
}

/// The report is a contract: these keys, no more, no fewer.
#[test]
fn json_schema_is_stable() {
    let root = mid_merge(
        "schema",
        &[
            ("m.py", Some(CLASS.0), Some(CLASS.1), Some(CLASS.2)),
            (
                "n.py",
                Some(SAME_LINE.0),
                Some(SAME_LINE.1),
                Some(SAME_LINE.2),
            ),
        ],
    );
    let cert = root.join("cert.json");
    let (_, doc) = land(
        &root,
        &["--dry-run", "--certificate", cert.to_str().unwrap()],
    );
    let keys = |v: &Value| -> Vec<String> {
        let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
        k.sort();
        k
    };
    assert_eq!(
        keys(&doc),
        [
            "files",
            "merge",
            "not_examined",
            "proof_allowances",
            "schema",
            "summary",
            "union_allowances"
        ]
    );
    assert_eq!(doc["schema"], "weave-land/1");
    assert_eq!(keys(&doc["merge"]), ["base", "mode", "ours", "theirs"]);
    assert_eq!(
        keys(&doc["summary"]),
        ["not_examined", "proven", "refused", "verified"]
    );
    assert_eq!(
        doc["proof_allowances"],
        serde_json::json!(["imp_used", "subsume_ins", "nest"])
    );
    for f in doc["files"].as_array().unwrap() {
        assert_eq!(
            keys(f),
            [
                "attempts", "findings", "kind", "path", "reason", "reasons", "rule", "sha256",
                "status", "weave"
            ]
        );
    }
    assert_eq!(doc["summary"]["proven"], 1);
    assert_eq!(doc["summary"]["refused"], 1);
    let written: Value = serde_json::from_str(&std::fs::read_to_string(&cert).unwrap()).unwrap();
    assert_eq!(written, doc, "--certificate writes the --json document");
}

// ---------------------------------------------------- one-sided resolutions
//
// On a real conflict between two reference patches
// (f04 adds `exp.IsFinite`, f07 adds `exp.TimeToUnix`, adjacent entries of one
// dict inside `BigQueryGenerator`), an answer that keeps only one side passed
// the gate as VERIFIED, silently dropping the other side's entry. The base is
// sqlglot @ d2fbb21's `sqlglot/generators/bigquery.py` (MIT), unmodified.
//
// weave now merges that conflict itself (an element union of the dict) and the
// certificate's `elem_union` check admits it, so it lands without a resolver.
// The one-sided answers are kept under test on a collection weave does not
// unite — an ordered list in product code — where the resolver is still asked.

const BQ_PATH: &str = "sqlglot/generators/bigquery.py";
const BQ_BASE: &str = include_str!("fixtures/sqlglot-d2fbb21-bigquery.py");
const BQ_ANCHOR: &str = "        exp.IntDiv: rename_func(\"DIV\"),\n";
const BQ_F04: &str = "        exp.IsFinite: lambda self, e: self.sql(\n            exp.and_(\n                exp.not_(exp.IsInf(this=e.this.copy())), exp.not_(exp.IsNan(this=e.this.copy()))\n            )\n        ),\n";
const BQ_F07: &str = "        exp.TimeToUnix: rename_func(\"UNIX_SECONDS\"),\n";

fn bq(insert: &[&str]) -> String {
    assert_eq!(BQ_BASE.matches(BQ_ANCHOR).count(), 1);
    BQ_BASE.replace(BQ_ANCHOR, &format!("{BQ_ANCHOR}{}", insert.concat()))
}

fn bq_merge(name: &str) -> PathBuf {
    let (ours, theirs) = (bq(&[BQ_F04]), bq(&[BQ_F07]));
    mid_merge(
        name,
        &[(BQ_PATH, Some(BQ_BASE), Some(&ours), Some(&theirs))],
    )
}

fn dropped(f: &Value) -> bool {
    f["reasons"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r == "DROPPED")
}

#[test]
fn sqlglot_conflict_lands_as_an_element_union_without_the_resolver() {
    for mode in [
        &[][..],
        &["--base", "HEAD~1", "--ours", "HEAD", "--theirs", "theirs"][..],
    ] {
        let root = bq_merge("union-landed");
        // a resolver that would drop theirs' entry, were it asked
        let ours = bq(&[BQ_F04]);
        let resolver = fake_resolver(&root, &[&ours, &ours]);
        let mut args = vec!["--resolver", resolver.as_str()];
        args.extend_from_slice(mode);
        let (code, doc) = land(&root, &args);
        let f = file(&doc, BQ_PATH);
        assert_eq!(code, 0, "{f:#}");
        assert_eq!(f["status"], "VERIFIED", "{f:#}");
        assert_eq!(f["rule"], "elem_union", "{f:#}");
        assert_eq!(f["weave"], "clean", "{f:#}");
        assert_eq!(f["attempts"], 0, "{f:#}");
        assert!(!root.join("in.1").exists(), "the resolver was not called");
        assert!(f["reason"].as_str().unwrap().contains("elem_union"), "{f:#}");
        assert_eq!(doc["union_allowances"], serde_json::json!(["elem_union", "nest_eu"]));
        if mode.is_empty() {
            // landed and staged, with both entries, by weave's key order
            let landed = read(&root, BQ_PATH);
            assert!(landed == bq(&[BQ_F04, BQ_F07]) || landed == bq(&[BQ_F07, BQ_F04]), "{landed}");
            assert_eq!(unmerged(&root), "");
            assert_eq!(f["sha256"].as_str().unwrap().len(), 64);
        }
    }
}

#[test]
fn a_land_with_no_resolver_lands_an_element_union() {
    let root = bq_merge("union-no-resolver");
    let (code, doc) = land(&root, &[]);
    let f = file(&doc, BQ_PATH);
    assert_eq!((code, f["status"].as_str()), (0, Some("VERIFIED")), "{f:#}");
    assert_eq!(doc["summary"]["verified"], 1);
}

// Both sides add an entry at one point of an ordered list in product code:
// not a union by weave's policy, so the resolver is asked, and a one-sided
// answer is refused exactly as the sqlglot one was.
const STEPS_PATH: &str = "pipeline/steps.py";
const STEPS_BASE: &str = "STEPS = [\n    parse_input,\n    normalise_units,\n    write_report,\n]\n";
const STEPS_OURS: &str = "STEPS = [\n    parse_input,\n    normalise_units,\n    drop_outliers,\n    write_report,\n]\n";
const STEPS_THEIRS: &str = "STEPS = [\n    parse_input,\n    normalise_units,\n    fill_gaps,\n    write_report,\n]\n";

fn steps_merge(name: &str) -> PathBuf {
    mid_merge(
        name,
        &[(STEPS_PATH, Some(STEPS_BASE), Some(STEPS_OURS), Some(STEPS_THEIRS))],
    )
}

#[test]
fn one_sided_resolution_ours_only_is_refused() {
    for mode in [
        &[][..],
        &["--base", "HEAD~1", "--ours", "HEAD", "--theirs", "theirs"][..],
    ] {
        let root = steps_merge("onesided-ours");
        let resolver = fake_resolver(&root, &[STEPS_OURS, STEPS_OURS]);
        let mut args = vec!["--resolver", resolver.as_str()];
        args.extend_from_slice(mode);
        let (code, doc) = land(&root, &args);
        let f = file(&doc, STEPS_PATH);
        assert_eq!(f["weave"], "conflicted", "{f:#}");
        assert_eq!(f["status"], "REFUSED", "{f:#}");
        assert_eq!(code, 1);
        assert!(dropped(f), "{f:#}");
        assert!(
            f["findings"].to_string().contains("`fill_gaps`"),
            "the finding names theirs' dropped line: {f:#}"
        );
    }
}

#[test]
fn one_sided_resolution_theirs_only_is_refused() {
    let root = steps_merge("onesided-theirs");
    let resolver = fake_resolver(&root, &[STEPS_THEIRS, STEPS_THEIRS]);
    let (code, doc) = land(&root, &["--resolver", &resolver]);
    let f = file(&doc, STEPS_PATH);
    assert_eq!((code, f["status"].as_str()), (1, Some("REFUSED")), "{f:#}");
    assert!(dropped(f), "{f:#}");
    assert!(f["findings"].to_string().contains("`drop_outliers`"), "{f:#}");
    // the retry is told which line is missing
    let req = request(&root, 2);
    assert!(
        req["findings"].to_string().contains("`drop_outliers`"),
        "{req:#}"
    );
}

#[test]
fn one_sided_resolution_union_is_verified_in_either_order() {
    let ins = |x: &str| STEPS_BASE.replace("    write_report,\n", &format!("{x}    write_report,\n"));
    for (i, union) in [
        ins("    drop_outliers,\n    fill_gaps,\n"),
        ins("    fill_gaps,\n    drop_outliers,\n"),
    ]
    .iter()
    .enumerate()
    {
        let root = steps_merge(&format!("onesided-union-{i}"));
        let resolver = fake_resolver(&root, &[union]);
        let (code, doc) = land(&root, &["--resolver", &resolver]);
        let f = file(&doc, STEPS_PATH);
        assert_eq!((code, f["status"].as_str()), (0, Some("VERIFIED")), "{f:#}");
        assert_eq!(f["rule"], "gate", "{f:#}");
        assert_eq!(&read(&root, STEPS_PATH), union);
    }
}
