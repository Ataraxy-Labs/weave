//! The conflicts whose answer does not depend on anybody's intent.
//!
//! A three-way merge is correct for every intent exactly where the two edits
//! commute — where applying them in either order gives the same file. Some
//! conflicts git and the entity model both refuse are of that kind, and each
//! rule here names one, with the guards that keep it from firing where the
//! edits do not commute:
//!
//! - **D0 agreement** — the two sides wrote the same lines.
//! - **D1 subsumption** — one side's change to a region contains the
//!   other's: every base line the smaller edit removed, the larger removed too;
//!   every line it wrote, the larger wrote in the same place. The larger side's
//!   text is the answer. Guards: blank lines are layout, not edits; lines are
//!   aligned globally, not by search; a line the smaller edit deleted that the
//!   larger side still states as often as base did was moved, not deleted; a
//!   deletion is carried only by a deletion (a region one side emptied and the
//!   other rewrote is modify/delete, not subsumption); never into a side whose
//!   file does not parse when the other side's does.
//! - **D3 set union** — a region that is nothing but items of an unordered
//!   collection (import lines, the lines of an ignore file or a requirements
//!   file) merges as the union of the two sides' additions minus the union of
//!   their removals. Guards: every line of the region in every version is such
//!   an item; no side states one item twice; an item changed differently by
//!   both sides is a conflict, except an import's list of names from one
//!   module, which merges as a set (so replacing an import is a replacement,
//!   not a delete and an add); no local name is bound twice by two different
//!   import lines, inside the region or out. An import one side moved to a new
//!   module path while the other deleted it stays deleted unless some code of
//!   the answer uses a name it binds.
//! - **D3 changelog union** — in a file that is a changelog by name, whose
//!   `weave-set` gitattribute declares the section a set (bare, or naming the
//!   section's heading), both sides inserted whole unordered-list entries at
//!   one point inside one section, and nothing else there: the union, in a
//!   canonical order. Undeclared, a conflict: two differently worded entries
//!   can describe one change, and the text cannot tell. The argument and the
//!   guards are in [`crate::changelog`].
//! - **D3 data key union** — in a JSON, TOML or YAML file, both sides inserted
//!   distinct keys (or dependency-array items) after the same base line, and
//!   kept every base line there: the union, each side's lines verbatim where
//!   that side put them, accepted only if the file's value is then the
//!   key-by-key three-way merge of the three values. See
//!   [`crate::dataunion`].
//!   A data file whose one side already has, as a value, the key-by-key
//!   merge of the two — the other side's change is carried in value, and
//!   nothing it wrote was a comment — is that side's text ("D1 data value
//!   carried").
//! - **D3 container insertion union** — in a Python, JS/TS or INI file, both
//!   sides inserted whole new elements at one point of one container and
//!   kept every base line there: the union, merged by key. By default only a
//!   container that is a set by the language (a dict or object literal with
//!   constant keys, `__all__`, an import or export name list, an INI
//!   section's keys); a list, statement sequence, match arms or INI
//!   multi-line value only where the file's `weave-set` gitattribute
//!   declares it a set. Certified on the whole answer. See
//!   [`crate::insertion`].
//! - **D4 layout-equal creations** — both sides created the file and the two
//!   differ only in layout: either is the answer (the smaller, so the choice
//!   does not depend on which side is called ours; likewise for two edits
//!   that differ only in layout — unless one side's edits, blank lines
//!   included, carry the other's: then the other made no layout edit the
//!   first did not, and the first is the answer, D1).
//! - **D6 layout-only side** — one side's change is layout only (see
//!   [`crate::layout`]; comments are content): the answer is the other side's.
//!   Asked of the whole file first, then of each region, where a region is
//!   judged in its context — the file with that side's region put back to
//!   base must be layout-equal to the file with its own.
//!
//! These run only after the merge has already refused, and only on the
//! regions git's line merge left in conflict. A region no rule settles leaves
//! the whole file conflicted. What comes out is checked by the same gate as any
//! composition (`crate::verify`), and refused if one side deleted a
//! declaration the other changed: line rules do not get to decide
//! modify/delete.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::ops::Range;

use sem_core::parser::registry::ParserRegistry;

use crate::host::{Host, LineMergeStyle, SetScope};
use crate::layout::layout_equal;

/// A settled file: its text and the rules that settled it, in order.
pub(crate) struct Settled {
    pub content: String,
    pub rules: Vec<&'static str>,
}

/// One region of the line merge.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Seg {
    Stable(Vec<String>),
    Hunk {
        ours: Vec<String>,
        base: Vec<String>,
        theirs: Vec<String>,
    },
}

