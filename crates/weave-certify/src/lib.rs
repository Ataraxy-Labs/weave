//! Merge certificate ("proof mode"): an independent check that a merge result
//! `M` of base `O`, ours `A` and theirs `B` is the pushout-style selection.
//!
//! Each version is cut into REGIONS by raw line ranges: every top-level entity
//! sem-core extracts (parent_id = None, type != "chunk") is a region keyed
//! `type::name#k` (k = occurrence ordinal); the text before the first entity is
//! `^`; the text after entity `e` up to the next entity is `after:e`. Entities
//! whose line ranges overlap are fused into one region keyed `k1+k2`. Nothing
//! depends on line numbers, and every byte of the file lands in one region
//! (so a BOM or a trailing newline is part of a region and is compared).
//!
//! Normalisation: CRLF -> LF on all four inputs. Nothing else.
//!
//! Rule, per key present in any version (absent = `None`):
//!   A == B -> M = A;  A == O -> M = B;  B == O -> M = A;  otherwise the key is
//!   BOTH-CHANGED and M is certified at that key only by an enabled allowance.
//! Order: for every pair of keys in M, their relative order is the three-way
//! selection of their order in O, A and B (free only when no side has both).
//!
//! This file must not import any weave merge module.

use sem_core::parser::registry::ParserRegistry;
use std::collections::{HashMap, HashSet};

use std::rc::Rc;

pub mod elem;

struct Ent {
    id: String,
    parent: Option<String>,
    s: usize, // first line, 0-based
    e: usize, // last line, inclusive
    tn: String,
}

struct Src {
    t: String,
    off: Vec<usize>,
    ents: Vec<Ent>,
    /// The parse tree, for languages `elem_union` reads (else `None`).
    tree: Option<tree_sitter::Tree>,
}

pub struct Version {
    pub keys: Vec<String>,
    pub text: HashMap<String, String>,
    /// For a single-entity region key: (sem-core id, first line, last line).
    span: HashMap<String, (String, usize, usize)>,
    /// Every region key: its byte range [lo, hi) in the source.
    bytes: HashMap<String, (usize, usize)>,
    src: Rc<Src>,
}

pub fn normalize(s: &str) -> String {
    s.replace("\r\n", "\n")
}

/// Line start offsets, with a final entry = text.len().
fn line_offsets(t: &str) -> Vec<usize> {
    let mut v = vec![0];
    for (i, c) in t.bytes().enumerate() {
        if c == b'\n' && i + 1 < t.len() {
            v.push(i + 1);
        }
    }
    if t.is_empty() {
        v.clear();
    }
    v.push(t.len());
    v
}

pub fn decompose(reg: &ParserRegistry, path: &str, t: &str) -> Version {
    let off = line_offsets(t);
    let n = off.len() - 1; // number of lines
    let ents = reg
        .extract_entities(path, t)
        .into_iter()
        .filter(|e| {
            e.entity_type != "chunk"
                && e.start_line >= 1
                && e.start_line <= n
                && e.end_line >= e.start_line
        })
        .map(|e| Ent {
            s: e.start_line - 1,
            e: e.end_line.min(n) - 1,
            tn: format!("{}::{}", e.entity_type, e.name),
            id: e.id,
            parent: e.parent_id,
        })
        .collect();
    // Entities come from `extract_entities` exactly as before; the tree is a
    // second parse, taken only where `elem_union` can use it.
    let tree = elem::lang(path)
        .and_then(|_| reg.extract_entities_with_tree(path, t))
        .and_then(|(_, tree)| tree);
    build(
        Rc::new(Src {
            t: t.to_string(),
            off,
            ents,
            tree,
        }),
        0,
        n,
        None,
    )
}

