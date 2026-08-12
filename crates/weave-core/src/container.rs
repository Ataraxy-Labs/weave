//! The container merge: a class body is a file in miniature.
//!
//! A class, interface, `impl` block or object literal merges member-wise —
//! members are unordered children, so two sides touching different members is
//! not a conflict. A prior implementation got
//! the *decision* right and the *reconstruction* wrong, in three ways that
//! all cost real code on real merges:
//!
//!   1. it rebuilt the body out of the member chunks alone. Anything the
//!      chunker did not claim — a class-level attribute block, a section
//!      comment, a bare statement, the blank lines between members — simply
//!      ceased to exist: reconstructing from claimed members only can silently
//!      drop untouched bytes neither side asked to remove, while still
//!      reporting a clean merge.
//!
//!   2. it indexed child spans against the entity's `start_line` while slicing
//!      the entity's *region* text, which begins earlier when a doc comment is
//!      bundled in. Every chunk was then shifted up by the length of that
//!      comment: tails truncated, header text duplicated into the first member.
//!
//!   3. it used ours' member sequence as the skeleton and appended theirs-only
//!      members after everything else — the peer-relative rule `v2::plan`
//!      exists to have abolished at the top level.
//!
//! The fix is to give the body the same discipline the file gets. A version is
//! DECOMPOSED into segments that tile it — every byte belongs to exactly one
//! segment, and the segments concatenate back to the input. Members merge by
//! identity; the text between them travels with the member it leads; placement
//! is a function of the shared base order. Because the decomposition is a
//! partition, "some text was silently dropped" is not a thing the
//! reconstruction can express.
//!
//! And because a partition is only as honest as the parser feeding it, the
//! result is checked before it is returned: a line all three versions agree on
//! must appear in the output as many times as all three agree it should. If it
//! does not, this merge is not trustworthy and the caller is told there is no
//! container merge here — which costs a whole-entity conflict, and never a
//! silent deletion.

use std::collections::{HashMap, HashSet};

use sem_core::model::entity::SemanticEntity;

use crate::host::{Host, LineMergeStyle};
use crate::merge::ScopeMarkers;
use crate::merge::{
    diffy_merge, extract_container_wrapper, extract_member_name, scoped_conflict_marker,
    try_decorator_aware_merge,
};

/// The caller's line-level merge, asked for a clean answer only — the second
/// opinion this module used to spawn `git` for itself.
fn granted_line_merge(host: &Host, b: &str, o: &str, t: &str) -> Option<String> {
    let line_merge = host.line_merge?;
    line_merge(LineMergeStyle::Plain, b, o, t).map(|m| m.content)
}
use crate::statement::{License, LicensedGap};

/// Result of a container merge attempt.
pub(crate) struct InnerMergeResult {
    /// Merged content (may contain per-member conflict markers)
    pub(crate) content: String,
    /// Whether any members had conflicts
    pub(crate) has_conflicts: bool,
}

/// A member's identity within one container: its name, plus the ordinal that
/// separates same-named siblings (a `@property` and its `@x.setter`, two
/// overloads of one method). Keying a `HashMap` on the bare name collapsed
/// those into one and dropped the rest.
type MemberKey = (String, u32);

/// One member of a container body, with the unclaimed text that leads it.
struct Member {
    key: MemberKey,
    /// Text between the previous member (or the header) and this member's own
    /// first line: blank lines, section comments, class-level statements.
    lead: String,
    text: String,
}

/// One version of a container body, tiled.
///
/// `header + Σ(lead + text) + tail + footer` reproduces the input exactly (up
/// to a trailing newline), which is the property the whole module rests on.
struct Decomposed {
    header: String,
    /// Text between the declaration and the first member: the class-level
    /// attribute block, the leading docstring, a bare statement.
    ///
    /// It is a segment of the CONTAINER, not of the member that happens to
    /// follow it — exactly as `file_header` belongs to the file and not to its
    /// first declaration. Attaching it to the first member made the whole block
    /// move, duplicate or vanish whenever the sides disagreed about which
    /// member came first.
    prelude: String,
    members: Vec<Member>,
    /// Unclaimed text after the last member, before the footer.
    tail: String,
    footer: String,
}

