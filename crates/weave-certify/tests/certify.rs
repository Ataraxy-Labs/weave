//! Synthetic witnesses for the merge certificate. No real-repo content.

use sem_core::parser::plugins::create_default_registry;
use weave_certify::*;

struct Run {
    hard: Vec<(String, String)>,
    both: Vec<(String, Vec<(&'static str, &'static str)>)>,
    built: Option<String>,
}

fn run(o: Option<&str>, a: &str, b: &str, m: &str) -> Run {
    let reg = create_default_registry();
    let d = |t: &str| decompose(&reg, "x.rs", &normalize(t));
    let (o, a, b, m) = (o.map(d), d(a), d(b), d(m));
    let r = check("x.rs", o.as_ref(), Some(&a), Some(&b), &m);
    let built = construct(o.as_ref(), Some(&a), Some(&b), &[]);
    Run { hard: r.hard, both: r.both, built }
}

impl Run {
    fn v(&self, i: usize, allowance: &str) -> &'static str {
        self.both[i].1.iter().find(|(n, _)| *n == allowance).unwrap().1
    }
}

#[test]
fn nest_admits_edits_to_different_methods_of_one_class_only() {
    let base = "class C {\n    f() {\n        return 1;\n    }\n\n    g() {\n        return 2;\n    }\n}\n";
    let reg = create_default_registry();
    let d = |t: &str| decompose(&reg, "x.ts", t);
    let a = base.replace("return 1", "return 10");
    let b = base.replace("return 2", "return 20");
    let m = base.replace("return 1", "return 10").replace("return 2", "return 20");
    let (vo, va, vb, vm) = (d(base), d(&a), d(&b), d(&m));
    let r = check("x.ts", Some(&vo), Some(&va), Some(&vb), &vm);
    assert!(r.hard.is_empty());
    assert!(r.certified(&["nest"]), "{:?}", r.both);
    assert_eq!(construct(Some(&vo), Some(&va), Some(&vb), &["nest"]).as_deref(), Some(m.as_str()));
    // both sides edit the same method on different lines: diff3 would admit, nest must not
    let base2 = "class C {\n    f() {\n        let a = 1;\n        let b = 2;\n        let c = 3;\n    }\n}\n";
    let a2 = base2.replace("a = 1", "a = 10");
    let b2 = base2.replace("c = 3", "c = 30");
    let m2 = base2.replace("a = 1", "a = 10").replace("c = 3", "c = 30");
    let r = check("x.ts", Some(&d(base2)), Some(&d(&a2)), Some(&d(&b2)), &d(&m2));
    assert!(r.certified(&["diff3"]));
    assert!(!r.certified(&["nest"]));
}

#[test]
fn union_admits_disjoint_joint_insertions_only() {
    let o = "a\nb\n";
    let a = "a\nx\nb\n";
    let b = "a\ny\nb\n";
    assert_eq!(diff3(o, a, b), None);
    assert_eq!(union(o, a, b, true).as_deref(), Some("a\nx\ny\nb\n"));
    assert_eq!(union(o, a, b, false).as_deref(), Some("a\ny\nx\nb\n"));
    // a shared line: near-duplicate insertion is not a union
    assert_eq!(union(o, "a\nx\nz\nb\n", "a\nz\nb\n", true), None);
    // an edit, not a pure insertion
    assert_eq!(union(o, "a2\nx\nb\n", "a\ny\nb\n", true), None);
}

#[test]
fn subsume_requires_containment() {
    let o = "a\nb\nc\n";
    let a = "a\nb2\nc\n";
    let b = "a\nb2\nc\nd\n";
    assert_eq!(subsume(o, a, b, false), Some(b));
    assert_eq!(subsume(o, a, b, true), Some(b));
    // b also deletes a base line a kept: contained, but not insertions-only
    let b2 = "a\nb2\nd\n";
    assert_eq!(subsume(o, a, b2, false), Some(b2));
    assert_eq!(subsume(o, a, b2, true), None);
    assert_eq!(subsume(o, "a\nb3\nc\n", b, false), None);
}

const BASE: &str = "use a::X;\n\nfn f() {\n    1\n}\n\nfn g() {\n    2\n}\n";

#[test]
fn disjoint_entity_edits_certify_and_construct() {
    let a = BASE.replace("    1\n", "    10\n");
    let b = BASE.replace("    2\n", "    20\n");
    let m = BASE.replace("    1\n", "    10\n").replace("    2\n", "    20\n");
    let r = run(Some(BASE), &a, &b, &m);
    assert!(r.hard.is_empty() && r.both.is_empty());
    assert_eq!(r.built.as_deref(), Some(m.as_str()));
}