/// Regions of lines [lo, hi) cut at the entities whose parent is `parent`.
fn build(src: Rc<Src>, lo: usize, hi: usize, parent: Option<&str>) -> Version {
    let mut raw: Vec<(usize, usize, &str, &str)> = src
        .ents
        .iter()
        .filter(|e| e.parent.as_deref() == parent && e.s >= lo && e.s < hi)
        .map(|e| (e.s, e.e.min(hi - 1), e.tn.as_str(), e.id.as_str()))
        .collect();
    raw.sort();
    let mut seen: HashMap<&str, usize> = HashMap::new();
    let mut ents: Vec<(usize, usize, String, Option<&str>)> = Vec::new();
    for (s, e, tn, id) in raw {
        let c = seen.entry(tn).or_insert(0);
        let key = format!("{tn}#{c}");
        *c += 1;
        match ents.last_mut() {
            Some(last) if s <= last.1 => {
                last.1 = last.1.max(e);
                last.2 = format!("{}+{}", last.2, key);
                last.3 = None; // fused regions are not descended into
            }
            _ => ents.push((s, e, key, Some(id))),
        }
    }
    let (mut keys, mut text, mut span, mut bytes) =
        (vec![], HashMap::new(), HashMap::new(), HashMap::new());
    let mut push = |k: String, a: usize, b: usize| {
        keys.push(k.clone());
        text.insert(k.clone(), src.t[src.off[a]..src.off[b]].to_string());
        bytes.insert(k, (src.off[a], src.off[b]));
    };
    push("^".into(), lo, ents.first().map_or(hi, |e| e.0));
    for (i, (s, e, k, id)) in ents.iter().enumerate() {
        push(k.clone(), *s, e + 1);
        push(
            format!("after:{k}"),
            e + 1,
            ents.get(i + 1).map_or(hi, |x| x.0),
        );
        if let Some(id) = id {
            span.insert(k.clone(), (id.to_string(), *s, *e));
        }
    }
    Version {
        keys,
        text,
        span,
        bytes,
        src: src.clone(),
    }
}

/// The children of region `k` as a Version of their own, if `k` is one entity.
fn descend(v: Option<&Version>, k: &str) -> Option<Version> {
    let v = v?;
    let (id, s, e) = v.span.get(k)?;
    Some(build(v.src.clone(), *s, e + 1, Some(id)))
}

/// Three-way selection. `None` = both changed, differently.
pub fn select<'a, T: PartialEq>(
    o: Option<&'a T>,
    a: Option<&'a T>,
    b: Option<&'a T>,
) -> Option<Option<&'a T>> {
    if a == b {
        Some(a)
    } else if a == o {
        Some(b)
    } else if b == o {
        Some(a)
    } else {
        None
    }
}

// ---------------------------------------------------------------- allowance 1
// diff3 over lines, git-merge-file semantics: changes not separated by a line
// unchanged on BOTH sides form one chunk; a chunk changed differently on both
// sides is a conflict (so adjacent edits conflict, as in git).

fn split_lines(t: &str) -> Vec<&str> {
    t.split_inclusive('\n').collect()
}