impl Decomposed {
    fn keys(&self) -> Vec<MemberKey> {
        self.members.iter().map(|m| m.key.clone()).collect()
    }
    fn index(&self) -> HashMap<&MemberKey, &Member> {
        self.members.iter().map(|m| (&m.key, m)).collect()
    }
}

/// Join line slices back into text, one newline per line.
fn join(lines: &[&str]) -> String {
    let mut s = String::new();
    for l in lines {
        s.push_str(l);
        s.push('\n');
    }
    s
}

/// The span of one member inside the body, as inclusive line indices.
pub(crate) struct MemberSpan {
    pub(crate) name: String,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

/// Split a container into header / members / footer so that the pieces tile it.
///
/// `start_line` must be the 1-based line number of `content`'s FIRST line in
/// the original file — not the entity's declaration line, which is a different
/// number whenever a doc comment was bundled into the region (defect 2).
fn decompose(content: &str, children: &[&SemanticEntity], start_line: usize) -> Option<Decomposed> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() < 2 {
        return None;
    }
    let (header_end, body_end) = wrapper_bounds(content, &lines)?;
    if header_end > body_end {
        return None;
    }

    let spans = child_spans(children, start_line, header_end, body_end)
        .or_else(|| indentation_member_spans(&lines, header_end, body_end))?;
    if spans.is_empty() {
        return None;
    }

    // Pull each member's own leading decorators and doc comment into it, but
    // never past the previous member's end: a comment claimed twice would be
    // emitted twice.
    let mut members: Vec<Member> = Vec::new();
    let mut seen: HashMap<String, u32> = HashMap::new();
    let mut cursor = header_end;
    for (i, span) in spans.iter().enumerate() {
        let floor = if i == 0 { header_end } else { cursor };
        let own_start = attach_leading_trivia(&lines, span.start, floor);
        let lead = join(&lines[cursor..own_start]);
        let text = join(&lines[own_start..=span.end]);
        let ordinal = {
            let n = seen.entry(span.name.clone()).or_insert(0);
            let v = *n;
            *n += 1;
            v
        };
        members.push(Member {
            key: (span.name.clone(), ordinal),
            lead,
            text,
        });
        cursor = span.end + 1;
    }

    let prelude = members
        .first_mut()
        .map(|m| std::mem::take(&mut m.lead))
        .unwrap_or_default();
    Some(Decomposed {
        header: join(&lines[..header_end]),
        prelude,
        members,
        tail: join(&lines[cursor..body_end]),
        footer: join(&lines[body_end..]),
    })
}

/// (first body line, first footer line) as indices into `lines`.
fn wrapper_bounds(content: &str, lines: &[&str]) -> Option<(usize, usize)> {
    let (header, footer) = extract_container_wrapper(content)?;
    let header_end = header.lines().count();
    let footer_lines = footer.lines().count();
    let body_end = lines.len().checked_sub(footer_lines)?;
    Some((header_end, body_end))
}

/// Member spans from sem-core children, or `None` when the children do not
/// describe this text (positions out of range, nested, or overlapping) — in
/// which case the indentation scan is the honest reading.
fn child_spans(
    children: &[&SemanticEntity],
    start_line: usize,
    body_start: usize,
    body_end: usize,
) -> Option<Vec<MemberSpan>> {
    if children.is_empty() {
        return None;
    }
    let mut spans: Vec<MemberSpan> = Vec::new();
    for child in children {
        let start = child.start_line.checked_sub(start_line)?;
        let end = child.end_line.checked_sub(start_line)?;
        if start > end || start < body_start || end >= body_end {
            return None;
        }
        if let Some(prev) = spans.last() {
            // Children arrive sorted by start line; anything that does not
            // strictly follow its predecessor is a nested or overlapping entity
            // and the span list is not a partition.
            if start <= prev.end {
                return None;
            }
        }
        spans.push(MemberSpan {
            name: child.name.clone(),
            start,
            end,
        });
    }
    Some(spans)
}

