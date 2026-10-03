//! Sub-entity element union: two sides that each inserted new elements into
//! one collection INSIDE one entity — the entries of a registry map, the
//! cases of a switch, the rows of a test table, the names of an export object.
//!
//! The entity ladder (`v2::resolve::intra_entity`, and the member ladder of
//! `container.rs`) refuses these because the two insertions land on
//! neighbouring lines. Line adjacency is not a conflict when the collection's
//! meaning does not depend on the order of its elements: then inserting `p`
//! and inserting `q` commute, and the union is the answer for every intent.
//! This rung decides that question element by element, with a parse tree,
//! and refuses (returns `None`, leaving the conflict) whenever it cannot.
//!
//! # The merge
//!
//! The three texts are parsed the same way (Python is dedented first; a JS
//! class member or a bare `return` is wrapped). The trees are merged top-down:
//!
//! - a node whose three texts agree, or that only one side changed, is taken
//!   whole;
//! - a node that is a recognised **collection** is merged element by element
//!   (below);
//! - any other node both sides changed must have the same children in all
//!   three versions — same count, same kinds — and is merged child by child,
//!   the text between children taken only where at most one side changed it.
//!
//! A collection is read as `prefix · slot* · tail · suffix`, where a slot is
//! one element with the text that leads it (blank lines, comments, the
//! indentation) and what follows it on its own line (its comma, a trailing
//! comment). Slots tile the collection, so nothing between elements can be
//! lost. Each side is aligned with the base — by key where the collection is
//! keyed, by longest common subsequence of element text otherwise — and must
//! be a set of insertions, deletions of elements the other side left alone,
//! and modifications: an element both sides modified is merged recursively
//! (that is how the descent reaches a switch inside a loop inside a method),
//! one modified by one side is that side's. Base elements keep base order; a
//! move is a refusal.
//!
//! Insertions sit after the nearest base element their side wrote them after.
//! Where both sides inserted at one point, the two runs are ordered by key —
//! merged element by element when every element is a bare line, kept whole
//! (the run whose keys sort first going first) when a run carries comments,
//! blank lines or several elements on one line. Nothing consults which side is
//! called ours, so the union is the same from either direction.
//!
//! # Which collections are unions — the policy
//!
//! | construct | default | why |
//! |---|---|---|
//! | Go map / keyed struct literal, JS object, Python dict, JS export/import name list | **auto** | a mapping: distinct keys are independent entries |
//! | Go expression switch with a tag, no `fallthrough`, case labels that are literals or (qualified) names | **auto** | at most one case of distinct constant labels matches; Go has no implicit fallthrough |
//! | JS `switch` whose inserted cases and the case before them all end in `break`/`return`/`throw`/`continue`, labels literals or names | **auto** | the same, once fallthrough is ruled out |
//! | declarations inserted into one statement list with distinct names (JS `function` — hoisted; Go top-level `func`/`type` — package scope; Python `def`) | **auto** | neither side's new declaration can refer to the other's, and each keeps its place relative to every base statement |
//! | Go `const` block without `iota` | **auto** | each constant's value is written out |
//! | Go `const` block with `iota`, insertions after the last base spec | **auto** | no existing constant changes value |
//! | Go `const` block with `iota`, insertions ahead of a base spec | **opt-in** | every later constant is renumbered |
//! | slice / array / list / tuple literal in a test file | **auto** | a table of independent test cases; order is execution order only |
//! | slice / array / list / tuple literal elsewhere | **opt-in** | position is meaning in general |
//! | statement sequences: registry calls `r.bind("k", v)` | **opt-in** (`weave-set=bind`) | `app.get("/u/:id")` before `app.get("/u/me")` is a different program |
//! | type switches, tagless switches, `select`, Python `match` | **never** | the first matching arm wins and arms overlap |
//!
//! Opt-in is the `weave-set` gitattribute (see [`crate::host::SetScope`]): a
//! collection is named by what it is bound to (`Builtins`, `tests`), the
//! function it is an argument of, the method a registry calls (`bind`,
//! `staticFrame.bind`), or a `const` block's type (`Opcode`).
//!
//! Keys are the element's identity: a constant, a field or property name, or
//! — for map keys and case labels — a (qualified) name such as `exp.Foo` or
//! `OpBitNot`. A name's identity is its spelling; two different names bound
//! to one value would collide unseen. That is the one residual assumption,
//! stated here rather than hidden, and the same for every construct.
//!
//! # Refusals (each leaves the conflict as it was)
//!
//! - a key inserted by both sides with different text, or already present;
//!   two inserted case labels in common (`case "a", "b"` vs `case "a", "c"`);
//! - an element both sides modified that does not itself merge;
//! - a base element deleted by one side and modified by the other, or moved;
//! - insertions by both sides at one point of a collection that is not a set
//!   by the table above; in an order-sensitive collection, an insertion next to
//!   an element the other side changed;
//! - `fallthrough` anywhere in a Go switch; a JS case that can fall through;
//! - a key that is not decidable (computed, spread, a call);
//! - any version that does not parse, and a result that does not parse,
//!   drops a line all three kept, or loses a line a side wrote.

use std::collections::{HashMap, HashSet};

use tree_sitter::{Node, Tree};

use crate::host::{Host, SetScope};
use crate::merge::PARSER_REGISTRY;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lang {
    Go,
    Js,
    Py,
}

fn lang(path: &str) -> Option<Lang> {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let ext = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "go" => Some(Lang::Go),
        "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "mts" | "cts" => Some(Lang::Js),
        "py" | "pyi" => Some(Lang::Py),
        _ => None,
    }
}

/// Is `path` a test file, whose literal tables are tables of test cases?
pub(crate) fn is_test_path(path: &str) -> bool {
    let p = path.replace('\\', "/");
    let name = p.rsplit('/').next().unwrap_or(&p);
    let in_test_dir = p
        .split('/')
        .rev()
        .skip(1)
        .any(|d| matches!(d, "test" | "tests" | "__tests__" | "testdata"));
    name.ends_with("_test.go")
        || (name.starts_with("test_") && name.ends_with(".py"))
        || name.ends_with("_test.py")
        || name.contains(".test.")
        || name.contains(".spec.")
        || in_test_dir
}

/// Merge three versions of one entity (or one member, or one attribute block)
/// by element union, or `None`.
pub(crate) fn union(base: &str, ours: &str, theirs: &str, path: &str, host: &Host) -> Option<String> {
    let l = lang(path)?;
    if base == ours || base == theirs || ours == theirs || base.trim().is_empty() {
        return None;
    }
    let scope_cell: std::cell::OnceCell<SetScope> = std::cell::OnceCell::new();
    let scope = || {
        scope_cell
            .get_or_init(|| SetScope::read(host, path))
            .clone()
    };
    union_with(base, ours, theirs, path, l, &scope)
}

fn union_with(
    base: &str,
    ours: &str,
    theirs: &str,
    path: &str,
    l: Lang,
    scope: &dyn Fn() -> SetScope,
) -> Option<String> {
    let frame = Frame::find(l, path, [base, ours, theirs])
        .or_else(|| why(|| "a version does not parse as a fragment".into()))?;
    let srcs = [
        frame.wrap(base)?,
        frame.wrap(ours)?,
        frame.wrap(theirs)?,
    ];
    let trees: Vec<Tree> = srcs
        .iter()
        .map(|s| parse(path, s))
        .collect::<Option<Vec<_>>>()?;
    let m = M {
        lang: l,
        test_file: is_test_path(path),
        scope,
        srcs: [&srcs[0], &srcs[1], &srcs[2]],
    };
    let roots = [trees[0].root_node(), trees[1].root_node(), trees[2].root_node()];
    // Text outside the root node (leading indentation, a final newline).
    let lead = m.trivial(std::array::from_fn(|v| &srcs[v][..roots[v].start_byte()]))?;
    let tail = m.trivial(std::array::from_fn(|v| &srcs[v][roots[v].end_byte()..]))?;
    let body = m.merge_node(roots)?;
    let merged = format!("{lead}{body}{tail}");
    // The answer parses the way the inputs did.
    parse(path, &merged).or_else(|| why(|| "the union does not parse".into()))?;
    let out = frame.unwrap(&merged)?;
    if let Err(line) = certify(base, ours, theirs, &out) {
        return why(|| format!("the union fails certification at line {line:?}"));
    }
    Some(out)
}

/// A refusal, said out loud when `WEAVE_DEBUG_ELEMENTS` is set: which guard
/// declined. Always `None`.
fn why<T>(reason: impl FnOnce() -> String) -> Option<T> {
    if std::env::var_os("WEAVE_DEBUG_ELEMENTS").is_some() {
        eprintln!("weave elements: refused: {}", reason());
    }
    None
}

fn parse(path: &str, text: &str) -> Option<Tree> {
    let (_, tree) = PARSER_REGISTRY.extract_entities_with_tree(path, text)?;
    let tree = tree?;
    (!tree.root_node().has_error()).then_some(tree)
}

// ---------------------------------------------------------------------------
// Framing: what a fragment needs around it to parse
// ---------------------------------------------------------------------------

struct Frame {
    pre: &'static str,
    post: &'static str,
    /// Python: the indentation every non-blank line carried, removed before
    /// parsing and restored after.
    indent: String,
}

impl Frame {
    fn find(l: Lang, path: &str, texts: [&str; 3]) -> Option<Frame> {
        let indent = if l == Lang::Py {
            common_indent(&texts)?
        } else {
            String::new()
        };
        let wrappers: &[(&str, &str)] = match l {
            Lang::Go => &[("", ""), ("package weave_fragment\n", "")],
            Lang::Js => &[
                ("", ""),
                ("class WeaveFragment {\n", "\n}\n"),
                ("function weaveFragment() {\n", "\n}\n"),
            ],
            Lang::Py => &[("", "")],
        };
        for (pre, post) in wrappers {
            let f = Frame {
                pre,
                post,
                indent: indent.clone(),
            };
            if texts
                .iter()
                .all(|t| f.wrap(t).is_some_and(|w| parse(path, &w).is_some()))
            {
                return Some(f);
            }
        }
        None
    }

    fn wrap(&self, text: &str) -> Option<String> {
        let mut body = String::with_capacity(text.len());
        if self.indent.is_empty() {
            body.push_str(text);
        } else {
            for line in text.split_inclusive('\n') {
                if line.trim().is_empty() {
                    body.push_str(if line.ends_with('\n') { "\n" } else { "" });
                } else {
                    body.push_str(line.strip_prefix(self.indent.as_str())?);
                }
            }
        }
        Some(format!("{}{}{}", self.pre, body, self.post))
    }

    fn unwrap(&self, merged: &str) -> Option<String> {
        let body = merged.strip_prefix(self.pre)?.strip_suffix(self.post)?;
        if self.indent.is_empty() {
            return Some(body.to_string());
        }
        let mut out = String::with_capacity(body.len() + 64);
        for line in body.split_inclusive('\n') {
            if !line.trim().is_empty() {
                out.push_str(&self.indent);
            }
            out.push_str(line);
        }
        Some(out)
    }
}

