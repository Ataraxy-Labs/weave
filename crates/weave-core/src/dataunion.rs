//! D3 over a data file: two sides that each added distinct keys to one table
//! (or distinct items to a dependency-style array) at the same place.
//!
//! Keys of a JSON object, a TOML table or a YAML mapping are a set; adding
//! `b` and adding `c` commute whenever `b ≠ c`. The line merge refuses them
//! only because they landed on neighbouring lines. So the rule has two
//! halves:
//!
//! 1. **The text** ([`union`], one region at a time). Every base line of the
//!    region survives on both sides — exactly, except that a side may have
//!    added a trailing comma to it so something could follow it. The lines a
//!    side inserted are kept verbatim, where that side put them: after the
//!    same base line. Two sides' insertions after one base line are two runs,
//!    each in its own side's order, and the run whose text sorts first goes
//!    first (the convention of every set union here), so the answer does not
//!    depend on which side is called ours. Where two runs meet at the end of
//!    a comma-separated container, the first gets the comma the second one's
//!    line needs. Nothing is re-serialised, so every comment, indent, quote
//!    style and key order either side wrote is what the answer says.
//!
//! 2. **The certificate** ([`certify`], the whole file). The text is only a
//!    candidate. It is accepted when its value — what every consumer of the
//!    file reads — equals the three-way merge of the three values, computed
//!    key by key: a key both sides agree on, or only one side changed, takes
//!    that value; objects recurse; a scalar array under a dependency-style
//!    key merges as a set, and so does any array the file's `weave-set`
//!    gitattribute declares one (a YAML list of hooks, compared item by
//!    item as values — see [`crate::host::SetScope`]); anything else both sides changed (the same key
//!    with two different values, a key one side deleted and the other
//!    edited) is a conflict. A file any side of which does not load, or whose
//!    candidate does not load, is not settled.
//!
//! So the rule can only ever produce a file whose value is the key union, and
//! whose text is the base's lines and the two sides' own lines. The existing
//! gates — every line both sides kept survives, no key stated twice, the file
//! still loads — run after it, as they do after every rule.
//!
//! Not covered (conflict, as before): both sides created the file (there is no
//! base line to anchor either side's insertions); JSON with comments or
//! trailing commas (no strict value to certify); a base line a side edited or
//! deleted inside the region; the same key added by both with the same value
//! next to distinct ones.

use serde_json::Value;

/// Keys whose arrays list things in no particular order: dependency and
/// feature lists, globs, keywords. An array under any of these, at any depth
/// (`project.optional-dependencies.dev`), whose items are all scalars, merges
/// as a set.
const SET_KEYS: &[&str] = &[
    "dependencies",
    "optional-dependencies",
    "dependency-groups",
    "dev-dependencies",
    "build-dependencies",
    "requires",
    "features",
    "keywords",
    "categories",
    "classifiers",
    "members",
    "workspaces",
    "files",
    "include",
    "exclude",
    "ignore",
    "ignorePatterns",
    "lib",
    "types",
];

fn blank(l: &str) -> bool {
    l.trim().is_empty()
}

/// A line's text with its trailing comma (and end of line) taken off, and
/// whether it had one.
fn uncomma(l: &str) -> (&str, bool) {
    let t = l.trim_end();
    match t.strip_suffix(',') {
        Some(s) => (s.trim_end(), true),
        None => (t, false),
    }
}

fn eol(l: &str) -> &str {
    if l.ends_with("\r\n") {
        "\r\n"
    } else if l.ends_with('\n') {
        "\n"
    } else {
        ""
    }
}

/// `l` with a trailing comma, or `None` if a comment ends it (the comma would
/// land inside the comment).
fn with_comma(l: &str, f: Format) -> Option<String> {
    let body = l.trim_end();
    if has_comment(body, f) {
        return None;
    }
    Some(format!("{body},{}", eol(l)))
}

/// One side's region: which base lines it kept (base index → its line), and
/// what it inserted after each (`None` = before the first base line).
struct Side {
    kept: Vec<String>,
    after: Vec<(Option<usize>, Vec<String>)>,
}