/// Member spans read off indentation, for bodies sem-core reports no children
/// for: object literals, enum variants, plain field blocks. A member starts at
/// the first indentation level inside the body and runs until the next one
/// does; decorators, comments and continuation lines are not starts.
fn indentation_member_spans(
    lines: &[&str],
    body_start: usize,
    body_end: usize,
) -> Option<Vec<MemberSpan>> {
    if body_start >= body_end {
        return None;
    }
    let member_indent = lines[body_start..body_end]
        .iter()
        .find(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())?;

    let mut spans: Vec<MemberSpan> = Vec::new();
    for (i, line) in lines.iter().enumerate().take(body_end).skip(body_start) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let starts_member = indent == member_indent
            && !trimmed.starts_with("//")
            && !trimmed.starts_with("/*")
            && !trimmed.starts_with('*')
            && !trimmed.starts_with('#')
            && !trimmed.starts_with('@')
            && !trimmed.starts_with('}')
            && trimmed != ",";
        if starts_member {
            if let Some(prev) = spans.last_mut() {
                prev.end = i - 1;
            }
            spans.push(MemberSpan {
                name: extract_member_name(trimmed),
                start: i,
                end: body_end - 1,
            });
        }
    }
    // A member's span must not swallow what follows it: the blank lines and the
    // next member's own decorators and doc comment belong to that member's
    // lead, so the gap survives a deletion intact and the comment travels with
    // the thing it documents.
    for span in spans.iter_mut() {
        while span.end > span.start
            && (lines[span.end].trim().is_empty() || is_trivia_line(lines[span.end]))
        {
            span.end -= 1;
        }
        // Anonymous struct-literal entries (Go slices of `{ Name: "x", ... }`)
        // all present as `{`; without a distinguishing name they would collide.
        if span.name == "{" || span.name == "{}" {
            if let Some(better) =
                crate::merge::derive_name_from_struct_literal(&join(&lines[span.start..=span.end]))
            {
                span.name = better;
            }
        }
    }
    if spans.is_empty() {
        None
    } else {
        Some(spans)
    }
}

/// A decorator, annotation or comment line — text that belongs to whatever it
/// precedes rather than standing on its own.
fn is_trivia_line(line: &str) -> bool {
    let t = line.trim();
    t.starts_with('@')
        || t.starts_with("#[")
        || t.starts_with("//")
        || t.starts_with("/*")
        || t.starts_with("* ")
        || t == "*"
        || t == "*/"
}

/// Walk back from a member's declaration over the decorators, annotations and
/// doc comment that belong to it, stopping at `floor`.
fn attach_leading_trivia(lines: &[&str], start: usize, floor: usize) -> usize {
    let mut own = start;
    while own > floor && is_trivia_line(lines[own - 1]) {
        own -= 1;
    }
    own
}

// ---------------------------------------------------------------------------
// Placement: the top-level rule, applied one level down
// ---------------------------------------------------------------------------

/// Order-preserving union of two asserted sequences (the `v2::plan` helper,
/// over member keys).
fn union(a: &[MemberKey], b: &[MemberKey]) -> Vec<MemberKey> {
    let a_set: HashSet<&MemberKey> = a.iter().collect();
    let b_set: HashSet<&MemberKey> = b.iter().collect();
    let mut out: Vec<MemberKey> = Vec::with_capacity(a.len() + b.len());
    let mut emitted: HashSet<MemberKey> = HashSet::new();
    let (mut i, mut j) = (0usize, 0usize);
    while i < a.len() || j < b.len() {
        if i < a.len() && emitted.contains(&a[i]) {
            i += 1;
            continue;
        }
        if j < b.len() && emitted.contains(&b[j]) {
            j += 1;
            continue;
        }
        let next = if i >= a.len() {
            let x = b[j].clone();
            j += 1;
            x
        } else if j >= b.len() || a[i] == b[j] || !b_set.contains(&a[i]) {
            let x = a[i].clone();
            i += 1;
            x
        } else if !a_set.contains(&b[j]) {
            let x = b[j].clone();
            j += 1;
            x
        } else {
            let x = a[i].clone();
            i += 1;
            x
        };
        if emitted.insert(next.clone()) {
            out.push(next);
        }
    }
    out
}