/// Settle a conflicted file by the rules above, or `None`.
pub(crate) fn settle(
    base: &str,
    ours: &str,
    theirs: &str,
    path: &str,
    registry: &ParserRegistry,
    host: &Host,
) -> Option<Settled> {
    let one = |content: &str, rule: &'static str| {
        Some(Settled {
            content: content.to_string(),
            rules: vec![rule],
        })
    };
    // D4: both created it, differing only in layout. Either is the answer;
    // the smaller, so it is the same whichever side is called ours.
    if base.is_empty() && layout_equal(ours, theirs, path) {
        return one(ours.min(theirs), "D4 layout-equal creations");
    }
    // D6 over the whole file. Both sides layout-only is left alone: the
    // answer would be whichever side is called ours.
    if !base.is_empty() {
        match (
            layout_equal(base, ours, path),
            layout_equal(base, theirs, path),
        ) {
            (true, false) => return one(theirs, "D6 ours is layout only"),
            (false, true) => return one(ours, "D6 theirs is layout only"),
            _ => {}
        }
    }
    // The two sides say the same thing in different layouts. If one side's
    // edits, blank lines and all, carry the other's, the other made no layout
    // edit the first did not, and the first is the answer: a one-sided layout
    // edit is carried like any other. Otherwise either is the answer; the
    // smaller, so it is the same whichever side is called ours.
    if !base.is_empty() && layout_equal(ours, theirs, path) {
        return match crate::subsumption::exact_carrier(base, ours, theirs) {
            Some(crate::subsumption::Superset::Ours) => one(ours, "D1 ours carries theirs"),
            Some(crate::subsumption::Superset::Theirs) => one(theirs, "D1 theirs carries ours"),
            None => one(ours.min(theirs), "D6 the sides differ only in layout"),
        };
    }

    // The file's `weave-set` declaration, read once and only if a rule asks
    // (an order-sensitive container stands between a region and a union).
    let scope_cell: std::cell::OnceCell<SetScope> = std::cell::OnceCell::new();
    let scope = || {
        scope_cell
            .get_or_init(|| SetScope::read(host, path))
            .clone()
    };
    let declared = |key_path: &[String]| match scope() {
        SetScope::Off => false,
        SetScope::All => true,
        SetScope::Named(names) => key_path.iter().any(|k| names.contains(k)),
    };

    // A data file one side of which already says, as a value, everything the
    // merge of the two says.
    if crate::dataunion::applies(path) {
        if let Some((text, rule)) = crate::dataunion::carried(base, ours, theirs, path, &declared) {
            return one(text, rule);
        }
    }

    // Region by region. The marker writers join an unterminated last line to
    // the marker, so a region read back from that is not the side's text:
    // every version is read with its last line ended, and whether the answer
    // ends its own is merged on its own — it is one bit, and a side that
    // changed it wins. Two creations that disagree about it are left alone.
    let open = |t: &str| !t.is_empty() && !t.ends_with('\n');
    let (o_open, a_open, b_open) = (open(base), open(ours), open(theirs));
    let unterminated = if base.is_empty() {
        if a_open != b_open && !ours.is_empty() && !theirs.is_empty() {
            return None;
        }
        a_open || b_open
    } else if a_open != o_open {
        a_open
    } else {
        b_open
    };
    let close = |t: &str| {
        if open(t) {
            format!("{t}\n")
        } else {
            t.to_string()
        }
    };
    let (base, ours, theirs) = (&close(base), &close(ours), &close(theirs));
    let reopen = |mut s: Settled| {
        if unterminated && s.content.ends_with('\n') {
            s.content.pop();
        }
        s
    };
    let segs = segments(base, ours, theirs, host)?;
    let hunks = segs
        .iter()
        .filter(|s| matches!(s, Seg::Hunk { .. }))
        .count();
    if hunks == 0 {
        return None;
    }
    let stable: HashSet<String> = segs
        .iter()
        .filter_map(|s| match s {
            Seg::Stable(l) => Some(l),
            _ => None,
        })
        .flatten()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    let parses = |text: &str| -> Option<bool> {
        let (_, tree) = registry.extract_entities_with_tree(path, text)?;
        tree.map(|t| !t.root_node().has_error())
    };
    let (ours_parses, theirs_parses) = (parses(ours), parses(theirs));
    // A side whose file does not parse, where the other side's does, is never
    // the answer for a region.
    let may_take_ours = !(ours_parses == Some(false) && theirs_parses == Some(true));
    let may_take_theirs = !(theirs_parses == Some(false) && ours_parses == Some(true));

    // The changelog and data unions read their region's neighbourhood from
    // the text both sides agree on: the stable lines above it, and the first
    // non-blank stable line below it (none if another region comes first).
    // A changelog section is united only where the file's `weave-set`
    // declaration names it (or the whole file): two differently worded
    // entries can describe one change, and that is not a set's to decide.
    let changelog = !base.is_empty()
        && crate::changelog::is_changelog(path)
        && !matches!(scope(), SetScope::Off);
    let data = !base.is_empty() && crate::dataunion::applies(path);
    let containers = !base.is_empty() && crate::insertion::applies(path);
    // One side's whole text, every region as that side wrote it, and the
    // line region `i` begins at in it.
    let side_text = |i: usize, take_ours: bool| -> (String, usize) {
        let (mut text, mut n, mut at) = (String::new(), 0, 0);
        for (j, s) in segs.iter().enumerate() {
            let lines = match s {
                Seg::Stable(l) => l,
                Seg::Hunk { ours, theirs, .. } => {
                    if take_ours {
                        ours
                    } else {
                        theirs
                    }
                }
            };
            if j == i {
                at = n;
            }
            n += lines.len();
            text.extend(lines.iter().map(String::as_str));
        }
        (text, at)
    };
    let mut claims: Vec<(usize, Vec<crate::insertion::Claim>)> = Vec::new();
    let mut dead: Vec<(String, Vec<String>)> = Vec::new();
    let above = |i: usize| -> Vec<&String> {
        segs[..i]
            .iter()
            .filter_map(|s| match s {
                Seg::Stable(l) => Some(l),
                _ => None,
            })
            .flatten()
            .collect()
    };
    let below = |i: usize| -> Option<&String> {
        for s in &segs[i + 1..] {
            match s {
                Seg::Stable(l) => {
                    if let Some(l) = l.iter().find(|l| !blank(l)) {
                        return Some(l);
                    }
                }
                Seg::Hunk { .. } => return None,
            }
        }
        None
    };
    let mut picks: Vec<Vec<String>> = Vec::new();
    let mut rules: Vec<&'static str> = Vec::new();
    for (i, s) in segs.iter().enumerate() {
        let Seg::Hunk {
            ours: a,
            base: o,
            theirs: b,
        } = s
        else {
            continue;
        };
        let (a_carries, b_carries) = (
            may_take_ours && carried(o, a, b),
            may_take_theirs && carried(o, b, a),
        );
        let (pick, rule) = if a == b {
            (a.clone(), "D0 agreement")
        } else if a_carries && b_carries {
            // The same edit laid out twice: the answer must not depend on
            // which side is called ours.
            (std::cmp::min(a, b).clone(), "D1 both carry the same edit")
        } else if a_carries {
            (a.clone(), "D1 ours carries theirs")
        } else if b_carries {
            (b.clone(), "D1 theirs carries ours")
        } else if let Some(u) = set_union(o, a, b, path, &stable, &mut dead) {
            (u, "D3 set union")
        } else if let Some(u) = changelog
            .then(|| {
                let above = above(i);
                declared(&crate::changelog::sections(&above))
                    .then(|| crate::changelog::union(o, a, b, &above, below(i)))
                    .flatten()
            })
            .flatten()
        {
            (u, "D3 changelog union")
        } else if let Some(u) = data
            .then(|| crate::dataunion::union(o, a, b, below(i), path))
            .flatten()
        {
            (u, "D3 data key union")
        } else if let Some((u, c)) = containers
            .then(|| {
                let ((ta, at_a), (tb, at_b)) = (side_text(i, true), side_text(i, false));
                crate::insertion::union(
                    o,
                    a,
                    b,
                    crate::insertion::SideText {
                        text: &ta,
                        at: at_a,
                    },
                    crate::insertion::SideText {
                        text: &tb,
                        at: at_b,
                    },
                    path,
                    registry,
                    &scope,
                )
            })
            .flatten()
        {
            claims.push((picks.len(), c));
            (u, crate::insertion::RULE)
        } else {
            // The file with this region as `this` and every other as ours'.
            let in_context = |this: &[String]| {
                let mut with = String::new();
                for (j, t) in segs.iter().enumerate() {
                    match t {
                        Seg::Stable(l) => with.extend(l.iter().map(String::as_str)),
                        Seg::Hunk { ours, .. } if j != i => {
                            with.extend(ours.iter().map(String::as_str))
                        }
                        Seg::Hunk { .. } => with.extend(this.iter().map(String::as_str)),
                    }
                }
                with
            };
            let (base_ctx, ours_ctx, theirs_ctx) = (in_context(o), in_context(a), in_context(b));
            let ours_only_layout = layout_equal(&base_ctx, &ours_ctx, path);
            let theirs_only_layout = layout_equal(&base_ctx, &theirs_ctx, path);
            match (ours_only_layout, theirs_only_layout) {
                (true, false) if may_take_theirs => (b.clone(), "D6 ours is layout only here"),
                (false, true) if may_take_ours => (a.clone(), "D6 theirs is layout only here"),
                _ if layout_equal(&ours_ctx, &theirs_ctx, path) => {
                    let text = |v: &[String]| v.concat();
                    match crate::subsumption::exact_carrier(&text(o), &text(a), &text(b)) {
                        Some(crate::subsumption::Superset::Ours) if may_take_ours => {
                            (a.clone(), "D1 ours carries theirs")
                        }
                        Some(crate::subsumption::Superset::Theirs) if may_take_theirs => {
                            (b.clone(), "D1 theirs carries ours")
                        }
                        _ => (
                            std::cmp::min(a, b).clone(),
                            "D6 the sides differ only in layout here",
                        ),
                    }
                }
                _ => return None,
            }
        };
        picks.push(pick);
        rules.push(rule);
    }
    // An import the set union kept against the other side's deletion stays
    // only if some code of the answer uses a name it binds.
    if !dead.is_empty() {
        let ext = extension(path);
        let code: HashSet<String> = segs
            .iter()
            .filter_map(|s| match s {
                Seg::Stable(l) => Some(l),
                _ => None,
            })
            .chain(picks.iter())
            .flatten()
            .filter(|l| !is_import(l.trim(), &ext))
            .flat_map(|l| crate::binding::identifiers(l))
            .map(str::to_string)
            .collect();
        for (line, names) in dead {
            if names.iter().any(|n| code.contains(n)) {
                continue;
            }
            for (pick, rule) in picks.iter_mut().zip(&rules) {
                if *rule != "D3 set union" {
                    continue;
                }
                if let Some(at) = pick.iter().position(|l| *l == line) {
                    pick.remove(at);
                    break;
                }
            }
        }
    }
    let mut out = String::new();
    let mut next = picks.into_iter();
    // The line each region's answer begins at in `out`.
    let (mut pick_at, mut n) = (Vec::new(), 0);
    for s in &segs {
        match s {
            Seg::Stable(l) => {
                n += l.len();
                out.extend(l.iter().map(String::as_str));
            }
            Seg::Hunk { .. } => {
                let p = next.next()?;
                pick_at.push(n);
                n += p.len();
                out.extend(p.iter().map(String::as_str));
            }
        }
    }
    // The data union's text is a candidate until its value is the three-way
    // merge of the three values.
    if rules.contains(&"D3 data key union")
        && !crate::dataunion::certify(base, ours, theirs, &out, path, &declared)
    {
        return None;
    }
    // The container union's text is a candidate until the whole answer holds
    // each container it claimed, with exactly the keys inserted there.
    if !claims.is_empty() {
        let mut all = Vec::new();
        for (k, cs) in claims {
            for mut c in cs {
                c.rows = c.rows.start + pick_at[k]..c.rows.end + pick_at[k];
                all.push(c);
            }
        }
        if !crate::insertion::certify(base, &out, &all, path, registry) {
            return None;
        }
    }
    rules.sort_unstable();
    rules.dedup();
    Some(reopen(Settled {
        content: out,
        rules,
    }))
}