#[test]
fn dropping_one_side_is_rejected() {
    let a = BASE.replace("    1\n", "    10\n");
    let b = BASE.replace("    2\n", "    20\n");
    let r = run(Some(BASE), &a, &b, &a);
    assert_eq!(r.hard.len(), 1);
    assert_eq!(r.hard[0].0, "mismatch");
}

#[test]
fn same_line_dual_edit_is_both_changed_and_diff3_conflicts() {
    let a = BASE.replace("    1\n", "    10\n");
    let b = BASE.replace("    1\n", "    11\n");
    let r = run(Some(BASE), &a, &b, &a);
    assert_eq!(r.both.len(), 1);
    assert_eq!(r.v(0, "diff3"), "conflict");
    assert!(r.built.is_none());
}

#[test]
fn line_disjoint_intra_entity_edits_are_admitted_only_by_diff3() {
    let base = "fn f() {\n    let a = 1;\n    let b = 2;\n    let c = 3;\n}\n";
    let a = base.replace("a = 1", "a = 10");
    let b = base.replace("c = 3", "c = 30");
    let m = base.replace("a = 1", "a = 10").replace("c = 3", "c = 30");
    let r = run(Some(base), &a, &b, &m);
    assert!(r.hard.is_empty());
    assert_eq!(r.both.len(), 1);
    assert_eq!(r.v(0, "diff3"), "admit");
    // adjacent lines conflict, as in git merge-file
    let b2 = base.replace("b = 2", "b = 20");
    let r = run(Some(base), &a, &b2, &m);
    assert_eq!(r.v(0, "diff3"), "conflict");
}

#[test]
fn bom_is_compared() {
    let a = BASE.replace("use a::X;", "use a::Y;");
    let b = format!("\u{feff}{BASE}");
    let r = run(Some(BASE), &a, &b, &a);
    assert!(!r.both.is_empty() || !r.hard.is_empty());
}

#[test]
fn trailing_newline_is_compared() {
    let a = BASE.replace("    1\n", "    10\n");
    let b = BASE.trim_end().to_string();
    let m = a.clone();
    let r = run(Some(BASE), &a, &b, &m);
    assert!(!r.hard.is_empty());
}

#[test]
fn crlf_is_normalised() {
    let a = BASE.replace("    1\n", "    10\n");
    let r = run(Some(BASE), &a, BASE, &a.replace('\n', "\r\n"));
    assert!(r.hard.is_empty() && r.both.is_empty());
}

#[test]
fn joint_insertions_at_one_point_do_not_conflict() {
    let a = BASE.replace("fn g()", "fn p() {\n    3\n}\n\nfn g()");
    let b = BASE.replace("fn g()", "fn q() {\n    4\n}\n\nfn g()");
    let m = BASE.replace("fn g()", "fn q() {\n    4\n}\n\nfn p() {\n    3\n}\n\nfn g()");
    let r = run(Some(BASE), &a, &b, &m);
    assert!(r.hard.is_empty() && r.both.is_empty(), "{:?} {:?}", r.hard, r.both);
    assert_eq!(r.built.as_deref(), Some(m.as_str()));
}

#[test]
fn duplicated_entity_is_rejected() {
    let a = BASE.replace("fn g()", "fn p() {\n    3\n}\n\nfn g()");
    let m = a.replace("fn g()", "fn p() {\n    3\n}\n\nfn g()");
    let r = run(Some(BASE), &a, BASE, &m);
    assert!(r.hard.iter().any(|h| h.0 == "unexpected"));
}

#[test]
fn reorder_against_both_sides_is_rejected() {
    let m = "use a::X;\n\nfn g() {\n    2\n}\n\nfn f() {\n    1\n}\n";
    let r = run(Some(BASE), BASE, BASE, m);
    assert!(r.hard.iter().any(|h| h.0 == "order" || h.0 == "mismatch"));
}

#[test]
fn import_set_union() {
    let o = "use a::X;\nuse a::Y;\n";
    let a = "use a::X;\nuse a::Y;\nuse b::P;\n";
    let b = "use a::X;\nuse a::Y;\nuse c::Q;\n";
    let m = "use a::X;\nuse a::Y;\nuse b::P;\nuse c::Q;\n";
    assert_eq!(import_set("x.rs", o, a, b, m, true), "admit");
    // a name bound twice
    let b2 = "use a::X;\nuse a::Y;\nuse c::P;\n";
    let m2 = "use a::X;\nuse a::Y;\nuse b::P;\nuse c::P;\n";
    assert_eq!(import_set("x.rs", o, a, b2, m2, true), "mismatch");
    // both sides replace the same line differently: strict declines, loose admits
    let a3 = "use a::X;\nuse n::Y;\n";
    let b3 = "use a::X;\nuse a::Z;\n";
    let m3 = "use a::X;\nuse n::Y;\nuse a::Z;\n";
    assert_eq!(import_set("x.rs", o, a3, b3, m3, true), "decline");
    assert_eq!(import_set("x.rs", o, a3, b3, m3, false), "admit");
    // a non-import change declines
    let a4 = "use a::X;\nuse a::Y;\nfn z() {}\n";
    assert_eq!(import_set("x.rs", o, a4, b, m, true), "decline");
}