/// The order the merged members are emitted in, decided the way `v2::plan`
/// decides top-level placement: survivors keep the shared base coordinate
/// unless exactly one side moved them, and an addition sits after the NEAREST
/// surviving member it was written after. Nothing consults which side is ours,
/// so the sequence is the same from either direction.
fn member_order(base: &[MemberKey], ours: &[MemberKey], theirs: &[MemberKey]) -> Vec<MemberKey> {
    let ours_set: HashSet<&MemberKey> = ours.iter().collect();
    let theirs_set: HashSet<&MemberKey> = theirs.iter().collect();
    let base_set: HashSet<&MemberKey> = base.iter().collect();

    // Survivors: base members at least one side still has.
    let base_seq: Vec<MemberKey> = base
        .iter()
        .filter(|k| ours_set.contains(k) || theirs_set.contains(k))
        .cloned()
        .collect();
    let side_seq = |side: &[MemberKey]| -> Vec<MemberKey> {
        side.iter()
            .filter(|k| base_set.contains(k))
            .cloned()
            .collect()
    };
    let ours_seq = side_seq(ours);
    let theirs_seq = side_seq(theirs);
    let restrict = |seq: &[MemberKey], to: &[MemberKey]| -> Vec<MemberKey> {
        seq.iter().filter(|k| to.contains(k)).cloned().collect()
    };
    let ours_moved = ours_seq != restrict(&base_seq, &ours_seq);
    let theirs_moved = theirs_seq != restrict(&base_seq, &theirs_seq);
    let chosen = match (ours_moved, theirs_moved) {
        (false, true) => theirs_seq,
        (true, false) => ours_seq,
        // Members are unordered children — that premise is why this merge is
        // allowed at all — so the shared coordinate is the answer that neither
        // side can claim the other overrode.
        _ => base_seq.clone(),
    };
    let survivors = union(&chosen, &base_seq);
    let rank: HashMap<&MemberKey, u32> = survivors
        .iter()
        .enumerate()
        .map(|(r, k)| (k, r as u32))
        .collect();

    // (anchor, tier, key) — exactly the top-level Anchor.
    let mut placed: Vec<(u32, u8, MemberKey)> = Vec::new();
    for k in &survivors {
        placed.push(((rank[k] + 1) * 2, 0, k.clone()));
    }
    let mut added: HashSet<&MemberKey> = HashSet::new();
    for side in [ours, theirs] {
        for (pos, k) in side.iter().enumerate() {
            if base_set.contains(k) || !added.insert(k) {
                continue;
            }
            // The nearest surviving member this one was written after.
            let nearest = side[..pos]
                .iter()
                .rev()
                .find_map(|prev| rank.get(prev).copied());
            let anchor = nearest.map_or(1, |r| (r + 1) * 2 + 1);
            placed.push((anchor, 1, k.clone()));
        }
    }
    // A member both sides added independently gets one anchor, the smaller —
    // the same number from either direction.
    let mut best: HashMap<MemberKey, (u32, u8)> = HashMap::new();
    for (anchor, tier, k) in placed {
        let e = best.entry(k).or_insert((anchor, tier));
        if anchor < e.0 {
            *e = (anchor, tier);
        }
    }
    let mut out: Vec<(u32, u8, MemberKey)> =
        best.into_iter().map(|(k, (a, t))| (a, t, k)).collect();
    out.sort();

    // Within one gap, the key is the last thing to ask. Two members added at
    // the same anchor are only *apparently* simultaneous: if some version holds
    // both, that version already ordered them, and the name is nobody's
    // decision (see `v2::plan::gap_order` — the same rule, one level
    // down).
    let mut i = 0;
    while i < out.len() {
        let mut j = i;
        while j < out.len() && out[j].0 == out[i].0 && out[j].1 == out[i].1 {
            j += 1;
        }
        if out[i].1 == 1 && j - i > 1 {
            let group: Vec<MemberKey> = out[i..j].iter().map(|(_, _, k)| k.clone()).collect();
            let ordered = order_within_gap(&group, ours, theirs);
            for (slot, k) in out[i..j].iter_mut().zip(ordered) {
                slot.2 = k;
            }
        }
        i = j;
    }
    out.into_iter().map(|(_, _, k)| k).collect()
}