/// For each line of `o`, the index of its LCS partner in `x`, if any.
fn lcs_map(o: &[&str], x: &[&str]) -> Option<Vec<Option<usize>>> {
    let mut map = vec![None; o.len()];
    let p = o.iter().zip(x).take_while(|(a, b)| a == b).count();
    let s = o[p..]
        .iter()
        .rev()
        .zip(x[p..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    for (i, slot) in map.iter_mut().enumerate().take(p) {
        *slot = Some(i);
    }
    for i in 0..s {
        map[o.len() - 1 - i] = Some(x.len() - 1 - i);
    }
    let (om, xm) = (&o[p..o.len() - s], &x[p..x.len() - s]);
    let (n, m) = (om.len(), xm.len());
    if (n + 1) * (m + 1) > 20_000_000 {
        return None; // too large: the allowance declines (fail closed)
    }
    let w = m + 1;
    let mut dp = vec![0u32; (n + 1) * w];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i * w + j] = if om[i] == xm[j] {
                dp[(i + 1) * w + j + 1] + 1
            } else {
                dp[(i + 1) * w + j].max(dp[i * w + j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if om[i] == xm[j] {
            map[p + i] = Some(p + j);
            i += 1;
            j += 1;
        } else if dp[(i + 1) * w + j] >= dp[i * w + j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    Some(map)
}

/// Clean diff3 merge of line sequences, or `None` on conflict / too large.
pub fn diff3(o: &str, a: &str, b: &str) -> Option<String> {
    diff3_with(o, a, b, |_, _, _| None)
}

// ---------------------------------------------------------------- allowance 3
// Joint insertion: a diff3 conflict chunk in which BOTH sides only inserted
// (no base line in the chunk) and the two inserted blocks share no non-blank
// line is resolved as A's block then B's (`a_first`) or B's then A's. M must
// equal one of the two, with the same choice at every such chunk.

pub fn union(o: &str, a: &str, b: &str, a_first: bool) -> Option<String> {
    diff3_with(o, a, b, |oc, ac, bc| {
        let nb = |t: &str| {
            t.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(String::from)
                .collect::<HashSet<_>>()
        };
        if !oc.is_empty() || !nb(ac).is_disjoint(&nb(bc)) {
            return None;
        }
        Some(if a_first {
            format!("{ac}{bc}")
        } else {
            format!("{bc}{ac}")
        })
    })
}

// ---------------------------------------------------------------- allowance 4
// Subsumption: `x`'s line edit of `o` is contained in `y`'s: every line x
// inserted survives into y (LCS x~y), and every base line x deleted y deleted
// too. Then the region is y. `same_deletes`: y deleted no base line x kept, so
// y's edit beyond x's is insertions only (y cannot remove what x still uses).

fn contains_edit(o: &str, x: &str, y: &str, same_deletes: bool) -> Option<bool> {
    let (ol, xl, yl) = (split_lines(o), split_lines(x), split_lines(y));
    let (ox, oy, xy) = (lcs_map(&ol, &xl)?, lcs_map(&ol, &yl)?, lcs_map(&xl, &yl)?);
    let kept: HashSet<usize> = ox.iter().flatten().copied().collect();
    let inserted_ok = (0..xl.len()).all(|j| kept.contains(&j) || xy[j].is_some());
    let deleted_ok = (0..ol.len()).all(|i| {
        (ox[i].is_some() || oy[i].is_none())
            && (!same_deletes || ox[i].is_none() || oy[i].is_some())
    });
    Some(inserted_ok && deleted_ok)
}

pub fn subsume<'a>(o: &str, a: &'a str, b: &'a str, same_deletes: bool) -> Option<&'a str> {
    if contains_edit(o, b, a, same_deletes)? {
        Some(a)
    } else if contains_edit(o, a, b, same_deletes)? {
        Some(b)
    } else {
        None
    }
}

fn diff3_with(
    o: &str,
    a: &str,
    b: &str,
    fallback: impl Fn(&str, &str, &str) -> Option<String>,
) -> Option<String> {
    let (ol, al, bl) = (split_lines(o), split_lines(a), split_lines(b));
    let (ma, mb) = (lcs_map(&ol, &al)?, lcs_map(&ol, &bl)?);
    let mut out = String::new();
    let (mut i, mut j, mut k) = (0, 0, 0);
    loop {
        let stable = (i..ol.len()).find(|&x| ma[x].is_some() && mb[x].is_some());
        let (i2, j2, k2) = match stable {
            Some(x) => (x, ma[x].unwrap(), mb[x].unwrap()),
            None => (ol.len(), al.len(), bl.len()),
        };
        if (i2, j2, k2) == (i, j, k) {
            if i == ol.len() {
                return Some(out);
            }
            out.push_str(ol[i]);
            (i, j, k) = (i + 1, j + 1, k + 1);
            continue;
        }
        let (oc, ac, bc) = (ol[i..i2].concat(), al[j..j2].concat(), bl[k..k2].concat());
        match select(Some(&oc), Some(&ac), Some(&bc)) {
            Some(t) => out.push_str(t?),
            None => out.push_str(&fallback(&oc, &ac, &bc)?),
        }
        (i, j, k) = (i2, j2, k2);
    }
}

// ---------------------------------------------------------------- allowance 2
// Import blocks as a SET of import lines: every line either side added or
// removed is an import line, M's lines are O minus both sides' removals plus
// both sides' additions, M's import lines are distinct, no name is bound twice,
// and M keeps each side's order. `strict` also requires that the two sides
// removed disjoint lines (a line both removed and each replaced differently is
// a same-line dual edit, not two independent additions).

fn is_import(l: &str) -> bool {
    let t = l.trim_start();
    let t = t
        .strip_prefix("pub(crate) ")
        .or_else(|| t.strip_prefix("pub "))
        .unwrap_or(t);
    [
        "use ",
        "import ",
        "from ",
        "using ",
        "global using ",
        "#include",
        "extern crate ",
    ]
    .iter()
    .any(|p| t.starts_with(p))
}

fn last_seg(s: &str, sep: &str) -> String {
    s.rsplit(sep).next().unwrap_or(s).trim().to_string()
}

fn alias_or(item: &str, sep: &str) -> Option<String> {
    let item = item.trim();
    if item.is_empty() || item == "*" || item.ends_with("::*") || item.ends_with(".*") {
        return Some(String::new());
    }
    Some(match item.split_once(" as ") {
        Some((_, a)) => a.trim().to_string(),
        None => last_seg(item, sep),
    })
}

/// Names an import line binds, by file extension; `None` = can't tell (decline).
pub fn bound_names(path: &str, l: &str) -> Option<Vec<String>> {
    let ext = path.rsplit('.').next().unwrap_or("");
    let t = l.trim().trim_end_matches(';').trim();
    let t = t
        .strip_prefix("pub(crate) ")
        .or_else(|| t.strip_prefix("pub "))
        .unwrap_or(t);
    if t.starts_with("#include") {
        return Some(vec![]);
    }
    let names: Vec<Option<String>> = match ext {
        "rs" => {
            let body = t
                .strip_prefix("use ")
                .or_else(|| t.strip_prefix("extern crate "))?;
            match body.split_once('{') {
                None => vec![alias_or(body, "::")],
                Some((pre, rest)) => {
                    let inner = rest.strip_suffix('}')?;
                    if inner.contains('{') {
                        return None;
                    }
                    let pre = pre.trim_end_matches("::");
                    inner
                        .split(',')
                        .map(|i| {
                            if i.trim() == "self" {
                                Some(last_seg(pre, "::"))
                            } else {
                                alias_or(i, "::")
                            }
                        })
                        .collect()
                }
            }
        }
        "py" => {
            if t.contains('(') {
                return None;
            }
            if let Some(r) = t.strip_prefix("from ") {
                r.split_once(" import ")?
                    .1
                    .split(',')
                    .map(|i| alias_or(i, "\u{0}"))
                    .collect()
            } else {
                let r = t.strip_prefix("import ")?;
                r.split(',')
                    .map(|i| match i.split_once(" as ") {
                        Some((_, a)) => Some(a.trim().to_string()),
                        None => Some(i.trim().split('.').next()?.to_string()),
                    })
                    .collect()
            }
        }
        "js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs" | "mts" | "cts" => {
            let r = t.strip_prefix("import ")?;
            let r = r.strip_prefix("type ").unwrap_or(r);
            if r.starts_with('"') || r.starts_with('\'') {
                return Some(vec![]);
            }
            let (spec, _) = r.split_once(" from ")?;
            let mut out = vec![];
            let (def, braces) = match spec.split_once('{') {
                Some((d, b)) => (d, Some(b.strip_suffix('}')?.to_string())),
                None => (spec, None),
            };
            for d in def.split(',').filter(|d| !d.trim().is_empty()) {
                out.push(alias_or(d, "\u{0}"));
            }
            for i in braces.iter().flat_map(|b| b.split(',')) {
                out.push(alias_or(i.trim().trim_start_matches("type "), "\u{0}"));
            }
            out
        }
        "java" | "kt" | "kts" | "scala" | "groovy" => {
            let r = t.strip_prefix("import ")?;
            vec![alias_or(r.strip_prefix("static ").unwrap_or(r), ".")]
        }
        "cs" => {
            let r = t
                .strip_prefix("global ")
                .unwrap_or(t)
                .strip_prefix("using ")?;
            match r.split_once('=') {
                Some((a, _)) => vec![Some(a.trim().to_string())],
                None => vec![],
            }
        }
        _ => return None,
    };
    names
        .into_iter()
        .filter(|n| n.as_deref() != Some(""))
        .collect()
}

fn counts<'a>(ls: &[&'a str]) -> HashMap<&'a str, i64> {
    let mut m = HashMap::new();
    for l in ls {
        *m.entry(*l).or_insert(0) += 1;
    }
    m
}

/// "admit" | "mismatch" | "decline" for the import-set allowance.
pub fn import_set(path: &str, o: &str, a: &str, b: &str, m: &str, strict: bool) -> &'static str {
    let (ol, al, bl, ml) = (
        split_lines(o),
        split_lines(a),
        split_lines(b),
        split_lines(m),
    );
    let (co, ca, cb, cm) = (counts(&ol), counts(&al), counts(&bl), counts(&ml));
    let g = |c: &HashMap<&str, i64>, l: &str| *c.get(l).unwrap_or(&0);
    let mut all: Vec<&str> = ol
        .iter()
        .chain(&al)
        .chain(&bl)
        .chain(&ml)
        .copied()
        .collect();
    all.sort();
    all.dedup();
    if all
        .iter()
        .any(|l| (g(&ca, l) != g(&co, l) || g(&cb, l) != g(&co, l)) && !is_import(l))
    {
        return "decline";
    }
    let mut deleted_by_both = false;
    for &l in &all {
        let (o_, a_, b_) = (g(&co, l), g(&ca, l), g(&cb, l));
        deleted_by_both |= a_ < o_ && b_ < o_;
        let del = (o_ - a_).max(0).max((o_ - b_).max(0));
        let add = (a_ - o_).max(0).max((b_ - o_).max(0));
        if g(&cm, l) != o_ - del + add {
            return "mismatch";
        }
    }
    if strict && deleted_by_both {
        return "decline";
    }
    let imports: Vec<&str> = ml.iter().copied().filter(|l| is_import(l)).collect();
    let mut names = HashSet::new();
    let mut lines = HashSet::new();
    for l in &imports {
        if !lines.insert(l.trim()) {
            return "mismatch";
        }
        match bound_names(path, l) {
            None => return "decline",
            Some(ns) => {
                for n in ns {
                    if !names.insert(n) {
                        return "mismatch";
                    }
                }
            }
        }
    }
    for side in [&al, &bl] {
        let sset: HashSet<&str> = side.iter().copied().collect();
        let mseq: Vec<&str> = imports
            .iter()
            .copied()
            .filter(|l| sset.contains(l))
            .collect();
        let sseq: Vec<&str> = side
            .iter()
            .copied()
            .filter(|l| is_import(l) && cm.contains_key(l))
            .collect();
        if mseq != sseq {
            return "mismatch";
        }
    }
    "admit"
}

// ---------------------------------------------------------------- allowance 2b
// imp_used: imp_strict, plus a use check against the MERGED file. Every import
// line one side added (vs base) that M keeps must bind at least one name that
// M references outside its import lines; an import line one side removed must
// not be in M more often than on that side. An added line whose bound names
// can't be told (or that binds none: a namespace `using`, `#include`, a
// wildcard, a side-effect import) declines. References are a lexical
// identifier scan of M with import lines, comments and strings removed.

/// `admit` | `mismatch` | `conflict` (an added import M keeps is unused) |
/// `decline`. `m_file` is the whole merged file (for references); `m` the region.
pub fn import_used(path: &str, o: &str, a: &str, b: &str, m: &str, m_file: &str) -> &'static str {
    let v = import_set(path, o, a, b, m, true);
    if v != "admit" {
        return v;
    }
    let (ol, al, bl, ml) = (
        split_lines(o),
        split_lines(a),
        split_lines(b),
        split_lines(m),
    );
    let (co, ca, cb, cm) = (counts(&ol), counts(&al), counts(&bl), counts(&ml));
    let g = |c: &HashMap<&str, i64>, l: &str| *c.get(l).unwrap_or(&0);
    for &l in ol.iter().chain(&al).chain(&bl) {
        let (o_, a_, b_) = (g(&co, l), g(&ca, l), g(&cb, l));
        if is_import(l) && (a_ < o_ || b_ < o_) && g(&cm, l) > a_.min(b_) {
            return "mismatch"; // one side removed it; M must not reintroduce it
        }
    }
    let refs = references(path, m_file);
    let used = |n: &str| {
        refs.contains(n)
            || (path.ends_with(".cs")
                && n.len() > 9
                && n.ends_with("Attribute")
                && refs.contains(&n[..n.len() - 9]))
    };
    for &l in &ml {
        let (o_, a_, b_) = (g(&co, l), g(&ca, l), g(&cb, l));
        if !is_import(l) || (a_ <= o_ && b_ <= o_) {
            continue;
        }
        match bound_names(path, l) {
            Some(ns) if !ns.is_empty() => {
                if !ns.iter().any(|n| used(n)) {
                    return "conflict";
                }
            }
            _ => return "decline",
        }
    }
    "admit"
}

