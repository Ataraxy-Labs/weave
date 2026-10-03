//! D3 over a changelog: two sides that each added entries at the same place in
//! one release section.
//!
//! A changelog section is a list of entries whose order carries no meaning —
//! a reader of "### Features" reads a set of features, and nobody's intent is
//! served by one bullet being above another. So two sides that each inserted
//! whole new entries at one point, and touched nothing else there, made edits
//! that commute under set semantics: the answer is the union, and the only
//! choice left is the order, which is fixed here so it does not depend on
//! which side is called ours (see [`union`]).
//!
//! That argument holds only where order is conventionally free, which is why
//! the rule is scoped to files that are changelogs by name ([`is_changelog`]).
//! A list in a README is often a sequence of steps; a numbered list anywhere
//! says its order out loud. Neither is touched.
//!
//! And it holds only where distinct text means distinct changes, which the
//! text cannot show: two sides that each describe the same feature, in their
//! own words, wrote two entries for one change, and their union says it
//! twice. So the rule is opt-in, like the ordered containers of
//! [`crate::insertion`]: it runs only where the file's `weave-set`
//! gitattribute declares the section a set — bare, or naming the section's
//! heading or a heading above it ([`sections`]). Undeclared, the entries two
//! sides added at one point are a conflict.
//!
//! Lines both sides wrote identically at the start or end of their insertions
//! — the heading of an "Unreleased" section each of them created — are
//! agreement, written once, and the entries between are what is united.
//!
//! Guards, each a conflict:
//! - the base region holds anything but blank lines — a side edited or
//!   deleted an existing line there, or the two insertions are not at one
//!   point;
//! - a side's insertion holds anything but whole unordered-list entries (a
//!   bullet and its more-indented continuation lines) at one indent with one
//!   marker — a heading, a paragraph, a numbered item, a continuation that
//!   extends an entry above the region;
//! - no heading above the region — the insertions are not inside a section;
//! - the line after the region is indented deeper than the entries — the
//!   insertion splits an existing entry or its sub-list;
//! - the two sides lay the region out differently (blank lines before or
//!   after the entries, or between them);
//! - a side states one entry twice.

/// Files that are changelogs or release notes by name.
pub(crate) fn is_changelog(path: &str) -> bool {
    let lower = path.replace('\\', "/").to_ascii_lowercase();
    let name = lower.rsplit('/').next().unwrap_or(&lower);
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) => (s, e),
        None => (name, ""),
    };
    let prose = matches!(ext, "" | "md" | "markdown" | "rst" | "txt");
    let named = ["changelog", "changes", "history", "news", "release"]
        .iter()
        .any(|p| stem.starts_with(p));
    let in_changelog_dir = lower
        .split('/')
        .collect::<Vec<_>>()
        .windows(2)
        .any(|w| w[0] == "docs" && w[1].starts_with("changelog"));
    prose && (named || (in_changelog_dir && !name.is_empty()))
}

fn blank(l: &str) -> bool {
    l.trim().is_empty()
}

fn indent(l: &str) -> usize {
    l.len() - l.trim_start().len()
}

/// The marker of an unordered-list item, if `l` is one.
fn bullet(l: &str) -> Option<char> {
    let t = l.trim_start();
    let mut chars = t.chars();
    let m = chars.next()?;
    (matches!(m, '-' | '*' | '+') && chars.next().is_some_and(|c| c == ' ' || c == '\t'))
        .then_some(m)
}

/// An ATX heading, or the underline of a setext / reStructuredText one.
fn heading(prev: Option<&str>, l: &str) -> bool {
    let t = l.trim();
    if t.starts_with('#') {
        let hashes = t.chars().take_while(|c| *c == '#').count();
        return hashes <= 6 && (t[hashes..].starts_with([' ', '\t']) || t.len() == hashes);
    }
    let mut chars = t.chars();
    let Some(c) = chars.next() else {
        return false;
    };
    t.len() >= 3
        && "=-~^*+#`'\".:_".contains(c)
        && t.chars().all(|x| x == c)
        && prev.is_some_and(|p| !blank(p))
}