#[test]
fn bound_names_by_language() {
    assert_eq!(bound_names("x.rs", "use a::{B, c::D as E, self};"), Some(vec!["B".into(), "E".into(), "a".into()]));
    assert_eq!(bound_names("x.py", "from m import a, b as c"), Some(vec!["a".into(), "c".into()]));
    assert_eq!(bound_names("x.ts", "import D, { a, b as c } from 'm';"), Some(vec!["D".into(), "a".into(), "c".into()]));
    assert_eq!(bound_names("x.java", "import p.q.R;"), Some(vec!["R".into()]));
    assert_eq!(bound_names("x.cs", "using System.Net;"), Some(vec![]));
}

#[test]
fn no_base_add_add_needs_agreement() {
    let a = "fn f() {\n    1\n}\n";
    let b = "fn f() {\n    2\n}\n";
    let r = run(None, a, b, a);
    assert_eq!(r.both.len(), 1);
    let r = run(None, a, a, a);
    assert!(r.hard.is_empty() && r.both.is_empty());
}

// ------------------------------------------------------------------ imp_used

fn run_at(path: &str, o: &str, a: &str, b: &str, m: &str) -> Report {
    let reg = create_default_registry();
    let d = |t: &str| decompose(&reg, path, t);
    check(path, Some(&d(o)), Some(&d(a)), Some(&d(b)), &d(m))
}

fn verdict_at(r: &Report, key: &str, allowance: &str) -> &'static str {
    let (_, v) = r.both.iter().find(|(k, _)| k == key).unwrap_or_else(|| panic!("no both-changed {key}: {:?}", r.both));
    v.iter().find(|(n, _)| *n == allowance).unwrap().1
}

// A test file migrated two ways at once. Theirs keeps the old attribute names and
// adds alias imports for them; ours drops a namespace import and rewrites the
// class body (another region) to new attributes. The merge keeps theirs' aliases
// and ours' body: the aliases are dead. imp_strict admits the header; imp_used
// must not.
const CS_BASE: &str = "using OldFramework;\nusing Shop.Core;\n\nnamespace Shop.Tests;\n\n[Fixture]\npublic class CartTests\n{\n    [Check]\n    public void empty_cart_has_no_total()\n    {\n        Assert.Equal(0, new Cart().Total);\n    }\n}\n";

fn cs_theirs() -> String {
    CS_BASE.replace(
        "using OldFramework;\n",
        "using OldFramework;\nusing FixtureAttribute = Other.Runner.ClassAttribute;\nusing CheckAttribute = Other.Runner.MethodAttribute;\n",
    )
}

#[test]
fn imp_used_rejects_alias_imports_orphaned_by_a_body_rewrite() {
    let theirs = cs_theirs();
    let body_new = "[Serializable]\npublic class CartTests\n{\n    [Fact]\n    public void empty_cart_has_no_total()\n    {\n        new Cart().Total.ShouldBe(0);\n    }\n}\n";
    let ours = format!("using Shop.Core;\n\nnamespace Shop.Tests;\n\n{body_new}");
    let merged = format!(
        "using FixtureAttribute = Other.Runner.ClassAttribute;\nusing CheckAttribute = Other.Runner.MethodAttribute;\nusing Shop.Core;\n\nnamespace Shop.Tests;\n\n{body_new}"
    );
    let r = run_at("t.cs", CS_BASE, &ours, &theirs, &merged);
    assert!(r.hard.is_empty(), "{:?}", r.hard);
    assert_eq!(verdict_at(&r, "^", "imp_strict"), "admit", "the strict import rule admits it");
    assert_eq!(verdict_at(&r, "^", "imp_used"), "conflict");
    assert!(r.certified(&["imp_strict", "subsume_ins", "nest"]));
    assert!(!r.certified(&["imp_used", "subsume_ins", "nest"]));
}

#[test]
fn imp_used_admits_aliases_still_used_through_the_attribute_stem() {
    // control: ours changes only a method body, so [Fixture]/[Check] survive in M
    let theirs = cs_theirs();
    let ours = CS_BASE.replace("using OldFramework;\n", "").replace("Assert.Equal(0, new Cart().Total);", "Assert.Equal(0m, new Cart().Total);");
    let merged = theirs.replace("using OldFramework;\n", "").replace("Assert.Equal(0, new Cart().Total);", "Assert.Equal(0m, new Cart().Total);");
    let r = run_at("t.cs", CS_BASE, &ours, &theirs, &merged);
    assert!(r.hard.is_empty(), "{:?}", r.hard);
    assert_eq!(verdict_at(&r, "^", "imp_strict"), "admit");
    assert_eq!(verdict_at(&r, "^", "imp_used"), "admit");
    assert!(r.certified(&["imp_used", "subsume_ins", "nest"]));
}

