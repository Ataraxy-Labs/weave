//! D3 over a container: two sides that each inserted new elements at one
//! point of one container — the entries of a registry dict, the names of an
//! `__all__` or an export list, the keys of an INI section.
//!
//! **The argument.** Inserting element `p` and inserting element `q` into a
//! container commute exactly when the container's meaning does not depend on
//! the order of its elements and `p`, `q` are distinct: then either order is
//! the same value, and the union is the pushout of the two edits. The line
//! merge refuses them only because they landed on neighbouring lines. So the
//! rule settles a region when, and only when, a recogniser can say three
//! things about it:
//!
//! 1. every line of the region is either a base line both sides kept
//!    (exactly, or with a trailing comma added so something can follow it)
//!    or part of a whole element a side inserted — nothing existing was
//!    modified, deleted or moved;
//! 2. every inserted element is a direct child of one container of a kind
//!    the recogniser knows, in each side's own text and in the answer;
//! 3. the container is a set: by the language ([`Spec::set`]), or because
//!    its owner declared it one with the `weave-set` gitattribute
//!    ([`crate::host::SetScope`]).
//!
//! **Which containers are sets by the language** (each arm of [`container`]
//! and [`find_ini`] states its own argument):
//!
//! - Python `dict` / JS-TS object literals with distinct constant keys. A
//!   dict's lookup semantics do not depend on insertion order. Iteration
//!   order is observable (`dict` preserves it, `Object.keys` too), and a
//!   program that iterates a registry could see the difference — but for the
//!   registries this rule meets, order is conventionally irrelevant, and the
//!   order written is canonical (below), never the side's name. Keys must be
//!   constants whose identity is decidable from the text: plain strings
//!   without escapes, decimal integers. A computed key, a spread/`**`
//!   splat, a getter or setter refuses.
//! - Python sets of constants, `__all__`, parenthesised `from m import (…)`
//!   names; JS-TS `export { … }` and `import { … }` lists. Each is a set of
//!   names by definition: `from m import *` reads `__all__` as a set, and an
//!   import or export clause binds each name once whatever its position.
//! - INI keys inside one section (`setup.cfg`, `tox.ini`, …). `configparser`
//!   reads a section as a mapping and rejects a key stated twice.
//! - TOML keys, YAML mapping keys and JSON object keys are the data key union
//!   ([`crate::dataunion`]); ignore files and requirements are the line-set
//!   union in [`crate::determinate`]. Not repeated here.
//!
//! **Which are not.** A list, an array, a tuple, a statement sequence, match
//! or switch arms, the lines of an INI multi-line value (`console_scripts`),
//! a YAML sequence: their order is part of their meaning in general (the
//! first matching arm wins; statements run in order; a parametrize list
//! names test ids by position). Inserting two elements at one point of one
//! of those is two claims about which comes first, and that is intent — a
//! conflict, unless the file's owner declared the container a set with
//! `weave-set` (a bare attribute for every container in the file, or a list
//! of container names). With the declaration, union applies exactly as for
//! a set by the language; without it, nothing changes.
//!
//! **The answer.** Base lines stay where base had them. Each side's inserted
//! elements go where that side put them (after the same base line). Where
//! both sides inserted after the same base line, the two runs are merged by
//! key — at each step the element whose key sorts first — so each side's
//! own order is kept, the result does not depend on which side is called
//! ours, and a fleet of branches that each add one bare element to a sorted
//! run lands on one tree whatever order they merge in. A comment or blank line
//! directly above an element travels with it, and a run that has one is a
//! group its author laid out: it is kept whole, the run whose first key sorts
//! first going first. An element both sides inserted with the same text is
//! written once.
//!
//! **Guards**, each a conflict:
//! - a base line of the region changed, deleted, reordered, or lost its comma;
//! - an inserted line that is not part of a whole element of the container
//!   (a stray statement, a closing bracket, a comment the parser attaches
//!   elsewhere, two elements on one line);
//! - an element whose key is not a decidable constant, or a splat/spread;
//! - one key inserted by both sides with different text, or twice by one;
//! - an inserted key the container already has — in the answer, or in the
//!   base (an element moved into the insertion point from elsewhere);
//! - the two sides' runs at one point end differently (one with a trailing
//!   comma, one without), or have different trailing lines;
//! - an order-sensitive container without a declaration;
//! - the answer does not parse, or its container no longer holds exactly the
//!   inserted keys at the inserted rows (checked on the whole answer, after
//!   every region is settled: [`certify`]);
//! - and every gate any settled file passes (`crate::verify`).

use std::collections::HashSet;
use std::ops::Range;

use sem_core::parser::registry::ParserRegistry;
use tree_sitter::{Node, Point};

use crate::host::SetScope;

/// The rule's name, as it appears in the audit.
pub(crate) const RULE: &str = "D3 container insertion union";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lang {
    Py,
    Js,
    Ini,
}

fn lang(path: &str) -> Option<Lang> {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let ext = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "py" | "pyi" => Some(Lang::Py),
        "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "mts" | "cts" => Some(Lang::Js),
        "ini" | "cfg" => Some(Lang::Ini),
        _ => match name {
            ".editorconfig" | ".flake8" | ".coveragerc" | ".pylintrc" | "pylintrc" => {
                Some(Lang::Ini)
            }
            _ => None,
        },
    }
}

/// Is `path` a file this rule has a recogniser for?
pub(crate) fn applies(path: &str) -> bool {
    lang(path).is_some()
}

fn blank(l: &str) -> bool {
    l.trim().is_empty()
}

fn indent(l: &str) -> usize {
    l.len() - l.trim_start().len()
}

// ---------------------------------------------------------------------------
// The region: base lines kept, runs inserted
// ---------------------------------------------------------------------------

/// A line's text without its trailing comma or end of line, and whether it
/// had the comma.
fn uncomma(l: &str) -> (&str, bool) {
    let t = l.trim_end();
    match t.strip_suffix(',') {
        Some(s) => (s.trim_end(), true),
        None => (t, false),
    }
}