/// The whitespace prefix every non-blank line of all three texts shares, when
/// the first non-blank line of each is indented by exactly it.
fn common_indent(texts: &[&str; 3]) -> Option<String> {
    let first = texts[0].lines().find(|l| !l.trim().is_empty())?;
    let ind: String = first.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
    if ind.is_empty() {
        return Some(ind);
    }
    for t in texts {
        for l in t.lines() {
            if !l.trim().is_empty() && !l.starts_with(&ind) {
                return None;
            }
        }
    }
    Some(ind)
}

// ---------------------------------------------------------------------------
// The tree merge
// ---------------------------------------------------------------------------

struct M<'a> {
    lang: Lang,
    test_file: bool,
    scope: &'a dyn Fn() -> SetScope,
    /// base, ours, theirs
    srcs: [&'a str; 3],
}

fn kids(n: Node) -> Vec<Node> {
    (0..n.child_count() as u32).filter_map(|i| n.child(i)).collect()
}

fn norm(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

impl<'a> M<'a> {
    fn txt(&self, v: usize, n: Node) -> &'a str {
        &self.srcs[v][n.start_byte()..n.end_byte()]
    }

    /// The three-way answer where at most one side changed `x`.
    fn trivial<'s>(&self, x: [&'s str; 3]) -> Option<&'s str> {
        let [b, o, t] = x;
        if o == t || b == t {
            Some(o)
        } else if b == o {
            Some(t)
        } else {
            None
        }
    }

    fn merge_node(&self, n: [Node; 3]) -> Option<String> {
        let t: [&str; 3] = std::array::from_fn(|v| self.txt(v, n[v]));
        if let Some(x) = self.trivial(t) {
            return Some(x.to_string());
        }
        if n[0].kind() != n[1].kind() || n[0].kind() != n[2].kind() {
            return why(|| format!("both changed a `{}` into different kinds", n[0].kind()));
        }
        let colls: [Option<Coll>; 3] = std::array::from_fn(|v| self.collection(v, n[v]));
        if let [Some(cb), Some(co), Some(ct)] = colls {
            if cb.kind == co.kind && cb.kind == ct.kind {
                return self.merge_collection(n, [cb, co, ct]);
            }
            return why(|| format!("a `{}` is a different collection in each version", n[0].kind()));
        }
        // Descend: the same shape in all three, merged child by child.
        let c: [Vec<Node>; 3] = std::array::from_fn(|v| kids(n[v]));
        let line = n[0].start_position().row + 1;
        if c[0].len() != c[1].len() || c[0].len() != c[2].len() || c[0].is_empty() {
            return why(|| format!("both changed the shape of a `{}` (fragment line {line})", n[0].kind()));
        }
        for i in 0..c[0].len() {
            if c[0][i].kind() != c[1][i].kind() || c[0][i].kind() != c[2][i].kind() {
                return why(|| format!("both changed the shape of a `{}` (fragment line {line})", n[0].kind()));
            }
        }
        // Only collections compose finer than a line. Outside one, an edit only
        // ours made and an edit only theirs made (or one both made) on a
        // shared base line is the conflict a line merge would have reported —
        // `f(a, b)` → `f(x, b)` and `f(a, y)` is two claims about one call.
        let mut rows: [Vec<(usize, usize)>; 3] = Default::default(); // ours-only, theirs-only, both
        for i in 0..c[0].len() {
            let t: [&str; 3] = std::array::from_fn(|v| self.txt(v, c[v][i]));
            let span = (c[0][i].start_position().row, c[0][i].end_position().row);
            match (t[1] != t[0], t[2] != t[0]) {
                (true, false) => rows[0].push(span),
                (false, true) => rows[1].push(span),
                (true, true) if t[1] != t[2] => rows[2].push(span),
                _ => {}
            }
        }
        let meets = |a: &[(usize, usize)], b: &[(usize, usize)]| {
            a.iter().any(|x| b.iter().any(|y| x.0 <= y.1 && y.0 <= x.1))
        };
        if meets(&rows[0], &rows[1]) || meets(&rows[0], &rows[2]) || meets(&rows[1], &rows[2]) {
            return why(|| format!("edits by both sides share a line of a `{}` (line {line})", n[0].kind()));
        }
        let mut out = String::new();
        let mut prev: [usize; 3] = std::array::from_fn(|v| n[v].start_byte());
        for i in 0..c[0].len() {
            let gap: [&str; 3] =
                std::array::from_fn(|v| &self.srcs[v][prev[v]..c[v][i].start_byte()]);
            out.push_str(
                self.trivial(gap)
                    .or_else(|| why(|| format!("both changed the text between children (line {line})")))?,
            );
            out.push_str(&self.merge_node([c[0][i], c[1][i], c[2][i]])?);
            prev = std::array::from_fn(|v| c[v][i].end_byte());
        }
        let gap: [&str; 3] = std::array::from_fn(|v| &self.srcs[v][prev[v]..n[v].end_byte()]);
        out.push_str(self.trivial(gap)?);
        Some(out)
    }
}

// ---------------------------------------------------------------------------
// Collections
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// A mapping by the language: keys are identities.
    Keyed,
    /// Switch cases, keyed by their labels.
    Switch,
    /// A sequence literal: position is meaning unless it is a test table or
    /// declared.
    Positional,
    /// A statement list.
    Statements,
    /// A Go `const ( … )` block.
    Const { iota: bool },
}

struct Coll<'t> {
    kind: Kind,
    /// Elements are separated by commas.
    comma: bool,
    /// End of the opening delimiter (or the node start), start of the closing
    /// delimiter (or the node end).
    open_end: usize,
    close_start: usize,
    elems: Vec<Node<'t>>,
    /// Every child of the node, for slot boundaries.
    children: Vec<Node<'t>>,
}

impl<'a> M<'a> {
    /// What collection `n` is in version `v`, if a recogniser knows it.
    fn collection<'t>(&self, v: usize, n: Node<'t>) -> Option<Coll<'t>> {
        let src = self.srcs[v];
        let k = n.kind();
        let (kind, comma, open, close): (Kind, bool, &str, &str) = match (self.lang, k) {
            (Lang::Go, "literal_value") => {
                let keyed = kids(n).iter().any(|c| c.kind() == "keyed_element");
                let kind = if keyed { Kind::Keyed } else { Kind::Positional };
                (kind, true, "{", "}")
            }
            (Lang::Go, "expression_switch_statement") => {
                // A tagless switch is a chain of boolean conditions: order is
                // meaning.
                n.child_by_field_name("value")?;
                (Kind::Switch, false, "{", "}")
            }
            (Lang::Go, "block") => (Kind::Statements, false, "{", "}"),
            (Lang::Go, "source_file") => (Kind::Statements, false, "", ""),
            (Lang::Go, "const_declaration") => {
                if !kids(n).iter().any(|c| c.kind() == "(") {
                    return None;
                }
                let iota = contains_ident(n, src, "iota");
                (Kind::Const { iota }, false, "(", ")")
            }
            (Lang::Js, "object") => (Kind::Keyed, true, "{", "}"),
            (Lang::Js, "export_clause" | "named_imports") => (Kind::Keyed, true, "{", "}"),
            (Lang::Js, "array") => (Kind::Positional, true, "[", "]"),
            (Lang::Js, "switch_body") => (Kind::Switch, false, "{", "}"),
            (Lang::Js, "statement_block") => (Kind::Statements, false, "{", "}"),
            (Lang::Js, "program") => (Kind::Statements, false, "", ""),
            (Lang::Py, "dictionary" | "set") => (Kind::Keyed, true, "{", "}"),
            (Lang::Py, "list") => {
                if is_dunder_all(n, src) {
                    (Kind::Keyed, true, "[", "]")
                } else {
                    (Kind::Positional, true, "[", "]")
                }
            }
            (Lang::Py, "tuple") => {
                if !kids(n).iter().any(|c| c.kind() == "(") {
                    return None;
                }
                (Kind::Positional, true, "(", ")")
            }
            (Lang::Py, "block" | "module") => {
                // Arms of a `match` are first-match-wins.
                if n.parent().is_some_and(|p| p.kind() == "match_statement") {
                    return None;
                }
                (Kind::Statements, false, "", "")
            }
            _ => return None,
        };
        let children = kids(n);
        let (open_idx, open_end) = if open.is_empty() {
            (None, n.start_byte())
        } else {
            let i = children.iter().position(|c| c.kind() == open)?;
            (Some(i), children[i].end_byte())
        };
        let (close_idx, close_start) = if close.is_empty() {
            (None, n.end_byte())
        } else {
            let i = children.iter().rposition(|c| c.kind() == close)?;
            (Some(i), children[i].start_byte())
        };
        let lo = open_idx.map_or(0, |i| i + 1);
        let hi = close_idx.unwrap_or(children.len());
        if lo > hi {
            return None;
        }
        let mut elems = Vec::new();
        for c in &children[lo..hi] {
            if c.kind() == "comment" || (comma && c.kind() == ",") {
                continue;
            }
            if !c.is_named() {
                // Stray punctuation between elements (`;` in a Go block is a
                // terminator token): not an element, and not ours to place.
                if matches!(c.kind(), ";" | "\n") {
                    continue;
                }
                return None;
            }
            elems.push(*c);
        }
        Some(Coll {
            kind,
            comma,
            open_end,
            close_start,
            elems,
            children,
        })
    }
}

fn contains_ident(n: Node, src: &str, name: &str) -> bool {
    let mut stack = vec![n];
    while let Some(x) = stack.pop() {
        if (x.kind() == name || x.kind() == "identifier")
            && &src[x.start_byte()..x.end_byte()] == name
        {
            return true;
        }
        stack.extend(kids(x));
    }
    false
}

fn is_dunder_all(n: Node, src: &str) -> bool {
    n.parent().is_some_and(|p| {
        p.kind() == "assignment"
            && p.child_by_field_name("left")
                .is_some_and(|l| &src[l.start_byte()..l.end_byte()] == "__all__")
    })
}

/// One element of one version, with the text around it.
#[derive(Clone)]
struct Slot<'t> {
    node: Node<'t>,
    /// The version it was read from.
    v: usize,
    /// Text from the end of the previous slot to the element.
    lead: String,
    body: String,
    /// Whether a separating comma follows the element.
    comma: bool,
    /// What follows the element on its line, its comma removed.
    rest: String,
    /// The element's identity in its collection: `None` when it has none a
    /// union may rely on.
    key: Option<String>,
    /// Labels (switch) or names (declarations) that must stay unique.
    names: Vec<String>,
    /// The layout-free text, for alignment and "the same element".
    norm: String,
}