#[test]
fn imp_used_declines_imports_that_bind_no_name() {
    // an added namespace `using` binds nothing it can check: decline, not admit
    let theirs = CS_BASE.replace("using Shop.Core;\n", "using Shop.Core;\nusing Other.Runner;\n");
    let ours = CS_BASE.replace("using OldFramework;\n", "");
    let merged = theirs.replace("using OldFramework;\n", "");
    let r = run_at("t.cs", CS_BASE, &ours, &theirs, &merged);
    assert_eq!(verdict_at(&r, "^", "imp_strict"), "admit");
    assert_eq!(verdict_at(&r, "^", "imp_used"), "decline");
}

const RS_BASE: &str = "use a::X;\nuse a::Y;\n\nfn f() -> u32 {\n    X + Y\n}\n\nfn g() -> u32 {\n    2\n}\n";

#[test]
fn imp_used_rust_names_used_unused_and_hidden_in_comments_or_strings() {
    let ours = RS_BASE.replace("use a::Y;\n", "use a::Y;\nuse b::P;\n");
    let theirs = RS_BASE.replace("use a::X;\n", "use a::X;\nuse c::Q;\n");
    let with = |body: &str| format!("use a::X;\nuse c::Q;\nuse a::Y;\nuse b::P;\n\nfn f() -> u32 {{\n    X + Y\n}}\n\nfn g() -> u32 {{\n    {body}\n}}\n");
    // the header's added lines P and Q: used in g's body on both sides
    let ours_u = ours.replace("    2\n", "    P::k()\n");
    let theirs_u = theirs.replace("fn f() -> u32 {\n    X + Y\n}", "fn f() -> u32 {\n    X + Y + Q\n}");
    let m = with("P::k()").replace("X + Y\n", "X + Y + Q\n");
    let r = run_at("x.rs", RS_BASE, &ours_u, &theirs_u, &m);
    assert!(r.hard.is_empty(), "{:?}", r.hard);
    assert_eq!(verdict_at(&r, "^", "imp_strict"), "admit");
    assert_eq!(verdict_at(&r, "^", "imp_used"), "admit");
    // P never referenced: imp_strict admits, imp_used rejects
    let r = run_at("x.rs", RS_BASE, &ours, &theirs_u, &with("2").replace("X + Y\n", "X + Y + Q\n"));
    assert_eq!(verdict_at(&r, "^", "imp_strict"), "admit");
    assert_eq!(verdict_at(&r, "^", "imp_used"), "conflict");
    // P only in a comment or a string: still unused
    for hide in ["// P::k()\n    2", "\"P\".len() as u32", "r#\"P\"#.len() as u32", "/* P /* nested */ P */ 2"] {
        let ours_h = ours.replace("    2\n", &format!("    {hide}\n"));
        let m = with(hide).replace("X + Y\n", "X + Y + Q\n");
        let r = run_at("x.rs", RS_BASE, &ours_h, &theirs_u, &m);
        assert!(r.hard.is_empty(), "{hide}: {:?}", r.hard);
        assert_eq!(verdict_at(&r, "^", "imp_used"), "conflict", "{hide}");
    }
}

#[test]
fn imp_used_rejects_a_removed_import_reintroduced() {
    // ours removes X; theirs duplicates it; the count rule leaves one X in M.
    // imp_strict already rejects this (theirs' order has X twice); imp_used
    // states the rule directly, so it holds whatever imp_strict's order check does.
    let o = "use a::X;\nuse a::Y;\n";
    let a = "use a::Y;\n";
    let b = "use a::X;\nuse a::Y;\nuse a::X;\n";
    let m = "use a::Y;\nuse a::X;\n";
    assert_ne!(import_set("x.rs", o, a, b, m, true), "admit");
    assert_ne!(import_used("x.rs", o, a, b, m, "use a::Y;\nuse a::X;\nfn f() { X; Y; }\n"), "admit");
    // ours removes X, theirs keeps it, and M keeps it too: one-sided removal dropped
    assert_ne!(import_used("x.rs", o, a, o, o, "use a::X;\nuse a::Y;\nfn f() { X; Y; }\n"), "admit");
    // control: the plain one-sided removal
    assert_eq!(import_used("x.rs", o, a, o, a, "use a::Y;\nfn f() { Y; }\n"), "admit");
}