/// One side's region read against the base region.
struct Split {
    /// Where each base line is in the side's region.
    kept: Vec<usize>,
    /// What the side inserted: after which base line (`None` = before the
    /// first), and which of its lines.
    runs: Vec<(Option<usize>, Range<usize>)>,
}

/// Every base line kept, in order, exactly or with a comma added — or `None`.
fn split(o: &[String], x: &[String]) -> Option<Split> {
    let strip = |v: &[String]| -> Vec<String> {
        v.iter()
            .map(|l| format!("{}\n", uncomma(l).0))
            .collect::<Vec<_>>()
    };
    let img = crate::determinate::image(&strip(o), &strip(x));
    let mut kept = Vec::with_capacity(o.len());
    let mut last: Option<usize> = None;
    for (k, base_line) in o.iter().enumerate() {
        let j = *img.get(&k)?;
        if last.is_some_and(|p| j <= p) {
            return None;
        }
        let (bt, bc) = uncomma(base_line);
        let (st, sc) = uncomma(&x[j]);
        if bt != st || (bc && !sc) || indent(base_line) != indent(&x[j]) {
            return None;
        }
        kept.push(j);
        last = Some(j);
    }
    let at: std::collections::HashMap<usize, usize> =
        kept.iter().enumerate().map(|(k, j)| (*j, k)).collect();
    let mut runs: Vec<(Option<usize>, Range<usize>)> = Vec::new();
    let mut gap: Option<usize> = None;
    for j in 0..x.len() {
        if let Some(k) = at.get(&j) {
            gap = Some(*k);
            continue;
        }
        match runs.last_mut() {
            Some((g, r)) if *g == gap && r.end == j => r.end = j + 1,
            _ => runs.push((gap, j..j + 1)),
        }
    }
    Some(Split { kept, runs })
}

// ---------------------------------------------------------------------------
// Containers
// ---------------------------------------------------------------------------

/// What a container is, by the language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Spec {
    kind: &'static str,
    /// Order-free by the language's semantics: a union without declaration.
    set: bool,
    /// Elements end with a separating comma.
    comma: bool,
    /// The container's extent is its elements' (a Python block, a module):
    /// it may begin on the first inserted line.
    implicit: bool,
}

const fn spec(kind: &'static str, set: bool, comma: bool, implicit: bool) -> Spec {
    Spec {
        kind,
        set,
        comma,
        implicit,
    }
}

/// A container around some inserted lines of one text.
#[derive(Debug)]
struct Found {
    spec: Spec,
    /// Which container: its kind and the names of its enclosing scopes,
    /// outermost first. Compared across the sides, the answer and the base.
    sig: String,
    /// What a `weave-set` declaration calls it.
    names: Vec<String>,
    /// The elements on the inserted lines: rows (end exclusive) and key.
    inside: Vec<(Range<usize>, String)>,
    /// The keys of its other elements.
    others: Vec<String>,
}

fn text<'a>(n: Node, src: &'a str) -> &'a str {
    n.utf8_text(src.as_bytes()).unwrap_or("")
}

fn field<'a>(n: Node, name: &str, src: &'a str) -> Option<&'a str> {
    n.child_by_field_name(name).map(|c| text(c, src))
}

/// Which kind of container `n` is, if a recogniser knows it. Each arm says
/// why its container is, or is not, a set.
fn container(lang: Lang, n: Node, src: &str) -> Option<Spec> {
    match (lang, n.kind()) {
        // A mapping: lookup does not depend on insertion order; distinct
        // constant keys are independent entries (see the module docs for the
        // iteration-order caveat).
        (Lang::Py, "dictionary") => Some(spec("dict", true, true, false)),
        // A set literal: unordered by definition.
        (Lang::Py, "set") => Some(spec("set", true, true, false)),
        // `__all__` is read as a set of names by `from m import *` and by
        // every tool that reads it.
        (Lang::Py, "list" | "tuple") if is_dunder_all(n, src) => {
            Some(spec("__all__", true, true, false))
        }
        // A list or tuple is a sequence: position is meaning.
        (Lang::Py, "list" | "tuple") => Some(spec("list", false, true, false)),
        // `from m import (a, b)` binds each name once, in any order.
        (Lang::Py, "import_from_statement")
            if (0..n.child_count() as u32).any(|i| n.child(i).is_some_and(|c| c.kind() == "(")) =>
        {
            Some(spec("import names", true, true, false))
        }
        // The first matching arm wins: order is meaning.
        (Lang::Py, "block") if n.parent().is_some_and(|p| p.kind() == "match_statement") => {
            Some(spec("match arms", false, false, true))
        }
        // Statements run in order.
        (Lang::Py, "block" | "module") => Some(spec("statements", false, false, true)),
        (Lang::Js, "object") => Some(spec("object", true, true, false)),
        // An export or import clause names each binding once.
        (Lang::Js, "export_clause") => Some(spec("export names", true, true, false)),
        (Lang::Js, "named_imports") => Some(spec("import names", true, true, false)),
        (Lang::Js, "array") => Some(spec("array", false, true, false)),
        // Cases fall through, and the first match wins.
        (Lang::Js, "switch_body") => Some(spec("switch cases", false, false, false)),
        (Lang::Js, "statement_block") => Some(spec("statements", false, false, false)),
        (Lang::Js, "program") => Some(spec("statements", false, false, true)),
        _ => None,
    }
}

fn is_dunder_all(n: Node, src: &str) -> bool {
    n.parent().is_some_and(|p| {
        matches!(p.kind(), "assignment" | "augmented_assignment")
            && field(p, "left", src) == Some("__all__")
            && p.child_by_field_name("right").map(|r| r.id()) == Some(n.id())
    })
}

/// What one child of a container is.
enum Elem {
    /// Punctuation or a comment: not an element.
    Punct,
    /// An element, with its key.
    Key(String),
    /// Something that is not allowed on an inserted line of this container.
    Refused,
}

