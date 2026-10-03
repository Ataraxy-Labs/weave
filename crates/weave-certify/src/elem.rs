//! Allowance `elem_union`: a both-changed region whose merge `M` unites the
//! elements two sides inserted into one collection — a map, a switch, a const
//! block, a test table, a run of named declarations — checked from the four
//! parse trees alone. Nothing here comes from weave's merge: the recogniser,
//! the keys and the policy are restated, and the check reads `M` rather than
//! building it, so a merge that lies about what it did is caught here.
//!
//! The four subtrees (base `O`, ours `A`, theirs `B`, merge `M`) are walked in
//! parallel from the region's node:
//!
//! - a node whose three-way selection over O/A/B is defined must be exactly
//!   that in M;
//! - a node of a collection kind in the table below is read as items
//!   (elements and comments), and checked as a collection (below);
//! - any other node must have the same children, by count and kind, in all
//!   four; the text between children is selected, and the children walked.
//!
//! A collection is admitted when:
//!
//! 1. each side, aligned with base (by key where the collection is keyed, by
//!    text otherwise; a lone changed element facing a lone changed element is
//!    a modification), yields the selected set: a base item both kept is
//!    there once, as the side that changed it wrote it; one either side
//!    deleted (and the other left alone) is gone; one both changed
//!    differently is walked again, recursively; one deleted by a side and
//!    changed by the other is a refusal;
//! 2. every item a side inserted is in M VERBATIM, and an item both inserted
//!    is in M once (and both wrote it the same, else refusal — that is the
//!    key collision for a keyed collection);
//! 3. M holds those items and nothing else, and M read as a sequence is an
//!    interleaving of A's and B's sequences: each side's order is kept, so in
//!    particular base items keep base order;
//! 4. where both sides inserted at one point (between the same two shared
//!    items) the collection is a set by the policy table; where one side
//!    inserted into an order-sensitive collection, the other side left the
//!    neighbours of the insertion alone;
//! 5. no key, case label, constant or declared name is stated twice in M; a
//!    Go switch has no `fallthrough`; a JS case inserted, and the case before
//!    it, end in `break`/`return`/`throw`/`continue`;
//! 6. between items M has only layout and separators (one comma between two
//!    elements of a comma-separated collection);
//! 7. no version's subtree has a parse error, M's included.
//!
//! # Policy (auto only)
//!
//! | construct | joint insertion |
//! |---|---|
//! | Go keyed literal, JS object / export / import names, Python dict / set / `__all__` | yes |
//! | Go tagged expression switch, JS switch (with the guards of 5) | yes |
//! | Go `const ( … )` without `iota` | yes |
//! | Go `const ( … )` with `iota` | only after every base constant |
//! | Go / JS / Python sequence literal in a test file, bound to a name | yes |
//! | statement list | only named declarations (JS `function`, Python `def`), distinct names |
//! | anything else | no (and a one-sided insertion must not touch the other side's edit) |
//!
//! Declared (`weave-set`) collections are NOT admitted: the certificate has no
//! view of the repository's attributes, and declines. Those land through the
//! resolver, as before.

use std::collections::{HashMap, HashSet};
use tree_sitter::Node;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fail {
    /// The check cannot tell (not a recognised shape, not by the policy).
    Decline(String),
    /// M is not what the elements of O, A and B allow.
    Mismatch(String),
}

type Res<T = ()> = Result<T, Fail>;