#[test]
fn imp_used_by_language() {
    let cases: [(&str, &str, &str, &str, &str); 4] = [
        ("x.py", "import os\n", "import os\nimport json as j\n", "import os\nfrom m import k\n", "\ndef f():\n    return j.dumps(k)\n"),
        ("x.ts", "import a from 'a';\n", "import a from 'a';\nimport { B as C } from 'b';\n", "import a from 'a';\nimport * as ns from 'n';\n", "\nexport const f = () => <C x={ns.y} />;\n"),
        ("x.java", "import p.A;\n", "import p.A;\nimport q.B;\n", "import p.A;\nimport static r.S.c;\n", "\nclass T { B b = c(); }\n"),
        ("x.cs", "using S = p.A;\n", "using S = p.A;\nusing T = q.B;\n", "using S = p.A;\nusing U = r.C;\n", "\nclass K { T t; U u; }\n"),
    ];
    for (path, o, a, b, body) in cases {
        let m: String = {
            let mut v: Vec<&str> = a.lines().collect();
            v.push(b.lines().last().unwrap());
            v.join("\n") + "\n"
        };
        assert_eq!(import_set(path, o, a, b, &m, true), "admit", "{path}");
        assert_eq!(import_used(path, o, a, b, &m, &format!("{m}{body}")), "admit", "{path}");
        let c = if path.ends_with(".py") { "#" } else { "//" };
        let hidden: String = body.lines().map(|l| format!("{c} {l}\n")).collect();
        assert_eq!(import_used(path, o, a, b, &m, &format!("{m}{hidden}")), "conflict", "{path} comment only");
    }
}

#[test]
fn references_skip_comments_strings_and_prefixes() {
    let r = references("x.py", "import z\nx = f\"{a}\" + b'c'  # d\n'''e\nf'''\ng()\n");
    assert!(r.contains("x") && r.contains("g"), "{r:?}");
    for n in ["z", "a", "c", "d", "e", "f"] {
        assert!(!r.contains(n), "{n} in {r:?}");
    }
    let r = references("x.cs", "var p = @\"C:\\\"; var q = h; // i\n");
    assert!(r.contains("q") && r.contains("h") && !r.contains("C") && !r.contains("i"), "{r:?}");
    let r = references("x.rs", "fn f<'a>(s: &'a str) -> char { let c = 'x'; w }\n");
    assert!(r.contains("w") && r.contains("str") && !r.contains("x"), "{r:?}");
    let r = references("x.ts", "const t = `lit ${u}` + v; /* w */\n");
    assert!(r.contains("v") && !r.contains("lit") && !r.contains("w"), "{r:?}");
}

// ---------------------------------------------------------------- elem_union

/// Every both-changed region's `elem_union` verdict, and whether the whole
/// file is certified with it.
fn eu(path: &str, o: &str, a: &str, b: &str, m: &str) -> (Vec<&'static str>, bool) {
    let r = run_at(path, o, a, b, m);
    assert!(r.hard.is_empty(), "{path}: {:?}", r.hard);
    let v = r.both.iter().map(|(_, v)| v.iter().find(|(n, _)| *n == "elem_union").unwrap().1).collect();
    (v, r.certified(&["elem_union", "nest_eu", "nest"]))
}

const PY_CLASS: &str = "class Gen:\n    TRANSFORMS = {\n        exp.Abs: rename_func(\"ABS\"),\n        exp.IntDiv: rename_func(\"DIV\"),\n        exp.Mod: rename_func(\"MOD\"),\n    }\n\n    def f(self):\n        return 1\n";

fn py_sides() -> (String, String, String) {
    let at = "        exp.IntDiv: rename_func(\"DIV\"),\n";
    let ins = |x: &str| PY_CLASS.replace(at, &format!("{at}{x}"));
    let a = ins("        exp.IsFinite: lambda self, e: self.sql(\n            e.this\n        ),\n");
    let b = ins("        exp.TimeToUnix: rename_func(\"UNIX_SECONDS\"),\n");
    let m = ins("        exp.IsFinite: lambda self, e: self.sql(\n            e.this\n        ),\n        exp.TimeToUnix: rename_func(\"UNIX_SECONDS\"),\n");
    (a, b, m)
}

#[test]
fn elem_union_admits_two_entries_added_at_one_point_of_a_dict_in_a_class() {
    let (a, b, m) = py_sides();
    let (v, ok) = eu("gen.py", PY_CLASS, &a, &b, &m);
    assert_eq!(v, ["admit"]);
    assert!(ok);
    // the other order is a union too (a dict is a set of keys)
    let m2 = m.replace(
        "        exp.IsFinite: lambda self, e: self.sql(\n            e.this\n        ),\n        exp.TimeToUnix: rename_func(\"UNIX_SECONDS\"),\n",
        "        exp.TimeToUnix: rename_func(\"UNIX_SECONDS\"),\n        exp.IsFinite: lambda self, e: self.sql(\n            e.this\n        ),\n",
    );
    assert_eq!(eu("gen.py", PY_CLASS, &a, &b, &m2).0, ["admit"]);
    // and a method one side added elsewhere in the class does not get in the way
    let a3 = a.replace("        return 1\n", "        return 1\n\n    def g(self):\n        return 2\n");
    let m3 = m.replace("        return 1\n", "        return 1\n\n    def g(self):\n        return 2\n");
    assert_eq!(eu("gen.py", PY_CLASS, &a3, &b, &m3).0, ["admit"]);
}