/// A Python constant whose identity is decidable from its text: a plain
/// string (no escapes, no f/b prefix, no implicit concatenation) or a
/// decimal integer. `1`, `1.0` and `True` are one dict key in Python, so
/// only integers are admitted among the numbers, and never booleans.
fn py_constant(n: Node, src: &str) -> Option<String> {
    match n.kind() {
        "string" => {
            let mut content = String::new();
            for i in 0..n.child_count() as u32 {
                let c = n.child(i)?;
                match c.kind() {
                    "string_start" => {
                        let q = text(c, src);
                        if q.chars().any(|ch| !matches!(ch, '"' | '\'' | 'u' | 'U')) {
                            return None;
                        }
                    }
                    "string_content" => content.push_str(text(c, src)),
                    "string_end" => {}
                    _ => return None,
                }
            }
            (!content.contains('\\')).then(|| format!("s:{content}"))
        }
        "integer" => decimal(text(n, src)).map(|d| format!("i:{d}")),
        _ => None,
    }
}

fn decimal(t: &str) -> Option<&str> {
    (!t.is_empty() && t.chars().all(|c| c.is_ascii_digit()) && (t == "0" || !t.starts_with('0')))
        .then_some(t)
}

/// A JS/TS property name decidable from its text. Object keys are strings,
/// so `1` and `"1"` are one key: both read as `1`.
fn js_property(n: Node, src: &str) -> Option<String> {
    let k = match n.kind() {
        "property_identifier" => text(n, src).to_string(),
        "string" => {
            let mut s = String::new();
            for i in 0..n.named_child_count() as u32 {
                let c = n.named_child(i)?;
                if c.kind() != "string_fragment" {
                    return None; // an escape sequence
                }
                s.push_str(text(c, src));
            }
            s
        }
        "number" => decimal(text(n, src))?.to_string(),
        _ => return None,
    };
    (k != "__proto__").then_some(k)
}

/// The key of an element of an order-sensitive container: its text, with
/// layout and the separating comma taken off.
fn text_key(n: Node, src: &str) -> String {
    let t = text(n, src)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    t.trim_end_matches(',').trim_end().to_string()
}

fn first_line_key(n: Node, src: &str) -> String {
    text(n, src).lines().next().unwrap_or("").trim().to_string()
}

fn element(lang: Lang, s: Spec, parent: Node, ch: Node, src: &str) -> Elem {
    if ch.kind() == "comment" || !ch.is_named() {
        return Elem::Punct;
    }
    let key = |k: Option<String>| k.map_or(Elem::Refused, Elem::Key);
    match (lang, s.kind) {
        (Lang::Py, "dict") if ch.kind() == "pair" => key(ch
            .child_by_field_name("key")
            .and_then(|k| py_constant(k, src))),
        (Lang::Py, "set" | "__all__") => key(py_constant(ch, src)),
        (Lang::Py, "import names") => {
            if parent.child_by_field_name("module_name").map(|m| m.id()) == Some(ch.id()) {
                return Elem::Punct;
            }
            match ch.kind() {
                "dotted_name" => Elem::Key(text(ch, src).to_string()),
                "aliased_import" => key(field(ch, "alias", src).map(str::to_string)),
                _ => Elem::Refused,
            }
        }
        (Lang::Py, "match arms") if ch.kind() == "case_clause" => {
            Elem::Key(first_line_key(ch, src))
        }
        (Lang::Js, "object") => match ch.kind() {
            "pair" => key(ch
                .child_by_field_name("key")
                .and_then(|k| js_property(k, src))),
            "shorthand_property_identifier" => Elem::Key(text(ch, src).to_string()),
            "method_definition" => {
                let accessor = (0..ch.child_count() as u32)
                    .filter_map(|i| ch.child(i))
                    .take_while(|c| !c.is_named())
                    .any(|c| matches!(c.kind(), "get" | "set" | "static"));
                if accessor {
                    return Elem::Refused;
                }
                key(ch
                    .child_by_field_name("name")
                    .and_then(|k| js_property(k, src)))
            }
            _ => Elem::Refused,
        },
        (Lang::Js, "export names" | "import names")
            if matches!(ch.kind(), "export_specifier" | "import_specifier") =>
        {
            key(field(ch, "alias", src)
                .or_else(|| field(ch, "name", src))
                .map(str::to_string))
        }
        (Lang::Js, "switch cases") if matches!(ch.kind(), "switch_case" | "switch_default") => {
            Elem::Key(first_line_key(ch, src))
        }
        (_, "list" | "array" | "statements") => Elem::Key(text_key(ch, src)),
        _ => Elem::Refused,
    }
}

/// The names a scope gives what it contains.
fn naming(n: Node, src: &str) -> Vec<String> {
    let one = |f: &str| {
        field(n, f, src)
            .map(|t| vec![t.to_string()])
            .unwrap_or_default()
    };
    match n.kind() {
        "assignment"
        | "augmented_assignment"
        | "assignment_expression"
        | "augmented_assignment_expression" => one("left"),
        "variable_declarator" | "keyword_argument" => one("name"),
        "function_definition"
        | "class_definition"
        | "function_declaration"
        | "generator_function_declaration"
        | "class_declaration"
        | "method_definition" => one("name"),
        "pair" => field(n, "key", src)
            .map(|k| vec![k.trim_matches(['"', '\'', '`']).to_string()])
            .unwrap_or_default(),
        "call" | "call_expression" => match field(n, "function", src) {
            Some(f) => {
                let last = f.rsplit('.').next().unwrap_or(f).to_string();
                if last == f {
                    vec![last]
                } else {
                    vec![f.to_string(), last]
                }
            }
            None => Vec::new(),
        },
        _ => Vec::new(),
    }
}

/// The container's identity and its declared names: the naming chain of its
/// ancestors, and the innermost one's names.
fn identify(n: Node, s: Spec, src: &str) -> (String, Vec<String>) {
    let mut chain: Vec<String> = Vec::new();
    let mut innermost: Option<Vec<String>> = None;
    let mut at = n.parent();
    while let Some(p) = at {
        let names = naming(p, src);
        if let Some(first) = names.first() {
            chain.push(first.clone());
            if innermost.is_none() {
                innermost = Some(names);
            }
        }
        at = p.parent();
    }
    chain.reverse();
    (
        format!("{}@{}", s.kind, chain.join("/")),
        innermost.unwrap_or_default(),
    )
}