impl Slot<'_> {
    /// The same element, laid out the same: its lead, its text and what
    /// follows it on its line (a trailing comment is content). The comma is
    /// not compared — adding one so something can follow is not an edit.
    fn same(&self, other: &Slot) -> bool {
        self.lead == other.lead
            && self.body.trim_end() == other.body.trim_end()
            && self.rest.trim_end() == other.rest.trim_end()
    }
    fn render(&self, comma: bool) -> String {
        format!(
            "{}{}{}{}",
            self.lead,
            self.body,
            if comma { "," } else { "" },
            self.rest
        )
    }
    /// A slot on a line of its own with nothing but indentation before it:
    /// no comment, no blank line, no other element on the line.
    fn bare(&self) -> bool {
        self.lead.trim_matches([' ', '\t']) == "\n"
    }
}

impl<'a> M<'a> {
    fn slots<'t>(&self, v: usize, c: &Coll<'t>) -> Option<(Vec<Slot<'t>>, String)> {
        let src = self.srcs[v];
        let mut out = Vec::with_capacity(c.elems.len());
        let mut cursor = c.open_end;
        for e in &c.elems {
            let idx = c.children.iter().position(|x| x.id() == e.id())?;
            let mut end = e.end_byte();
            let mut comma = false;
            let mut j = idx + 1;
            if c.comma && j < c.children.len() && c.children[j].kind() == "," {
                end = c.children[j].end_byte();
                comma = true;
                j += 1;
            }
            // A comment on the element's own line travels with it.
            if j < c.children.len()
                && c.children[j].kind() == "comment"
                && c.children[j].start_position().row == e.end_position().row
                && c.children[j].start_byte() < c.close_start
            {
                end = c.children[j].end_byte();
            }
            if e.start_byte() < cursor {
                return None;
            }
            let after = &src[e.end_byte()..end];
            let rest = if comma {
                after.replacen(',', "", 1)
            } else {
                after.to_string()
            };
            let (key, names) = self.key(v, c.kind, *e);
            out.push(Slot {
                node: *e,
                v,
                lead: src[cursor..e.start_byte()].to_string(),
                body: src[e.start_byte()..e.end_byte()].to_string(),
                comma,
                rest,
                key,
                names,
                norm: norm(&src[e.start_byte()..e.end_byte()]),
            });
            cursor = end;
        }
        if cursor > c.close_start {
            return None;
        }
        Some((out, src[cursor..c.close_start].to_string()))
    }
}

// ---------------------------------------------------------------------------
// Keys
// ---------------------------------------------------------------------------

/// A string literal's content when it has no escapes or substitutions.
fn plain_string(n: Node, src: &str) -> Option<String> {
    let t = &src[n.start_byte()..n.end_byte()];
    if t.contains('\\') || t.contains("${") {
        return None;
    }
    let inner = t
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| t.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
        .or_else(|| t.strip_prefix('`').and_then(|s| s.strip_suffix('`')))?;
    Some(format!("s:{inner}"))
}

fn decimal(t: &str) -> Option<String> {
    (!t.is_empty() && t.chars().all(|c| c.is_ascii_digit()) && (t == "0" || !t.starts_with('0')))
        .then(|| format!("i:{t}"))
}

/// A name, possibly qualified: `x`, `pkg.X`, `exp.Foo`, `a.b.c`.
fn qualified_name(n: Node, src: &str) -> Option<String> {
    let t = &src[n.start_byte()..n.end_byte()];
    let ok = !t.is_empty()
        && t.split('.').all(|seg| {
            let mut ch = seg.chars();
            ch.next().is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$')
                && ch.all(|c| c.is_alphanumeric() || c == '_' || c == '$')
        });
    ok.then(|| format!("n:{t}"))
}

impl<'a> M<'a> {
    /// A constant or a name usable as a key or label.
    fn label(&self, v: usize, n: Node) -> Option<String> {
        let src = self.srcs[v];
        let t = &src[n.start_byte()..n.end_byte()];
        match (self.lang, n.kind()) {
            (Lang::Go, "literal_element") => {
                let inner = n.named_child(0)?;
                self.label(v, inner)
            }
            (Lang::Go, "interpreted_string_literal" | "raw_string_literal" | "rune_literal") => {
                plain_string(n, src)
            }
            (Lang::Go, "int_literal") => decimal(t),
            (Lang::Go, "identifier" | "field_identifier" | "selector_expression") => {
                qualified_name(n, src)
            }
            (Lang::Js, "string") => plain_string(n, src),
            (Lang::Js, "number") => decimal(t).map(|d| format!("s:{}", &d[2..])),
            (Lang::Js, "property_identifier" | "shorthand_property_identifier") => {
                Some(format!("s:{t}")).filter(|k| k != "s:__proto__")
            }
            (Lang::Js, "identifier" | "member_expression") => qualified_name(n, src),
            (Lang::Py, "string") => {
                if t.starts_with(['f', 'F', 'b', 'B', 'r', 'R']) {
                    return None;
                }
                plain_string(n, src)
            }
            (Lang::Py, "integer") => decimal(t),
            (Lang::Py, "identifier" | "attribute") => {
                if matches!(t, "True" | "False" | "None") {
                    return None;
                }
                qualified_name(n, src)
            }
            _ => None,
        }
    }

    /// The element's key and the names it claims.
    fn key(&self, v: usize, kind: Kind, e: Node) -> (Option<String>, Vec<String>) {
        let src = self.srcs[v];
        let text = &src[e.start_byte()..e.end_byte()];
        match kind {
            Kind::Keyed => {
                let k = match (self.lang, e.kind()) {
                    (Lang::Go, "keyed_element") => e
                        .child_by_field_name("key")
                        .or_else(|| e.named_child(0))
                        .and_then(|k| self.label(v, k)),
                    (Lang::Js, "pair") => e
                        .child_by_field_name("key")
                        .and_then(|k| self.label(v, k)),
                    (Lang::Js, "shorthand_property_identifier") => self.label(v, e),
                    (Lang::Js, "method_definition") => {
                        let accessor = kids(e)
                            .iter()
                            .take_while(|c| !c.is_named())
                            .any(|c| matches!(c.kind(), "get" | "set" | "static"));
                        if accessor {
                            None
                        } else {
                            e.child_by_field_name("name")
                                .and_then(|k| self.label(v, k))
                        }
                    }
                    (Lang::Js, "export_specifier" | "import_specifier") => e
                        .child_by_field_name("alias")
                        .or_else(|| e.child_by_field_name("name"))
                        .map(|k| format!("s:{}", &src[k.start_byte()..k.end_byte()])),
                    (Lang::Py, "pair") => e
                        .child_by_field_name("key")
                        .and_then(|k| self.label(v, k)),
                    // A set literal or `__all__`: the element is its own key.
                    (Lang::Py, "string" | "integer") => self.label(v, e),
                    _ => None,
                };
                (k.clone(), k.into_iter().collect())
            }
            Kind::Switch => {
                let labels: Option<Vec<String>> = match (self.lang, e.kind()) {
                    (Lang::Go, "expression_case") => e.child_by_field_name("value").and_then(|vl| {
                        let ls: Vec<Node> = kids(vl).into_iter().filter(|c| c.is_named()).collect();
                        ls.iter().map(|l| self.label(v, *l)).collect()
                    }),
                    (Lang::Js, "switch_case") => e
                        .child_by_field_name("value")
                        .and_then(|l| self.label(v, l))
                        .map(|l| vec![l]),
                    (Lang::Go, "default_case") | (Lang::Js, "switch_default") => {
                        Some(vec!["default".to_string()])
                    }
                    _ => None,
                };
                match labels {
                    Some(mut ls) if !ls.is_empty() => {
                        ls.sort();
                        (Some(format!("case:{}", ls.join("|"))), ls)
                    }
                    _ => (None, Vec::new()),
                }
            }
            Kind::Const { .. } => {
                let names: Vec<String> = kids(e)
                    .iter()
                    .filter(|c| c.kind() == "identifier")
                    .map(|c| src[c.start_byte()..c.end_byte()].to_string())
                    .collect();
                if e.kind() != "const_spec" || names.is_empty() {
                    return (None, Vec::new());
                }
                (Some(format!("const:{}", names.join(","))), names)
            }
            Kind::Positional => (Some(format!("t:{}", norm(text))), Vec::new()),
            Kind::Statements => {
                let names = self.declared_name(v, e).into_iter().collect();
                (Some(format!("t:{}", norm(text))), names)
            }
        }
    }

    /// The name a hoisted declaration binds, if `e` is one.
    fn declared_name(&self, v: usize, e: Node) -> Option<String> {
        let src = self.srcs[v];
        let name = |n: Node| -> Option<String> {
            n.child_by_field_name("name")
                .map(|x| src[x.start_byte()..x.end_byte()].to_string())
        };
        match (self.lang, e.kind()) {
            (Lang::Js, "function_declaration" | "generator_function_declaration") => {
                name(e).map(|n| format!("fn:{n}"))
            }
            (Lang::Go, "function_declaration" | "type_declaration")
                if e.parent().is_some_and(|p| p.kind() == "source_file") =>
            {
                let n = name(e).or_else(|| {
                    e.named_child(0)
                        .and_then(|s| name(s))
                })?;
                Some(format!("fn:{n}"))
            }
            (Lang::Go, "method_declaration")
                if e.parent().is_some_and(|p| p.kind() == "source_file") =>
            {
                let recv = e.child_by_field_name("receiver")?;
                let rt = norm(&src[recv.start_byte()..recv.end_byte()]);
                let rt = rt.split_whitespace().last()?.trim_matches(['(', ')', '*']).to_string();
                Some(format!("fn:{rt}.{}", name(e)?))
            }
            (Lang::Py, "function_definition") => name(e).map(|n| format!("fn:{n}")),
            (Lang::Py, "decorated_definition") => {
                let d = e.child_by_field_name("definition")?;
                (d.kind() == "function_definition")
                    .then(|| name(d).map(|n| format!("fn:{n}")))
                    .flatten()
            }
            _ => None,
        }
    }

    /// A registry call statement `recv.method("key", …)`: (callee, key).
    fn registry_call(&self, v: usize, e: Node) -> Option<(String, String)> {
        let src = self.srcs[v];
        let call = match (self.lang, e.kind()) {
            (Lang::Js, "expression_statement") | (Lang::Py, "expression_statement") => {
                e.named_child(0)?
            }
            (Lang::Go, "expression_statement") => e.named_child(0)?,
            _ => return None,
        };
        if !matches!(call.kind(), "call_expression" | "call") {
            return None;
        }
        let f = call.child_by_field_name("function")?;
        let callee = norm(&src[f.start_byte()..f.end_byte()]);
        qualified_name(f, src)?;
        let args = call.child_by_field_name("arguments")?;
        let first = args.named_child(0)?;
        let key = self.label(v, first).filter(|k| k.starts_with("s:"))?;
        Some((callee, key))
    }