/// The names a `weave-set` declaration may give the section a region is in:
/// the title of the nearest heading above it and of each enclosing heading,
/// brackets off (`## [Unreleased]` is `Unreleased`), and each title's first
/// word too, since a gitattribute value holds no whitespace
/// (`## 1.2.0 - 2026-01-01` is also `1.2.0`).
pub(crate) fn sections(above: &[&String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut deepest = usize::MAX;
    for i in (0..above.len()).rev() {
        let l = above[i].trim();
        let prev = i.checked_sub(1).map(|p| above[p].as_str());
        if !heading(prev, l) {
            continue;
        }
        let (level, title) = if l.starts_with('#') {
            let n = l.chars().take_while(|c| *c == '#').count();
            (n, l[n..].trim().trim_end_matches('#').trim().to_string())
        } else {
            // A setext underline: its title is the line above it.
            let level = if l.starts_with('=') { 1 } else { 2 };
            (level, prev.unwrap_or("").trim().to_string())
        };
        if level >= deepest {
            continue;
        }
        deepest = level;
        let title = title
            .trim_matches(|c: char| c == '[' || c == ']')
            .trim()
            .to_string();
        if let Some(first) = title.split_whitespace().next() {
            let first = first
                .trim_matches(|c: char| c == '[' || c == ']')
                .to_string();
            if first != title {
                out.push(first);
            }
        }
        out.push(title);
        if level == 1 {
            break;
        }
    }
    out
}

/// A side's region, read as entries.
struct Block {
    /// Blank lines before the first entry.
    lead: usize,
    entries: Vec<Vec<String>>,
    /// Blank lines between consecutive entries.
    seps: Vec<usize>,
    /// Blank lines after the last entry.
    trail: usize,
    indent: usize,
    marker: char,
}

fn block(x: &[String]) -> Option<Block> {
    let mut b = Block {
        lead: 0,
        entries: Vec::new(),
        seps: Vec::new(),
        trail: 0,
        indent: 0,
        marker: ' ',
    };
    let mut pending = 0;
    for l in x {
        if blank(l) {
            pending += 1;
            continue;
        }
        let starts = match bullet(l) {
            Some(m) if b.entries.is_empty() => {
                b.indent = indent(l);
                b.marker = m;
                b.lead = pending;
                true
            }
            Some(m) if indent(l) == b.indent => {
                if m != b.marker {
                    return None;
                }
                b.seps.push(pending);
                true
            }
            _ => false,
        };
        if starts {
            b.entries.push(vec![l.clone()]);
        } else {
            // A continuation: of an entry this side wrote, directly below it,
            // and deeper than it.
            if b.entries.is_empty() || pending > 0 || indent(l) <= b.indent {
                return None;
            }
            b.entries.last_mut()?.push(l.clone());
        }
        pending = 0;
    }
    b.trail = pending;
    (!b.entries.is_empty()).then_some(b)
}

fn text(entry: &[String]) -> String {
    entry
        .iter()
        .map(|l| l.trim())
        .collect::<Vec<_>>()
        .join("\n")
}

/// D3 over one changelog region. `above` is the file's text before the
/// region, `below` the first line after it.
///
/// The canonical order: each side's new entries stay together, in the order
/// that side wrote them, and the run whose text sorts first goes first — the
/// same convention as the other set unions, so the answer does not depend on
/// which side is called ours. An entry equal (line by line, trimmed) to one
/// already written is written once.
pub(crate) fn union(
    o: &[String],
    a: &[String],
    b: &[String],
    above: &[&String],
    below: Option<&String>,
) -> Option<Vec<String>> {
    if !o.iter().all(|l| blank(l)) {
        return None;
    }
    // Lines both sides wrote the same, at the start or end of the region
    // (the heading of a section both created, a blank line), are agreement:
    // written once, and read as the context of the rest.
    let p = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let s = a[p..]
        .iter()
        .rev()
        .zip(b[p..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let (prefix, suffix) = (&a[..p], &a[a.len() - s..]);
    let (a, b) = (&a[p..a.len() - s], &b[p..b.len() - s]);
    let above: Vec<&String> = above.iter().copied().chain(prefix).collect();
    let below = suffix.iter().find(|l| !blank(l)).or(below);
    let (x, y) = (block(a)?, block(b)?);
    if (x.indent, x.marker, x.lead, x.trail) != (y.indent, y.marker, y.lead, y.trail) {
        return None;
    }
    let mut seps: Vec<usize> = x.seps.iter().chain(&y.seps).copied().collect();
    seps.sort_unstable();
    seps.dedup();
    let sep = match seps.as_slice() {
        [] => 0,
        [one] => *one,
        _ => return None,
    };
    // One spelling of a blank line, or the layout is not the sides' to agree on.
    let mut blanks: Vec<&String> = a.iter().chain(b).filter(|l| blank(l)).collect();
    blanks.sort();
    blanks.dedup();
    let empty = "\n".to_string();
    let blank_line = match blanks.as_slice() {
        [] => &empty,
        [one] => *one,
        _ => return None,
    };
    // Inside a section.
    let has_heading = above
        .iter()
        .enumerate()
        .any(|(i, l)| heading(i.checked_sub(1).map(|p| above[p].as_str()), l));
    if !has_heading {
        return None;
    }
    // Not splitting an entry, or a sub-list, below.
    if below.is_some_and(|l| !blank(l) && indent(l) > x.indent) {
        return None;
    }
    let (mut first, mut second) = (x.entries, y.entries);
    for run in [&first, &second] {
        let mut seen: Vec<String> = run.iter().map(|e| text(e)).collect();
        seen.sort();
        let n = seen.len();
        seen.dedup();
        if seen.len() != n {
            return None; // a side states one entry twice
        }
    }
    let joined = |r: &[Vec<String>]| r.iter().map(|e| text(e)).collect::<Vec<_>>().join("\n");
    if joined(&second) < joined(&first) {
        std::mem::swap(&mut first, &mut second);
    }
    let mut written: Vec<String> = Vec::new();
    let mut out: Vec<String> = prefix.to_vec();
    out.extend(std::iter::repeat_n(blank_line.clone(), x.lead));
    for e in first.into_iter().chain(second) {
        let t = text(&e);
        if written.contains(&t) {
            continue;
        }
        if !written.is_empty() {
            out.extend(std::iter::repeat_n(blank_line.clone(), sep));
        }
        written.push(t);
        out.extend(e);
    }
    out.extend(std::iter::repeat_n(blank_line.clone(), x.trail));
    out.extend(suffix.iter().cloned());
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|l| format!("{l}\n")).collect()
    }

    #[test]
    fn changelogs_are_recognised_by_name() {
        for p in [
            "CHANGELOG.md",
            "changelog",
            "pkg/CHANGES.rst",
            "HISTORY.md",
            "NEWS",
            "RELEASE_NOTES.md",
            "releases.md",
            "docs/changelog/2.0.md",
            "docs/changelog.md",
        ] {
            assert!(is_changelog(p), "{p}");
        }
        for p in [
            "README.md",
            "src/changelog.rs",
            "CHANGELOG.json",
            "docs/guide.md",
        ] {
            assert!(!is_changelog(p), "{p}");
        }
    }

    #[test]
    fn headings_are_seen() {
        assert!(heading(None, "### Features\n"));
        assert!(heading(Some("1.0.0"), "=====\n"));
        assert!(!heading(None, "=====\n"));
        assert!(!heading(None, "- item\n"));
        assert!(!heading(None, "#hashtag\n"));
    }

    #[test]
    fn same_point_entries_union_in_canonical_order() {
        let above = v(&["# Changes", "", "## Unreleased", ""]);
        let above: Vec<&String> = above.iter().collect();
        let a = v(&["- zeta: added", ""]);
        let b = v(&["- alpha: added", "  with a second line", ""]);
        let u = union(&[], &a, &b, &above, None).expect("union");
        let w = union(&[], &b, &a, &above, None).expect("union");
        assert_eq!(u, w);
        assert_eq!(
            u,
            v(&[
                "- alpha: added",
                "  with a second line",
                "- zeta: added",
                ""
            ])
        );
    }

    #[test]
    fn a_continuation_of_an_entry_above_is_refused() {
        let above = v(&["## Unreleased", "- one"]);
        let above: Vec<&String> = above.iter().collect();
        let a = v(&["  more about one"]);
        let b = v(&["- two"]);
        assert!(union(&[], &a, &b, &above, None).is_none());
    }

    #[test]
    fn numbered_items_are_refused() {
        let above = v(&["## Steps"]);
        let above: Vec<&String> = above.iter().collect();
        assert!(union(&[], &v(&["1. one"]), &v(&["1. two"]), &above, None).is_none());
    }
}