#[test]
fn elem_union_refuses_a_tampered_merge() {
    let (a, b, m) = py_sides();
    let finite = "        exp.IsFinite: lambda self, e: self.sql(\n            e.this\n        ),\n";
    let unix = "        exp.TimeToUnix: rename_func(\"UNIX_SECONDS\"),\n";
    let abs = "        exp.Abs: rename_func(\"ABS\"),\n";
    let div = "        exp.IntDiv: rename_func(\"DIV\"),\n";
    let cases = [
        // one side's element removed
        ("ours dropped", m.replace(finite, "")),
        ("theirs dropped", m.replace(unix, "")),
        // base order changed
        ("base reordered", m.replace(&format!("{abs}{div}"), &format!("{div}{abs}"))),
        // a side's element not verbatim
        ("rewritten", m.replace("UNIX_SECONDS", "UNIX_MILLIS")),
        // an element nobody wrote
        ("invented", m.replace(unix, &format!("{unix}        exp.Ln: rename_func(\"LN\"),\n"))),
        // stated twice
        ("doubled", m.replace(unix, &format!("{unix}{unix}"))),
        // a base element dropped
        ("base dropped", m.replace(abs, "")),
    ];
    for (what, bad) in cases {
        let (v, ok) = eu("gen.py", PY_CLASS, &a, &b, &bad);
        assert_ne!(v, ["admit"], "{what}");
        assert!(!ok, "{what}");
    }
}

#[test]
fn elem_union_refuses_one_key_inserted_twice_differently() {
    let at = "        exp.IntDiv: rename_func(\"DIV\"),\n";
    let a = PY_CLASS.replace(at, &format!("{at}        exp.Ln: rename_func(\"LN\"),\n"));
    let b = PY_CLASS.replace(at, &format!("{at}        exp.Ln: rename_func(\"LOG\"),\n"));
    let m = PY_CLASS.replace(at, &format!("{at}        exp.Ln: rename_func(\"LN\"),\n        exp.Ln: rename_func(\"LOG\"),\n"));
    assert_ne!(eu("gen.py", PY_CLASS, &a, &b, &m).0, ["admit"]);
    // the same entry from both sides is one entry
    let same = PY_CLASS.replace(at, &format!("{at}        exp.Ln: rename_func(\"LN\"),\n"));
    let other = same.replace("        return 1\n", "        return 3\n");
    assert_eq!(eu("gen.py", PY_CLASS, &same, &other, &other).0, ["admit"], "{:?}", eu_why("gen.py", PY_CLASS, &same, &other, &other));
}

const GO_SWITCH: &str = "package vm\n\nfunc (vm *VM) Run() {\n\tfor {\n\t\tswitch op {\n\t\tcase OpPush:\n\t\t\tvm.push()\n\n\t\tcase OpEnd:\n\t\t\treturn\n\t\t}\n\t}\n}\n";

#[test]
fn elem_union_go_switch_cases_and_fallthrough() {
    let ins = |x: &str| GO_SWITCH.replace("\t\tcase OpEnd:", &format!("{x}\t\tcase OpEnd:"));
    let (sl, bn) = ("\t\tcase OpShiftLeft:\n\t\t\tvm.shl()\n\n", "\t\tcase OpBitNot:\n\t\t\tvm.not()\n\n");
    let (a, b) = (ins(sl), ins(bn));
    let m = ins(&format!("{bn}{sl}"));
    assert_eq!(eu("vm/vm.go", GO_SWITCH, &a, &b, &m).0, ["admit"], "{:?}", eu_why("vm/vm.go", GO_SWITCH, &a, &b, &m));
    // a case label both sides use
    let b2 = ins("\t\tcase OpShiftLeft, OpX:\n\t\t\tvm.x()\n\n");
    let m2 = ins(&format!("{sl}\t\tcase OpShiftLeft, OpX:\n\t\t\tvm.x()\n\n"));
    assert_ne!(eu("vm/vm.go", GO_SWITCH, &a, &b2, &m2).0, ["admit"]);
    // fallthrough makes adjacency meaning
    let b3 = ins("\t\tcase OpBitNot:\n\t\t\tvm.not()\n\t\t\tfallthrough\n\n");
    let m3 = ins(&format!("{sl}\t\tcase OpBitNot:\n\t\t\tvm.not()\n\t\t\tfallthrough\n\n"));
    assert_ne!(eu("vm/vm.go", GO_SWITCH, &a, &b3, &m3).0, ["admit"]);
}