/// A declaration one side deleted and the other changed. Line rules cannot
/// see that a region is one declaration, so the answer is refused whenever the
/// file has one.
pub(crate) fn has_modify_delete(
    base: &str,
    ours: &str,
    theirs: &str,
    path: &str,
    registry: &ParserRegistry,
) -> bool {
    type Key = (String, String);
    let read = |text: &str| -> HashMap<Key, Vec<String>> {
        let mut out: HashMap<Key, Vec<String>> = HashMap::new();
        let entities = registry
            .extract_entities_with_tree(path, text)
            .map(|(e, _)| e)
            .unwrap_or_default();
        // A fallback `chunk` is a slice of lines named by position.
        for e in entities
            .into_iter()
            .filter(|e| e.parent_id.is_none() && e.entity_type != "chunk")
        {
            out.entry((e.entity_type, e.name))
                .or_default()
                .push(e.content);
        }
        out
    };
    let (b, o, t) = (read(base), read(ours), read(theirs));
    let empty = Vec::new();
    b.iter().any(|(k, before)| {
        let (in_o, in_t) = (o.get(k).unwrap_or(&empty), t.get(k).unwrap_or(&empty));
        // Changed in more than layout: a re-indented or re-wrapped
        // declaration deleted by the other side loses nothing.
        let changed = |side: &Vec<String>| {
            side.iter().any(|c| {
                !before
                    .iter()
                    .any(|b| b == c || crate::layout::tokens_equal(b, c, path))
            })
        };
        (in_o.len() < before.len() && changed(in_t)) || (in_t.len() < before.len() && changed(in_o))
    })
}