fn side(o: &[String], x: &[String]) -> Option<Side> {
    let strip = |v: &[String]| -> Vec<String> {
        v.iter()
            .map(|l| format!("{}\n", uncomma(l).0))
            .collect::<Vec<_>>()
    };
    let (so, sx) = (strip(o), strip(x));
    let img = crate::determinate::image(&so, &sx);
    // Every base line kept, and in order.
    let mut kept = Vec::with_capacity(o.len());
    let mut last: Option<usize> = None;
    for (k, base_line) in o.iter().enumerate() {
        let j = *img.get(&k)?;
        if last.is_some_and(|p| j <= p) {
            return None;
        }
        // Kept exactly, or with a comma added — never with one taken away.
        let (base_text, base_comma) = uncomma(base_line);
        let (side_text, side_comma) = uncomma(&x[j]);
        if base_text != side_text || (base_comma && !side_comma) {
            return None;
        }
        kept.push(x[j].clone());
        last = Some(j);
    }
    let mut after: Vec<(Option<usize>, Vec<String>)> = Vec::new();
    let at: std::collections::HashMap<usize, usize> = img.iter().map(|(k, j)| (*j, *k)).collect();
    let mut gap: Option<usize> = None;
    for (j, l) in x.iter().enumerate() {
        if let Some(k) = at.get(&j) {
            gap = Some(*k);
            continue;
        }
        match after.last_mut() {
            Some((g, run)) if *g == gap => run.push(l.clone()),
            _ => after.push((gap, vec![l.clone()])),
        }
    }
    Some(Side { kept, after })
}