/// Order members that share one anchor by the order a version wrote them in,
/// falling back to the key for pairs no version holds together.
///
/// `group` arrives sorted by key, which is both the fallback and the tie-break
/// inside the linear extension, so the result is a function of the two
/// sequences and the key — never of which sequence was called "ours".
fn order_within_gap(
    group: &[MemberKey],
    ours: &[MemberKey],
    theirs: &[MemberKey],
) -> Vec<MemberKey> {
    let mut edges: HashSet<(usize, usize)> = HashSet::new();
    let mut reversed: HashSet<(usize, usize)> = HashSet::new();
    for side in [ours, theirs] {
        let seen: Vec<usize> = side
            .iter()
            .filter_map(|k| group.iter().position(|g| g == k))
            .collect();
        for a in 0..seen.len() {
            for b in a + 1..seen.len() {
                if edges.contains(&(seen[b], seen[a])) {
                    reversed.insert((seen[a], seen[b]));
                    reversed.insert((seen[b], seen[a]));
                }
                edges.insert((seen[a], seen[b]));
            }
        }
    }
    let mut indeg = vec![0usize; group.len()];
    let mut succ: Vec<Vec<usize>> = vec![Vec::new(); group.len()];
    for (u, v) in &edges {
        if reversed.contains(&(*u, *v)) {
            continue;
        }
        succ[*u].push(*v);
        indeg[*v] += 1;
    }
    let mut ready: Vec<usize> = (0..group.len()).filter(|i| indeg[*i] == 0).collect();
    let mut placed: Vec<usize> = Vec::with_capacity(group.len());
    while !ready.is_empty() {
        ready.sort_by_key(|i| group[*i].clone());
        let next = ready.remove(0);
        placed.push(next);
        for v in succ[next].clone() {
            indeg[v] -= 1;
            if indeg[v] == 0 {
                ready.push(v);
            }
        }
    }
    for i in 0..group.len() {
        if !placed.contains(&i) {
            placed.push(i); // a cycle: the key already ordered these
        }
    }
    placed.into_iter().map(|i| group[i].clone()).collect()
}

// ---------------------------------------------------------------------------
// The merge
// ---------------------------------------------------------------------------

/// Three-way merge of a piece of text that is not a member: a header, a footer,
/// the gap that leads a member.
///
/// The gaps are where a class keeps the things the grammar gave no name to —
/// attribute blocks, section comments, bare statements — so "when in doubt take
/// ours" is not a tie-break here, it is a silent deletion of theirs. Both sides
/// adding an attribute in the same slot is the commonest shape of it, and it
/// ends in markers, like any other unmergeable edit.
fn merge_trivia(
    name: &str,
    b: &str,
    o: &str,
    t: &str,
    fmt: &ScopeMarkers<'_>,
    has_conflict: &mut bool,
    license: License,
    evidence: &mut Vec<LicensedGap>,
    host: &Host,
) -> String {
    if o == t {
        return o.to_string();
    }
    if b == o {
        return t.to_string();
    }
    if b == t {
        return o.to_string();
    }
    if let Some(merged) = diffy_merge(b, o, t).or_else(|| granted_line_merge(host, b, o, t)) {
        return merged;
    }
    // An attribute block is a sequence of statements, not merely opaque
    // trivia, so it deserves the same second attempt a member body gets
    // rather than going straight to a conflict marker: the statement fold
    // gets a shot at it, from the same position, after diff3 has already
    // refused.
    if let Some(inner) = crate::statement::statement_merge_with(b, o, t, fmt, license, evidence) {
        if !inner.has_conflicts {
            return inner.content;
        }
    }
    *has_conflict = true;
    scoped_conflict_marker(name, Some(b), Some(o), Some(t), false, false, fmt)
}