// ---------------------------------------------------------------------------
// The regions
// ---------------------------------------------------------------------------

/// git's line merge (the granted one, else the in-process one) in diff3
/// form, read back into stable runs and conflicted regions.
fn segments(base: &str, ours: &str, theirs: &str, host: &Host) -> Option<Vec<Seg>> {
    let text = match host
        .line_merge
        .and_then(|m| m(LineMergeStyle::Labelled, base, ours, theirs))
    {
        Some(m) => m.content,
        None => match diffy::MergeOptions::new()
            .set_conflict_style(diffy::ConflictStyle::Diff3)
            .merge(base, ours, theirs)
        {
            Ok(clean) => clean,
            Err(conflicted) => conflicted,
        },
    };
    let marker = |l: &str, c: char| {
        let t = l.trim_end_matches(['\n', '\r']);
        t.len() >= 7
            && t.chars().take(7).all(|x| x == c)
            && (t.len() == 7 || t[7..].starts_with(' '))
    };
    #[derive(PartialEq)]
    enum At {
        Stable,
        Ours,
        Base,
        Theirs,
    }
    let mut at = At::Stable;
    let mut segs = Vec::new();
    let (mut s, mut a, mut o, mut b) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for line in text.split_inclusive('\n') {
        match at {
            At::Stable if marker(line, '<') => {
                if !s.is_empty() {
                    segs.push(Seg::Stable(std::mem::take(&mut s)));
                }
                at = At::Ours;
            }
            At::Stable => s.push(line.to_string()),
            At::Ours if marker(line, '|') => at = At::Base,
            At::Ours => a.push(line.to_string()),
            At::Base if marker(line, '=') => at = At::Theirs,
            At::Base => o.push(line.to_string()),
            At::Theirs if marker(line, '>') => {
                segs.push(Seg::Hunk {
                    ours: std::mem::take(&mut a),
                    base: std::mem::take(&mut o),
                    theirs: std::mem::take(&mut b),
                });
                at = At::Stable;
            }
            At::Theirs => b.push(line.to_string()),
        }
    }
    if at != At::Stable {
        return None;
    }
    if !s.is_empty() {
        segs.push(Seg::Stable(s));
    }
    Some(segs)
}

// ---------------------------------------------------------------------------
// D1
// ---------------------------------------------------------------------------

fn blank(l: &str) -> bool {
    l.trim().is_empty()
}

/// One unit of change of `b` against `a`, with zero context.
pub(crate) struct Op {
    pub old: Range<usize>,
    pub new: Range<usize>,
}

/// The changes that turn `a` into `b`, line by line (trailing space is not
/// part of a line).
pub(crate) fn ops(a: &[String], b: &[String]) -> Vec<Op> {
    let join = |v: &[String]| {
        let mut s = String::new();
        for l in v {
            s.push_str(l.trim_end());
            s.push('\n');
        }
        s
    };
    let (ja, jb) = (join(a), join(b));
    let patch = diffy::DiffOptions::new()
        .set_context_len(0)
        .create_patch(&ja, &jb);
    // diffy numbers a non-empty range from 1, and an empty one by the line
    // it follows.
    let zero = |r: diffy::HunkRange| {
        let s = if !r.is_empty() {
            r.start() - 1
        } else {
            r.start()
        };
        s..s + r.len()
    };
    patch
        .hunks()
        .iter()
        .map(|h| Op {
            old: zero(h.old_range()),
            new: zero(h.new_range()),
        })
        .collect()
}

/// `a` index → `b` index for every line the alignment keeps.
pub(crate) fn image(a: &[String], b: &[String]) -> HashMap<usize, usize> {
    let mut map = HashMap::new();
    let (mut i, mut j) = (0, 0);
    for op in ops(a, b) {
        while i < op.old.start {
            map.insert(i, j);
            i += 1;
            j += 1;
        }
        i = op.old.end;
        j = op.new.end;
    }
    while i < a.len() {
        map.insert(i, j);
        i += 1;
        j += 1;
    }
    map
}

fn count_trimmed(v: &[String]) -> HashMap<&str, usize> {
    let mut m: HashMap<&str, usize> = HashMap::new();
    for l in v {
        *m.entry(l.trim()).or_insert(0) += 1;
    }
    m
}

/// `x`'s region carries `y`'s change of base region `o`. See the module docs.
fn carried(o: &[String], x: &[String], y: &[String]) -> bool {
    let img = image(o, x);
    let y_in_x: HashSet<usize> = image(y, x).into_keys().collect();
    let (co, cx, cy) = (count_trimmed(o), count_trimmed(x), count_trimmed(y));
    let changes = ops(o, y);
    if changes.is_empty() {
        return false;
    }
    for op in changes {
        // Every base line y removed, x removed too.
        if op
            .old
            .clone()
            .any(|k| img.contains_key(&k) && !blank(&o[k]))
        {
            return false;
        }
        let written: HashSet<&str> = y[op.new.clone()].iter().map(|l| l.trim()).collect();
        // …and did not merely move it (y deleted it, x still states it).
        if op.old.clone().any(|k| {
            let l = o[k].trim();
            l.len() > 3
                && !written.contains(l)
                && cy.get(l).copied().unwrap_or(0) < co[l]
                && cx.get(l).copied().unwrap_or(0) >= co[l]
        }) {
            return false;
        }
        let ins: Vec<&str> = y[op.new.clone()]
            .iter()
            .map(|l| l.trim_end())
            .filter(|l| !blank(l))
            .collect();
        // Where y's change sits in x: between the images of its base anchors.
        let lo = img
            .iter()
            .filter(|(k, _)| **k < op.old.start)
            .map(|(_, v)| v + 1)
            .max()
            .unwrap_or(0);
        let hi = img
            .iter()
            .filter(|(k, _)| **k >= op.old.end)
            .map(|(_, v)| *v)
            .min()
            .unwrap_or(x.len());
        let window: Vec<&str> = x[lo.min(hi)..hi]
            .iter()
            .map(|l| l.trim_end())
            .filter(|l| !blank(l))
            .collect();
        if ins.is_empty() {
            // A deletion is carried only by a deletion.
            if op.old.clone().any(|k| !blank(&o[k])) && !window.is_empty() {
                return false;
            }
            continue;
        }
        // Every line y wrote survives into x under the global alignment…
        if op
            .new
            .clone()
            .any(|j| !blank(&y[j]) && !y_in_x.contains(&j))
        {
            return false;
        }
        // …contiguously, between the images of its anchors.
        if !window.windows(ins.len()).any(|w| w == ins.as_slice()) {
            return false;
        }
    }
    true
}

