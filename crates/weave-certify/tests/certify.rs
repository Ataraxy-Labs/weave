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
