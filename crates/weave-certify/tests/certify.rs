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