#[test]
fn elem_union_go_iota_block_only_at_the_tail() {
    let base = "package vm\n\nfunc ops() {\n\tconst (\n\t\tOpPush Opcode = iota\n\t\tOpPop\n\t\tOpEnd\n\t)\n}\n";
    let a = base.replace("\tOpEnd\n", "\tOpEnd\n\tOpA\n");
    let b = base.replace("\tOpEnd\n", "\tOpEnd\n\tOpB\n");
    let m = base.replace("\tOpEnd\n", "\tOpEnd\n\tOpA\n\tOpB\n");
    assert_eq!(eu("vm/op.go", base, &a, &b, &m).0, ["admit"], "{:?}", eu_why("vm/op.go", base, &a, &b, &m));
    let a = base.replace("\tOpEnd\n", "\tOpA\n\tOpEnd\n");
    let b = base.replace("\tOpEnd\n", "\tOpB\n\tOpEnd\n");
    let m = base.replace("\tOpEnd\n", "\tOpA\n\tOpB\n\tOpEnd\n");
    assert_ne!(eu("vm/op.go", base, &a, &b, &m).0, ["admit"]);
}

#[test]
fn elem_union_js_object_and_ordered_lists() {
    let base = "export const registry = {\n  alpha: 1,\n  omega: 9,\n};\n";
    let a = base.replace("  omega", "  beta: 2,\n  omega");
    let b = base.replace("  omega", "  gamma: 3,\n  omega");
    let m = base.replace("  omega", "  beta: 2,\n  gamma: 3,\n  omega");
    assert_eq!(eu("src/reg.js", base, &a, &b, &m).0, ["admit"]);
    // a missing comma is not a separator
    let bad = base.replace("  omega", "  beta: 2\n  gamma: 3,\n  omega");
    assert_ne!(eu("src/reg.js", base, &a, &b, &bad).0, ["admit"]);
    // an array outside a test file is ordered: both inserting at one point is not a union
    let base = "export const steps = [\n  alpha,\n  omega,\n];\n";
    let a = base.replace("  omega", "  beta,\n  omega");
    let b = base.replace("  omega", "  gamma,\n  omega");
    let m = base.replace("  omega", "  beta,\n  gamma,\n  omega");
    assert_eq!(eu("src/steps.js", base, &a, &b, &m).0, ["decline"]);
    // in a test file, a named table is
    assert_eq!(eu("test/steps.test.js", base, &a, &b, &m).0, ["admit"]);
}

#[test]
fn elem_union_statements_one_side_insertion_must_not_touch_the_other_side_s_edit() {
    let base = "function f() {\n  a();\n  b();\n  c();\n  d();\n}\n";
    // ours inserts after a(); theirs edits d(): apart, a union
    let a = base.replace("  b();\n", "  x();\n  b();\n");
    let b = base.replace("  d();\n", "  d(1);\n");
    let m = base.replace("  b();\n", "  x();\n  b();\n").replace("  d();\n", "  d(1);\n");
    assert_eq!(eu("src/f.js", base, &a, &b, &m).0, ["admit"]);
    // theirs edits b(), right next to ours' insertion: refused
    let b = base.replace("  b();\n", "  b(1);\n");
    let m = base.replace("  b();\n", "  x();\n  b(1);\n");
    assert_eq!(eu("src/f.js", base, &a, &b, &m).0, ["decline"]);
}

/// Why `elem_union` declines or rejects each both-changed region (debugging aid).
#[allow(dead_code)]
fn eu_why(path: &str, o: &str, a: &str, b: &str, m: &str) -> Vec<String> {
    let reg = create_default_registry();
    let d = |t: &str| decompose(&reg, path, t);
    let (o, a, b, m) = (d(o), d(a), d(b), d(m));
    let r = check(path, Some(&o), Some(&a), Some(&b), &m);
    r.both.iter().map(|(k, _)| format!("{k}: {:?}", elem_union(path, k, Some(&o), &a, &b, &m))).collect()
}

/// `ELEM_PATH=<repo path> ELEM_DIR=<dir with o a b m> cargo test -p
/// weave-certify elem_why_files -- --ignored --nocapture`: why `elem_union`
/// declines each both-changed region of a real merge.
#[test]
#[ignore]
fn elem_why_files() {
    let (path, dir) = (std::env::var("ELEM_PATH").unwrap(), std::env::var("ELEM_DIR").unwrap());
    let r = |n: &str| normalize(&std::fs::read_to_string(format!("{dir}/{n}")).unwrap());
    for line in eu_why(&path, &r("o"), &r("a"), &r("b"), &r("m")) {
        println!("{line}");
    }
    println!("whole file: {:?}", eu_file(&path, &r("o"), &r("a"), &r("b"), &r("m")));
}