/// Identifiers in `t` outside import lines, comments and strings. Lexical, by
/// file extension. Where the scan can err it errs toward reading code as a
/// string (a name missed, so the file is rejected), not the reverse: nested
/// Rust block comments, raw and triple-quoted strings, C# verbatim strings and
/// Rust lifetimes are handled; template `${..}` / f-string holes count as
/// string (missed names only cost coverage).
pub fn references(path: &str, t: &str) -> HashSet<String> {
    let ext = path.rsplit('.').next().unwrap_or("");
    let (hash_comment, rust, cs) = (ext == "py", ext == "rs", ext == "cs");
    let js = matches!(
        ext,
        "js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs" | "mts" | "cts"
    );
    let char_rule = rust || ext == "scala"; // `'` is a char literal only if it closes at once
    let multiline_quotes = rust; // Rust "..." may span lines; elsewhere a newline ends it
    let code: String = t.split_inclusive('\n').filter(|l| !is_import(l)).collect();
    let s: Vec<char> = code.chars().collect();
    let mut out = HashSet::new();
    let (mut i, n) = (0, s.len());
    let at = |i: usize, p: &str| p.chars().enumerate().all(|(k, c)| s.get(i + k) == Some(&c));
    // skip a quoted string opened at i (after any prefix) with `q` repeated `qn` times
    let skip_quoted = |mut i: usize,
                       q: char,
                       qn: usize,
                       escapes: bool,
                       doubled: bool,
                       span_lines: bool|
     -> usize {
        i += qn;
        while i < n {
            if escapes && s[i] == '\\' {
                i += 2;
                continue;
            }
            if s[i] == q {
                if doubled && s.get(i + 1) == Some(&q) {
                    i += 2;
                    continue;
                }
                if (0..qn).all(|k| s.get(i + k) == Some(&q)) {
                    return i + qn;
                }
            }
            if s[i] == '\n' && qn == 1 && !span_lines {
                return i + 1;
            }
            i += 1;
        }
        n
    };
    while i < n {
        let c = s[i];
        if hash_comment && c == '#' || !hash_comment && at(i, "//") {
            while i < n && s[i] != '\n' {
                i += 1;
            }
        } else if !hash_comment && at(i, "/*") {
            let mut depth = 0;
            while i < n {
                if at(i, "/*") {
                    depth += 1;
                    i += 2;
                } else if at(i, "*/") {
                    depth -= 1;
                    i += 2;
                    if depth == 0 || !rust {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
        } else if !rust && at(i, "\"\"\"") || hash_comment && at(i, "'''") {
            i = skip_quoted(i, c, 3, true, false, true);
        } else if cs && (at(i, "@\"") || at(i, "$@\"") || at(i, "@$\"")) {
            let p = if c == '@' && s[i + 1] == '"' { 1 } else { 2 };
            i = skip_quoted(i + p, '"', 1, false, true, true);
        } else if c == '"' || c == '`' {
            i = skip_quoted(i, c, 1, c == '"', false, c == '`' || multiline_quotes);
        } else if c == '\'' {
            if char_rule {
                // 'x' or '\..' closes within a few chars: a char literal; else a lifetime / symbol
                let close = if s.get(i + 1) == Some(&'\\') {
                    (i + 3..(i + 12).min(n)).find(|&k| s[k] == '\'')
                } else if s.get(i + 2) == Some(&'\'') {
                    Some(i + 2)
                } else {
                    None
                };
                i = close.map_or(i + 1, |k| k + 1);
            } else {
                i = skip_quoted(i, '\'', 1, true, false, false);
            }
        } else if c.is_alphabetic() || c == '_' || c == '$' && js {
            let st = i;
            while i < n && (s[i].is_alphanumeric() || s[i] == '_' || s[i] == '$' && js) {
                i += 1;
            }
            let id: String = s[st..i].iter().collect();
            let quote_next = matches!(s.get(i), Some('"') | Some('\''));
            if rust && (id == "r" || id == "br") && matches!(s.get(i), Some('"') | Some('#')) {
                // raw string r"..", r#".."#: no escapes
                let h = s[i..].iter().take_while(|x| **x == '#').count();
                if s.get(i + h) == Some(&'"') {
                    let close = format!("\"{}", "#".repeat(h));
                    i += h + 1;
                    while i < n && !at(i, &close) {
                        i += 1;
                    }
                    i = (i + close.chars().count()).min(n);
                    continue;
                }
            }
            // a string prefix (py f"", rb""; rust b"") is not a reference
            let prefix = quote_next && id.len() <= 2 && id.chars().all(|x| "rRbBfFuU".contains(x));
            if !prefix {
                out.insert(id);
            }
        } else if c.is_ascii_digit() {
            while i < n && (s[i].is_alphanumeric() || s[i] == '_') {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    out
}

// ---------------------------------------------------------------- the check

pub struct Report {
    /// Failures no allowance can lift: (rule, key).
    pub hard: Vec<(String, String)>,
    /// Both-changed keys, each with every allowance's verdict on it:
    /// "admit" (lifts it and M matches), "mismatch" (lifts it, M differs),
    /// "conflict" (the allowance's own rule conflicts), "decline" (n/a).
    pub both: Vec<(String, Vec<(&'static str, &'static str)>)>,
}

pub const ALLOWANCES: [&str; 11] = [
    "diff3",
    "imp_strict",
    "imp_loose",
    "union",
    "subsume_any",
    "subsume",
    "subsume_ins",
    "nest",
    "imp_used",
    "elem_union",
    "nest_eu",
];

impl Report {
    /// Certified with this set of allowances enabled.
    pub fn certified(&self, allow: &[&str]) -> bool {
        self.hard.is_empty()
            && self
                .both
                .iter()
                .all(|(_, v)| v.iter().any(|(a, s)| allow.contains(a) && *s == "admit"))
    }
}

fn verdict(t: Option<String>, m: &str) -> &'static str {
    match t {
        None => "conflict",
        Some(t) if t == m => "admit",
        Some(_) => "mismatch",
    }
}

fn allowances(
    path: &str,
    k: &str,
    o: Option<&Version>,
    a: &Version,
    b: &Version,
    m: &Version,
) -> Vec<(&'static str, &'static str)> {
    let (ok, a_, b_, m_) = (get(o, k), &a.text[k], &b.text[k], m.text[k].as_str());
    let o_ = ok.map_or("", |s| s.as_str());
    let region = k == "^" || k.starts_with("after:");
    let imp = |s| {
        if region && ok.is_some() {
            import_set(path, o_, a_, b_, m_, s)
        } else {
            "decline"
        }
    };
    let un = match (union(o_, a_, b_, true), union(o_, a_, b_, false)) {
        (Some(x), _) if x == m_ => "admit",
        (_, Some(y)) if y == m_ => "admit",
        (None, None) => "conflict",
        _ => "mismatch",
    };
    // `nest`: the children are the selection, recursively. `nest_eu`: the
    // same, where a child may also be admitted by `elem_union`.
    let (nest, nest_eu) = match (
        descend(o, k),
        descend(Some(a), k),
        descend(Some(b), k),
        descend(Some(m), k),
    ) {
        (so, Some(sa), Some(sb), Some(sm)) if so.is_some() || ok.is_none() => {
            let sub = check(path, so.as_ref(), Some(&sa), Some(&sb), &sm);
            let v = |allow: &[&str]| {
                if !sub.hard.is_empty() {
                    "mismatch"
                } else if sub.certified(allow) {
                    "admit"
                } else {
                    "conflict"
                }
            };
            (v(&["nest"]), v(&["nest", "nest_eu", "elem_union"]))
        }
        _ => ("decline", "decline"),
    };
    let eu = match elem_union(path, k, o, a, b, m) {
        Ok(()) => "admit",
        Err(elem::Fail::Mismatch(_)) => "mismatch",
        Err(elem::Fail::Decline(_)) => "decline",
    };
    vec![
        ("diff3", verdict(diff3(o_, a_, b_), m_)),
        ("imp_strict", imp(true)),
        ("imp_loose", imp(false)),
        ("union", un),
        (
            "subsume_any",
            verdict(subsume(o_, a_, b_, false).map(String::from), m_),
        ),
        (
            "subsume",
            if ok.is_some() {
                verdict(subsume(o_, a_, b_, false).map(String::from), m_)
            } else {
                "decline"
            },
        ),
        (
            "subsume_ins",
            if ok.is_some() {
                verdict(subsume(o_, a_, b_, true).map(String::from), m_)
            } else {
                "decline"
            },
        ),
        ("nest", nest),
        (
            "imp_used",
            if region && ok.is_some() {
                import_used(path, o_, a_, b_, m_, &m.src.t)
            } else {
                "decline"
            },
        ),
        ("elem_union", eu),
        ("nest_eu", nest_eu),
    ]
}

/// The `elem_union` check of region `k` (see [`elem`]): `Ok` = admitted.
/// Only a region present in all four versions, with a base, in a language
/// [`elem`] reads, is examined; anything else declines.
pub fn elem_union(
    path: &str,
    k: &str,
    o: Option<&Version>,
    a: &Version,
    b: &Version,
    m: &Version,
) -> Result<(), elem::Fail> {
    let no = |why: &str| elem::Fail::Decline(why.to_string());
    let o = o.ok_or_else(|| no("no base"))?;
    let vs = [o, a, b, m];
    let mut ranges = [(0, 0); 4];
    let mut roots = Vec::with_capacity(4);
    for (v, x) in vs.iter().enumerate() {
        ranges[v] = *x
            .bytes
            .get(k)
            .ok_or_else(|| no("not a region of this version"))?;
        roots.push(
            x.src
                .tree
                .as_ref()
                .ok_or_else(|| no("no parse tree"))?
                .root_node(),
        );
    }
    elem::check_region(
        path,
        std::array::from_fn(|v| vs[v].src.t.as_str()),
        [roots[0], roots[1], roots[2], roots[3]],
        ranges,
    )
}

/// `elem_union` over the whole file (see [`elem::check_file`]).
pub fn elem_union_file(
    path: &str,
    o: Option<&Version>,
    a: &Version,
    b: &Version,
    m: &Version,
) -> Result<(), elem::Fail> {
    let no = |why: &str| elem::Fail::Decline(why.to_string());
    let vs = [o.ok_or_else(|| no("no base"))?, a, b, m];
    let mut roots = Vec::with_capacity(4);
    for x in vs {
        roots.push(
            x.src
                .tree
                .as_ref()
                .ok_or_else(|| no("no parse tree"))?
                .root_node(),
        );
    }
    elem::check_file(
        path,
        std::array::from_fn(|v| vs[v].src.t.as_str()),
        [roots[0], roots[1], roots[2], roots[3]],
    )
}

fn get<'a>(v: Option<&'a Version>, k: &str) -> Option<&'a String> {
    v.and_then(|v| v.text.get(k))
}

fn pos(v: Option<&Version>) -> Option<HashMap<&str, usize>> {
    v.map(|v| {
        v.keys
            .iter()
            .enumerate()
            .map(|(i, k)| (k.as_str(), i))
            .collect()
    })
}

fn union_keys(vs: &[Option<&Version>]) -> Vec<String> {
    let mut seen = HashSet::new();
    vs.iter()
        .flatten()
        .flat_map(|v| v.keys.iter())
        .filter(|k| seen.insert(k.as_str()))
        .cloned()
        .collect()
}

pub fn check(
    path: &str,
    o: Option<&Version>,
    a: Option<&Version>,
    b: Option<&Version>,
    m: &Version,
) -> Report {
    let mut r = Report {
        hard: vec![],
        both: vec![],
    };
    for k in union_keys(&[o, a, b, Some(m)]) {
        let (ok, ak, bk, mk) = (get(o, &k), get(a, &k), get(b, &k), m.text.get(&k));
        match select(ok, ak, bk) {
            Some(e) if e == mk => {}
            Some(None) => r.hard.push(("unexpected".into(), k)),
            Some(Some(_)) => r
                .hard
                .push((if mk.is_none() { "dropped" } else { "mismatch" }.into(), k)),
            None => {
                let v = match (a, b, ak.is_some() && bk.is_some() && mk.is_some()) {
                    (Some(a), Some(b), true) => allowances(path, &k, o, a, b, m),
                    _ => ALLOWANCES.iter().map(|n| (*n, "decline")).collect(),
                };
                r.both.push((k, v));
            }
        }
    }
    // Order: pairwise three-way selection of relative order.
    let (po, pa, pb) = (pos(o), pos(a), pos(b));
    let rel = |p: &Option<HashMap<&str, usize>>, x: &str, y: &str| {
        p.as_ref().and_then(|p| Some(p.get(x)? < p.get(y)?))
    };
    for i in 0..m.keys.len() {
        for j in i + 1..m.keys.len() {
            let (x, y) = (m.keys[i].as_str(), m.keys[j].as_str());
            let (ro, ra, rb) = (rel(&po, x, y), rel(&pa, x, y), rel(&pb, x, y));
            let want = match (ra, rb) {
                (None, None) => continue,
                (Some(v), None) | (None, Some(v)) => Some(v),
                _ => select(ro.as_ref(), ra.as_ref(), rb.as_ref()).map(|s| *s.unwrap()),
            };
            if want != Some(true) {
                r.hard.push(("order".into(), format!("{x} < {y}")));
                return r;
            }
        }
    }
    r
}

/// Build the merge by selection alone, lifting a both-changed key only by the
/// enabled constructive allowances ("nest": recurse into children;
/// "subsume_ins": the containing side; "diff3"), tried in that order.
/// Order: A's keys, then each B-only key right after its nearest preceding B
/// key already placed. `None` if some key cannot be selected.
pub fn construct(
    o: Option<&Version>,
    a: Option<&Version>,
    b: Option<&Version>,
    allow: &[&str],
) -> Option<String> {
    let mut seq: Vec<String> = a.map_or(vec![], |a| a.keys.clone());
    if let Some(b) = b {
        for (i, k) in b.keys.iter().enumerate() {
            if seq.contains(k) {
                continue;
            }
            let at = b.keys[..i]
                .iter()
                .rev()
                .find_map(|p| seq.iter().position(|s| s == p))
                .map_or(0, |p| p + 1);
            seq.insert(at, k.clone());
        }
    }
    let mut out = String::new();
    for k in &seq {
        let (ok, ak, bk) = (get(o, k), get(a, k), get(b, k));
        match select(ok, ak, bk) {
            Some(Some(t)) => out.push_str(t),
            Some(None) => {}
            None => {
                let nested = || {
                    let so = descend(o, k);
                    if ok.is_some() && so.is_none() {
                        return None;
                    }
                    construct(
                        so.as_ref(),
                        Some(&descend(a, k)?),
                        Some(&descend(b, k)?),
                        allow,
                    )
                };
                let t = allow.contains(&"nest").then(nested).flatten();
                let t = t.or_else(|| {
                    let o_ = ok.map(|s| s.as_str())?;
                    allow
                        .contains(&"subsume_ins")
                        .then(|| subsume(o_, ak?, bk?, true).map(String::from))
                        .flatten()
                });
                let t = t.or_else(|| {
                    allow
                        .contains(&"diff3")
                        .then(|| diff3(ok.map_or("", |s| s), ak?, bk?))
                        .flatten()
                });
                out.push_str(&t?);
            }
        }
    }
    Some(out)
}