fn decline<T>(why: impl Into<String>) -> Res<T> {
    Err(Fail::Decline(why.into()))
}
fn mismatch<T>(why: impl Into<String>) -> Res<T> {
    Err(Fail::Mismatch(why.into()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Go,
    Js,
    Py,
}

pub fn lang(path: &str) -> Option<Lang> {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "go" => Some(Lang::Go),
        "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "mts" | "cts" => Some(Lang::Js),
        "py" | "pyi" => Some(Lang::Py),
        _ => None,
    }
}

fn is_test_path(path: &str) -> bool {
    let p = path.replace('\\', "/");
    let name = p.rsplit('/').next().unwrap_or(&p);
    let in_test_dir = p.split('/').rev().skip(1).any(|d| matches!(d, "test" | "tests" | "__tests__" | "testdata"));
    name.ends_with("_test.go")
        || (name.starts_with("test_") && name.ends_with(".py"))
        || name.ends_with("_test.py")
        || name.contains(".test.")
        || name.contains(".spec.")
        || in_test_dir
}

/// O, A, B, M.
const O: usize = 0;
const A: usize = 1;
const B: usize = 2;
const MM: usize = 3;

pub struct Ctx<'a> {
    lang: Lang,
    test_file: bool,
    src: [&'a str; 4],
}

/// Check one region. `nodes` are the region's byte ranges `[lo, hi)` in the
/// four texts, `roots` the four parse trees' roots.
pub fn check_region(path: &str, src: [&str; 4], roots: [Node; 4], ranges: [(usize, usize); 4]) -> Res {
    let Some(l) = lang(path) else { return decline("no element grammar for this language") };
    let cx = Ctx { lang: l, test_file: is_test_path(path), src };
    // The region (a run of whole lines) as the maximal syntax nodes inside it:
    // the same sequence, by kind, in all four versions; the text around them
    // selected, each walked.
    let mut seqs: Vec<Vec<Node>> = Vec::with_capacity(4);
    for v in 0..4 {
        let mut out = Vec::new();
        cover(roots[v], ranges[v].0, ranges[v].1, &mut out)?;
        if out.iter().any(|x| x.has_error()) {
            return decline("a version does not parse");
        }
        seqs.push(out);
    }
    if seqs[O].is_empty() || (1..4).any(|v| seqs[v].len() != seqs[O].len()) {
        return decline("the region holds a different run of nodes in each version");
    }
    for i in 0..seqs[O].len() {
        if (1..4).any(|v| seqs[v][i].kind() != seqs[O][i].kind()) {
            return decline("the region holds a different run of nodes in each version");
        }
    }
    let mut prev: [usize; 4] = std::array::from_fn(|v| ranges[v].0);
    for i in 0..seqs[O].len() {
        let n: [Node; 4] = std::array::from_fn(|v| seqs[v][i]);
        cx.selected(std::array::from_fn(|v| &src[v][prev[v]..n[v].start_byte()]), "text between the region's nodes")?;
        cx.walk(n)?;
        prev = std::array::from_fn(|v| n[v].end_byte());
    }
    cx.selected(std::array::from_fn(|v| &src[v][prev[v]..ranges[v].1]), "text after the region's nodes")
}

/// The whole file as one region: for a merge whose per-region certificate
/// fails only because both sides inserted top-level declarations at one point
/// (the text after an inserted entity is then a region neither side wrote).
/// The file's top-level statements are a statement list, under the same
/// policy as any other.
pub fn check_file(path: &str, src: [&str; 4], roots: [Node; 4]) -> Res {
    check_region(path, src, roots, std::array::from_fn(|v| (0, src[v].len())))
}

/// The maximal nodes inside `[lo, hi)`, in order. A leaf that crosses an edge
/// of the range declines.
fn cover<'t>(n: Node<'t>, lo: usize, hi: usize, out: &mut Vec<Node<'t>>) -> Res {
    if n.end_byte() <= lo || n.start_byte() >= hi {
        return Ok(());
    }
    if n.start_byte() >= lo && n.end_byte() <= hi {
        out.push(n);
        return Ok(());
    }
    let ch = kids(n);
    if ch.is_empty() {
        return decline("a token crosses the region's edge");
    }
    for c in ch {
        cover(c, lo, hi, out)?;
    }
    Ok(())
}

fn kids(n: Node) -> Vec<Node> {
    (0..n.child_count() as u32).filter_map(|i| n.child(i)).collect()
}

fn norm(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Three-way selection over strings; `None` = both changed, differently.
fn sel3<'s>(o: Option<&'s str>, a: Option<&'s str>, b: Option<&'s str>) -> Option<Option<&'s str>> {
    crate::select(o.as_ref(), a.as_ref(), b.as_ref()).map(|x| x.copied())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Keyed,
    Switch,
    Positional,
    Statements,
    Const { iota: bool },
}

struct Coll<'t> {
    kind: Kind,
    comma: bool,
    open_end: usize,
    close_start: usize,
    /// Elements and comments, in order.
    items: Vec<Node<'t>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Id {
    Base(usize),
    /// Inserted: its alignment key and its occurrence among its side's
    /// insertions with that key.
    Ins(String, usize),
}

#[derive(Clone)]
enum Want {
    Exact(String),
    /// Both sides changed base item `i` differently: walk it.
    Walk(usize),
}