#[test]
fn elem_union_leaves_a_key_base_already_states_twice_to_base() {
    // sqlglot's PostgresGenerator.TRANSFORMS states one key twice (the later
    // entry wins); two entries added elsewhere are still a union.
    let base = "class G:\n    T = {\n        exp.A: f,\n        exp.U: g,\n        exp.U: h,\n        exp.Z: z,\n    }\n";
    let ins = |x: &str| base.replace("        exp.Z: z,\n", &format!("{x}        exp.Z: z,\n"));
    let (a, b) = (ins("        exp.B: b,\n"), ins("        exp.C: c,\n"));
    let m = ins("        exp.B: b,\n        exp.C: c,\n");
    assert_eq!(eu("g.py", base, &a, &b, &m).0, ["admit"], "{:?}", eu_why("g.py", base, &a, &b, &m));
    // an insertion may not join the duplicate
    let b2 = ins("        exp.U: k,\n");
    let m2 = ins("        exp.B: b,\n        exp.U: k,\n");
    assert_ne!(eu("g.py", base, &a, &b2, &m2).0, ["admit"]);
}

fn eu_file(path: &str, o: &str, a: &str, b: &str, m: &str) -> Result<(), elem::Fail> {
    let reg = create_default_registry();
    let d = |t: &str| decompose(&reg, path, t);
    elem_union_file(path, Some(&d(o)), &d(a), &d(b), &d(m))
}

#[test]
fn elem_union_whole_file_admits_declarations_both_sides_appended() {
    // Both sides append a function at the end of a Go file: the per-region
    // certificate fails (the text after ours' new function is a region no side
    // wrote); the file as one statement list of package-scope declarations is
    // a union.
    let base = "package lib\n\nfunc a() int {\n\treturn 1\n}\n";
    let (fx, fy) = ("\nfunc x() int {\n\treturn 2\n}\n", "\nfunc y() int {\n\treturn 3\n}\n");
    let (a, b) = (format!("{base}{fx}"), format!("{base}{fy}"));
    let m = format!("{base}{fx}{fy}");
    let r = run_at("lib/lib.go", base, &a, &b, &m);
    assert!(!r.hard.is_empty(), "the per-region certificate cannot see it");
    assert_eq!(eu_file("lib/lib.go", base, &a, &b, &m), Ok(()));
    assert_eq!(eu_file("lib/lib.go", base, &a, &b, &format!("{base}{fy}{fx}")), Ok(()));
    // tampered: a side's function dropped, or base's changed
    assert!(eu_file("lib/lib.go", base, &a, &b, &a).is_err());
    assert!(eu_file("lib/lib.go", base, &a, &b, &m.replace("return 1", "return 0")).is_err());
    // one name declared by both sides, differently: refused
    let b2 = format!("{base}\nfunc x() int {{\n\treturn 9\n}}\n");
    assert!(eu_file("lib/lib.go", base, &a, &b2, &format!("{base}{fx}\nfunc x() int {{\n\treturn 9\n}}\n")).is_err());
    // statements that are not declarations, at one point: not a union
    let pb = "x = 1\n";
    assert!(eu_file("m.py", pb, "x = 1\nf()\n", "x = 1\ng()\n", "x = 1\nf()\ng()\n").is_err());
    assert_eq!(eu_file("m.py", pb, "x = 1\n\ndef f():\n    pass\n", "x = 1\n\ndef g():\n    pass\n", "x = 1\n\ndef f():\n    pass\n\ndef g():\n    pass\n"), Ok(()));
}

/// `ELEM_CASES=<dir> cargo test -p weave-certify elem_cases -- --ignored
/// --nocapture`: for each `<dir>/<case>/{path,o,a,b,m}`, whether `weave land`'s
/// certificate (v2 rule set plus elem_union / nest_eu, or the whole file)
/// admits `m`. For mutation runs over real merges.
#[test]
#[ignore]
fn elem_cases() {
    let root = std::env::var("ELEM_CASES").unwrap();
    let reg = create_default_registry();
    let mut dirs: Vec<_> = std::fs::read_dir(&root).unwrap().flatten().map(|e| e.path()).collect();
    dirs.sort();
    for d in dirs {
        let r = |n: &str| normalize(&std::fs::read_to_string(d.join(n)).unwrap());
        let path = r("path").trim().to_string();
        let v = |t: &str| decompose(&reg, &path, t);
        let (o, a, b, m) = (v(&r("o")), v(&r("a")), v(&r("b")), v(&r("m")));
        let rep = check(&path, Some(&o), Some(&a), Some(&b), &m);
        let regional = rep.certified(&["imp_used", "subsume_ins", "nest", "elem_union", "nest_eu"]);
        let admitted = regional || elem_union_file(&path, Some(&o), &a, &b, &m).is_ok();
        println!("{} {}", d.file_name().unwrap().to_string_lossy(), if admitted { "ADMIT" } else { "refuse" });
    }
}