    /// The names a `weave-set` declaration may call this collection by.
    fn names_of(&self, v: usize, n: Node) -> Vec<String> {
        let src = self.srcs[v];
        let t = |x: Node| norm(&src[x.start_byte()..x.end_byte()]);
        let mut at = n.parent();
        let mut out = Vec::new();
        while let Some(p) = at {
            let field = |f: &str| p.child_by_field_name(f).map(t);
            let found: Vec<String> = match p.kind() {
                "assignment" | "augmented_assignment" | "assignment_expression" => {
                    field("left").into_iter().collect()
                }
                "short_var_declaration" | "assignment_statement" => {
                    field("left").into_iter().collect()
                }
                "variable_declarator" | "var_spec" | "const_spec" | "keyword_argument" => {
                    field("name").into_iter().collect()
                }
                "pair" | "keyed_element" => field("key")
                    .map(|k| k.trim_matches(['"', '\'', '`']).to_string())
                    .into_iter()
                    .collect(),
                "call" | "call_expression" => match field("function") {
                    Some(f) => {
                        let last = f.rsplit('.').next().unwrap_or(&f).to_string();
                        if last == f {
                            vec![f]
                        } else {
                            vec![f.clone(), last]
                        }
                    }
                    None => Vec::new(),
                },
                "function_declaration" | "method_declaration" | "function_definition" => {
                    field("name").into_iter().collect()
                }
                _ => Vec::new(),
            };
            if !found.is_empty() {
                out.extend(found);
                break;
            }
            at = p.parent();
        }
        out
    }

    /// A table of test cases: a sequence literal in a test file bound to a
    /// name (`tests := []struct{…}{…}`, `cases = [...]`). A literal passed to
    /// a call is not one — `@pytest.mark.parametrize` names its test ids by
    /// position, and stays opt-in.
    fn test_table(&self, n: Node) -> bool {
        if !self.test_file {
            return false;
        }
        let mut at = n.parent();
        while let Some(p) = at {
            match p.kind() {
                "composite_literal" | "expression_list" | "parenthesized_expression" => {
                    at = p.parent();
                }
                "short_var_declaration" | "var_spec" | "assignment_statement" | "assignment"
                | "variable_declarator" | "assignment_expression" => return true,
                _ => return false,
            }
        }
        false
    }

    fn declared(&self, names: &[String]) -> bool {
        !names.is_empty() && (self.scope)().admits(names)
    }
}

// ---------------------------------------------------------------------------
// Alignment
// ---------------------------------------------------------------------------

/// One side read against the base: where each base element went (`None` =
/// deleted), and the runs it inserted after each base element (`None` =
/// before the first).
struct Align {
    fate: Vec<Option<usize>>,
    runs: Vec<(Option<usize>, Vec<usize>)>,
}

fn align_keyed(base: &[Slot], side: &[Slot]) -> Option<Align> {
    let key = |s: &Slot| s.key.clone().unwrap_or_else(|| format!("#{}", s.norm));
    // A key stated twice is told apart by its occurrence — but only a key the
    // base already stated twice (a dict whose later entry wins), and only
    // while the side keeps every copy: otherwise "which copy went" is a guess.
    let idents = |v: &[Slot]| -> Vec<String> {
        let mut n: HashMap<String, usize> = HashMap::new();
        v.iter()
            .map(|s| {
                let k = key(s);
                let c = n.entry(k.clone()).or_insert(0);
                *c += 1;
                format!("{k}#{}", *c - 1)
            })
            .collect()
    };
    let count = |v: &[Slot]| -> HashMap<String, usize> {
        let mut n = HashMap::new();
        for s in v {
            *n.entry(key(s)).or_insert(0) += 1;
        }
        n
    };
    let (cb, cs) = (count(base), count(side));
    for (k, n) in &cs {
        let b = cb.get(k).copied().unwrap_or(0);
        if *n > 1 && *n != b || b > 1 && *n != b && *n != 0 {
            return None;
        }
    }
    let at: HashMap<String, usize> = idents(side).into_iter().enumerate().map(|(j, k)| (k, j)).collect();
    let fate = idents(base).iter().map(|k| at.get(k).copied()).collect();
    finish(fate, side.len())
}

