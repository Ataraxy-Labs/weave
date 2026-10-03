//! The post-merge key invariant: no merge path may write a `case` label, a
//! map / object / dict key or a struct-literal field twice into one container
//! when neither side states it twice.
//!
//! A multi-agent run pushed a broken main twice this way: two features each
//! added `case 3:` (different bodies) to the OpCreate `switch arg` in expr's
//! `vm/vm.go`. `git merge-file` merges that line-cleanly, the
//! entity merge's fallback took git's answer, and `case 3:` is too short a
//! line for the line rules to count. The fixtures are the real three stages
//! of both fatal merges (expr is MIT-licensed; see
//! `tests/fixtures/expr-vm/LICENSE-expr`).

use weave_core::entity_merge;

fn fixture(name: &str) -> String {
    let p = format!("{}/tests/fixtures/expr-vm/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{p}: {e}"))
}

fn git_merge_file(base: &str, ours: &str, theirs: &str) -> (bool, String) {
    let dir = std::env::temp_dir().join(format!(
        "weave-dupkeys-{}-{}",
        std::process::id(),
        base.len()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    for (n, t) in [("o", ours), ("b", base), ("t", theirs)] {
        std::fs::write(dir.join(n), t).unwrap();
    }
    let out = std::process::Command::new("git")
        .args(["merge-file", "-p", "o", "b", "t"])
        .current_dir(&dir)
        .output()
        .expect("git merge-file");
    let _ = std::fs::remove_dir_all(&dir);
    (out.status.success(), String::from_utf8(out.stdout).unwrap())
}

fn assert_refused(base: &str, ours: &str, theirs: &str, path: &str) {
    for (a, b) in [(ours, theirs), (theirs, ours)] {
        let r = entity_merge(base, a, b, path);
        assert!(
            !r.is_clean(),
            "{path}: a merge that states one key twice must not be clean:\n{}",
            r.content
        );
        assert!(
            r.conflicts
                .iter()
                .any(|c| c.entity_name.contains("keys:") || c.entity_name.contains("arm")),
            "{path}: the refusal names the key: {:?}",
            r.conflicts
                .iter()
                .map(|c| &c.entity_name)
                .collect::<Vec<_>>()
        );
    }
}

fn assert_clean(base: &str, ours: &str, theirs: &str, path: &str) {
    let r = entity_merge(base, ours, theirs, path);
    assert!(
        r.is_clean(),
        "{path}: expected a clean merge:\n{}",
        r.content
    );
}

#[test]
fn expr_vm_a_two_features_each_add_case_3_to_one_switch() {
    let (b, o, t) = (
        fixture("a.base.go.txt"),
        fixture("a.ours.go.txt"),
        fixture("a.theirs.go.txt"),
    );
    // git merges it line-cleanly, and that answer does not compile
    let (clean, merged) = git_merge_file(&b, &o, &t);
    assert!(clean);
    assert_eq!(
        merged.matches("\t\t\tcase 3:\n").count(),
        3,
        "two in one switch, one elsewhere"
    );
    assert_refused(&b, &o, &t, "vm/vm.go");
}

#[test]
fn expr_vm_b_two_features_each_add_case_3_to_one_switch() {
    let (b, o, t) = (
        fixture("b.base.go.txt"),
        fixture("b.ours.go.txt"),
        fixture("b.theirs.go.txt"),
    );
    assert!(git_merge_file(&b, &o, &t).0);
    assert_refused(&b, &o, &t, "vm/vm.go");
}

#[test]
fn go_switch_case_added_by_both_sides() {
    let base = "package p\n\nfunc f(x int) int {\n\tswitch x {\n\tcase 1:\n\t\treturn 10\n\tcase 2:\n\t\treturn 20\n\t}\n\treturn 0\n}\n";
    let ours = "package p\n\nfunc f(x int) int {\n\tswitch x {\n\tcase 1:\n\t\treturn 10\n\tcase 3:\n\t\treturn 31\n\tcase 2:\n\t\treturn 20\n\t}\n\treturn 0\n}\n";
    let theirs = "package p\n\nfunc f(x int) int {\n\tswitch x {\n\tcase 1:\n\t\treturn 10\n\tcase 2:\n\t\treturn 20\n\tcase 3:\n\t\treturn 32\n\t}\n\treturn 0\n}\n";
    assert_refused(base, ours, theirs, "p.go");
}

#[test]
fn go_map_literal_key_added_by_both_sides() {
    let base = "package p\n\nvar m = map[string]int{\n\t\"a\": 1,\n\t\"b\": 2,\n\t\"c\": 3,\n}\n";
    let ours = "package p\n\nvar m = map[string]int{\n\t\"a\": 1,\n\t\"x\": 10,\n\t\"b\": 2,\n\t\"c\": 3,\n}\n";
    let theirs = "package p\n\nvar m = map[string]int{\n\t\"a\": 1,\n\t\"b\": 2,\n\t\"c\": 3,\n\t\"x\": 20,\n}\n";
    assert_refused(base, ours, theirs, "p.go");
}

#[test]
fn js_switch_case_and_object_key_added_by_both_sides() {
    let base = "function f(x) {\n  switch (x) {\n    case 1:\n      return 'one';\n    case 2:\n      return 'two';\n  }\n}\n";
    let ours = "function f(x) {\n  switch (x) {\n    case 1:\n      return 'one';\n    case 9:\n      return 'nine';\n    case 2:\n      return 'two';\n  }\n}\n";
    let theirs = "function f(x) {\n  switch (x) {\n    case 1:\n      return 'one';\n    case 2:\n      return 'two';\n    case 9:\n      return 'NINE';\n  }\n}\n";
    assert_refused(base, ours, theirs, "f.js");

    let base = "const codes = {\n  D1: 'first message',\n  D2: 'second message',\n  D3: 'third message',\n};\n";
    let ours = "const codes = {\n  D1: 'first message',\n  D9: 'from ours',\n  D2: 'second message',\n  D3: 'third message',\n};\n";
    let theirs = "const codes = {\n  D1: 'first message',\n  D2: 'second message',\n  D3: 'third message',\n  \"D9\": 'from theirs',\n};\n";
    assert_refused(base, ours, theirs, "codes.js");
}

#[test]
fn python_dict_key_added_by_both_sides() {
    let base = "TABLE = {\n    'a': 1,\n    'b': 2,\n    'c': 3,\n}\n";
    let ours = "TABLE = {\n    'a': 1,\n    'z': 26,\n    'b': 2,\n    'c': 3,\n}\n";
    let theirs = "TABLE = {\n    'a': 1,\n    'b': 2,\n    'c': 3,\n    \"z\": 0,\n}\n";
    assert_refused(base, ours, theirs, "t.py");
}

#[test]
fn distinct_keys_and_one_key_in_two_switches_stay_clean() {
    // each side adds `case 3:`, but to a different switch: two keys
    let base = "package p\n\nfunc f(x, y int) int {\n\tswitch x {\n\tcase 1:\n\t\treturn 10\n\t}\n\tswitch y {\n\tcase 1:\n\t\treturn 11\n\t}\n\treturn 0\n}\n";
    let ours = "package p\n\nfunc f(x, y int) int {\n\tswitch x {\n\tcase 1:\n\t\treturn 10\n\tcase 3:\n\t\treturn 30\n\t}\n\tswitch y {\n\tcase 1:\n\t\treturn 11\n\t}\n\treturn 0\n}\n";
    let theirs = "package p\n\nfunc f(x, y int) int {\n\tswitch x {\n\tcase 1:\n\t\treturn 10\n\t}\n\tswitch y {\n\tcase 1:\n\t\treturn 11\n\tcase 3:\n\t\treturn 31\n\t}\n\treturn 0\n}\n";
    assert_clean(base, ours, theirs, "p.go");

    // different keys into one map
    let base = "package p\n\nvar m = map[string]int{\n\t\"a\": 1,\n\t\"b\": 2,\n\t\"c\": 3,\n}\n";
    let ours = "package p\n\nvar m = map[string]int{\n\t\"a\": 1,\n\t\"x\": 10,\n\t\"b\": 2,\n\t\"c\": 3,\n}\n";
    let theirs = "package p\n\nvar m = map[string]int{\n\t\"a\": 1,\n\t\"b\": 2,\n\t\"c\": 3,\n\t\"y\": 20,\n}\n";
    assert_clean(base, ours, theirs, "p.go");
}

#[test]
fn a_duplicate_one_side_already_has_is_not_the_merges() {
    // JS tolerates a repeated key; ours already repeats `D1`. Theirs adds an
    // unrelated key. The merge introduces nothing.
    let base = "const codes = {\n  D1: 'first message',\n  D2: 'second message',\n  D3: 'third message',\n};\n";
    let ours = "const codes = {\n  D1: 'first message',\n  D2: 'second message',\n  D1: 'again',\n  D3: 'third message',\n};\n";
    let theirs = "const codes = {\n  D1: 'first message',\n  D2: 'second message',\n  D3: 'third message',\n  D4: 'fourth message',\n};\n";
    assert_clean(base, ours, theirs, "codes.js");
}