// ---------------------------------------------------------------------------
// D3
// ---------------------------------------------------------------------------

/// What an item line is, for the collection it belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Key {
    /// A line of a line-set file, or an import with no name list: the line.
    Line(String),
    /// A requirements entry: the package, whatever its version.
    Package(String),
    /// An import of a list of names from one module: the line around the list.
    Names(String, String),
}

#[derive(Debug, Clone)]
struct Item {
    key: Key,
    line: String,
    /// The names of a `Names` import.
    names: Vec<String>,
}

fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn extension(path: &str) -> String {
    file_name(path)
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default()
}

/// Files whose lines are an unordered set.
fn line_set_file(path: &str) -> bool {
    let n = file_name(path);
    matches!(
        n,
        ".gitignore"
            | ".dockerignore"
            | ".prettierignore"
            | ".eslintignore"
            | ".npmignore"
            | ".gcloudignore"
            | ".vercelignore"
            | "go.sum"
    ) || ((n.starts_with("requirements") || n.starts_with("constraints")) && n.ends_with(".txt"))
}

/// The item a line is, or `None` when it is not one (blank lines are handled
/// by the caller).
fn item(line: &str, path: &str) -> Option<Item> {
    let t = line.trim();
    let plain = |key: Key| Item {
        key,
        line: line.to_string(),
        names: Vec::new(),
    };
    if line_set_file(path) {
        let n = file_name(path);
        if (n.starts_with("requirements") || n.starts_with("constraints")) && !t.starts_with('#') {
            let name: String = t
                .chars()
                .take_while(|c| {
                    c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-' | '[' | ']')
                })
                .collect();
            if !name.is_empty() {
                return Some(plain(Key::Package(
                    name.to_ascii_lowercase().replace('_', "-"),
                )));
            }
        }
        return Some(plain(Key::Line(t.to_string())));
    }
    if !is_import(t, &extension(path)) {
        return None;
    }
    if let Some((pre, names, suf)) = name_list(t) {
        return Some(Item {
            key: Key::Names(pre, suf),
            line: line.to_string(),
            names,
        });
    }
    Some(plain(Key::Line(t.to_string())))
}

/// Is `t` a single-line import statement in the language of `ext`?
fn is_import(t: &str, ext: &str) -> bool {
    let t = t.split("//").next().unwrap_or(t).trim();
    let ident_path = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '*' | ',' | ' ' | '='))
    };
    match ext {
        "py" | "pyi" => {
            let t = t.split('#').next().unwrap_or(t).trim();
            if let Some(rest) = t.strip_prefix("import ") {
                return ident_path(rest);
            }
            if let Some(rest) = t.strip_prefix("from ") {
                return rest
                    .split_once(" import ")
                    .is_some_and(|(m, n)| ident_path(m) && !n.contains(['(', ')', '\\']));
            }
            false
        }
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "mts" | "cts" => {
            let quoted_end = |s: &str| {
                let s = s.trim_end_matches(';').trim_end();
                s.ends_with('\'') || s.ends_with('"')
            };
            (t.starts_with("import ")
                && (t.contains(" from ") || t[7..].trim_start().starts_with(['\'', '"']))
                && quoted_end(t))
                || ((t.starts_with("const ") || t.starts_with("let ") || t.starts_with("var "))
                    && t.contains("require(")
                    && t.trim_end_matches(';').ends_with(')'))
        }
        "go" => {
            let s = t.strip_prefix("import ").unwrap_or(t);
            let s = s.split_whitespace().last().unwrap_or("");
            s.len() >= 2 && s.starts_with('"') && s.ends_with('"')
        }
        "rs" => {
            let s = t.strip_prefix("pub ").unwrap_or(t);
            let s = s
                .strip_prefix("pub(crate) ")
                .or_else(|| s.strip_prefix("pub(super) "))
                .unwrap_or(s);
            (s.starts_with("use ") || s.starts_with("mod ") || s.starts_with("extern crate "))
                && s.ends_with(';')
                && (!s.contains('{') || s.contains('}'))
        }
        "c" | "h" | "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" | "cu" | "m" | "mm" => {
            let s = t.strip_prefix('#').map(str::trim_start).unwrap_or("");
            (s.starts_with("include") || s.starts_with("import"))
                && (s.ends_with('>') || s.ends_with('"'))
        }
        "cs" => {
            let s = t.strip_prefix("global ").unwrap_or(t);
            s.starts_with("using ") && s.ends_with(';') && ident_path(&s[6..s.len() - 1])
        }
        "java" | "kt" | "kts" | "scala" | "groovy" => {
            let s = t.trim_end_matches(';');
            s.strip_prefix("import ").is_some_and(|r| {
                let r = r.strip_prefix("static ").unwrap_or(r);
                !r.is_empty()
                    && r.chars()
                        .all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '*'))
            })
        }
        "swift" => t
            .strip_prefix("@testable ")
            .unwrap_or(t)
            .strip_prefix("import ")
            .is_some_and(|r| r.chars().all(|c| c.is_alphanumeric() || c == '_')),
        "dart" => {
            (t.starts_with("import ") || t.starts_with("export ") || t.starts_with("part "))
                && t.ends_with(';')
        }
        "php" => {
            (t.starts_with("use ") || t.starts_with("require") || t.starts_with("include"))
                && t.ends_with(';')
        }
        _ => false,
    }
}