/// D3 over one region of the data file at `path`. `below` is the first line
/// after the region. See the module docs.
pub(crate) fn union(
    o: &[String],
    a: &[String],
    b: &[String],
    below: Option<&String>,
    path: &str,
) -> Option<Vec<String>> {
    let f = format(path)?;
    let (x, y) = (side(o, a)?, side(o, b)?);
    if x.after.is_empty() || y.after.is_empty() {
        return None;
    }
    let run = |s: &Side, g: Option<usize>| -> Vec<String> {
        s.after
            .iter()
            .find(|(k, _)| *k == g)
            .map(|(_, r)| r.clone())
            .unwrap_or_default()
    };
    // The first significant line after the gap: a container's end or not.
    let next_after = |g: Option<usize>| -> Option<&String> {
        let from = g.map_or(0, |k| k + 1);
        o[from..].iter().find(|l| !blank(l)).or(below)
    };
    let closes = |l: Option<&String>| l.is_some_and(|l| l.trim_start().starts_with([']', '}']));
    let last_line = |r: &[String]| r.iter().rposition(|l| !blank(l));
    let mut out: Vec<String> = Vec::new();
    for g in std::iter::once(None).chain((0..o.len()).map(Some)) {
        if let Some(k) = g {
            // A base line: the side's spelling with a comma, if either added
            // one (both spell it the same up to that comma).
            let (p, q) = (&x.kept[k], &y.kept[k]);
            out.push(if uncomma(p).1 { p.clone() } else { q.clone() });
        }
        let (ra, rb) = (run(&x, g), run(&y, g));
        if ra.is_empty() || rb.is_empty() || ra == rb {
            out.extend(if ra.is_empty() { rb } else { ra });
            continue;
        }
        // Two runs that share a line are not two sets of distinct keys.
        let lines = |r: &[String]| -> Vec<String> {
            r.iter()
                .filter(|l| !blank(l))
                .map(|l| uncomma(l).0.trim().to_string())
                .collect()
        };
        let (la, lb) = (lines(&ra), lines(&rb));
        if la.iter().any(|l| lb.contains(l)) {
            return None;
        }
        let (mut first, mut second) = (ra, rb);
        if lb.join("\n") < la.join("\n") {
            std::mem::swap(&mut first, &mut second);
        }
        let (fi, si) = (last_line(&first)?, last_line(&second)?);
        let (f_comma, s_comma) = (uncomma(&first[fi]).1, uncomma(&second[si]).1);
        if f_comma != s_comma {
            return None;
        }
        if !f_comma && closes(next_after(g)) {
            // Both runs ended their container; now the first one does not.
            first[fi] = with_comma(&first[fi], f)?;
        }
        out.extend(first);
        out.extend(second);
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// The certificate
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    Json,
    Toml,
    Yaml,
}

fn format(path: &str) -> Option<Format> {
    match crate::datafile::format_name(path)? {
        "JSON" => Some(Format::Json),
        "TOML" => Some(Format::Toml),
        _ => Some(Format::Yaml),
    }
}

/// Is `path` a data file this rule reads?
pub(crate) fn applies(path: &str) -> bool {
    format(path).is_some()
}

/// The documents of `text`, as values. `None` when it does not load strictly.
fn load(text: &str, f: Format) -> Option<Vec<Value>> {
    match f {
        Format::Json => serde_json::from_str::<Value>(text).ok().map(|v| vec![v]),
        Format::Toml => {
            let t: toml::Value = toml::from_str(text).ok()?;
            serde_json::to_value(t).ok().map(|v| vec![v])
        }
        Format::Yaml => {
            use serde::Deserialize;
            let mut docs = Vec::new();
            for d in serde_yaml::Deserializer::from_str(text) {
                let v = serde_yaml::Value::deserialize(d).ok()?;
                docs.push(serde_json::to_value(v).ok()?);
            }
            Some(docs)
        }
    }
}

/// Whether the array at a key path was declared a set by the file's owner
/// (the `weave-set` gitattribute, [`crate::host::SetScope`]): every array
/// for a bare declaration, the arrays under a named key otherwise. A
/// sequence is ordered by default — a list of hooks, of steps, of matrix
/// entries — so only a declaration makes one a set.
pub(crate) type Declared<'a> = &'a dyn Fn(&[String]) -> bool;

/// Nothing declared.
#[cfg(test)]
fn undeclared(_: &[String]) -> bool {
    false
}

fn set_path(path: &[String]) -> bool {
    path.iter().any(|k| SET_KEYS.contains(&k.as_str()))
}

fn scalars(v: &[Value]) -> bool {
    v.iter()
        .all(|x| matches!(x, Value::String(_) | Value::Number(_) | Value::Bool(_)))
}

/// A requirement's distribution name, normalised (PEP 503): `PyYAML>=6` and
/// `pyyaml` name one package.
fn package(item: &Value) -> Option<String> {
    let s = item.as_str()?.trim();
    let name: String = s
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
        .collect();
    (!name.is_empty()).then(|| name.to_ascii_lowercase().replace(['_', '.'], "-"))
}

/// The three-way merge of three values, key by key. `Err` is a conflict.
/// `requirements`: set arrays hold PEP 508 requirements, so one package
/// named twice is one key with two values.
fn merge3(
    o: Option<&Value>,
    a: Option<&Value>,
    b: Option<&Value>,
    path: &mut Vec<String>,
    requirements: bool,
    declared: Declared,
) -> Result<Option<Value>, ()> {
    if a == b {
        return Ok(a.cloned());
    }
    if a == o {
        return Ok(b.cloned());
    }
    if b == o {
        return Ok(a.cloned());
    }
    match (o, a, b) {
        (None | Some(Value::Object(_)), Some(Value::Object(ma)), Some(Value::Object(mb))) => {
            let empty = serde_json::Map::new();
            let mo = match o {
                Some(Value::Object(m)) => m,
                _ => &empty,
            };
            let mut out = serde_json::Map::new();
            let keys: std::collections::BTreeSet<&String> =
                mo.keys().chain(ma.keys()).chain(mb.keys()).collect();
            for k in keys {
                path.push(k.clone());
                let v = merge3(
                    mo.get(k),
                    ma.get(k),
                    mb.get(k),
                    path,
                    requirements,
                    declared,
                );
                path.pop();
                if let Some(v) = v? {
                    out.insert(k.clone(), v);
                }
            }
            Ok(Some(Value::Object(out)))
        }
        (None | Some(Value::Array(_)), Some(Value::Array(va)), Some(Value::Array(vb)))
            if set_path(path) || declared(path) =>
        {
            let vo: &[Value] = match o {
                Some(Value::Array(v)) => v,
                _ => &[],
            };
            // A set by convention holds scalars; a set by declaration holds
            // whatever its owner put in it, compared by value.
            if !(scalars(vo) && scalars(va) && scalars(vb)) && !declared(path) {
                return Err(());
            }
            let gone = |side: &[Value]| -> Vec<&Value> {
                vo.iter().filter(|x| !side.contains(x)).collect()
            };
            let removed: Vec<&Value> = gone(va).into_iter().chain(gone(vb)).collect();
            let mut out: Vec<Value> = Vec::new();
            for x in vo.iter().chain(va).chain(vb) {
                if !removed.contains(&x) && !out.contains(x) {
                    out.push(x.clone());
                }
            }
            if requirements {
                let mut names: Vec<String> = out.iter().filter_map(package).collect();
                let n = names.len();
                names.sort();
                names.dedup();
                if names.len() != n {
                    return Err(()); // one package, two requirements
                }
            }
            Ok(Some(Value::Array(out)))
        }
        _ => Err(()),
    }
}

/// `m` (the candidate's value) says what `want` (the merged value) says, with
/// set arrays compared as sets — and a set array stating an item twice says
/// something else.
fn agrees(m: &Value, want: &Value, path: &mut Vec<String>, declared: Declared) -> bool {
    match (m, want) {
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter().all(|(k, v)| {
                    path.push(k.clone());
                    let ok = y.get(k).is_some_and(|w| agrees(v, w, path, declared));
                    path.pop();
                    ok
                })
        }
        (Value::Array(x), Value::Array(y))
            if (set_path(path) && scalars(x) && scalars(y)) || declared(path) =>
        {
            let key = |v: &Vec<Value>| {
                let mut s: Vec<String> = v.iter().map(|e| e.to_string()).collect();
                s.sort();
                s
            };
            let (kx, ky) = (key(x), key(y));
            let mut dedup = kx.clone();
            dedup.dedup();
            dedup.len() == kx.len() && kx == ky
        }
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| agrees(p, q, path, declared))
        }
        _ => m == want,
    }
}