/// A container read as the several scopes it is: one `(key, text)` per member,
/// where `key` is the member's name with the ordinal that separates same-named
/// siblings.
///
/// `decompose` is given no sem-core children on purpose. The disjointness check
/// asks its question of a base body and of a MERGED body, and the merged body
/// is text this process just built — nothing has parsed it, so there are no
/// children to hand over. The indentation reading is the only one available for
/// both sides of that comparison, and it has to be the same reading for both or
/// the members will not line up.
pub(crate) fn member_texts(content: &str) -> Option<Vec<((String, u32), String)>> {
    let d = decompose(content, &[], 1)?;
    Some(d.members.into_iter().map(|m| (m.key, m.text)).collect())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn container_merge(
    base: &str,
    ours: &str,
    theirs: &str,
    base_children: &[&SemanticEntity],
    ours_children: &[&SemanticEntity],
    theirs_children: &[&SemanticEntity],
    base_start_line: usize,
    ours_start_line: usize,
    theirs_start_line: usize,
    marker_format: &ScopeMarkers<'_>,
    decorators_compose: bool,
    license: License,
    evidence: &mut Vec<LicensedGap>,
    host: &Host,
) -> Option<InnerMergeResult> {
    let b = decompose(base, base_children, base_start_line)?;
    let o = decompose(ours, ours_children, ours_start_line)?;
    let t = decompose(theirs, theirs_children, theirs_start_line)?;

    let (bi, oi, ti) = (b.index(), o.index(), t.index());
    let order = member_order(&b.keys(), &o.keys(), &t.keys());

    let mut has_conflict = false;
    // The fold's licensed compositions are collected here and handed back to the
    // caller only if the whole container merge is accepted — evidence for a
    // result nobody ships is not evidence.
    let mut evidence_local: Vec<LicensedGap> = Vec::new();
    let mut out = String::new();
    out.push_str(&merge_trivia(
        "declaration",
        &b.header,
        &o.header,
        &t.header,
        marker_format,
        &mut has_conflict,
        license,
        &mut evidence_local,
        host,
    ));

    out.push_str(&merge_trivia(
        "attributes",
        &b.prelude,
        &o.prelude,
        &t.prelude,
        marker_format,
        &mut has_conflict,
        license,
        &mut evidence_local,
        host,
    ));

    for key in &order {
        let (mb, mo, mt) = (bi.get(key), oi.get(key), ti.get(key));
        let name = &key.0;
        // The gap leading a member travels with it, so blank lines and section
        // comments survive relocation instead of being re-synthesised.
        //
        // Only the versions that HAVE this member get a say in its lead. A
        // version that deleted it did not empty the gap in front of it — the
        // text moved to the next member — and reading the absence as an empty
        // gap deleted the class attributes whenever one side dropped the first
        // method.
        fn lead_of<'a>(m: &Option<&&'a Member>) -> Option<&'a str> {
            m.map(|x| x.lead.as_str())
        }
        let lead = match (lead_of(&mo), lead_of(&mt)) {
            (Some(lo), Some(lt)) => merge_trivia(
                &format!("before {name}"),
                lead_of(&mb).unwrap_or(lo),
                lo,
                lt,
                marker_format,
                &mut has_conflict,
                license,
                &mut evidence_local,
                host,
            ),
            (Some(lo), None) => lo.to_string(),
            (None, Some(lt)) => lt.to_string(),
            (None, None) => lead_of(&mb).unwrap_or("").to_string(),
        };
        let body = match (
            mb.map(|m| m.text.as_str()),
            mo.map(|m| m.text.as_str()),
            mt.map(|m| m.text.as_str()),
        ) {
            (Some(pb), Some(po), Some(pt)) => {
                if po == pt {
                    Some(po.to_string())
                } else if pb == po {
                    Some(pt.to_string())
                } else if pb == pt {
                    Some(po.to_string())
                } else {
                    diffy_merge(pb, po, pt)
                        .or_else(|| granted_line_merge(host, pb, po, pt))
                        .or_else(|| try_decorator_aware_merge(pb, po, pt, decorators_compose))
                        // One level further down: the member's own body is a
                        // sequence of statements. Same table, same fold.
                        .or_else(|| {
                            crate::statement::statement_merge_with(
                                pb,
                                po,
                                pt,
                                marker_format,
                                license,
                                &mut evidence_local,
                            )
                            .filter(|r| !r.has_conflicts)
                            .map(|r| r.content)
                        })
                        .or_else(|| {
                            has_conflict = true;
                            Some(scoped_conflict_marker(
                                name,
                                Some(pb),
                                Some(po),
                                Some(pt),
                                false,
                                false,
                                marker_format,
                            ))
                        })
                }
            }
            // One side deleted it. An untouched deletion is accepted; a
            // deletion racing an edit is the member's conflict, not the class's.
            (Some(pb), Some(po), None) => {
                if pb == po {
                    None
                } else {
                    has_conflict = true;
                    Some(scoped_conflict_marker(
                        name,
                        Some(pb),
                        Some(po),
                        None,
                        false,
                        true,
                        marker_format,
                    ))
                }
            }
            (Some(pb), None, Some(pt)) => {
                if pb == pt {
                    None
                } else {
                    has_conflict = true;
                    Some(scoped_conflict_marker(
                        name,
                        Some(pb),
                        None,
                        Some(pt),
                        true,
                        false,
                        marker_format,
                    ))
                }
            }
            (None, Some(po), None) => Some(po.to_string()),
            (None, None, Some(pt)) => Some(pt.to_string()),
            (None, Some(po), Some(pt)) => {
                if po == pt {
                    Some(po.to_string())
                } else {
                    has_conflict = true;
                    Some(scoped_conflict_marker(
                        name,
                        None,
                        Some(po),
                        Some(pt),
                        false,
                        false,
                        marker_format,
                    ))
                }
            }
            (Some(_), None, None) | (None, None, None) => None,
        };
        // A member this merge does not emit takes its own lead with it: the
        // blank line separated it from what came before, and the text that is
        // NOT a separator has already moved into the next member's lead on the
        // side that deleted this one, which is where the three-way merge finds
        // it.
        if let Some(text) = body {
            out.push_str(&lead);
            out.push_str(&text);
            if !text.ends_with('\n') {
                out.push('\n');
            }
        }
    }

    out.push_str(&merge_trivia(
        "trailing members",
        &b.tail,
        &o.tail,
        &t.tail,
        marker_format,
        &mut has_conflict,
        license,
        &mut evidence_local,
        host,
    ));
    out.push_str(&merge_trivia(
        "closing",
        &b.footer,
        &o.footer,
        &t.footer,
        marker_format,
        &mut has_conflict,
        license,
        &mut evidence_local,
        host,
    ));
    if !ours.ends_with('\n') {
        while out.ends_with('\n') {
            out.pop();
        }
    }

    // The backstop. A partition cannot drop text, but the partition is only as
    // good as the parse that produced it; this asks the finished bytes rather
    // than trusting the derivation. `None` costs the caller a whole-entity
    // conflict, which is a decision a human can see — unlike a deletion.
    if drops_unanimous_lines(base, ours, theirs, &out) {
        return None;
    }

    evidence.append(&mut evidence_local);
    Some(InnerMergeResult {
        content: out,
        has_conflicts: has_conflict,
    })
}