impl<'a> Ctx<'a> {
    fn txt(&self, v: usize, n: Node) -> &'a str {
        &self.src[v][n.start_byte()..n.end_byte()]
    }

    /// The three-way selection of `x[O..=B]` must be `x[MM]`.
    fn selected(&self, x: [&str; 4], what: &str) -> Res {
        match sel3(Some(x[O]), Some(x[A]), Some(x[B])) {
            Some(Some(s)) if s == x[MM] => Ok(()),
            Some(_) => mismatch(format!("{what}: the merge is not the selection")),
            None => decline(format!("{what}: both sides changed it")),
        }
    }

    fn walk(&self, n: [Node; 4]) -> Res {
        let t: [&str; 4] = std::array::from_fn(|v| self.txt(v, n[v]));
        match sel3(Some(t[O]), Some(t[A]), Some(t[B])) {
            Some(Some(s)) => {
                return if s == t[MM] { Ok(()) } else { mismatch(format!("a `{}` is not the selection", n[O].kind())) };
            }
            Some(None) => return mismatch("selection deletes a node"),
            None => {}
        }
        if (1..4).any(|v| n[v].kind() != n[O].kind()) {
            return decline(format!("a `{}` changed kind", n[O].kind()));
        }
        let colls: Vec<Option<Coll>> = (0..4).map(|v| self.collection(v, n[v])).collect();
        if colls.iter().all(|c| c.is_some()) {
            let colls: Vec<Coll> = colls.into_iter().flatten().collect();
            if colls.iter().any(|c| c.kind != colls[O].kind || c.comma != colls[O].comma) {
                return decline(format!("a `{}` is a different collection in each version", n[O].kind()));
            }
            let c: [Coll; 4] = colls.try_into().ok().expect("four");
            return self.collection_check(n, &c);
        }
        let c: [Vec<Node>; 4] = std::array::from_fn(|v| kids(n[v]));
        if c[O].is_empty() || (1..4).any(|v| c[v].len() != c[O].len()) {
            return decline(format!("both sides changed the shape of a `{}`", n[O].kind()));
        }
        for i in 0..c[O].len() {
            if (1..4).any(|v| c[v][i].kind() != c[O][i].kind()) {
                return decline(format!("both sides changed the shape of a `{}`", n[O].kind()));
            }
        }
        let mut prev: [usize; 4] = std::array::from_fn(|v| n[v].start_byte());
        for i in 0..c[O].len() {
            self.selected(std::array::from_fn(|v| &self.src[v][prev[v]..c[v][i].start_byte()]), "text between children")?;
            self.walk(std::array::from_fn(|v| c[v][i]))?;
            prev = std::array::from_fn(|v| c[v][i].end_byte());
        }
        self.selected(std::array::from_fn(|v| &self.src[v][prev[v]..n[v].end_byte()]), "text after the last child")
    }

    // ------------------------------------------------------------ recognise

    fn collection<'t>(&self, v: usize, n: Node<'t>) -> Option<Coll<'t>> {
        let src = self.src[v];
        let (kind, comma, open, close) = match (self.lang, n.kind()) {
            (Lang::Go, "literal_value") => {
                let keyed = kids(n).iter().any(|c| c.kind() == "keyed_element");
                (if keyed { Kind::Keyed } else { Kind::Positional }, true, "{", "}")
            }
            (Lang::Go, "expression_switch_statement") => {
                n.child_by_field_name("value")?;
                (Kind::Switch, false, "{", "}")
            }
            (Lang::Go, "block") => (Kind::Statements, false, "{", "}"),
            // A whole file (see `check_file`): its top-level statements.
            (Lang::Go, "source_file") | (Lang::Js, "program") | (Lang::Py, "module") => (Kind::Statements, false, "", ""),
            (Lang::Go, "const_declaration") => {
                if !kids(n).iter().any(|c| c.kind() == "(") {
                    return None;
                }
                (Kind::Const { iota: has_ident(n, src, "iota") }, false, "(", ")")
            }
            (Lang::Js, "object" | "export_clause" | "named_imports") => (Kind::Keyed, true, "{", "}"),
            (Lang::Js, "array") => (Kind::Positional, true, "[", "]"),
            (Lang::Js, "switch_body") => (Kind::Switch, false, "{", "}"),
            (Lang::Js, "statement_block") => (Kind::Statements, false, "{", "}"),
            (Lang::Py, "dictionary" | "set") => (Kind::Keyed, true, "{", "}"),
            (Lang::Py, "list") => {
                let all = n.parent().is_some_and(|p| {
                    p.kind() == "assignment"
                        && p.child_by_field_name("left").is_some_and(|l| &src[l.start_byte()..l.end_byte()] == "__all__")
                });
                (if all { Kind::Keyed } else { Kind::Positional }, true, "[", "]")
            }
            (Lang::Py, "tuple") => {
                if !kids(n).iter().any(|c| c.kind() == "(") {
                    return None;
                }
                (Kind::Positional, true, "(", ")")
            }
            (Lang::Py, "block") => {
                if n.parent().is_some_and(|p| p.kind() == "match_statement") {
                    return None;
                }
                (Kind::Statements, false, "", "")
            }
            _ => return None,
        };
        let ch = kids(n);
        let (lo, open_end) = if open.is_empty() {
            (0, n.start_byte())
        } else {
            let i = ch.iter().position(|c| c.kind() == open)?;
            (i + 1, ch[i].end_byte())
        };
        let (hi, close_start) = if close.is_empty() {
            (ch.len(), n.end_byte())
        } else {
            let i = ch.iter().rposition(|c| c.kind() == close)?;
            (i, ch[i].start_byte())
        };
        if lo > hi {
            return None;
        }
        let mut items = Vec::new();
        for c in &ch[lo..hi] {
            if comma && c.kind() == "," || !c.is_named() && matches!(c.kind(), ";" | "\n") {
                continue;
            }
            if !c.is_named() {
                return None;
            }
            items.push(*c);
        }
        Some(Coll { kind, comma, open_end, close_start, items })
    }

    /// A sequence literal bound to a name in a test file.
    fn test_table(&self, n: Node) -> bool {
        if !self.test_file {
            return false;
        }
        let mut at = n.parent();
        while let Some(p) = at {
            match p.kind() {
                "composite_literal" | "expression_list" | "parenthesized_expression" => at = p.parent(),
                "short_var_declaration" | "var_spec" | "assignment_statement" | "assignment" | "variable_declarator"
                | "assignment_expression" => return true,
                _ => return false,
            }
        }
        false
    }

    // ------------------------------------------------------------------ keys

    fn label(&self, v: usize, n: Node) -> Option<String> {
        let src = self.src[v];
        let t = &src[n.start_byte()..n.end_byte()];
        let plain = |t: &str| -> Option<String> {
            if t.contains('\\') || t.contains("${") {
                return None;
            }
            let q = t.chars().next()?;
            if !matches!(q, '"' | '\'' | '`') || t.len() < 2 || !t.ends_with(q) {
                return None;
            }
            Some(format!("s:{}", &t[1..t.len() - 1]))
        };
        let dec = |t: &str| {
            (!t.is_empty() && t.bytes().all(|c| c.is_ascii_digit()) && (t == "0" || !t.starts_with('0'))).then(|| t.to_string())
        };
        let qual = |t: &str| {
            let ok = !t.is_empty()
                && t.split('.').all(|seg| {
                    let mut c = seg.chars();
                    c.next().is_some_and(|x| x.is_alphabetic() || x == '_' || x == '$')
                        && c.all(|x| x.is_alphanumeric() || x == '_' || x == '$')
                });
            ok.then(|| format!("n:{t}"))
        };
        match (self.lang, n.kind()) {
            (Lang::Go, "literal_element") => self.label(v, n.named_child(0)?),
            (Lang::Go, "interpreted_string_literal" | "raw_string_literal" | "rune_literal") => plain(t),
            (Lang::Go, "int_literal") => dec(t).map(|d| format!("i:{d}")),
            (Lang::Go, "identifier" | "field_identifier" | "selector_expression") => qual(t),
            (Lang::Js, "string") => plain(t),
            (Lang::Js, "number") => dec(t).map(|d| format!("s:{d}")),
            (Lang::Js, "property_identifier" | "shorthand_property_identifier") => {
                (t != "__proto__").then(|| format!("s:{t}"))
            }
            (Lang::Js, "identifier" | "member_expression") => qual(t),
            (Lang::Py, "string") => plain(t),
            (Lang::Py, "integer") => dec(t).map(|d| format!("i:{d}")),
            (Lang::Py, "identifier" | "attribute") => {
                if matches!(t, "True" | "False" | "None") {
                    None
                } else {
                    qual(t)
                }
            }
            _ => None,
        }
    }

    /// An element's identity in its collection (`None` = none a union may
    /// rely on), and the names it claims (labels, constants, declarations).
    fn key(&self, v: usize, kind: Kind, e: Node) -> (Option<String>, Vec<String>) {
        let src = self.src[v];
        let field_label = |f: &str| e.child_by_field_name(f).and_then(|k| self.label(v, k));
        match kind {
            Kind::Keyed => {
                let k = match (self.lang, e.kind()) {
                    (Lang::Go, "keyed_element") => e
                        .child_by_field_name("key")
                        .or_else(|| e.named_child(0))
                        .and_then(|k| self.label(v, k)),
                    (Lang::Js, "pair") | (Lang::Py, "pair") => field_label("key"),
                    (Lang::Js, "shorthand_property_identifier") => self.label(v, e),
                    (Lang::Js, "method_definition") => {
                        let accessor = kids(e)
                            .iter()
                            .take_while(|c| !c.is_named())
                            .any(|c| matches!(c.kind(), "get" | "set" | "static" | "async" | "*"));
                        if accessor {
                            None
                        } else {
                            field_label("name")
                        }
                    }
                    (Lang::Js, "export_specifier" | "import_specifier") => e
                        .child_by_field_name("alias")
                        .or_else(|| e.child_by_field_name("name"))
                        .map(|k| format!("s:{}", &src[k.start_byte()..k.end_byte()])),
                    (Lang::Py, "string" | "integer") => self.label(v, e),
                    _ => None,
                };
                (k.clone().map(|k| format!("k:{k}")), k.into_iter().collect())
            }
            Kind::Switch => {
                let labels: Option<Vec<String>> = match (self.lang, e.kind()) {
                    (Lang::Go, "expression_case") => e.child_by_field_name("value").and_then(|vl| {
                        kids(vl).into_iter().filter(|c| c.is_named()).map(|l| self.label(v, l)).collect()
                    }),
                    (Lang::Js, "switch_case") => field_label("value").map(|l| vec![l]),
                    (Lang::Go, "default_case") | (Lang::Js, "switch_default") => Some(vec!["default".into()]),
                    _ => None,
                };
                match labels {
                    Some(mut ls) if !ls.is_empty() => {
                        ls.sort();
                        (Some(format!("case:{}", ls.join("|"))), ls)
                    }
                    _ => (None, vec![]),
                }
            }
            Kind::Const { .. } => {
                if e.kind() != "const_spec" {
                    return (None, vec![]);
                }
                let names: Vec<String> = kids(e)
                    .iter()
                    .filter(|c| c.kind() == "identifier")
                    .map(|c| src[c.start_byte()..c.end_byte()].to_string())
                    .filter(|n| n != "_")
                    .collect();
                if names.is_empty() {
                    return (None, vec![]);
                }
                (Some(format!("const:{}", names.join(","))), names)
            }
            Kind::Positional => (Some(format!("t:{}", norm(self.txt(v, e)))), vec![]),
            Kind::Statements => (Some(format!("t:{}", norm(self.txt(v, e)))), self.declared(v, e).into_iter().collect()),
        }
    }

    /// The name a declaration whose order is not observable binds.
    fn declared(&self, v: usize, e: Node) -> Option<String> {
        let src = self.src[v];
        let name = |n: Node| n.child_by_field_name("name").map(|x| src[x.start_byte()..x.end_byte()].to_string());
        match (self.lang, e.kind()) {
            (Lang::Js, "function_declaration" | "generator_function_declaration") => name(e),
            (Lang::Py, "function_definition") => name(e),
            (Lang::Py, "decorated_definition") => {
                let d = e.child_by_field_name("definition")?;
                (d.kind() == "function_definition").then(|| name(d)).flatten()
            }
            // Go package scope: order of declaration is not observable.
            (Lang::Go, "function_declaration") if e.parent().is_some_and(|p| p.kind() == "source_file") => name(e),
            (Lang::Go, "type_declaration") if e.parent().is_some_and(|p| p.kind() == "source_file") => {
                let specs: Vec<Node> = kids(e).into_iter().filter(|c| c.kind() == "type_spec").collect();
                match specs.as_slice() {
                    [one] => name(*one),
                    _ => None,
                }
            }
            (Lang::Go, "method_declaration") if e.parent().is_some_and(|p| p.kind() == "source_file") => {
                let recv = e.child_by_field_name("receiver")?;
                let rt = norm(&src[recv.start_byte()..recv.end_byte()]);
                let rt = rt.split_whitespace().last()?.trim_matches(['(', ')', '*']).to_string();
                Some(format!("{rt}.{}", name(e)?))
            }
            _ => None,
        }
        .map(|n| format!("decl:{n}"))
    }

    /// The key items are aligned by, and whether it is a decidable identity.
    fn align_key(&self, v: usize, kind: Kind, it: Node) -> (String, bool) {
        if it.kind() == "comment" {
            return (format!("c:{}", norm(self.txt(v, it))), true);
        }
        match self.key(v, kind, it).0 {
            Some(k) => (k, true),
            None => (format!("t:{}", norm(self.txt(v, it))), false),
        }
    }

    // ------------------------------------------------------------- the check

    fn collection_check(&self, n: [Node; 4], c: &[Coll; 4]) -> Res {
        let kind = c[O].kind;
        let what = format!("{kind:?} `{}`", n[O].kind());
        // Delimiters: selected.
        self.selected(std::array::from_fn(|v| &self.src[v][n[v].start_byte()..c[v].open_end]), "an opening delimiter")?;
        self.selected(std::array::from_fn(|v| &self.src[v][c[v].close_start..n[v].end_byte()]), "a closing delimiter")?;
        let cells = c[O].items.len().max(1) * c[A].items.len().max(1) + c[O].items.len().max(1) * c[B].items.len().max(1);
        if cells > 4_000_000 {
            return decline(format!("{what}: too large"));
        }
        let keys: [Vec<(String, bool)>; 4] =
            std::array::from_fn(|v| c[v].items.iter().map(|it| self.align_key(v, kind, *it)).collect());

        // 1. each side against base. Items of a keyed collection align by key
        // only; a text-identified item that changed is paired with the base
        // item it replaced (see `align`), by its signature: the name it
        // declares, else its kind and first line.
        let by_text = matches!(kind, Kind::Positional | Kind::Statements);
        let sigs: [Vec<String>; 4] = std::array::from_fn(|v| {
            c[v].items
                .iter()
                .map(|it| {
                    if it.kind() == "comment" {
                        return String::new();
                    }
                    self.declared(v, *it).unwrap_or_else(|| {
                        format!("{}:{}", it.kind(), norm(self.txt(v, *it).lines().next().unwrap_or("")))
                    })
                })
                .collect()
        });
        let fate: [Vec<Option<usize>>; 2] = [
            align(&keys[O], &keys[A], &c[O].items, &c[A].items, by_text.then_some((&sigs[O], &sigs[A]))),
            align(&keys[O], &keys[B], &c[O].items, &c[B].items, by_text.then_some((&sigs[O], &sigs[B]))),
        ];
        // identities of each side's items
        let ids = |s: usize| -> Vec<Id> {
            let v = s + 1;
            let mut owner: Vec<Option<usize>> = vec![None; c[v].items.len()];
            for (i, f) in fate[s].iter().enumerate() {
                if let Some(j) = f {
                    owner[*j] = Some(i);
                }
            }
            let mut ord: HashMap<&str, usize> = HashMap::new();
            (0..c[v].items.len())
                .map(|j| match owner[j] {
                    Some(i) => Id::Base(i),
                    None => {
                        let k = keys[v][j].0.as_str();
                        let n = ord.entry(k).or_insert(0);
                        *n += 1;
                        Id::Ins(k.to_string(), *n - 1)
                    }
                })
                .collect()
        };
        let side_ids = [ids(0), ids(1)];

        // the selected set, and what M must hold for each
        let mut want: HashMap<Id, Want> = HashMap::new();
        for i in 0..c[O].items.len() {
            let t = |s: usize| fate[s][i].map(|j| self.txt(s + 1, c[s + 1].items[j]));
            match sel3(Some(self.txt(O, c[O].items[i])), t(0), t(1)) {
                Some(None) => {}
                Some(Some(x)) => {
                    want.insert(Id::Base(i), Want::Exact(x.to_string()));
                }
                None if t(0).is_some() && t(1).is_some() => {
                    want.insert(Id::Base(i), Want::Walk(i));
                }
                None => return decline(format!("{what}: an item one side deleted and the other changed")),
            }
        }
        let mut ins_text: HashMap<&Id, (usize, &str)> = HashMap::new();
        for s in 0..2 {
            let v = s + 1;
            for (j, id) in side_ids[s].iter().enumerate() {
                if let Id::Ins(..) = id {
                    if !keys[v][j].1 {
                        return decline(format!("{what}: an inserted element has no decidable key"));
                    }
                    let t = self.txt(v, c[v].items[j]);
                    match ins_text.get(id) {
                        Some((_, other)) if *other != t => {
                            return decline(format!("{what}: both sides inserted one key with different text"));
                        }
                        _ => {
                            ins_text.insert(id, (s, t));
                            want.insert(id.clone(), Want::Exact(t.to_string()));
                        }
                    }
                }
            }
        }

        // each side's sequence of selected items, and which are shared
        let seq = |s: usize| -> Vec<(Id, usize)> {
            side_ids[s].iter().enumerate().filter(|(_, id)| want.contains_key(id)).map(|(j, id)| (id.clone(), j)).collect()
        };
        let sa = seq(0);
        let sb = seq(1);
        let in_a: HashSet<&Id> = sa.iter().map(|(id, _)| id).collect();
        let in_b: HashSet<&Id> = sb.iter().map(|(id, _)| id).collect();
        let shared = |id: &Id| in_a.contains(id) && in_b.contains(id);

        // 4. the policy, per insertion point
        self.policy(&what, n[O], c, &fate, &sa, &sb, &shared)?;

        // 3. M is an interleaving of the two sequences
        let path = self.interleave(&what, c, &want, &sa, &sb, &shared)?;

        // 5. names, labels, fallthrough
        let m_items = &c[MM].items;
        // What base already states twice (a dict whose later entry wins) is
        // base's business, as long as no inserted item joins it and nothing
        // states it more often than base did.
        let claims = |v: usize, it: Node| -> Vec<String> {
            if it.kind() == "comment" {
                return vec![];
            }
            let (k, mut ids) = self.key(v, kind, it);
            if matches!(kind, Kind::Keyed | Kind::Switch | Kind::Const { .. }) {
                ids.extend(k);
            }
            ids.sort();
            ids.dedup();
            ids
        };
        let mut in_o: HashMap<String, usize> = HashMap::new();
        for it in &c[O].items {
            for x in claims(O, *it) {
                *in_o.entry(x).or_insert(0) += 1;
            }
        }
        let mut holders: HashMap<String, Vec<usize>> = HashMap::new();
        for (k, it) in m_items.iter().enumerate() {
            for x in claims(MM, *it) {
                holders.entry(x).or_default().push(k);
            }
        }
        for (x, ks) in &holders {
            if ks.len() > 1
                && (ks.len() > in_o.get(x).copied().unwrap_or(0) || ks.iter().any(|k| !matches!(path[*k], Id::Base(_))))
            {
                return mismatch(format!("{what}: `{x}` is stated twice"));
            }
        }
        let inserted: Vec<bool> = path.iter().map(|id| !shared(id)).collect();
        if kind == Kind::Switch && inserted.iter().any(|x| *x) {
            match self.lang {
                Lang::Go => {
                    if has_kind(n[MM], "fallthrough_statement") {
                        return decline(format!("{what}: `fallthrough`"));
                    }
                }
                Lang::Js => {
                    let cases: Vec<usize> = (0..m_items.len()).filter(|k| m_items[*k].kind() != "comment").collect();
                    for (p, &k) in cases.iter().enumerate() {
                        if inserted[k]
                            && (!js_case_terminates(m_items[k]) || p > 0 && !js_case_terminates(m_items[cases[p - 1]]))
                        {
                            return decline(format!("{what}: an inserted case could fall through"));
                        }
                    }
                }
                Lang::Py => return decline("no switch in Python"),
            }
        }
        // Python: a statement sits at the column its version wrote it at.
        if self.lang == Lang::Py && kind == Kind::Statements {
            let col = |v: usize, j: usize| c[v].items[j].start_position().column;
            for (k, id) in path.iter().enumerate() {
                let mcol = m_items[k].start_position().column;
                let ok = match id {
                    Id::Base(i) => mcol == col(O, *i),
                    Id::Ins(..) => {
                        let (s, _) = ins_text[id];
                        let j = side_ids[s].iter().position(|x| x == id).expect("inserted by s");
                        mcol == col(s + 1, j)
                    }
                };
                if !ok {
                    return mismatch(format!("{what}: a statement moved column"));
                }
            }
        }

        // 6. between items, only layout and separators
        self.separators(&what, &c[MM])
    }

    #[allow(clippy::too_many_arguments)]
    fn policy(
        &self,
        what: &str,
        node: Node,
        c: &[Coll; 4],
        fate: &[Vec<Option<usize>>; 2],
        sa: &[(Id, usize)],
        sb: &[(Id, usize)],
        shared: &dyn Fn(&Id) -> bool,
    ) -> Res {
        let kind = c[O].kind;
        // runs of one side's own items, by the shared item before them
        let runs = |sq: &[(Id, usize)]| -> Vec<(Option<Id>, Option<Id>, Vec<usize>)> {
            let mut out: Vec<(Option<Id>, Option<Id>, Vec<usize>)> = Vec::new();
            let mut anchor: Option<Id> = None;
            for (id, j) in sq {
                if shared(id) {
                    if let Some(last) = out.last_mut() {
                        if last.0 == anchor && last.1.is_none() {
                            last.1 = Some(id.clone());
                        }
                    }
                    anchor = Some(id.clone());
                } else {
                    match out.last_mut() {
                        Some(last) if last.0 == anchor && last.1.is_none() => last.2.push(*j),
                        _ => out.push((anchor.clone(), None, vec![*j])),
                    }
                }
            }
            out
        };
        let (ra, rb) = (runs(sa), runs(sb));
        let decl = |v: usize, j: usize| self.declared(v, c[v].items[j]).is_some();
        let last_base_elem = (0..c[O].items.len()).rev().find(|i| c[O].items[*i].kind() != "comment");
        for (s, (mine, theirs)) in [(&ra, &rb), (&rb, &ra)].into_iter().enumerate() {
            let (v, other) = (s + 1, 1 - s);
            for (anchor, next, run) in mine {
                let joint = theirs.iter().find(|r| r.0 == *anchor);
                if let Some(jr) = joint {
                    if s == 1 {
                        continue; // checked from ours
                    }
                    let ok = match kind {
                        Kind::Keyed | Kind::Switch | Kind::Const { iota: false } => true,
                        Kind::Const { iota: true } => {
                            matches!(anchor, Some(Id::Base(i)) if Some(*i) == last_base_elem) && next.is_none() && jr.1.is_none()
                        }
                        Kind::Positional => self.test_table(node),
                        Kind::Statements => {
                            run.iter().all(|j| c[v].items[*j].kind() == "comment" || decl(v, *j))
                                && jr.2.iter().all(|j| c[B].items[*j].kind() == "comment" || decl(B, *j))
                        }
                    };
                    if !ok {
                        return decline(format!("{what}: both sides inserted at one point of an ordered collection"));
                    }
                    continue;
                }
                // One side inserted here. Into an order-sensitive collection,
                // the other side must have left the neighbours alone.
                let sensitive = match kind {
                    Kind::Positional => !self.test_table(node),
                    Kind::Statements => !run.iter().all(|j| c[v].items[*j].kind() == "comment" || decl(v, *j)),
                    _ => false,
                };
                if !sensitive {
                    continue;
                }
                let ov = other + 1;
                let changed = |id: &Option<Id>| match id {
                    Some(Id::Base(i)) => match fate[other][*i] {
                        None => true,
                        Some(j) => self.txt(ov, c[ov].items[j]) != self.txt(O, c[O].items[*i]),
                    },
                    _ => false,
                };
                if changed(anchor) || changed(next) {
                    return decline(format!("{what}: an insertion next to an item the other side changed"));
                }
                let lo = match anchor {
                    Some(Id::Base(i)) => *i + 1,
                    None => 0,
                    Some(Id::Ins(..)) => return decline(format!("{what}: an insertion after a joint insertion")),
                };
                let hi = match next {
                    Some(Id::Base(i)) => *i,
                    None => c[O].items.len(),
                    Some(Id::Ins(..)) => return decline(format!("{what}: an insertion before a joint insertion")),
                };
                if (lo..hi).any(|i| fate[other][i].is_none()) {
                    return decline(format!("{what}: an insertion next to an item the other side deleted"));
                }
            }
        }
        Ok(())
    }

    /// M's items, matched in order against A's and B's selected sequences: an
    /// item only one side has is taken from that side's next; a shared item
    /// must be next in both. Returns the identity of each item of M.
    fn interleave(
        &self,
        what: &str,
        c: &[Coll; 4],
        want: &HashMap<Id, Want>,
        sa: &[(Id, usize)],
        sb: &[(Id, usize)],
        shared: &dyn Fn(&Id) -> bool,
    ) -> Res<Vec<Id>> {
        let m = &c[MM].items;
        let walked: std::cell::RefCell<HashMap<(usize, usize), bool>> = Default::default();
        // why the last both-changed item that failed to match did not
        let inner: std::cell::RefCell<Option<(usize, Fail)>> = Default::default();
        let matches = |id: &Id, k: usize| -> bool {
            match &want[id] {
                Want::Exact(t) => self.txt(MM, m[k]) == t,
                Want::Walk(i) => {
                    let i = *i;
                    if let Some(r) = walked.borrow().get(&(i, k)) {
                        return *r;
                    }
                    let side = |s: usize| sa_sb_node(c, s, id, sa, sb);
                    let r = match (side(0), side(1)) {
                        (Some(a), Some(b)) if m[k].kind() == c[O].items[i].kind() => {
                            match self.walk([c[O].items[i], a, b, m[k]]) {
                                Ok(()) => true,
                                Err(e) => {
                                    *inner.borrow_mut() = Some((k, e));
                                    false
                                }
                            }
                        }
                        _ => false,
                    };
                    walked.borrow_mut().insert((i, k), r);
                    r
                }
            }
        };
        // states: (i, j) -> predecessor, per step
        let mut layers: Vec<HashMap<(usize, usize), (usize, usize, Id)>> = Vec::with_capacity(m.len());
        let mut cur: HashSet<(usize, usize)> = HashSet::from([(0, 0)]);
        let mut budget = 2_000_000usize;
        for k in 0..m.len() {
            let mut next: HashMap<(usize, usize), (usize, usize, Id)> = HashMap::new();
            for &(i, j) in &cur {
                budget = budget.checked_sub(1).ok_or(Fail::Decline(format!("{what}: too many ways to read the merge")))?;
                let a = sa.get(i).map(|x| &x.0);
                let b = sb.get(j).map(|x| &x.0);
                if let Some(a) = a {
                    if !shared(a) && matches(a, k) {
                        next.entry((i + 1, j)).or_insert((i, j, a.clone()));
                    }
                }
                if let Some(b) = b {
                    if !shared(b) && matches(b, k) {
                        next.entry((i, j + 1)).or_insert((i, j, b.clone()));
                    }
                }
                if let (Some(a), Some(b)) = (a, b) {
                    if a == b && matches(a, k) {
                        next.entry((i + 1, j + 1)).or_insert((i, j, a.clone()));
                    }
                }
            }
            if next.is_empty() {
                // An item both sides changed that does not check is that
                // item's verdict (a decline stays a decline).
                if let Some((kk, e)) = inner.borrow_mut().take() {
                    if kk == k {
                        return Err(e);
                    }
                }
                return mismatch(format!(
                    "{what}: item {} of the merge is not the next item of either side (dropped, added, changed or reordered)",
                    k + 1
                ));
            }
            cur = next.keys().copied().collect();
            layers.push(next);
        }
        let end = (sa.len(), sb.len());
        if !cur.contains(&end) {
            return mismatch(format!("{what}: the merge leaves out an item a side has"));
        }
        let mut path = Vec::with_capacity(m.len());
        let mut at = end;
        for layer in layers.iter().rev() {
            let (i, j, id) = layer[&at].clone();
            path.push(id);
            at = (i, j);
        }
        path.reverse();
        Ok(path)
    }

    fn separators(&self, what: &str, c: &Coll) -> Res {
        let src = self.src[MM];
        let mut prev = c.open_end;
        let mut commas = 0usize;
        let mut elements = 0usize;
        let mut bounds: Vec<(usize, usize)> = c.items.iter().map(|n| (n.start_byte(), n.end_byte())).collect();
        bounds.push((c.close_start, c.close_start));
        for (k, (s, e)) in bounds.iter().enumerate() {
            let gap = &src[prev..*s];
            for ch in gap.chars().filter(|x| !x.is_whitespace()) {
                match ch {
                    ',' if c.comma => commas += 1,
                    ';' if !c.comma => {}
                    _ => return mismatch(format!("{what}: `{ch}` between items")),
                }
            }
            let last = k == c.items.len();
            let is_elem = !last && c.items[k].kind() != "comment";
            if c.comma && (is_elem || last) {
                let ok = if elements == 0 {
                    commas == 0
                } else if last {
                    commas <= 1
                } else {
                    commas == 1
                };
                if !ok {
                    return mismatch(format!("{what}: separators between elements"));
                }
                commas = 0;
            }
            if is_elem {
                elements += 1;
            }
            prev = *e;
        }
        Ok(())
    }
}