/// The certificate: `merged` loads, and its value is the three-way merge of
/// the other three. See the module docs.
pub(crate) fn certify(
    base: &str,
    ours: &str,
    theirs: &str,
    merged: &str,
    path: &str,
    declared: Declared,
) -> bool {
    let Some(f) = format(path) else {
        return false;
    };
    let (Some(o), Some(a), Some(b), Some(m)) = (
        load(base, f),
        load(ours, f),
        load(theirs, f),
        load(merged, f),
    ) else {
        return false;
    };
    if o.len() != a.len() || o.len() != b.len() || o.len() != m.len() {
        return false;
    }
    let requirements = requirements_file(f, path);
    o.iter().zip(&a).zip(&b).zip(&m).all(|(((o, a), b), m)| {
        match merge3(
            Some(o),
            Some(a),
            Some(b),
            &mut Vec::new(),
            requirements,
            declared,
        ) {
            Ok(Some(want)) => agrees(m, &want, &mut Vec::new(), declared),
            _ => false,
        }
    })
}

/// A comment on a line, outside any string.
fn has_comment(line: &str, f: Format) -> bool {
    let mut quote: Option<char> = None;
    let mut prev = ' ';
    let mut escaped = false;
    for c in line.chars() {
        match quote {
            Some(q) => {
                if escaped {
                    escaped = false;
                } else if c == '\\' && q == '"' {
                    escaped = true;
                } else if c == q {
                    quote = None;
                }
            }
            None => match c {
                '"' | '\'' => quote = Some(c),
                '#' if f != Format::Json && (f == Format::Toml || prev.is_whitespace()) => {
                    return true
                }
                '/' if f == Format::Json && (prev == '/' || prev == '*') => return true,
                '*' if f == Format::Json && prev == '/' => return true,
                _ => {}
            },
        }
        prev = c;
    }
    false
}