/// `import {a, b} from 'm'` / `from m import a, b` / `use m::{a, b};` split
/// into the text before the list, the names, and the text after it.
fn name_list(t: &str) -> Option<(String, Vec<String>, String)> {
    let split = |inner: &str| -> Vec<String> {
        inner
            .split(',')
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
            .collect()
    };
    if (t.starts_with("import ") && t.contains(" from "))
        || t.starts_with("use ")
        || t.starts_with("pub use ")
    {
        let (o, c) = (t.find('{')?, t.rfind('}')?);
        if o >= c || t[o + 1..c].contains(['{', '}']) {
            return None;
        }
        return Some((t[..=o].to_string(), split(&t[o + 1..c]), t[c..].to_string()));
    }
    if let Some(rest) = t.strip_prefix("from ") {
        let (module, names) = rest.split_once(" import ")?;
        let names = names.split('#').next().unwrap_or(names).trim();
        if names.contains(['(', ')', '\\', '*']) {
            return None;
        }
        return Some((
            format!("from {module} import "),
            split(names),
            String::new(),
        ));
    }
    None
}

/// The local names an import line binds, as far as the line says.
fn bindings(line: &str, ext: &str) -> Vec<String> {
    let t = line.trim().trim_end_matches(';').trim();
    match ext {
        "java" | "kt" | "kts" | "scala" | "groovy" => {
            let r = t.strip_prefix("import ").unwrap_or("");
            let r = r.strip_prefix("static ").unwrap_or(r);
            match r.rsplit('.').next() {
                Some(n) if n != "*" && !n.is_empty() => vec![n.to_string()],
                _ => Vec::new(),
            }
        }
        "rs" => {
            let r = t.split_once("use ").map(|(_, r)| r).unwrap_or("");
            if r.contains('{') {
                return Vec::new();
            }
            let local = r.rsplit_once(" as ").map(|(_, a)| a).unwrap_or(r);
            match local.rsplit("::").next() {
                Some(n) if n != "*" && n != "self" && !n.is_empty() => vec![n.trim().to_string()],
                _ => Vec::new(),
            }
        }
        "go" => {
            let s = t.strip_prefix("import ").unwrap_or(t);
            let parts: Vec<&str> = s.split_whitespace().collect();
            match parts.as_slice() {
                [alias, _] if *alias != "_" && *alias != "." => vec![alias.to_string()],
                [path] => path
                    .trim_matches('"')
                    .rsplit('/')
                    .next()
                    .map(|n| vec![n.to_string()])
                    .unwrap_or_default(),
                _ => Vec::new(),
            }
        }
        "cs" => t
            .split_once('=')
            .map(|(a, _)| {
                let a = a.trim();
                vec![a.rsplit(' ').next().unwrap_or(a).to_string()]
            })
            .unwrap_or_default(),
        "py" | "pyi" | "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "mts" | "cts" => {
            crate::binding::import_bindings(line)
        }
        _ => Vec::new(),
    }
}