/// The node a side holds for a both-changed base item.
fn sa_sb_node<'t>(c: &[Coll<'t>; 4], s: usize, id: &Id, sa: &[(Id, usize)], sb: &[(Id, usize)]) -> Option<Node<'t>> {
    let sq = if s == 0 { sa } else { sb };
    sq.iter().find(|(x, _)| x == id).map(|(_, j)| c[s + 1].items[*j])
}

/// Align a side's items with base: LCS over alignment keys. Then, given
/// signatures, within each gap between matched items: as many side items as
/// base items, kind for kind, is each base item changed into the one facing
/// it; otherwise a base item whose signature one side item of the gap shares
/// (each signature unique on both sides of the gap, in order) was changed
/// into that item. A base item left unpaired was deleted, a side item
/// inserted.
#[allow(clippy::type_complexity)]
fn align(
    kb: &[(String, bool)],
    ks: &[(String, bool)],
    nb: &[Node],
    ns: &[Node],
    sigs: Option<(&Vec<String>, &Vec<String>)>,
) -> Vec<Option<usize>> {
    let (n, m) = (kb.len(), ks.len());
    let w = m + 1;
    let mut dp = vec![0u32; (n + 1) * w];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i * w + j] = if kb[i].0 == ks[j].0 { dp[(i + 1) * w + j + 1] + 1 } else { dp[(i + 1) * w + j].max(dp[i * w + j + 1]) };
        }
    }
    let mut fate = vec![None; n];
    let mut pairs = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if kb[i].0 == ks[j].0 {
            pairs.push((i, j));
            i += 1;
            j += 1;
        } else if dp[(i + 1) * w + j] >= dp[i * w + j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    pairs.push((n, m));
    let (mut pi, mut pj) = (0, 0);
    for (bi, bj) in pairs {
        // A gap of as many side items as base items, kind for kind: each base
        // item was changed into the side item facing it.
        let (k, l) = (bi - pi, bj - pj);
        if let Some((sb, ss)) = sigs.filter(|_| k > 0 && l > 0) {
            if k == l && (0..k).all(|x| nb[pi + x].kind() == ns[pj + x].kind() && nb[pi + x].kind() != "comment") {
                for x in 0..k {
                    fate[pi + x] = Some(pj + x);
                }
            } else {
                let unique = |v: &[String]| {
                    let mut seen = HashSet::new();
                    v.iter().filter(|s| !s.is_empty()).all(|s| seen.insert(s))
                };
                if unique(&sb[pi..bi]) && unique(&ss[pj..bj]) {
                    let mut from = pj;
                    for i in pi..bi {
                        if sb[i].is_empty() {
                            continue;
                        }
                        match (from..bj).find(|j| ss[*j] == sb[i]) {
                            Some(j) => {
                                fate[i] = Some(j);
                                from = j + 1;
                            }
                            None if (pj..from).any(|j| ss[j] == sb[i]) => break, // out of order
                            None => {}
                        }
                    }
                }
            }
        }
        if bi < n {
            fate[bi] = Some(bj);
        }
        pi = bi + 1;
        pj = bj + 1;
    }
    fate
}

fn has_ident(n: Node, src: &str, name: &str) -> bool {
    let mut stack = vec![n];
    while let Some(x) = stack.pop() {
        if (x.kind() == name || x.kind() == "identifier") && &src[x.start_byte()..x.end_byte()] == name {
            return true;
        }
        stack.extend(kids(x));
    }
    false
}

fn has_kind(n: Node, kind: &str) -> bool {
    let mut stack = vec![n];
    while let Some(x) = stack.pop() {
        if x.kind() == kind {
            return true;
        }
        stack.extend(kids(x));
    }
    false
}

fn js_case_terminates(case: Node) -> bool {
    let body: Vec<Node> =
        kids(case).into_iter().skip_while(|c| c.kind() != ":").skip(1).filter(|c| c.is_named() && c.kind() != "comment").collect();
    body.last().is_some_and(|s| terminates(*s))
}

fn terminates(s: Node) -> bool {
    match s.kind() {
        "break_statement" | "return_statement" | "throw_statement" | "continue_statement" => true,
        "statement_block" => kids(s).into_iter().filter(|c| c.is_named() && c.kind() != "comment").last().is_some_and(terminates),
        _ => false,
    }
}