/// Every line `side` wrote or removed against `base`.
fn changed_lines(base: &str, side: &str) -> Vec<String> {
    let lines = |t: &str| -> Vec<String> { t.split_inclusive('\n').map(str::to_string).collect() };
    let (b, s) = (lines(base), lines(side));
    crate::determinate::ops(&b, &s)
        .into_iter()
        .flat_map(|op| {
            b[op.old]
                .iter()
                .chain(&s[op.new])
                .cloned()
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The whole-file form of the rule: when one side's value already is the
/// key-by-key merge of the two, that side's text is the answer — the other
/// side's change is carried, in value, and nothing of its text is lost that
/// a reader of the file could see, because none of the lines it wrote or
/// removed carries a comment. Both: the smaller text, so the answer does not
/// depend on which side is called ours.
///
/// This is subsumption judged by value, and it is looser than
/// [`crate::layout`] at one point, deliberately: the side not taken may have
/// written its change in a form the taken side does not share — a key moved
/// to another place in its table, an array on one line where the taken side
/// wraps it, an anchor where the taken side repeats the value. Those are the
/// same file to every consumer of it, and the taken side's form is kept. A
/// comment in any line the other side wrote or removed refuses the rule,
/// because a comment is not in the value and would be lost.
pub(crate) fn carried<'a>(
    base: &str,
    ours: &'a str,
    theirs: &'a str,
    path: &str,
    declared: Declared,
) -> Option<(&'a str, &'static str)> {
    let f = format(path)?;
    let (a, b) = (load(ours, f)?, load(theirs, f)?);
    let o = if base.trim().is_empty() {
        None
    } else {
        Some(load(base, f)?)
    };
    if a.len() != b.len() || o.as_ref().is_some_and(|o| o.len() != a.len()) {
        return None;
    }
    let requirements = requirements_file(f, path);
    let mut want = Vec::with_capacity(a.len());
    for (i, (x, y)) in a.iter().zip(&b).enumerate() {
        let base_doc = o.as_ref().map(|o| &o[i]);
        want.push(
            merge3(
                base_doc,
                Some(x),
                Some(y),
                &mut Vec::new(),
                requirements,
                declared,
            )
            .ok()??,
        );
    }
    let says_all = |side: &[Value]| {
        side.iter()
            .zip(&want)
            .all(|(v, w)| agrees(v, w, &mut Vec::new(), declared))
    };
    let quiet = |side: &str| !changed_lines(base, side).iter().any(|l| has_comment(l, f));
    let take_ours = says_all(&a) && quiet(theirs);
    let take_theirs = says_all(&b) && quiet(ours);
    match (take_ours, take_theirs) {
        (true, true) => Some((ours.min(theirs), "D1 data value carried by both")),
        (true, false) => Some((ours, "D1 data value carried by ours")),
        (false, true) => Some((theirs, "D1 data value carried by theirs")),
        _ => None,
    }
}

fn requirements_file(f: Format, path: &str) -> bool {
    f == Format::Toml
        && path
            .rsplit(['/', '\\'])
            .next()
            .is_some_and(|n| n == "pyproject.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|l| format!("{l}\n")).collect()
    }

    #[test]
    fn keys_after_one_line_union_in_canonical_order() {
        let o = v(&["a = 1"]);
        let x = v(&["a = 1", "zed = 2"]);
        let y = v(&["a = 1", "bee = 3"]);
        let u = union(&o, &x, &y, None, "t.toml").expect("union");
        assert_eq!(u, union(&o, &y, &x, None, "t.toml").expect("union"));
        assert_eq!(u, v(&["a = 1", "bee = 3", "zed = 2"]));
    }

    #[test]
    fn two_runs_meeting_at_a_containers_end_get_their_comma() {
        let o = v(&["  \"a\": 1"]);
        let x = v(&["  \"a\": 1,", "  \"c\": 3"]);
        let y = v(&["  \"a\": 1,", "  \"b\": 2"]);
        let close = "}\n".to_string();
        let u = union(&o, &x, &y, Some(&close), "p.json").expect("union");
        assert_eq!(u, v(&["  \"a\": 1,", "  \"b\": 2,", "  \"c\": 3"]));
    }

    #[test]
    fn a_url_is_not_a_comment_but_a_comment_is() {
        let o = v(&["  \"a\": 1"]);
        let x = v(&["  \"a\": 1,", "  \"url\": \"https://example.test/#top\""]);
        let y = v(&["  \"a\": 1,", "  \"home\": \"https://example.test/\""]);
        let close = "}\n".to_string();
        let u = union(&o, &x, &y, Some(&close), "p.json").expect("union");
        assert_eq!(
            u,
            v(&[
                "  \"a\": 1,",
                "  \"home\": \"https://example.test/\",",
                "  \"url\": \"https://example.test/#top\""
            ])
        );
        // The comma would land inside the comment.
        let x = v(&["a = [", "  \"x\",", "  \"b\" # note"]);
        let y = v(&["a = [", "  \"x\",", "  \"z\""]);
        let o = v(&["a = [", "  \"x\""]);
        let close = "]\n".to_string();
        assert!(union(&o, &x, &y, Some(&close), "t.toml").is_none());
    }

    #[test]
    fn an_edited_base_line_is_refused() {
        let o = v(&["a = 1"]);
        let x = v(&["a = 2", "b = 2"]);
        let y = v(&["a = 1", "c = 3"]);
        assert!(union(&o, &x, &y, None, "t.toml").is_none());
    }

    #[test]
    fn the_certificate_refuses_one_key_with_two_values() {
        let base = "[t]\na = 1\n";
        assert!(certify(
            base,
            "[t]\na = 1\nb = 2\n",
            "[t]\na = 1\nc = 3\n",
            "[t]\na = 1\nb = 2\nc = 3\n",
            "x.toml",
            &undeclared
        ));
        assert!(!certify(
            base,
            "[t]\na = 1\nb = 2\n",
            "[t]\na = 1\nb = 3\n",
            "[t]\na = 1\nb = 2\nb = 3\n",
            "x.toml",
            &undeclared
        ));
    }

    #[test]
    fn the_certificate_reads_requirements_by_package() {
        let base = "[project]\ndependencies = []\n";
        let ours = "[project]\ndependencies = [\"pyyaml>=6\"]\n";
        let theirs = "[project]\ndependencies = [\"PyYAML>=6\"]\n";
        let merged = "[project]\ndependencies = [\"pyyaml>=6\", \"PyYAML>=6\"]\n";
        assert!(!certify(
            base,
            ours,
            theirs,
            merged,
            "pyproject.toml",
            &undeclared
        ));
        assert!(certify(
            base,
            ours,
            theirs,
            merged,
            "other.toml",
            &undeclared
        ));
    }

    #[test]
    fn a_sequence_is_a_set_only_by_declaration() {
        let base = "- id: a\n";
        let ours = "- id: a\n- id: b\n";
        let theirs = "- id: a\n- id: c\n";
        let merged = "- id: a\n- id: b\n- id: c\n";
        assert!(!certify(base, ours, theirs, merged, "h.yaml", &undeclared));
        assert!(certify(base, ours, theirs, merged, "h.yaml", &|_| true));
        // declared or not, an item is stated once
        let twice = "- id: a\n- id: b\n- id: b\n";
        assert!(!certify(base, ours, ours, twice, "h.yaml", &|_| true));
    }

    #[test]
    fn yaml_mappings_compare_without_order() {
        assert!(certify(
            "a: 1\n",
            "a: 1\nb: 2\n",
            "a: 1\nc: 3\n",
            "a: 1\nc: 3\nb: 2\n",
            "x.yaml",
            &undeclared
        ));
    }
}