/// The rows a node occupies (end exclusive).
fn rows_of(n: Node) -> Range<usize> {
    let (s, e) = (n.start_position(), n.end_position());
    let last = if e.column == 0 && e.row > s.row {
        e.row - 1
    } else {
        e.row
    };
    s.row..last + 1
}

/// The container whose elements the non-blank lines of `rows` are, in a
/// parsed text.
fn find_tree(lang: Lang, src: &str, root: Node, rows: Range<usize>) -> Option<Found> {
    let lines: Vec<&str> = src.split_inclusive('\n').collect();
    if rows.is_empty() || rows.end > lines.len() {
        return None;
    }
    let first = rows.clone().find(|r| !blank(lines[*r]))?;
    let last = rows.clone().rev().find(|r| !blank(lines[*r]))?;
    let from = Point::new(first, indent(lines[first]));
    let to = Point::new(last, lines[last].trim_end().len());
    let mut at = root.descendant_for_point_range(from, to);
    while let Some(n) = at {
        if let Some(s) = container(lang, n, src) {
            if n.start_position().row < first || s.implicit {
                return partition(lang, n, s, src, &lines, rows);
            }
        }
        at = n.parent();
    }
    None
}

fn partition(
    lang: Lang,
    n: Node,
    s: Spec,
    src: &str,
    lines: &[&str],
    rows: Range<usize>,
) -> Option<Found> {
    let mut inside: Vec<(Range<usize>, String)> = Vec::new();
    let mut others: Vec<String> = Vec::new();
    let mut covered = vec![false; rows.len()];
    for i in 0..n.child_count() as u32 {
        let ch = n.child(i)?;
        let r = rows_of(ch);
        let overlaps = r.start < rows.end && r.end > rows.start;
        let e = element(lang, s, n, ch, src);
        if !overlaps {
            if let Elem::Key(k) = e {
                others.push(k);
            }
            continue;
        }
        if r.start < rows.start || r.end > rows.end {
            return None; // straddles the inserted lines
        }
        for row in r.clone() {
            covered[row - rows.start] = true;
        }
        match e {
            Elem::Punct if ch.kind() == "comment" => {}
            Elem::Punct if s.comma && ch.kind() == "," => {}
            Elem::Key(k) => {
                if inside.last().is_some_and(|(p, _)| p.end > r.start) {
                    return None; // two elements on one line
                }
                inside.push((r, k));
            }
            _ => return None,
        }
    }
    let all_covered = rows
        .clone()
        .zip(&covered)
        .all(|(row, c)| *c || blank(lines[row]));
    if !all_covered || inside.is_empty() {
        return None;
    }
    let (sig, names) = identify(n, s, src);
    Some(Found {
        spec: s,
        sig,
        names,
        inside,
        others,
    })
}