fn align_lcs(base: &[Slot], side: &[Slot]) -> Option<Align> {
    let (n, m) = (base.len(), side.len());
    let mut dp = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if base[i].norm == side[j].norm {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if base[i].norm == side[j].norm {
            pairs.push((i, j));
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    let mut fate: Vec<Option<usize>> = vec![None; n];
    let mut bounds = pairs.clone();
    bounds.push((n, m));
    let (mut pi, mut pj) = (0usize, 0usize);
    for (bi, bj) in bounds {
        // The gap: base [pi, bi), side [pj, bj). Base elements here were
        // modified or deleted; side elements are modifications or insertions.
        let (k, l) = (bi - pi, bj - pj);
        if k > 0 && l > 0 {
            for (i, j) in pair_gap(&base[pi..bi], &side[pj..bj])? {
                fate[pi + i] = Some(pj + j);
            }
        }
        if bi < n {
            fate[bi] = Some(bj);
        }
        pi = bi + 1;
        pj = bj + 1;
    }
    finish(fate, m)
}

/// Which element of a gap each modified base element became: one base and one
/// side element are each other's; otherwise every base element must find the
/// one side element of its signature (its declared name, else its first
/// line), in order, the signatures unique on both sides of the gap. A base
/// element with no partner was deleted; that is decided later, against what
/// the other side did to it.
fn pair_gap(base: &[Slot], side: &[Slot]) -> Option<Vec<(usize, usize)>> {
    if base.len() == 1 && side.len() == 1 {
        return Some(vec![(0, 0)]);
    }
    let sig = |s: &Slot| -> String {
        s.names
            .first()
            .cloned()
            .unwrap_or_else(|| norm(s.body.lines().next().unwrap_or("")))
    };
    let unique = |v: &[Slot]| {
        let mut seen = HashSet::new();
        v.iter().all(|s| seen.insert(sig(s)))
    };
    if !unique(base) || !unique(side) {
        return None;
    }
    let mut out = Vec::new();
    let mut from = 0;
    for (i, b) in base.iter().enumerate() {
        if let Some(j) = side[from..].iter().position(|s| sig(s) == sig(b)) {
            out.push((i, from + j));
            from += j + 1;
        } else if side.iter().any(|s| sig(s) == sig(b)) {
            return None; // matched out of order: a move
        }
    }
    Some(out)
}

/// Check the matched elements keep base order and collect the insertions.
fn finish(fate: Vec<Option<usize>>, m: usize) -> Option<Align> {
    let mut last: Option<usize> = None;
    let mut owner: Vec<Option<usize>> = vec![None; m];
    for (i, f) in fate.iter().enumerate() {
        if let Some(j) = f {
            if last.is_some_and(|p| *j <= p) {
                return None; // moved
            }
            last = Some(*j);
            owner[*j] = Some(i);
        }
    }
    let mut runs: Vec<(Option<usize>, Vec<usize>)> = Vec::new();
    let mut anchor: Option<usize> = None;
    for (j, o) in owner.iter().enumerate() {
        match o {
            Some(i) => anchor = Some(*i),
            None => match runs.last_mut() {
                Some((a, r)) if *a == anchor && r.last() == Some(&(j - 1)) => r.push(j),
                _ => runs.push((anchor, vec![j])),
            },
        }
    }
    Some(Align { fate, runs })
}

// ---------------------------------------------------------------------------
// The collection merge
// ---------------------------------------------------------------------------

/// An element of the answer: its slot, and the version it was read from.
#[derive(Clone)]
struct Piece<'t> {
    slot: Slot<'t>,
    v: usize,
    /// Comma decided for it when it is the last element.
    comma: bool,
    inserted: bool,
}

impl<'a> M<'a> {
    fn merge_collection(&self, n: [Node; 3], c: [Coll; 3]) -> Option<String> {
        let kind = c[0].kind;
        let line = n[0].start_position().row + 1;
        let what = format!("{:?} `{}` at fragment line {line}", kind, n[0].kind());
        let read: [(Vec<Slot>, String); 3] = [
            self.slots(0, &c[0]).or_else(|| why(|| format!("{what}: slots do not tile")))?,
            self.slots(1, &c[1]).or_else(|| why(|| format!("{what}: slots do not tile")))?,
            self.slots(2, &c[2]).or_else(|| why(|| format!("{what}: slots do not tile")))?,
        ];
        let (bs, os, ts) = (&read[0].0, &read[1].0, &read[2].0);
        let align = |side: &[Slot]| match kind {
            Kind::Positional | Kind::Statements => align_lcs(bs, side),
            _ => align_keyed(bs, side),
        };
        let (ao, at) = (
            align(os).or_else(|| why(|| format!("{what}: ours does not align with base (a move, or duplicate keys)")))?,
            align(ts).or_else(|| why(|| format!("{what}: theirs does not align with base (a move, or duplicate keys)")))?,
        );
        let sides: [(&Align, &Vec<Slot>, usize); 2] = [(&ao, os, 1), (&at, ts, 2)];

        // Did a side change the edge of base element `i` that faces a gap —
        // delete it, or rewrite its last line (`pred`: the gap follows it) or
        // its lead and first line (the gap precedes it)? A change deep inside
        // the element says nothing about what sits next to it.
        let edge_changed = |a: &Align, side: &[Slot], i: usize, pred: bool| match a.fate[i] {
            None => true,
            Some(j) => {
                let (b, s) = (&bs[i], &side[j]);
                if s.same(b) {
                    false
                } else if pred {
                    b.body.trim_end().lines().last() != s.body.trim_end().lines().last()
                } else {
                    b.lead != s.lead || b.body.lines().next() != s.body.lines().next()
                }
            }
        };
        let order_free = |anchor: Option<usize>| self.order_free_gap(kind, &c[0], n[0], bs, anchor);

        // Changes to existing elements compose at line granularity, as
        // everywhere in weave: an element only ours changed (or deleted) and
        // one only theirs changed may not share a base line — `{a: 1, b: 2}`
        // edited to `a: 9` and to `b: 8` is a one-line conflict. Insertions
        // are what this rung unites, and are not counted here.
        {
            let gone = |a: &Align, side: &[Slot], i: usize| match a.fate[i] {
                None => true,
                Some(j) => !side[j].same(&bs[i]),
            };
            let mut rows: [Vec<(usize, usize)>; 3] = Default::default();
            for (i, b) in bs.iter().enumerate() {
                let span = (b.node.start_position().row, b.node.end_position().row);
                match (gone(&ao, os, i), gone(&at, ts, i)) {
                    (true, false) => rows[0].push(span),
                    (false, true) => rows[1].push(span),
                    (true, true) => rows[2].push(span),
                    _ => {}
                }
            }
            let meets = |a: &[(usize, usize)], b: &[(usize, usize)]| {
                a.iter().any(|x| b.iter().any(|y| x.0 <= y.1 && y.0 <= x.1))
            };
            if meets(&rows[0], &rows[1]) || meets(&rows[0], &rows[2]) || meets(&rows[1], &rows[2]) {
                return why(|| format!("{what}: edits by both sides to elements on one line"));
            }
        }

        // Base elements, decided.
        let mut base_pieces: Vec<Option<Piece>> = Vec::with_capacity(bs.len());
        for i in 0..bs.len() {
            let (fo, ft) = (ao.fate[i], at.fate[i]);
            let piece = match (fo, ft) {
                (Some(j), Some(k)) => {
                    let (o, t, b) = (&os[j], &ts[k], &bs[i]);
                    let comma = *self.trivial_bool([b.comma, o.comma, t.comma]).as_ref()?;
                    let (slot, v) = if o.same(b) && t.same(b) {
                        (b.clone(), 0)
                    } else if o.same(b) {
                        (t.clone(), 2)
                    } else if t.same(b) || o.same(t) {
                        // Equal reads: the smaller text, whichever side wrote it.
                        if t.same(o) && t.render(t.comma) < o.render(o.comma) {
                            (t.clone(), 2)
                        } else {
                            (o.clone(), 1)
                        }
                    } else {
                        // One side moving the comment above an element and the
                        // other editing the element is two edits to adjacent
                        // lines: a conflict, as in a line merge.
                        if o.lead != t.lead || o.rest.trim_end() != t.rest.trim_end() {
                            return why(|| format!("{what}: one side changed the text around an element the other edited"));
                        }
                        let lead = self
                            .trivial([b.lead.as_str(), o.lead.as_str(), t.lead.as_str()])
                            .or_else(|| why(|| format!("{what}: both changed the text above one element")))?
                            .to_string();
                        let rest = self
                            .trivial([b.rest.as_str(), o.rest.as_str(), t.rest.as_str()])
                            .or_else(|| why(|| format!("{what}: both changed the text after one element")))?
                            .to_string();
                        let body = self.merge_node([b.node, o.node, t.node])?;
                        let mut s = b.clone();
                        s.lead = lead;
                        s.body = body;
                        s.rest = rest;
                        // Keys of a merged element must not move: a keyed
                        // element's key is in its body, and both sides kept it.
                        (s, 0)
                    };
                    Some(Piece {
                        slot,
                        v,
                        comma,
                        inserted: false,
                    })
                }
                (None, Some(k)) => {
                    if !ts[k].same(&bs[i]) {
                        return why(|| format!("{what}: an element deleted by one side, changed by the other"));
                    }
                    None
                }
                (Some(j), None) => {
                    if !os[j].same(&bs[i]) {
                        return why(|| format!("{what}: an element deleted by one side, changed by the other"));
                    }
                    None
                }
                (None, None) => None,
            };
            base_pieces.push(piece);
        }

        // Insertions, per anchor.
        let mut gaps: HashMap<Option<usize>, [Vec<Slot>; 2]> = HashMap::new();
        for (si, (a, side, _v)) in sides.iter().enumerate() {
            for (anchor, run) in &a.runs {
                let entry = gaps.entry(*anchor).or_insert_with(|| [Vec::new(), Vec::new()]);
                for j in run {
                    entry[si].push(side[*j].clone());
                }
            }
        }
        let order_sensitive_kind = !matches!(kind, Kind::Keyed | Kind::Const { iota: false })
            && !(kind == Kind::Positional && self.test_table(n[0]));
        let mut inserted: HashMap<Option<usize>, Vec<Piece>> = HashMap::new();
        let mut anchors: Vec<Option<usize>> = gaps.keys().copied().collect();
        anchors.sort();
        for anchor in anchors {
            let [ro, rt] = gaps.remove(&anchor)?;
            // Every inserted element must have a key a union can rely on.
            if let Some(s) = ro.iter().chain(&rt).find(|s| s.key.is_none()) {
                let t = s.norm.chars().take(60).collect::<String>();
                return why(|| format!("{what}: inserted element without a decidable key: {t}"));
            }
            // Next to an element the other side changed, an insertion into an
            // order-sensitive collection is a claim about order.
            // Hoisted declarations have no position to lose.
            let declarations = kind == Kind::Statements
                && ro.iter().chain(&rt).all(|s| !s.names.is_empty());
            if order_sensitive_kind && !declarations {
                let succ = anchor.map_or(0, |a| a + 1);
                let touched = |a: &Align, side: &[Slot]| {
                    anchor.is_some_and(|p| edge_changed(a, side, p, true))
                        || (succ < bs.len() && edge_changed(a, side, succ, false))
                };
                if !ro.is_empty() && touched(&at, ts) || !rt.is_empty() && touched(&ao, os) {
                    return why(|| format!("{what}: an insertion next to an element the other side changed"));
                }
            }
            let run: Vec<(Slot, usize)> = if ro.is_empty() || rt.is_empty() {
                let v = if ro.is_empty() { 2 } else { 1 };
                ro.into_iter().chain(rt).map(|s| (s, v)).collect()
            } else {
                if !order_free(anchor)
                    && !self.declared_union(kind, &c[0], n[0], &ro, &rt, bs)
                {
                    let names = self.names_of(0, n[0]);
                    return why(|| format!("{what}: both sides inserted at one point of an ordered collection (names {names:?})"));
                }
                order_runs(ro, rt)
                    .or_else(|| why(|| format!("{what}: one key inserted by both sides with different text")))?
            };
            inserted.insert(
                anchor,
                run.into_iter()
                    .map(|(slot, v)| Piece {
                        comma: slot.comma,
                        slot,
                        v,
                        inserted: true,
                    })
                    .collect(),
            );
        }

        // The answer's sequence.
        let mut seq: Vec<Piece> = Vec::new();
        if let Some(r) = inserted.remove(&None) {
            seq.extend(r);
        }
        for (i, p) in base_pieces.into_iter().enumerate() {
            if let Some(p) = p {
                seq.push(p);
            }
            if let Some(r) = inserted.remove(&Some(i)) {
                seq.extend(r);
            }
        }
        if !inserted.is_empty() {
            return None;
        }
        self.check_unique(kind, &seq)
            .or_else(|| why(|| format!("{what}: a key, label or name would be stated twice")))?;
        if kind == Kind::Switch {
            self.check_switch(&seq)
                .or_else(|| why(|| format!("{what}: a case could fall through (or `fallthrough`)")))?;
        }

        // Render.
        let prefix: [&str; 3] = std::array::from_fn(|v| &self.srcs[v][n[v].start_byte()..c[v].open_end]);
        let suffix: [&str; 3] = std::array::from_fn(|v| &self.srcs[v][c[v].close_start..n[v].end_byte()]);
        let tail: [&str; 3] = std::array::from_fn(|v| read[v].1.as_str());
        let mut out = String::new();
        out.push_str(self.trivial(prefix)?);
        let last = seq.len().saturating_sub(1);
        for (k, p) in seq.iter().enumerate() {
            // The comma goes right after the element, ahead of any trailing
            // comment, so adding one cannot comment it out.
            let comma = c[0].comma && (k < last || p.comma);
            out.push_str(&p.slot.render(comma));
        }
        out.push_str(self.trivial(tail)?);
        out.push_str(self.trivial(suffix)?);
        Some(out)
    }

    fn trivial_bool(&self, x: [bool; 3]) -> Option<bool> {
        let [b, o, t] = x;
        Some(if o == t || b == t { o } else { t })
    }

    /// May both sides' insertions at `anchor` be united by the language?
    fn order_free_gap(
        &self,
        kind: Kind,
        c: &Coll,
        n: Node,
        bs: &[Slot],
        anchor: Option<usize>,
    ) -> bool {
        match kind {
            Kind::Keyed | Kind::Const { iota: false } => true,
            // Appending after every base constant renumbers nothing that
            // exists; inserting ahead of one renumbers it and all after.
            Kind::Const { iota: true } => {
                !bs.is_empty() && anchor == Some(bs.len() - 1)
            }
            Kind::Switch => true, // the per-case guards are in `check_switch`
            Kind::Positional => self.test_table(n),
            Kind::Statements => {
                let _ = (c, n);
                false // decided per run in `declared_union`
            }
        }
    }

    /// The opt-in and the declaration rule for one gap both sides inserted at.
    fn declared_union(
        &self,
        kind: Kind,
        c: &Coll,
        n: Node,
        ro: &[Slot],
        rt: &[Slot],
        bs: &[Slot],
    ) -> bool {
        let all = || ro.iter().chain(rt);
        match kind {
            Kind::Statements => {
                // Hoisted declarations: order is not observable.
                if all().all(|s| !s.names.is_empty()) {
                    return true;
                }
                // A declared registry: every inserted statement calls one
                // declared function with a distinct string key, none of which
                // the base already registers.
                let calls: Option<Vec<(String, String)>> = all()
                    .map(|s| self.registry_call(s.v, s.node))
                    .collect();
                let Some(calls) = calls else {
                    return false;
                };
                let callee = &calls[0].0;
                if calls.iter().any(|(f, _)| f != callee) {
                    return false;
                }
                let last = callee.rsplit('.').next().unwrap_or(callee).to_string();
                if !self.declared(&[callee.clone(), last]) {
                    return false;
                }
                let mut keys: HashSet<&String> = HashSet::new();
                for (_, k) in &calls {
                    if !keys.insert(k) {
                        // The same key from both sides is one registration
                        // only if it is the same text — `order_runs` decides.
                    }
                }
                let base_keys: HashSet<String> = bs
                    .iter()
                    .filter_map(|s| self.registry_call(0, s.node))
                    .filter(|(f, _)| f == callee)
                    .map(|(_, k)| k)
                    .collect();
                !calls.iter().any(|(_, k)| base_keys.contains(k))
            }
            Kind::Positional | Kind::Const { iota: true } => {
                let mut names = self.names_of(0, n);
                if let Kind::Const { .. } = kind {
                    // A const block is named by its type and its first name.
                    if let Some(first) = c.elems.first() {
                        let src = self.srcs[0];
                        if let Some(ty) = first.child_by_field_name("type") {
                            names.push(src[ty.start_byte()..ty.end_byte()].to_string());
                        }
                        if let Some(nm) = first.child_by_field_name("name") {
                            names.push(src[nm.start_byte()..nm.end_byte()].to_string());
                        }
                    }
                }
                self.declared(&names)
            }
            _ => false,
        }
    }

    /// No key, label or declared name stated twice in the answer.
    fn check_unique(&self, kind: Kind, seq: &[Piece]) -> Option<()> {
        // What the base already states twice is the base's business (a dict
        // whose later entry wins); an insertion may not join it.
        let mut seen: HashSet<String> = HashSet::new();
        let mut base_ids: HashSet<String> = HashSet::new();
        for p in seq.iter().filter(|p| !p.inserted) {
            base_ids.extend(p.slot.names.iter().cloned());
            if let (Kind::Keyed | Kind::Switch | Kind::Const { .. }, Some(k)) = (kind, &p.slot.key) {
                base_ids.insert(k.clone());
            }
        }
        for p in seq {
            if !p.inserted {
                continue;
            }
            let mut ids: Vec<String> = p.slot.names.clone();
            match kind {
                Kind::Keyed | Kind::Switch | Kind::Const { .. } => {
                    if let Some(k) = &p.slot.key {
                        ids.push(k.clone());
                    }
                }
                // Text keys repeat legitimately (`c.emit(OpPop)` twice in one
                // block); what must not happen is both sides inserting one
                // element at two places — checked below.
                Kind::Positional | Kind::Statements => {}
            }
            ids.sort();
            ids.dedup();
            for id in ids {
                if base_ids.contains(&id) || !seen.insert(id) {
                    return None;
                }
            }
        }
        // One element inserted by both sides survives once (`order_runs`
        // folds it where both put it at one point); at two points it would be
        // stated twice.
        let mut origin: HashMap<&String, usize> = HashMap::new();
        for p in seq.iter().filter(|p| p.inserted) {
            if let Some(k) = &p.slot.key {
                match origin.get(k) {
                    Some(v) if *v != p.slot.v => return None,
                    _ => {
                        origin.insert(k, p.slot.v);
                    }
                }
            }
        }
        Some(())
    }

    /// Switch guards on the answer's case sequence.
    fn check_switch(&self, seq: &[Piece]) -> Option<()> {
        match self.lang {
            Lang::Go => {
                // `fallthrough` makes adjacency meaning.
                for p in seq {
                    let src = self.srcs[p.v];
                    if subtree_has(p.slot.node, src, |k| k == "fallthrough_statement") {
                        return None;
                    }
                }
                Some(())
            }
            Lang::Js => {
                for (k, p) in seq.iter().enumerate() {
                    if !p.inserted {
                        continue;
                    }
                    if !js_case_terminates(p.slot.node) {
                        return None;
                    }
                    if k > 0 && !js_case_terminates(seq[k - 1].slot.node) {
                        return None;
                    }
                }
                Some(())
            }
            Lang::Py => None,
        }
    }
}

fn subtree_has(n: Node, _src: &str, pred: impl Fn(&str) -> bool + Copy) -> bool {
    let mut stack = vec![n];
    while let Some(x) = stack.pop() {
        if pred(x.kind()) {
            return true;
        }
        // A nested switch's fallthrough is its own business — but refusing
        // there too is the fail-closed reading.
        stack.extend(kids(x));
    }
    false
}

/// Does a JS case end in an unconditional exit, so nothing falls out of it?
fn js_case_terminates(case: Node) -> bool {
    let body: Vec<Node> = kids(case)
        .into_iter()
        .skip_while(|c| c.kind() != ":")
        .skip(1)
        .filter(|c| c.is_named() && c.kind() != "comment")
        .collect();
    let Some(last) = body.last() else {
        return false; // an empty case groups with the next one
    };
    terminates(*last)
}

fn terminates(s: Node) -> bool {
    match s.kind() {
        "break_statement" | "return_statement" | "throw_statement" | "continue_statement" => true,
        "statement_block" => kids(s)
            .into_iter()
            .filter(|c| c.is_named() && c.kind() != "comment")
            .last()
            .is_some_and(terminates),
        _ => false,
    }
}

/// Order the two runs inserted at one point. Bare runs merge by key, keeping
/// each run's own order; a run with comments, blank lines or several elements
/// on a line is kept whole, the run whose keys sort first going first. One
/// key in both runs is one element if its text is the same, a refusal if not.
fn order_runs<'t>(a: Vec<Slot<'t>>, b: Vec<Slot<'t>>) -> Option<Vec<(Slot<'t>, usize)>> {
    let a: Vec<(Slot, usize)> = a.into_iter().map(|s| (s, 1)).collect();
    let b: Vec<(Slot, usize)> = b.into_iter().map(|s| (s, 2)).collect();
    let key = |s: &Slot| s.key.clone().unwrap_or_default();
    let mut out: Vec<(Slot, usize)> = Vec::new();
    let push = |x: (Slot<'t>, usize), out: &mut Vec<(Slot<'t>, usize)>| -> Option<()> {
        match out.iter_mut().find(|(s, _)| key(s) == key(&x.0)) {
            Some((s, _)) if s.norm == x.0.norm => {
                // The same element from both sides: once, the smaller text.
                if x.0.render(x.0.comma) < s.render(s.comma) {
                    *s = x.0;
                }
                Some(())
            }
            Some(_) => None,
            None => {
                out.push(x);
                Some(())
            }
        }
    };
    let bare = a.iter().chain(&b).all(|(s, _)| s.bare());
    if bare {
        let (mut i, mut j) = (0, 0);
        while i < a.len() || j < b.len() {
            let take_a = match (a.get(i), b.get(j)) {
                (Some(x), Some(y)) => {
                    let (kx, ky) = (key(&x.0), key(&y.0));
                    if kx == ky {
                        if x.0.norm != y.0.norm {
                            return None;
                        }
                        push(x.clone(), &mut out)?;
                        push(y.clone(), &mut out)?;
                        i += 1;
                        j += 1;
                        continue;
                    }
                    kx < ky
                }
                (Some(_), None) => true,
                _ => false,
            };
            if take_a {
                push(a[i].clone(), &mut out)?;
                i += 1;
            } else {
                push(b[j].clone(), &mut out)?;
                j += 1;
            }
        }
        return Some(out);
    }
    let rank = |r: &[(Slot, usize)]| -> (Vec<String>, Vec<String>) {
        (
            r.iter().map(|(s, _)| key(s)).collect(),
            r.iter().map(|(s, _)| s.render(s.comma)).collect(),
        )
    };
    let (first, second) = if rank(&a) <= rank(&b) { (a, b) } else { (b, a) };
    for x in first.into_iter().chain(second) {
        push(x, &mut out)?;
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// Certification
// ---------------------------------------------------------------------------

/// The answer keeps every line all three kept, carries every line each side
/// wrote (less what the other side deleted), and states no line more often
/// than the two sides' edits together do. Lines are compared trimmed and
/// without commas: the union adds separating commas, ahead of a trailing
/// comment where there is one.
fn certify(base: &str, ours: &str, theirs: &str, out: &str) -> Result<(), String> {
    if crate::container::drops_unanimous_lines(base, ours, theirs, out) {
        return Err("(a line all three versions keep)".into());
    }
    fn counts(s: &str) -> HashMap<String, usize> {
        let mut m = HashMap::new();
        for l in s.lines() {
            let t: String = l.trim().chars().filter(|c| *c != ',').collect();
            let t = t.trim_end();
            if !t.is_empty() {
                *m.entry(t.to_string()).or_insert(0) += 1;
            }
        }
        m
    }
    let (cb, co, ct, cx) = (counts(base), counts(ours), counts(theirs), counts(out));
    let get = |m: &HashMap<String, usize>, l: &str| m.get(l).copied().unwrap_or(0);
    let mut lines: HashSet<&String> = co.keys().collect();
    lines.extend(ct.keys());
    lines.extend(cx.keys());
    for l in lines {
        let (b, o, t, x) = (get(&cb, l), get(&co, l), get(&ct, l), get(&cx, l));
        // What the other side deleted of base's copies.
        let del_by_t = b.saturating_sub(t);
        let del_by_o = b.saturating_sub(o);
        if x < o.saturating_sub(del_by_t) || x < t.saturating_sub(del_by_o) {
            return Err(format!("lost: {l}"));
        }
        let ceiling = o.max(t).max((o + t).saturating_sub(b));
        if x > ceiling {
            return Err(format!("doubled: {l}"));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The iota gate
// ---------------------------------------------------------------------------

/// Go constants of an `iota` block whose value in `merged` is a value neither
/// side gave them: (block names, constant). A constant one side renumbered
/// keeps that side's value; a value the merge itself invented is a silent
/// renumbering of something existing code may depend on.
pub(crate) fn iota_renumbered(
    base: &str,
    ours: &str,
    theirs: &str,
    merged: &str,
    path: &str,
) -> Option<(Vec<String>, String)> {
    if lang(path) != Some(Lang::Go) {
        return None;
    }
    if !merged.contains("iota") && !base.contains("iota") {
        return None;
    }
    let read = |t: &str| -> Option<HashMap<String, (String, usize, Vec<String>)>> {
        let tree = parse(path, t)?;
        Some(iota_values(t, tree.root_node()))
    };
    let (Some(b), Some(o), Some(th), Some(m)) = (read(base), read(ours), read(theirs), read(merged))
    else {
        return None;
    };
    let mut names: Vec<&String> = b.keys().collect();
    names.sort();
    for n in names {
        let Some(vm) = m.get(n) else { continue };
        let val = |x: &(String, usize, Vec<String>)| (x.0.clone(), x.1);
        let (Some(vo), Some(vt)) = (o.get(n), th.get(n)) else {
            continue; // deleted by a side: its value is not the merge's to keep
        };
        if val(vm) != val(vo) && val(vm) != val(vt) {
            return Some((vm.2.clone(), n.clone()));
        }
    }
    None
}

/// Every constant of every `const` block that uses `iota`, with its
/// effective expression and spec index (which together are its value), and
/// the names its block answers to (its type, its first constant).
fn iota_values(src: &str, root: Node) -> HashMap<String, (String, usize, Vec<String>)> {
    let mut out = HashMap::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        if n.kind() == "const_declaration" && contains_ident(n, src, "iota") {
            let specs: Vec<Node> = kids(n).into_iter().filter(|c| c.kind() == "const_spec").collect();
            let mut block_names = Vec::new();
            if let Some(first) = specs.first() {
                if let Some(ty) = first.child_by_field_name("type") {
                    block_names.push(src[ty.start_byte()..ty.end_byte()].to_string());
                }
                if let Some(nm) = first.child_by_field_name("name") {
                    block_names.push(src[nm.start_byte()..nm.end_byte()].to_string());
                }
            }
            let mut expr = String::new();
            for (idx, s) in specs.iter().enumerate() {
                if let Some(v) = s.child_by_field_name("value") {
                    let ty = s
                        .child_by_field_name("type")
                        .map(|t| norm(&src[t.start_byte()..t.end_byte()]))
                        .unwrap_or_default();
                    expr = format!("{ty}={}", norm(&src[v.start_byte()..v.end_byte()]));
                }
                let uses_iota = expr.contains("iota");
                for c in kids(*s) {
                    if c.kind() == "identifier" {
                        let name = src[c.start_byte()..c.end_byte()].to_string();
                        if name == "_" {
                            continue;
                        }
                        // A value that does not depend on the index has no
                        // number to lose.
                        let at = if uses_iota { idx } else { 0 };
                        out.insert(name, (expr.clone(), at, block_names.clone()));
                    }
                }
            }
        }
        stack.extend(kids(n));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(path: &str, b: &str, o: &str, t: &str) -> Option<String> {
        let l = lang(path).unwrap();
        let off = || SetScope::Off;
        let a = union_with(b, o, t, path, l, &off);
        let z = union_with(b, t, o, path, l, &off);
        assert_eq!(a, z, "the union must not depend on which side is ours");
        a
    }

    fn ud(path: &str, b: &str, o: &str, t: &str, decl: &str) -> Option<String> {
        let l = lang(path).unwrap();
        let s = SetScope::from_attribute(Some(decl));
        let sc = move || s.clone();
        let a = union_with(b, o, t, path, l, &sc);
        let z = union_with(b, t, o, path, l, &sc);
        assert_eq!(a, z);
        a
    }

    fn dump(n: tree_sitter::Node, src: &str, depth: usize, out: &mut String) {
        let t = n.utf8_text(src.as_bytes()).unwrap_or("");
        let t: String = t.chars().take(40).collect::<String>().replace('\n', "\\n");
        out.push_str(&format!(
            "{}{} [{}..{}] {:?}\n",
            "  ".repeat(depth),
            n.kind(),
            n.start_byte(),
            n.end_byte(),
            t
        ));
        for i in 0..n.child_count() as u32 {
            dump(n.child(i).unwrap(), src, depth + 1, out);
        }
    }

    #[test]
    #[ignore]
    fn tsdump() {
        let path = std::env::var("TSDUMP_PATH").unwrap();
        let file = std::env::var("TSDUMP_FILE").unwrap();
        let text = std::fs::read_to_string(file).unwrap();
        let (_, tree) = PARSER_REGISTRY
            .extract_entities_with_tree(&path, &text)
            .unwrap();
        let tree = tree.unwrap();
        let mut out = String::new();
        dump(tree.root_node(), &text, 0, &mut out);
        println!("has_error={}\n{out}", tree.root_node().has_error());
    }

    // -- Go ---------------------------------------------------------------

    const GO_SWITCH_BASE: &str = "func (vm *VM) Run() {\n\tfor vm.ip < 3 {\n\t\tswitch op {\n\t\tcase OpAnd:\n\t\t\tvm.and()\n\n\t\tcase OpOr:\n\t\t\tvm.or()\n\n\t\tcase OpEnd:\n\t\t\treturn\n\t\t}\n\t}\n}\n";

    #[test]
    fn go_switch_cases_inserted_by_both_sides_unite() {
        let o = GO_SWITCH_BASE.replace(
            "\t\tcase OpEnd:",
            "\t\tcase OpShiftLeft:\n\t\t\tvm.shl()\n\n\t\tcase OpEnd:",
        );
        let t = GO_SWITCH_BASE.replace(
            "\t\tcase OpEnd:",
            "\t\tcase OpBitNot:\n\t\t\tvm.not()\n\n\t\tcase OpEnd:",
        );
        let m = u("vm.go", GO_SWITCH_BASE, &o, &t).expect("united");
        assert!(m.contains("case OpShiftLeft:\n\t\t\tvm.shl()\n\n\t\tcase OpBitNot:")
            || m.contains("case OpBitNot:\n\t\t\tvm.not()\n\n\t\tcase OpShiftLeft:"), "{m}");
        assert!(m.find("OpBitNot").unwrap() < m.find("OpShiftLeft").unwrap(), "by key: {m}");
        assert!(m.contains("case OpEnd:"));
    }

    #[test]
    fn go_switch_refusals() {
        // The same label from both sides with different bodies.
        let o = GO_SWITCH_BASE.replace("\t\tcase OpEnd:", "\t\tcase OpX:\n\t\t\ta()\n\n\t\tcase OpEnd:");
        let t = GO_SWITCH_BASE.replace("\t\tcase OpEnd:", "\t\tcase OpX:\n\t\t\tb()\n\n\t\tcase OpEnd:");
        assert!(u("vm.go", GO_SWITCH_BASE, &o, &t).is_none());
        // Both sides extend one case's label list.
        let o = GO_SWITCH_BASE.replace("case OpOr:", "case OpOr, OpXor:");
        let t = GO_SWITCH_BASE.replace("case OpOr:", "case OpOr, OpNor:");
        assert!(u("vm.go", GO_SWITCH_BASE, &o, &t).is_none());
        // A label already present.
        let o = GO_SWITCH_BASE.replace("\t\tcase OpEnd:", "\t\tcase OpY:\n\t\t\ta()\n\n\t\tcase OpEnd:");
        let t = GO_SWITCH_BASE.replace("\t\tcase OpEnd:", "\t\tcase OpZ, OpAnd:\n\t\t\tb()\n\n\t\tcase OpEnd:");
        assert!(u("vm.go", GO_SWITCH_BASE, &o, &t).is_none());
        // fallthrough anywhere in the switch.
        let base = GO_SWITCH_BASE.replace("vm.and()", "vm.and()\n\t\t\tfallthrough");
        let o = base.replace("\t\tcase OpEnd:", "\t\tcase OpY:\n\t\t\ta()\n\n\t\tcase OpEnd:");
        let t = base.replace("\t\tcase OpEnd:", "\t\tcase OpZ:\n\t\t\tb()\n\n\t\tcase OpEnd:");
        assert!(u("vm.go", &base, &o, &t).is_none());
        // A tagless switch is a chain of conditions.
        let base = "func f() {\n\tswitch {\n\tcase a > 1:\n\t\tx()\n\t}\n}\n";
        let o = base.replace("\t}\n}", "\tcase a > 0:\n\t\ty()\n\t}\n}");
        let t = base.replace("\t}\n}", "\tcase a > 2:\n\t\tz()\n\t}\n}");
        assert!(u("x.go", base, &o, &t).is_none());
        // A label that is a call.
        let o = GO_SWITCH_BASE.replace("\t\tcase OpEnd:", "\t\tcase f():\n\t\t\ta()\n\n\t\tcase OpEnd:");
        let t = GO_SWITCH_BASE.replace("\t\tcase OpEnd:", "\t\tcase OpZ:\n\t\t\tb()\n\n\t\tcase OpEnd:");
        assert!(u("vm.go", GO_SWITCH_BASE, &o, &t).is_none());
        // A type switch: overlapping types, first match wins.
        let base = "func f(x any) {\n\tswitch x.(type) {\n\tcase int:\n\t\ta()\n\t}\n}\n";
        let o = base.replace("\t}\n}", "\tcase error:\n\t\tb()\n\t}\n}");
        let t = base.replace("\t}\n}", "\tcase fmt.Stringer:\n\t\tc()\n\t}\n}");
        assert!(u("x.go", base, &o, &t).is_none());
    }

    #[test]
    fn go_map_literal_entries_unite_and_collide() {
        let base = "func TestX(t *testing.T) {\n\tconfig := map[string]struct {\n\t\tarity int\n\t}{\n\t\t\"now\":  {0},\n\t\t\"take\": {2},\n\t}\n\t_ = config\n}\n";
        let o = base.replace("\t\t\"take\": {2},\n", "\t\t\"take\": {2},\n\t\t\"chunk\":  {2},\n");
        let t = base.replace("\t\t\"take\": {2},\n", "\t\t\"take\": {2},\n\t\t\"zip\":    {2},\n");
        let m = u("builtin_test.go", base, &o, &t).expect("united");
        assert!(m.contains("\t\t\"take\": {2},\n\t\t\"chunk\":  {2},\n\t\t\"zip\":    {2},\n\t}"), "{m}");
        // One key, two values.
        let t2 = base.replace("\t\t\"take\": {2},\n", "\t\t\"take\": {2},\n\t\t\"chunk\":  {3},\n");
        assert!(u("builtin_test.go", base, &o, &t2).is_none());
        // Both modify one entry.
        let o3 = base.replace("\"now\":  {0}", "\"now\":  {1}");
        let t3 = base.replace("\"now\":  {0}", "\"now\":  {2}");
        assert!(u("builtin_test.go", base, &o3, &t3).is_none());
    }

    #[test]
    fn go_test_table_rows_unite_but_product_slices_need_a_declaration() {
        let base = "func TestExpr(t *testing.T) {\n\ttests := []struct {\n\t\tcode string\n\t\twant any\n\t}{\n\t\t{\n\t\t\t`1`,\n\t\t\t1,\n\t\t},\n\t}\n\t_ = tests\n}\n";
        let o = base.replace("\t\t},\n\t}\n", "\t\t},\n\t\t{\n\t\t\t`1 << 4`,\n\t\t\t16,\n\t\t},\n\t}\n");
        let t = base.replace("\t\t},\n\t}\n", "\t\t},\n\t\t{\n\t\t\t`~5`,\n\t\t\t-6,\n\t\t},\n\t}\n");
        let m = u("expr_test.go", base, &o, &t).expect("a test table unites");
        assert!(m.contains("`1 << 4`") && m.contains("`~5`"), "{m}");
        assert!(m.find("`1`").unwrap() < m.find("`1 << 4`").unwrap());
        // The same rows in product code: position is meaning.
        assert!(u("expr.go", base, &o, &t).is_none());
        // ...unless the file declares the table a set.
        assert!(ud("expr.go", base, &o, &t, "tests").is_some());
        assert!(ud("expr.go", base, &o, &t, "other").is_none());
    }

    #[test]
    fn go_function_local_const_blocks() {
        let base = "func f() {\n\tconst (\n\t\tA = iota\n\t\tB\n\t\tEnd\n\t)\n}\n";
        // Insertion ahead of `End` renumbers it: refused without a declaration.
        let o = base.replace("\t\tEnd\n", "\t\tC\n\t\tEnd\n");
        let t = base.replace("\t\tEnd\n", "\t\tD\n\t\tEnd\n");
        assert!(u("x.go", base, &o, &t).is_none());
        // Appended after every base constant: nothing existing moves.
        let o = base.replace("\t\tEnd\n", "\t\tEnd\n\t\tC\n");
        let t = base.replace("\t\tEnd\n", "\t\tEnd\n\t\tD\n");
        let m = u("x.go", base, &o, &t).expect("tail appends unite");
        assert!(m.contains("\t\tEnd\n\t\tC\n\t\tD\n"), "{m}");
        // Without iota every value is written out.
        let base = "func f() {\n\tconst (\n\t\tA = 1\n\t\tB = 2\n\t)\n}\n";
        let o = base.replace("\t\tB = 2\n", "\t\tC = 3\n\t\tB = 2\n");
        let t = base.replace("\t\tB = 2\n", "\t\tD = 4\n\t\tB = 2\n");
        assert!(u("x.go", base, &o, &t).is_some());
    }

    #[test]
    fn go_top_level_functions_appended_by_both_sides_unite() {
        let base = "package p\n\nfunc a() {}\n";
        let o = format!("{base}\nfunc b() {{}}\n");
        let t = format!("{base}\nfunc c() {{}}\n");
        let m = u("x.go", base, &o, &t).expect("united");
        assert!(m.contains("func b()") && m.contains("func c()"));
    }

    // -- iota gate ---------------------------------------------------------

    #[test]
    fn the_iota_gate_names_a_value_neither_side_gave() {
        let base = "package vm\n\nconst (\n\tOpA Opcode = iota\n\tOpB\n\tOpEnd\n)\n";
        let o = base.replace("\tOpEnd\n", "\tOpC\n\tOpEnd\n");
        let t = base.replace("\tOpEnd\n", "\tOpD\n\tOpEnd\n");
        let merged = base.replace("\tOpEnd\n", "\tOpC\n\tOpD\n\tOpEnd\n");
        let r = iota_renumbered(base, &o, &t, &merged, "vm/opcodes.go").expect("OpEnd moved");
        assert_eq!(r.1, "OpEnd");
        assert!(r.0.contains(&"Opcode".to_string()));
        // Appending after the last constant renumbers nothing that existed.
        let o = base.replace("\tOpEnd\n", "\tOpEnd\n\tOpC\n");
        let t = base.replace("\tOpEnd\n", "\tOpEnd\n\tOpD\n");
        let merged = base.replace("\tOpEnd\n", "\tOpEnd\n\tOpC\n\tOpD\n");
        assert!(iota_renumbered(base, &o, &t, &merged, "vm/opcodes.go").is_none());
        // One side's own renumbering is that side's.
        let o = base.replace("\tOpB\n", "\tOpC\n\tOpB\n");
        assert!(iota_renumbered(base, &o, base, &o, "vm/opcodes.go").is_none());
    }

    // -- JS ----------------------------------------------------------------

    const JS_IIFE: &str = "var jsonata = (function() {\n    var errorCodes = {\n        \"D3140\": \"Malformed\",\n        \"D3141\": \"{{{message}}}\"\n    };\n    function evaluate(expr) {\n        switch (expr.type) {\n            case 'path':\n                result = 1;\n                break;\n            case 'name':\n                return 2;\n        }\n    }\n    staticFrame.bind('clone', defineFunction(fn.clone, '<x:x>'));\n    return { evaluate };\n})();\n";

    #[test]
    fn js_object_entries_unite_across_a_last_line_comma() {
        let o = JS_IIFE.replace(
            "\"D3141\": \"{{{message}}}\"\n",
            "\"D3141\": \"{{{message}}}\",\n        \"D3150\": \"chunk\"\n",
        );
        let t = JS_IIFE.replace(
            "\"D3141\": \"{{{message}}}\"\n",
            "\"D3141\": \"{{{message}}}\",\n        \"D3160\": \"zip\"\n",
        );
        let m = u("src/jsonata.js", JS_IIFE, &o, &t).expect("united");
        assert!(m.contains("\"D3141\": \"{{{message}}}\",\n        \"D3150\": \"chunk\",\n        \"D3160\": \"zip\"\n    };"), "{m}");
    }

    #[test]
    fn js_switch_cases_unite_only_when_nothing_falls_through() {
        let o = JS_IIFE.replace(
            "            case 'name':",
            "            case 'chunk':\n                return 3;\n            case 'name':",
        );
        let t = JS_IIFE.replace(
            "            case 'name':",
            "            case 'zip':\n                result = 4;\n                break;\n            case 'name':",
        );
        let m = u("src/jsonata.js", JS_IIFE, &o, &t).expect("united");
        assert!(m.contains("case 'chunk'") && m.contains("case 'zip'"));
        // An inserted case that falls through.
        let t2 = JS_IIFE.replace(
            "            case 'name':",
            "            case 'zip':\n                result = 4;\n            case 'name':",
        );
        assert!(u("src/jsonata.js", JS_IIFE, &o, &t2).is_none());
        // An empty case groups with what follows it.
        let t3 = JS_IIFE.replace("            case 'name':", "            case 'zip':\n            case 'name':");
        assert!(u("src/jsonata.js", JS_IIFE, &o, &t3).is_none());
    }

    #[test]
    fn js_registry_calls_need_a_declaration() {
        let o = JS_IIFE.replace(
            "    return { evaluate };",
            "    staticFrame.bind('chunk', defineFunction(fn.chunk, '<an:a>'));\n    return { evaluate };",
        );
        let t = JS_IIFE.replace(
            "    return { evaluate };",
            "    staticFrame.bind('startsWith', defineFunction(fn.startsWith, '<s-s:b>'));\n    return { evaluate };",
        );
        assert!(u("src/jsonata.js", JS_IIFE, &o, &t).is_none(), "statements are ordered");
        let m = ud("src/jsonata.js", JS_IIFE, &o, &t, "bind").expect("declared registry");
        assert!(m.find("'chunk'").unwrap() < m.find("'startsWith'").unwrap());
        // A registered key twice, even declared.
        let t2 = JS_IIFE.replace(
            "    return { evaluate };",
            "    staticFrame.bind('clone', other);\n    return { evaluate };",
        );
        assert!(ud("src/jsonata.js", JS_IIFE, &o, &t2, "bind").is_none());
    }

    #[test]
    fn js_export_object_with_several_names_per_line() {
        let base = "var functions = (function() {\n    function a() {}\n    return {\n        sum, count,\n        decodeUrl\n    };\n})();\n";
        let o = base.replace(
            "function a() {}\n",
            "function a() {}\n    function chunk() {}\n",
        ).replace("        decodeUrl\n", "        decodeUrl,\n        chunk\n");
        let t = base.replace(
            "function a() {}\n",
            "function a() {}\n    function startsWith() {}\n    function endsWith() {}\n",
        ).replace("        decodeUrl\n", "        decodeUrl,\n        startsWith, endsWith\n");
        let m = u("src/functions.js", base, &o, &t).expect("united");
        assert!(m.contains("decodeUrl,\n        chunk,\n        startsWith, endsWith\n    };"), "{m}");
        assert!(m.contains("function chunk()") && m.contains("function endsWith()"));
    }

    #[test]
    fn js_arrays_are_ordered_outside_tests() {
        let base = "const xs = [\n    1,\n];\n";
        let o = base.replace("    1,\n", "    1,\n    2,\n");
        let t = base.replace("    1,\n", "    1,\n    3,\n");
        assert!(u("src/a.js", base, &o, &t).is_none());
        assert!(u("test/a.test.js", base, &o, &t).is_some());
    }

    // -- Python ------------------------------------------------------------

    const PY_ATTRS: &str = "    TRANSFORMS = {\n        **generator.Generator.TRANSFORMS,\n        exp.DateSub: f(\"DATE_SUB\"),\n        exp.IsNan: rename_func(\"isNaN\"),\n    }\n";

    #[test]
    fn python_dict_entries_with_dotted_keys_unite_by_key() {
        let o = PY_ATTRS.replace(
            "        exp.IsNan",
            "        exp.IsFinite: rename_func(\"isFinite\"),\n        exp.IsNan",
        );
        let t = PY_ATTRS.replace(
            "        exp.IsNan",
            "        exp.IntDiv: lambda self, e: self.func(\"intDiv\", e.this),\n        exp.IsNan",
        );
        let m = u("sqlglot/generators/clickhouse.py", PY_ATTRS, &o, &t).expect("united");
        let (i, f) = (m.find("exp.IntDiv").unwrap(), m.find("exp.IsFinite").unwrap());
        assert!(i < f, "{m}");
        assert!(m.starts_with("    TRANSFORMS = {\n"));
        // A splat inserted by a side has no key.
        let t2 = PY_ATTRS.replace("        exp.IsNan", "        **more,\n        exp.IsNan");
        assert!(u("sqlglot/generators/clickhouse.py", PY_ATTRS, &o, &t2).is_none());
        // One key, two values.
        let t3 = PY_ATTRS.replace(
            "        exp.IsNan",
            "        exp.IsFinite: rename_func(\"other\"),\n        exp.IsNan",
        );
        assert!(u("sqlglot/generators/clickhouse.py", PY_ATTRS, &o, &t3).is_none());
    }

    #[test]
    fn python_lists_need_a_declaration_outside_tests() {
        let base = "PLUGINS = [\n    \"a\",\n]\n";
        let o = base.replace("    \"a\",\n", "    \"a\",\n    \"b\",\n");
        let t = base.replace("    \"a\",\n", "    \"a\",\n    \"c\",\n");
        assert!(u("pkg/plugins.py", base, &o, &t).is_none());
        assert!(ud("pkg/plugins.py", base, &o, &t, "PLUGINS").is_some());
        assert!(u("tests/test_plugins.py", base, &o, &t).is_some());
        // A literal passed to a call is not a named table: parametrize names
        // its test ids by position.
        let base = "@pytest.mark.parametrize(\"x\", [\n    \"a\",\n])\ndef test_x(x):\n    assert x\n";
        let o = base.replace("    \"a\",\n", "    \"a\",\n    \"b\",\n");
        let t = base.replace("    \"a\",\n", "    \"a\",\n    \"c\",\n");
        assert!(u("tests/test_x.py", base, &o, &t).is_none());
        assert!(ud("tests/test_x.py", base, &o, &t, "parametrize").is_some());
    }

    #[test]
    fn a_comment_above_an_element_the_other_side_edited_is_a_conflict() {
        let base = "function f(a, b) {\n  return a + b;\n}\n";
        let o = "function f(a, b) {\n  // sums\n  return a + b;\n}\n";
        let t = "function f(a, b) {\n  return a - b;\n}\n";
        assert!(u("m.js", base, o, t).is_none());
    }

    #[test]
    fn outside_a_collection_nothing_composes_finer_than_a_line() {
        // Two edits to one call: not a collection, so not this rung's to unite.
        let base = "func f() {\n\tg(a, b)\n}\n";
        let o = base.replace("g(a, b)", "g(x, b)");
        let t = base.replace("g(a, b)", "g(a, y)");
        assert!(u("x.go", base, &o, &t).is_none());
        // Edits on different lines of one body are fine (diff3 would agree).
        let base = "func f() {\n\tm := map[string]int{\n\t\t\"a\": 1,\n\t}\n\tg(a)\n}\n";
        let o = base.replace("\t\t\"a\": 1,\n", "\t\t\"a\": 1,\n\t\t\"b\": 2,\n").replace("g(a)", "g(z)");
        let t = base.replace("\t\t\"a\": 1,\n", "\t\t\"a\": 1,\n\t\t\"c\": 3,\n");
        let m = u("x.go", base, &o, &t).expect("united");
        assert!(m.contains("g(z)") && m.contains("\"b\": 2") && m.contains("\"c\": 3"), "{m}");
        // Inside a collection too: two entries of a one-line dict, one edited
        // by each side, are one line edited twice.
        let base = "def f():\n    h = {\"a\": 1, \"b\": 2}\n    return h\n";
        let o = base.replace("\"a\": 1", "\"a\": 9");
        let t = base.replace("\"b\": 2", "\"b\": 8");
        assert!(u("m.py", base, &o, &t).is_none());
    }

    #[test]
    fn a_deletion_against_a_modification_refuses() {
        let base = "H = {\n    \"a\": 1,\n    \"b\": 2,\n}\n";
        let o = "H = {\n    \"b\": 2,\n    \"c\": 3,\n}\n";
        let t = "H = {\n    \"a\": 9,\n    \"b\": 2,\n    \"d\": 4,\n}\n";
        assert!(u("m.py", base, o, t).is_none());
        // Deletion against an untouched element is fine.
        let t = "H = {\n    \"a\": 1,\n    \"b\": 2,\n    \"d\": 4,\n}\n";
        let m = u("m.py", base, o, t).expect("united");
        assert!(!m.contains("\"a\"") && m.contains("\"c\"") && m.contains("\"d\""), "{m}");
    }
}