/// D3 over one region. `stable` holds the trimmed lines outside every region.
/// An added import that replaces a base import the other side deleted is kept
/// in the answer and reported in `dead_unless_used` with the names it binds.
fn set_union(
    o: &[String],
    a: &[String],
    b: &[String],
    path: &str,
    stable: &HashSet<String>,
    dead_unless_used: &mut Vec<(String, Vec<String>)>,
) -> Option<Vec<String>> {
    let read = |v: &[String]| -> Option<Vec<Option<Item>>> {
        v.iter()
            .map(|l| {
                if blank(l) {
                    Some(None)
                } else {
                    item(l, path).map(Some)
                }
            })
            .collect()
    };
    let (io, ia, ib) = (read(o)?, read(a)?, read(b)?);
    let by_key = |items: &[Option<Item>]| -> Option<BTreeMap<Key, Item>> {
        let mut m = BTreeMap::new();
        for it in items.iter().flatten() {
            if m.insert(it.key.clone(), it.clone()).is_some() {
                return None; // a side states one item twice
            }
        }
        Some(m)
    };
    let (mo, ma, mb) = (by_key(&io)?, by_key(&ia)?, by_key(&ib)?);
    if ma.is_empty() && mb.is_empty() {
        return None;
    }
    let value = |it: Option<&Item>| it.map(|i| (i.line.trim().to_string(), i.names.clone()));
    let mut merged: BTreeMap<Key, Option<Item>> = BTreeMap::new();
    let keys: Vec<Key> = mo
        .keys()
        .chain(ma.keys())
        .chain(mb.keys())
        .cloned()
        .collect();
    for k in keys {
        if merged.contains_key(&k) {
            continue;
        }
        let (vo, va, vb) = (mo.get(&k), ma.get(&k), mb.get(&k));
        let pick = if value(va) == value(vb) {
            va.cloned()
        } else if value(va) == value(vo) {
            vb.cloned()
        } else if value(vb) == value(vo) {
            va.cloned()
        } else if let (Key::Names(pre, suf), Some(x), Some(y)) = (&k, va, vb) {
            // Both edited one import's list of names: merge the names as a set.
            let before: Vec<String> = vo.map(|i| i.names.clone()).unwrap_or_default();
            let dropped = |side: &Item| -> Vec<String> {
                before
                    .iter()
                    .filter(|n| !side.names.contains(n))
                    .cloned()
                    .collect()
            };
            let gone: HashSet<String> = dropped(x).into_iter().chain(dropped(y)).collect();
            // Base's names in base's order, then the added ones by name: the
            // same list whichever side is called ours.
            let mut names: Vec<String> = before
                .iter()
                .filter(|n| !gone.contains(*n))
                .cloned()
                .collect();
            let mut added: Vec<String> = x
                .names
                .iter()
                .chain(y.names.iter())
                .filter(|n| !before.contains(n))
                .cloned()
                .collect();
            added.sort();
            added.dedup();
            names.extend(added);
            if names.is_empty() {
                None
            } else {
                // Base's spelling — its spacing inside the braces, its
                // indentation — or, with no base line, the smaller of the two.
                let model = vo.unwrap_or(if x.line <= y.line { x } else { y });
                let spaced = model.line.contains("{ ");
                let list = names.join(", ");
                let body = if pre.ends_with('{') && spaced {
                    format!("{pre} {list} {suf}")
                } else {
                    format!("{pre}{list}{suf}")
                };
                let indent = &model.line[..model.line.len() - model.line.trim_start().len()];
                let eol = if model.line.ends_with("\r\n") {
                    "\r\n"
                } else if model.line.ends_with('\n') {
                    "\n"
                } else {
                    ""
                };
                Some(Item {
                    key: k.clone(),
                    line: format!("{indent}{body}{eol}"),
                    names,
                })
            }
        } else {
            return None; // one item, changed differently by both sides
        };
        merged.insert(k, pick);
    }
    // A replacement against a deletion. One side rewrote a base import (a new
    // module path for the same name) and the other deleted it: read as keys,
    // that is "both removed the old line, one added a new one", and the union
    // brings back a binding one side took away. Which side is right is the
    // one question the code answers, and only the whole answer has the code:
    // the line is reported, and the caller drops it if no code of the answer
    // uses a name it binds (the deletion was the refactor's) and keeps it if
    // some code does (the line is needed).
    let ext = extension(path);
    for (k, v) in merged.iter() {
        let Some(added) = v.as_ref() else { continue };
        if mo.contains_key(k) || (ma.contains_key(k) && mb.contains_key(k)) {
            continue;
        }
        let names = bindings(&added.line, &ext);
        let replaces_a_deleted_binding = mo.iter().any(|(bk, bi)| {
            !ma.contains_key(bk)
                && !mb.contains_key(bk)
                && bindings(&bi.line, &ext).iter().any(|n| names.contains(n))
        });
        if replaces_a_deleted_binding {
            dead_unless_used.push((added.line.clone(), names));
        }
    }
    // An item git already merged outside the region is not stated twice.
    for (k, v) in merged.iter_mut() {
        let one_sided = !(ma.contains_key(k) && mb.contains_key(k));
        if !mo.contains_key(k)
            && one_sided
            && v.as_ref().is_some_and(|i| stable.contains(i.line.trim()))
        {
            *v = None;
        }
    }
    // No local name bound twice, by two different lines.
    let mut bound: HashMap<String, String> = HashMap::new();
    let lines = merged
        .values()
        .flatten()
        .map(|i| i.line.trim().to_string())
        .chain(stable.iter().cloned())
        .filter(|l| is_import(l, &ext));
    for l in lines {
        for n in bindings(&l, &ext) {
            match bound.get(&n) {
                Some(other) if *other != l => return None,
                _ => {
                    bound.insert(n, l.clone());
                }
            }
        }
    }
    // Base's order, whichever side is called ours: every item base had stays
    // where base had it; an addition goes after the nearest base item that
    // precedes it on the side that added it (the earlier one, if both added
    // it). The additions sharing a place keep their side's order, as two runs,
    // and the run whose text sorts first goes first — so the answer does not
    // depend on which side is called ours. Blank lines are base's.
    let base_at: HashMap<Key, usize> = io
        .iter()
        .enumerate()
        .filter_map(|(i, it)| it.as_ref().map(|it| (it.key.clone(), i)))
        .collect();
    let mut anchor: HashMap<Key, Option<usize>> = HashMap::new();
    let mut runs: [Vec<Key>; 2] = [Vec::new(), Vec::new()];
    for (s, side) in [&ia, &ib].into_iter().enumerate() {
        let mut last: Option<usize> = None;
        for it in side.iter().flatten() {
            match base_at.get(&it.key) {
                Some(i) => last = Some(*i),
                None => {
                    let e = anchor.entry(it.key.clone()).or_insert(last);
                    *e = (*e).min(last);
                    runs[s].push(it.key.clone());
                }
            }
        }
    }
    let text = |k: &Key| match merged.get(k) {
        Some(Some(it)) => it.line.trim().to_string(),
        _ => String::new(),
    };
    let places: std::collections::BTreeSet<Option<usize>> = anchor.values().copied().collect();
    let mut after: BTreeMap<Option<usize>, Vec<Key>> = BTreeMap::new();
    for at in places {
        let run = |r: &Vec<Key>| -> Vec<Key> {
            r.iter()
                .filter(|k| anchor.get(*k) == Some(&at))
                .cloned()
                .collect()
        };
        let (mut first, mut second) = (run(&runs[0]), run(&runs[1]));
        let joined = |ks: &[Key]| ks.iter().map(text).collect::<Vec<_>>().join("\n");
        if joined(&second) < joined(&first) {
            std::mem::swap(&mut first, &mut second);
        }
        let group = after.entry(at).or_default();
        for k in first.into_iter().chain(second) {
            if !group.contains(&k) {
                group.push(k);
            }
        }
    }
    let mut out: Vec<String> = Vec::new();
    let emit_after = |at: Option<usize>, out: &mut Vec<String>| {
        for k in after.get(&at).into_iter().flatten() {
            if let Some(Some(it)) = merged.get(k) {
                out.push(it.line.clone());
            }
        }
    };
    emit_after(None, &mut out);
    for (i, (line, it)) in o.iter().zip(io.iter()).enumerate() {
        match it {
            None => out.push(line.clone()),
            Some(it) => {
                if let Some(Some(m)) = merged.get(&it.key) {
                    out.push(m.line.clone());
                }
            }
        }
        emit_after(Some(i), &mut out);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merge::PARSER_REGISTRY;

    fn run(base: &str, ours: &str, theirs: &str, path: &str) -> Option<Settled> {
        settle(base, ours, theirs, path, &PARSER_REGISTRY, &Host::default())
    }

    fn v(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|l| format!("{l}\n")).collect()
    }

    #[test]
    fn carried_needs_the_other_sides_lines_in_place() {
        let o = v(&["alpha();"]);
        let x = v(&["alpha();", "bravo();", "charlie();"]);
        let y = v(&["alpha();", "bravo();"]);
        assert!(carried(&o, &x, &y));
        assert!(!carried(&o, &y, &x));
        // the same line written at another place is not carried
        let y2 = v(&["bravo();", "alpha();"]);
        assert!(!carried(&o, &x, &y2));
    }

    #[test]
    fn a_deletion_is_carried_only_by_a_deletion() {
        let o = v(&["keep();", "value = 1;", "tail();"]);
        let deleted = v(&["keep();", "tail();"]);
        let edited = v(&["keep();", "value = 2;", "tail();"]);
        assert!(!carried(&o, &edited, &deleted));
        let also_deleted = v(&["keep();", "tail();", "more();"]);
        assert!(carried(&o, &also_deleted, &deleted));
    }

    #[test]
    fn a_moved_line_is_not_a_deletion_carried() {
        let o = v(&["first_call();", "second_call();"]);
        let y = v(&["second_call();"]);
        let x = v(&["second_call();", "first_call();", "third_call();"]);
        assert!(!carried(&o, &x, &y));
    }

    #[test]
    fn blank_lines_are_not_edits() {
        let o = v(&["alpha();", "bravo();"]);
        let y = v(&["alpha();", "", "bravo();", "charlie();"]);
        let x = v(&["alpha();", "bravo();", "charlie();", "delta();"]);
        assert!(carried(&o, &x, &y));
    }

    #[test]
    fn import_union_keeps_both_additions() {
        let base = "import os\nimport sys\n\n\ndef f():\n    return os.getcwd()\n";
        let ours = "import os\nimport re\nimport sys\n\n\ndef f():\n    return os.getcwd()\n";
        let theirs = "import os\nimport json\nimport sys\n\n\ndef f():\n    return os.getcwd()\n";
        let s = run(base, ours, theirs, "m.py").expect("settled");
        assert_eq!(s.rules, vec!["D3 set union"]);
        assert!(s.content.contains("import re\n") && s.content.contains("import json\n"));
        assert_eq!(s.content.matches("import sys").count(), 1);
    }

    #[test]
    fn a_replaced_name_list_is_a_replacement() {
        let base = "import { a, b } from './m';\nexport const x = 1;\n";
        let ours = "import { a, c } from './m';\nexport const x = 1;\n";
        let theirs = "import { a, b, d } from './m';\nexport const x = 1;\n";
        let s = run(base, ours, theirs, "m.ts").expect("settled");
        assert!(
            s.content.starts_with("import { a, c, d } from './m';\n"),
            "{}",
            s.content
        );
    }

    #[test]
    fn one_name_bound_from_two_modules_is_refused() {
        let base = "import os\n\n\ndef f():\n    return 1\n";
        let ours = "import os\nfrom a import thing\n\n\ndef f():\n    return 1\n";
        let theirs = "import os\nfrom b import thing\n\n\ndef f():\n    return 1\n";
        assert!(run(base, ours, theirs, "m.py").is_none());
    }

    #[test]
    fn an_ignore_file_merges_as_a_set() {
        let base = "target/\n";
        let ours = "target/\n.env\n";
        let theirs = "target/\nnode_modules/\n";
        let s = run(base, ours, theirs, ".gitignore").expect("settled");
        assert_eq!(s.content, "target/\n.env\nnode_modules/\n");
        // both created it
        let s = run("", "a/\nb/\n", "a/\nc/\n", ".gitignore").expect("settled");
        assert_eq!(s.content, "a/\nb/\nc/\n");
    }

    #[test]
    fn requirements_pin_changed_twice_is_a_conflict() {
        let base = "requests==2.0\n";
        let ours = "requests==2.1\n";
        let theirs = "requests==2.2\n";
        assert!(run(base, ours, theirs, "requirements.txt").is_none());
    }

    #[test]
    fn a_layout_only_side_yields_the_other() {
        let base = "function f(a, b) {\n  return a + b;\n}\n";
        let ours = "function f(a,b){\n    return a+b;\n}\n";
        let theirs = "function f(a, b) {\n  return a - b;\n}\n";
        let s = run(base, ours, theirs, "m.ts").expect("settled");
        assert_eq!(s.content, theirs);
        // comments are content
        let commented = "// adds\nfunction f(a, b) {\n  return a + b;\n}\n";
        assert!(run(base, commented, theirs, "m.ts").is_none());
    }

    #[test]
    fn creations_that_differ_only_in_layout_are_one_file() {
        let (two, four) = ("class A {\n  int x;\n}\n", "class A {\n    int x;\n}\n");
        let s = run("", two, four, "A.java").expect("settled");
        let r = run("", four, two, "A.java").expect("settled");
        assert_eq!(
            s.content, r.content,
            "the answer must not depend on which side is ours"
        );
        assert!(s.content == two || s.content == four);
    }

    #[test]
    fn a_region_one_side_only_reformatted_takes_the_other() {
        let base = "function f() {\n  one(1, 2);\n  two();\n}\n";
        let ours = "function f() {\n  one(1,2);\n  two();\n}\n";
        let theirs = "function f() {\n  one(1, 3);\n  two();\n}\n";
        let s = run(base, ours, theirs, "m.ts").expect("settled");
        assert_eq!(s.content, theirs);
    }

    #[test]
    fn modify_delete_of_a_declaration_is_seen() {
        let base = "def a():\n    return 1\n\n\ndef b():\n    return 2\n";
        let ours = "def b():\n    return 2\n";
        let theirs = "def a():\n    return 10\n\n\ndef b():\n    return 2\n";
        assert!(has_modify_delete(
            base,
            ours,
            theirs,
            "m.py",
            &PARSER_REGISTRY
        ));
        let theirs = "def a():\n    return 1\n\n\ndef b():\n    return 20\n";
        assert!(!has_modify_delete(
            base,
            ours,
            theirs,
            "m.py",
            &PARSER_REGISTRY
        ));
    }
}