/// Every element key of every container named `sig` in a parsed text, or
/// `None` when there is none.
fn keys_named(lang: Lang, src: &str, root: Node, sig: &str) -> Option<Vec<String>> {
    let mut out: Option<Vec<String>> = None;
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        if let Some(s) = container(lang, n, src) {
            if identify(n, s, src).0 == sig {
                let keys = out.get_or_insert_with(Vec::new);
                for i in 0..n.child_count() as u32 {
                    if let Some(Elem::Key(k)) = n.child(i).map(|c| element(lang, s, n, c, src)) {
                        keys.push(k);
                    }
                }
            }
        }
        for i in 0..n.child_count() as u32 {
            if let Some(c) = n.child(i) {
                stack.push(c);
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// INI
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IniLine {
    Blank,
    Comment,
    Header,
    Key,
    Continuation,
    /// Not a line of an INI file: never part of an element.
    Other,
}

fn ini_line(l: &str) -> IniLine {
    let t = l.trim();
    if t.is_empty() {
        IniLine::Blank
    } else if indent(l) > 0 {
        IniLine::Continuation
    } else if t.starts_with('#') || t.starts_with(';') {
        IniLine::Comment
    } else if t.starts_with('[') && t.ends_with(']') {
        IniLine::Header
    } else if t.contains('=') || t.contains(':') {
        IniLine::Key
    } else {
        IniLine::Other
    }
}

/// `configparser`'s key: the text before the first delimiter, lower-cased.
fn ini_key(l: &str) -> String {
    let t = l.trim();
    let cut = t.find(['=', ':']).unwrap_or(t.len());
    t[..cut].trim().to_ascii_lowercase()
}

/// One entry of a multi-line value (`name = module:func`): the name.
fn ini_value_key(l: &str) -> String {
    let t = l.trim();
    match t.split_once('=') {
        Some((k, _)) => k.trim().to_string(),
        None => t.to_string(),
    }
}

/// INI, line by line. Two shapes:
///
/// - **keys of one section** — every inserted line is a key line at column 0
///   with its indented continuation lines, below a `[section]` header and
///   not splitting another key's multi-line value. A set: `configparser`
///   reads a section as a mapping and refuses a key stated twice.
/// - **lines of one multi-line value** — every inserted line is an indented
///   continuation line of the key above. A value is a string whose lines a
///   consumer reads in order (`console_scripts` is a list), so this is a
///   set only by declaration, named by the key (`console_scripts`) or
///   `section.key`.
fn find_ini(src: &str, rows: Range<usize>) -> Option<Found> {
    let lines: Vec<&str> = src.split_inclusive('\n').collect();
    if rows.is_empty() || rows.end > lines.len() {
        return None;
    }
    let kinds: Vec<IniLine> = lines.iter().map(|l| ini_line(l)).collect();
    let header = (0..rows.start)
        .rev()
        .find(|i| kinds[*i] == IniLine::Header)?;
    let section = lines[header]
        .trim()
        .trim_matches(['[', ']'])
        .trim()
        .to_string();
    let section_end = (header + 1..lines.len())
        .find(|i| kinds[*i] == IniLine::Header)
        .unwrap_or(lines.len());
    if section_end < rows.end {
        return None; // a header among the inserted lines
    }
    let entries = |from: usize, to: usize| -> Vec<(Range<usize>, String)> {
        let mut out: Vec<(Range<usize>, String)> = Vec::new();
        for i in from..to {
            match kinds[i] {
                IniLine::Key => out.push((i..i + 1, ini_key(lines[i]))),
                IniLine::Continuation => {
                    if let Some((r, _)) = out.last_mut() {
                        if r.end == i {
                            r.end = i + 1;
                        }
                    }
                }
                _ => {}
            }
        }
        out
    };
    let significant: Vec<usize> = rows
        .clone()
        .filter(|i| !matches!(kinds[*i], IniLine::Blank | IniLine::Comment))
        .collect();
    let first = *significant.first()?;
    if kinds[first] == IniLine::Key {
        // Keys of one section. Continuations only directly under a key of
        // the run, and the line after the run does not continue a value.
        let inside = entries(rows.start, rows.end);
        let covered: HashSet<usize> = inside.iter().flat_map(|(r, _)| r.clone()).collect();
        if significant.iter().any(|i| !covered.contains(i)) {
            return None;
        }
        if lines
            .get(rows.end)
            .is_some_and(|l| ini_line(l) == IniLine::Continuation)
        {
            return None;
        }
        let others: Vec<String> = entries(header + 1, section_end)
            .into_iter()
            .filter(|(r, _)| r.end <= rows.start || r.start >= rows.end)
            .map(|(_, k)| k)
            .collect();
        return Some(Found {
            spec: spec("ini keys", true, false, false),
            sig: format!("ini keys@{section}"),
            names: vec![section],
            inside,
            others,
        });
    }
    // Lines of one multi-line value: all continuation lines, no blank or
    // comment line among them, owned by the key above.
    if rows.clone().any(|i| kinds[i] != IniLine::Continuation) {
        return None;
    }
    let owner = (header + 1..rows.start)
        .rev()
        .take_while(|i| matches!(kinds[*i], IniLine::Key | IniLine::Continuation))
        .find(|i| kinds[*i] == IniLine::Key)?;
    if (owner + 1..rows.start).any(|i| kinds[i] != IniLine::Continuation) {
        return None;
    }
    let value_end = (owner + 1..lines.len())
        .find(|i| kinds[*i] != IniLine::Continuation)
        .unwrap_or(lines.len());
    let key = ini_key(lines[owner]);
    let inside: Vec<(Range<usize>, String)> = rows
        .clone()
        .map(|i| (i..i + 1, ini_value_key(lines[i])))
        .collect();
    let others: Vec<String> = (owner + 1..value_end)
        .filter(|i| !rows.contains(i))
        .map(|i| ini_value_key(lines[i]))
        .collect();
    Some(Found {
        spec: spec("ini value lines", false, false, false),
        sig: format!("ini value lines@{section}/{key}"),
        names: vec![key.clone(), format!("{section}.{key}")],
        inside,
        others,
    })
}

/// Every key of the INI container named `sig`.
fn ini_keys_named(src: &str, sig: &str) -> Option<Vec<String>> {
    let lines: Vec<&str> = src.split_inclusive('\n').collect();
    let mut section: Option<String> = None;
    let mut owner: Option<String> = None;
    let mut out: Option<Vec<String>> = None;
    for l in &lines {
        match ini_line(l) {
            IniLine::Header => {
                section = Some(l.trim().trim_matches(['[', ']']).trim().to_string());
                owner = None;
            }
            IniLine::Key => {
                let k = ini_key(l);
                if let Some(s) = &section {
                    if sig == format!("ini keys@{s}") {
                        out.get_or_insert_with(Vec::new).push(k.clone());
                    }
                }
                owner = Some(k);
            }
            IniLine::Continuation => {
                if let (Some(s), Some(k)) = (&section, &owner) {
                    if sig == format!("ini value lines@{s}/{k}") {
                        out.get_or_insert_with(Vec::new).push(ini_value_key(l));
                    }
                }
            }
            _ => {}
        }
    }
    // A container that exists but is empty still exists.
    if out.is_none() {
        let exists = match sig.split_once('@') {
            Some(("ini keys", s)) => lines.iter().any(|l| {
                ini_line(l) == IniLine::Header && l.trim().trim_matches(['[', ']']).trim() == s
            }),
            Some(("ini value lines", sk)) => lines
                .iter()
                .any(|l| ini_line(l) == IniLine::Key && sk.ends_with(&format!("/{}", ini_key(l)))),
            _ => false,
        };
        if exists {
            out = Some(Vec::new());
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Parsed texts
// ---------------------------------------------------------------------------

/// A text read the way its recogniser reads it.
enum Parsed<'a> {
    Tree(&'a str, tree_sitter::Tree),
    Ini(&'a str),
}

fn parse<'a>(l: Lang, text: &'a str, path: &str, registry: &ParserRegistry) -> Option<Parsed<'a>> {
    match l {
        Lang::Ini => Some(Parsed::Ini(text)),
        Lang::Py | Lang::Js => {
            let (_, tree) = registry.extract_entities_with_tree(path, text)?;
            let tree = tree?;
            (!tree.root_node().has_error()).then_some(Parsed::Tree(text, tree))
        }
    }
}

impl Parsed<'_> {
    fn find(&self, l: Lang, rows: Range<usize>) -> Option<Found> {
        match self {
            Parsed::Tree(src, t) => find_tree(l, src, t.root_node(), rows),
            Parsed::Ini(src) => find_ini(src, rows),
        }
    }

    fn keys_named(&self, l: Lang, sig: &str) -> Option<Vec<String>> {
        match self {
            Parsed::Tree(src, t) => keys_named(l, src, t.root_node(), sig),
            Parsed::Ini(src) => ini_keys_named(src, sig),
        }
    }
}

// ---------------------------------------------------------------------------
// The union
// ---------------------------------------------------------------------------

/// What the answer must hold, checked once the whole file is written: at
/// `rows` of the region's answer, the container `sig`, holding exactly
/// `keys` there.
pub(crate) struct Claim {
    pub rows: Range<usize>,
    sig: String,
    keys: Vec<String>,
}

/// One side's view of the region: its whole text (the other regions settled
/// the way this side wrote them) and the line at which the region begins.
pub(crate) struct SideText<'a> {
    pub text: &'a str,
    pub at: usize,
}

/// An inserted element with what travels with it.
#[derive(Clone)]
struct Unit {
    key: String,
    /// The comments and blank lines above it, then its own lines.
    lines: Vec<String>,
    /// How many of `lines` are the comments and blank lines above it.
    lead: usize,
}

impl Unit {
    fn has_comma(&self) -> bool {
        self.lines.last().is_some_and(|l| uncomma(l).1)
    }
    /// The element's text, for "the same element from both sides".
    fn body(&self) -> Vec<String> {
        self.lines
            .iter()
            .filter(|l| !blank(l))
            .map(|l| uncomma(l).0.trim().to_string())
            .collect()
    }
}

/// The units of one run, and the lines after its last element.
fn units(run: &[String], found: &Found, first_row: usize) -> Option<(Vec<Unit>, Vec<String>)> {
    let mut out = Vec::new();
    let mut from = 0;
    for (r, key) in &found.inside {
        let (s, e) = (
            r.start.checked_sub(first_row)?,
            r.end.checked_sub(first_row)?,
        );
        if s < from || e > run.len() {
            return None;
        }
        out.push(Unit {
            key: key.clone(),
            lines: run[from..e].to_vec(),
            lead: s - from,
        });
        from = e;
    }
    Some((out, run[from..].to_vec()))
}

/// `l` with a trailing comma, unless a comment ends it.
fn add_comma(l: &str, lang: Lang) -> Option<String> {
    let body = l.trim_end();
    let marker = if lang == Lang::Py { "#" } else { "//" };
    if body.contains(marker) || body.ends_with("*/") {
        return None;
    }
    let eol = &l[body.len()..];
    let eol = if eol.contains("\r\n") {
        "\r\n"
    } else if eol.contains('\n') {
        "\n"
    } else {
        ""
    };
    Some(format!("{body},{eol}"))
}

/// Put two runs in canonical order. Two runs of bare elements merge by key:
/// at each step the head whose key sorts first, so each run's order is kept,
/// the result is the same whichever run is called first, and two sorted runs
/// give a sorted run. A run with a comment or blank line in it is a group
/// its author laid out (`// Onboarding events` above three entries), and
/// interleaving would cut it: then the two runs stay whole, the one whose
/// keys sort first going first. Either way one key in both runs is one
/// element if its text is the same, and a conflict if not.
fn order(a: Vec<Unit>, b: Vec<Unit>) -> Option<Vec<Unit>> {
    if a.iter().chain(&b).all(|u| u.lead == 0) {
        return merge_by_key(a, b);
    }
    // By the runs' keys, then their text: a total order on what the runs
    // say, so the choice never falls to which side is called ours.
    let rank = |r: &[Unit]| -> (Vec<String>, Vec<Vec<String>>) {
        (
            r.iter().map(|u| u.key.clone()).collect(),
            r.iter().map(|u| u.lines.clone()).collect(),
        )
    };
    let (first, second) = if rank(&a) <= rank(&b) { (a, b) } else { (b, a) };
    let mut out: Vec<Unit> = Vec::new();
    for u in first.into_iter().chain(second) {
        match out.iter().find(|w| w.key == u.key) {
            Some(w) if w.body() == u.body() => {}
            Some(_) => return None,
            None => out.push(u),
        }
    }
    Some(out)
}

fn merge_by_key(a: Vec<Unit>, b: Vec<Unit>) -> Option<Vec<Unit>> {
    let (mut i, mut j) = (0, 0);
    let mut out: Vec<Unit> = Vec::new();
    let mut seen: Vec<(String, Vec<String>)> = Vec::new();
    let mut push = |u: Unit, out: &mut Vec<Unit>| -> Option<()> {
        match seen.iter().find(|(k, _)| *k == u.key) {
            Some((_, body)) if *body == u.body() => Some(()),
            Some(_) => None,
            None => {
                seen.push((u.key.clone(), u.body()));
                out.push(u);
                Some(())
            }
        }
    };
    while i < a.len() || j < b.len() {
        let take_a = match (a.get(i), b.get(j)) {
            (Some(x), Some(y)) if x.key == y.key => {
                if x.body() != y.body() {
                    return None;
                }
                // The same element: once, the smaller spelling.
                let u = if x.lines <= y.lines {
                    x.clone()
                } else {
                    y.clone()
                };
                push(u, &mut out)?;
                i += 1;
                j += 1;
                continue;
            }
            (Some(x), Some(y)) => x.key < y.key,
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
    Some(out)
}

/// D3 over one region of a file with a container recogniser. `a`/`b` are
/// the sides' views of the file (see [`SideText`]); `scope` reads the
/// file's `weave-set` declaration, and is asked only when an
/// order-sensitive container is found. The answer's lines, and the claims
/// [`certify`] must check on the whole answer.
pub(crate) fn union(
    o: &[String],
    a: &[String],
    b: &[String],
    ours: SideText,
    theirs: SideText,
    path: &str,
    registry: &ParserRegistry,
    scope: &dyn Fn() -> SetScope,
) -> Option<(Vec<String>, Vec<Claim>)> {
    let l = lang(path)?;
    let (x, y) = (split(o, a)?, split(o, b)?);
    if x.runs.is_empty() || y.runs.is_empty() {
        return None;
    }
    let (pa, pb) = (
        parse(l, ours.text, path, registry)?,
        parse(l, theirs.text, path, registry)?,
    );
    // Each run, read in its own side's text.
    type Read = (Option<usize>, Vec<Unit>, Vec<String>, Found);
    let read = |side: &Split, lines: &[String], p: &Parsed, at: usize| -> Option<Vec<Read>> {
        side.runs
            .iter()
            .map(|(g, r)| {
                let rows = at + r.start..at + r.end;
                let found = p.find(l, rows.clone())?;
                // An inserted key the side's container already has elsewhere.
                if found.inside.iter().any(|(_, k)| found.others.contains(k)) {
                    return None;
                }
                let (us, trail) = units(&lines[r.clone()], &found, rows.start)?;
                Some((*g, us, trail, found))
            })
            .collect()
    };
    let (ra, rb) = (read(&x, a, &pa, ours.at)?, read(&y, b, &pb, theirs.at)?);
    // Every container is a set, by the language or by declaration.
    let mut declared: Option<SetScope> = None;
    for (_, _, _, f) in ra.iter().chain(&rb) {
        if !f.spec.set && !declared.get_or_insert_with(scope).admits(&f.names) {
            return None;
        }
        // No side states one key twice among its insertions.
        let mut ks: Vec<&String> = f.inside.iter().map(|(_, k)| k).collect();
        ks.sort();
        let n = ks.len();
        ks.dedup();
        if ks.len() != n {
            return None;
        }
    }
    let run_at = |rs: &[Read], g: Option<usize>| rs.iter().position(|r| r.0 == g);
    let mut out: Vec<String> = Vec::new();
    let mut claims: Vec<Claim> = Vec::new();
    let mut emit_gap = |g: Option<usize>, out: &mut Vec<String>| -> Option<()> {
        let (ia, ib) = (run_at(&ra, g), run_at(&rb, g));
        let (units, trail, found) = match (ia, ib) {
            (None, None) => return Some(()),
            (Some(i), None) => (ra[i].1.clone(), ra[i].2.clone(), &ra[i].3),
            (None, Some(j)) => (rb[j].1.clone(), rb[j].2.clone(), &rb[j].3),
            (Some(i), Some(j)) => {
                let (fa, fb) = (&ra[i].3, &rb[j].3);
                if fa.sig != fb.sig || fa.spec != fb.spec || ra[i].2 != rb[j].2 {
                    return None;
                }
                let (ua, ub) = (ra[i].1.clone(), rb[j].1.clone());
                let mut merged = order(ua.clone(), ub.clone())?;
                if fa.spec.comma {
                    let (ca, cb) = (ua.last()?.has_comma(), ub.last()?.has_comma());
                    if ca != cb {
                        return None;
                    }
                    let n = merged.len();
                    for (k, u) in merged.iter_mut().enumerate() {
                        if (k + 1 < n || ca) && !u.has_comma() {
                            let last = u.lines.last_mut()?;
                            *last = add_comma(last, l)?;
                        }
                    }
                }
                (merged, ra[i].2.clone(), fa)
            }
        };
        let start = out.len();
        let keys: Vec<String> = units.iter().map(|u| u.key.clone()).collect();
        for u in units {
            out.extend(u.lines);
        }
        claims.push(Claim {
            rows: start..out.len(),
            sig: found.sig.clone(),
            keys,
        });
        out.extend(trail);
        Some(())
    };
    emit_gap(None, &mut out)?;
    for k in 0..o.len() {
        // A base line: the side's spelling with a comma, if either added one.
        let (p, q) = (&a[x.kept[k]], &b[y.kept[k]]);
        out.push(if uncomma(p).1 { p.clone() } else { q.clone() });
        emit_gap(Some(k), &mut out)?;
    }
    Some((out, claims))
}

/// The whole answer holds every claim: it parses; at each claim's rows is
/// the container it names, holding exactly the claimed keys there, none of
/// them stated anywhere else in that container, and none of them a key the
/// base's container of that name already had (an element moved, not
/// inserted). `claims` carry rows of `merged`.
pub(crate) fn certify(
    base: &str,
    merged: &str,
    claims: &[Claim],
    path: &str,
    registry: &ParserRegistry,
) -> bool {
    let Some(l) = lang(path) else {
        return false;
    };
    let (Some(pm), Some(pb)) = (
        parse(l, merged, path, registry),
        parse(l, base, path, registry),
    ) else {
        return false;
    };
    claims.iter().all(|c| {
        let Some(f) = pm.find(l, c.rows.clone()) else {
            return false;
        };
        let mut got: Vec<&String> = f.inside.iter().map(|(_, k)| k).collect();
        let mut want: Vec<&String> = c.keys.iter().collect();
        got.sort();
        want.sort();
        if f.sig != c.sig || got != want || got.windows(2).any(|w| w[0] == w[1]) {
            return false;
        }
        if c.keys.iter().any(|k| f.others.contains(k)) {
            return false;
        }
        match pb.keys_named(l, &c.sig) {
            Some(before) => !c.keys.iter().any(|k| before.contains(k)),
            None => false,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merge::PARSER_REGISTRY;

    fn v(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|l| format!("{l}\n")).collect()
    }

    fn found(path: &str, text: &str, rows: Range<usize>) -> Option<Found> {
        let l = lang(path)?;
        parse(l, text, path, &PARSER_REGISTRY)?.find(l, rows)
    }

    #[test]
    fn a_dict_entry_is_found_with_its_key() {
        let t = "H = {\n    \"a\": 1,\n    \"b\": 2,\n}\n";
        let f = found("m.py", t, 2..3).expect("found");
        assert_eq!(f.spec.kind, "dict");
        assert!(f.spec.set);
        assert_eq!(f.names, vec!["H".to_string()]);
        assert_eq!(f.inside, vec![(2..3, "s:b".to_string())]);
        assert_eq!(f.others, vec!["s:a".to_string()]);
    }

    #[test]
    fn keys_that_are_not_decidable_constants_are_refused() {
        for t in [
            "H = {\n    \"a\": 1,\n    KEY: 2,\n}\n",
            "H = {\n    \"a\": 1,\n    **more,\n}\n",
            "H = {\n    \"a\": 1,\n    f\"b{x}\": 2,\n}\n",
            "H = {\n    \"a\": 1,\n    \"\\x62\": 2,\n}\n",
            "H = {\n    \"a\": 1,\n    True: 2,\n}\n",
            "H = {\n    \"a\": 1,\n    1.0: 2,\n}\n",
        ] {
            assert!(found("m.py", t, 2..3).is_none(), "{t}");
        }
        for t in [
            "const H = {\n  a: 1,\n  [k]: 2,\n};\n",
            "const H = {\n  a: 1,\n  ...more,\n};\n",
            "const H = {\n  a: 1,\n  get b() { return 1; },\n};\n",
        ] {
            assert!(found("m.ts", t, 2..3).is_none(), "{t}");
        }
    }

    #[test]
    fn order_sensitive_containers_are_found_but_not_sets() {
        let t = "X = [\n    \"a\",\n    \"b\",\n]\n";
        let f = found("m.py", t, 2..3).expect("found");
        assert_eq!((f.spec.kind, f.spec.set), ("list", false));
        let t = "__all__ = [\n    \"a\",\n    \"b\",\n]\n";
        let f = found("m.py", t, 2..3).expect("found");
        assert_eq!((f.spec.kind, f.spec.set), ("__all__", true));
        let t = "def f(x):\n    match x:\n        case 1:\n            return 1\n        case 2:\n            return 2\n";
        let f = found("m.py", t, 4..6).expect("found");
        assert_eq!((f.spec.kind, f.spec.set), ("match arms", false));
        assert_eq!(f.names, vec!["f".to_string()]);
        let t = "def build(sub):\n    sub.add(\"a\")\n    sub.add(\"b\")\n    return sub\n";
        let f = found("m.py", t, 2..3).expect("found");
        assert_eq!((f.spec.kind, f.spec.set), ("statements", false));
        assert_eq!(f.names, vec!["build".to_string()]);
    }

    #[test]
    fn two_elements_on_one_line_are_refused() {
        let t = "H = {\n    \"a\": 1,\n    \"b\": 2, \"c\": 3,\n}\n";
        assert!(found("m.py", t, 2..3).is_none());
    }

    #[test]
    fn ini_keys_and_value_lines() {
        let t = "[metadata]\nname = p\nversion = 1\n[options.entry_points]\nconsole_scripts =\n    a = p.a:main\n    b = p.b:main\n";
        let f = found("setup.cfg", t, 2..3).expect("keys");
        assert_eq!((f.spec.kind, f.spec.set), ("ini keys", true));
        assert_eq!(f.inside, vec![(2..3, "version".to_string())]);
        let f = found("setup.cfg", t, 6..7).expect("value lines");
        assert_eq!((f.spec.kind, f.spec.set), ("ini value lines", false));
        assert!(f.names.contains(&"console_scripts".to_string()));
        // A key inserted between a key and its continuation splits the value.
        let t = "[s]\nk =\n    one\n";
        assert!(
            found("x.ini", "[s]\nk =\nnew = 1\n    one\n", 2..3).is_none(),
            "{t}"
        );
    }

    #[test]
    fn runs_merge_by_key_the_same_either_way() {
        let u = |k: &str| Unit {
            key: k.to_string(),
            lines: vec![format!("{k},\n")],
            lead: 0,
        };
        let a = vec![u("b"), u("d")];
        let b = vec![u("a"), u("c"), u("e")];
        let ab = merge_by_key(a.clone(), b.clone()).expect("merged");
        let ba = merge_by_key(b, a).expect("merged");
        let keys = |v: &[Unit]| v.iter().map(|u| u.key.clone()).collect::<Vec<_>>();
        assert_eq!(keys(&ab), keys(&ba));
        assert_eq!(keys(&ab), vec!["a", "b", "c", "d", "e"]);
        // one key, two texts
        let x = vec![Unit {
            key: "k".into(),
            lines: v(&["k = 1"]),
            lead: 0,
        }];
        let y = vec![Unit {
            key: "k".into(),
            lines: v(&["k = 2"]),
            lead: 0,
        }];
        assert!(merge_by_key(x, y).is_none());
    }

    #[test]
    fn a_commented_group_is_kept_whole() {
        let g = vec![
            Unit {
                key: "a".into(),
                lines: v(&["# group", "a,"]),
                lead: 1,
            },
            Unit {
                key: "z".into(),
                lines: v(&["z,"]),
                lead: 0,
            },
        ];
        let m = vec![Unit {
            key: "m".into(),
            lines: v(&["m,"]),
            lead: 0,
        }];
        let keys = |v: &[Unit]| v.iter().map(|u| u.key.clone()).collect::<Vec<_>>();
        let x = order(g.clone(), m.clone()).expect("ordered");
        assert_eq!(keys(&x), vec!["a", "z", "m"]);
        assert_eq!(keys(&order(m, g).expect("ordered")), keys(&x));
    }

    #[test]
    fn certify_refuses_an_answer_that_does_not_parse_or_restates_a_base_key() {
        let claim = || {
            vec![Claim {
                rows: 2..3,
                sig: "dict@H".to_string(),
                keys: vec!["s:b".to_string()],
            }]
        };
        let base = "H = {\n    \"a\": 1,\n}\n";
        let good = "H = {\n    \"a\": 1,\n    \"b\": 2,\n}\n";
        assert!(certify(base, good, &claim(), "m.py", &PARSER_REGISTRY));
        let broken = "H = {\n    \"a\": 1,\n    \"b\": 2,\n\n";
        assert!(!certify(base, broken, &claim(), "m.py", &PARSER_REGISTRY));
        // `b` was already in the base's container: moved, not inserted.
        let had_b = "H = {\n    \"a\": 1,\n}\nX = 1\nH2 = {\"b\": 0}\n";
        assert!(certify(had_b, good, &claim(), "m.py", &PARSER_REGISTRY));
        let had_b = "H = {\n    \"b\": 0,\n    \"a\": 1,\n}\n";
        assert!(!certify(had_b, good, &claim(), "m.py", &PARSER_REGISTRY));
        // Stated twice in the answer.
        let twice = "H = {\n    \"b\": 1,\n    \"b\": 2,\n}\n";
        assert!(!certify(base, twice, &claim(), "m.py", &PARSER_REGISTRY));
        // Another container at those rows.
        let other = "G = {\n    \"a\": 1,\n    \"b\": 2,\n}\n";
        assert!(!certify(base, other, &claim(), "m.py", &PARSER_REGISTRY));
    }

    #[test]
    fn split_refuses_an_edited_or_moved_base_line() {
        let o = v(&["    \"a\": 1,"]);
        assert!(split(&o, &v(&["    \"a\": 2,", "    \"b\": 2,"])).is_none());
        assert!(split(&o, &v(&["    \"b\": 2,"])).is_none());
        let s = split(&o, &v(&["    \"a\": 1,", "    \"b\": 2,"])).expect("split");
        assert_eq!(s.runs, vec![(Some(0), 1..2)]);
    }
}