/// Does `out` fail to carry the base lines that neither side deleted?
///
/// Multiset over non-blank lines, trailing whitespace ignored. Multiset, so
/// relocation is not a loss. Leading whitespace counts, because a
/// reconstruction that re-indents text nobody asked it to re-indent has lost
/// that text as surely as if it deleted it — rust-analyzer's nested `use` trees
/// came back dedented and syntactically broken, and a trimmed comparison called
/// that fine.
///
/// **Deletions from the two sides are additive.** Of the `n` copies base wrote,
/// ours deleted `n - min(n, ours)` and theirs deleted `n - min(n, theirs)`, and
/// nothing says those are the same copies — so the survivors are
/// `min(n,ours) + min(n,theirs) - n`, floored at zero. The bound used to be
/// `min(n, ours, theirs)`, which is what a merge must keep only if the two sides
/// always delete the *same* copies. On structural punctuation — a bare `}`, a
/// `{`, a `try{` — they never do: two sides deleting two different blocks each
/// take their own brace, and the correct merge has two fewer. That error is not
/// academic: the *developer's own committed resolution* can fail the old
/// bound on exactly this shape, because deleting different blocks that each
/// happen to share a brace is common on structural punctuation. A necessary
/// condition the ground truth violates is not a necessary condition.
///
/// The `min` with `n` keeps this a pure loss predicate: a side that *adds*
/// copies of a base line cannot raise the bar above what base wrote, so two
/// sides independently adding the same line — and a merge that keeps one — is
/// not reported here. Duplication has its own predicate.
///
/// This is the predicate weave is scored on: it is enforced here, inside the
/// merge, and re-stated independently by the suite that measures weave against
/// other merge tools, so the claim and the code cannot drift apart.
pub(crate) fn drops_unanimous_lines(base: &str, ours: &str, theirs: &str, out: &str) -> bool {
    fn counts(s: &str) -> HashMap<&str, usize> {
        let mut m: HashMap<&str, usize> = HashMap::new();
        for l in s.lines() {
            let t = l.trim_end();
            if !t.trim().is_empty() {
                *m.entry(t).or_default() += 1;
            }
        }
        m
    }
    let (cb, co, ct, cx) = (counts(base), counts(ours), counts(theirs), counts(out));
    cb.iter().any(|(line, n)| {
        let kept_ours = co.get(line).copied().unwrap_or(0).min(*n);
        let kept_theirs = ct.get(line).copied().unwrap_or(0).min(*n);
        // Both sides may between them have deleted every copy; that is a fact
        // about the input, not an underflow to repair.
        // `saturating_sub` would read the same and clippy asks for it, but the
        // internal lint that checks for silent-repair patterns would flag it
        // as one — and this is not a repair, it is the arithmetic: both sides
        // may between them have deleted every copy. Written out, both
        // checkers are told the truth.
        #[allow(clippy::implicit_saturating_sub)]
        let required = if kept_ours + kept_theirs > *n {
            kept_ours + kept_theirs - *n
        } else {
            0
        };
        required > cx.get(line).copied().unwrap_or(0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decomp(content: &str) -> Decomposed {
        decompose(content, &[], 1).expect("decomposes")
    }

    /// The property everything else rests on: the pieces are a partition.
    #[test]
    fn a_decomposition_tiles_the_container_it_came_from() {
        for content in [
            "class A:\n    x = 1\n    y = 2\n\n    def one(self):\n        return 1\n\n    def two(self):\n        return 2\n",
            "class A {\n\tx = 1;\n\n\t// a section\n\tone() {\n\t\treturn 1;\n\t}\n\n\ttwo() {\n\t\treturn 2;\n\t}\n}\n",
        ] {
            let d = decomp(content);
            let mut rebuilt = d.header.clone();
            for m in &d.members {
                rebuilt.push_str(&m.lead);
                rebuilt.push_str(&m.text);
            }
            rebuilt.push_str(&d.tail);
            rebuilt.push_str(&d.footer);
            assert_eq!(rebuilt, content, "decomposition lost or duplicated text");
        }
    }

    #[test]
    fn same_named_members_do_not_collapse_into_one() {
        let content = "class A:\n    @property\n    def x(self):\n        return self._x\n\n    @x.setter\n    def x(self, v):\n        self._x = v\n";
        let d = decomp(content);
        let keys = d.keys();
        assert_eq!(keys.len(), 2, "two members named x, not one: {keys:?}");
        assert_eq!(keys[0], ("x".to_string(), 0));
        assert_eq!(keys[1], ("x".to_string(), 1));
    }

    #[test]
    fn member_order_is_the_base_order_when_nobody_moved_anything() {
        let k = |n: &str| (n.to_string(), 0u32);
        let base = vec![k("a"), k("b"), k("c")];
        let ours = vec![k("a"), k("c")]; // deleted b
        let theirs = vec![k("a"), k("b"), k("new"), k("c")];
        // `b` is still in the sequence — whether it is EMITTED is the body
        // match's call, not placement's. What matters is that `new` sits where
        // theirs wrote it, between `b` and `c`, rather than after everything.
        assert_eq!(
            member_order(&base, &ours, &theirs),
            vec![k("a"), k("b"), k("new"), k("c")],
            "the addition follows the nearest survivor it was written after"
        );
    }

    #[test]
    fn member_order_does_not_depend_on_which_side_is_ours() {
        let k = |n: &str| (n.to_string(), 0u32);
        let base = vec![k("a"), k("b")];
        let x = vec![k("a"), k("b"), k("zed")];
        let y = vec![k("a"), k("mid"), k("b")];
        assert_eq!(member_order(&base, &x, &y), member_order(&base, &y, &x));
    }

    #[test]
    fn members_one_side_wrote_together_keep_that_side_s_order() {
        // Two members added into one gap by the same side. Sorted by name they
        // come back `helper, zebra`; the side that wrote them wrote the other
        // order, and that is a fact about the file.
        let k = |n: &str| (n.to_string(), 0u32);
        let base = vec![k("a"), k("b")];
        let ours = vec![k("a"), k("zebra"), k("helper"), k("b")];
        let theirs = vec![k("a"), k("b")];
        assert_eq!(
            member_order(&base, &ours, &theirs),
            vec![k("a"), k("zebra"), k("helper"), k("b")]
        );
    }

    #[test]
    fn members_no_version_orders_together_still_fall_back_to_the_name() {
        let k = |n: &str| (n.to_string(), 0u32);
        let base = vec![k("a")];
        let x = vec![k("a"), k("zed")];
        let y = vec![k("a"), k("mid")];
        assert_eq!(
            member_order(&base, &x, &y),
            vec![k("a"), k("mid"), k("zed")]
        );
        assert_eq!(member_order(&base, &x, &y), member_order(&base, &y, &x));
    }
}
