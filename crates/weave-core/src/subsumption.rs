//! The subsumption rule.
//!
//! If ours changed nothing, `merge(b, b, t) = t`: a side that changed
//! nothing has asserted nothing, so the other side's file is the answer. The
//! same argument does not need the hypothesis to be *nothing*. It needs the
//! hypothesis to be *nothing new*: if every hunk ours wrote is already carried
//! by a hunk theirs wrote, then theirs' file is a file in which ours' whole
//! edit has been applied, and taking it loses nothing.
//!
//! "Carried" is checked against the same base, hunk by hunk, so it is a
//! statement about two edits and not about two texts:
//!
//! - theirs' hunk deletes at least the base lines ours' hunk deletes
//!   (its base range contains ours'), and
//! - every line ours' hunk writes appears, in order, among the lines theirs'
//!   hunk writes.
//!
//! Both halves are needed. Without the range, two sides that insert the same
//! line at two different places read as agreement — for example, a `@Test`
//! annotation added to two different methods, where taking one side
//! drops the other's annotation. Without the subsequence, a rewrite that
//! happens to span ours' lines reads as carrying them when it replaced them.
//!
//! What the rule deliberately does not decide: two sides editing the same token
//! to values where one string extends the other (`4.0.0-beta-1` →
//! `4.0.0-beta-2` vs `4.0.0-beta-1-SNAPSHOT`). Textual extension is not
//! subsumption — neither hunk covers the other's range with the other's lines
//! inside it — so the predicate below rejects those, and version bumps stay
//! conflicts, which is what they are: a judgment.
//!
//! Blank lines and trailing whitespace are outside the comparison, so a side
//! whose only edit is blank lines produces no hunks and the rule never fires on
//! it — that edit is still an edit and still belongs to the pipeline.

use std::ops::Range;

/// Which side's file carries both edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Superset {
    Ours,
    Theirs,
}

/// One unit of change against base, with zero context: the base lines it
/// replaces, and the lines it writes in their place.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Edit {
    old: Range<usize>,
    ins: Vec<String>,
}

/// The comparison key for a line: trailing space removed, blank lines dropped,
/// the line's own indentation kept — indentation is program structure, not
/// formatting.
fn key_lines(text: &str) -> Vec<&str> {
    text.lines()
        .map(|l| l.trim_end())
        .filter(|l| !l.is_empty())
        .collect()
}

/// The hunks of `side` against `base`, both already reduced to key lines.
///
/// diffy renders a patch; with a context length of zero every hunk is exactly
/// one edit, and the hunk's `old_range` is exactly the base lines it replaces.
fn edits(base: &str, side: &str) -> Vec<Edit> {
    let patch = diffy::DiffOptions::new()
        .set_context_len(0)
        .create_patch(base, side);
    patch
        .hunks()
        .iter()
        .map(|h| Edit {
            old: h.old_range().range(),
            ins: h
                .lines()
                .iter()
                .filter_map(|l| match l {
                    diffy::Line::Insert(s) => Some(s.trim_end_matches('\n').to_string()),
                    _ => None,
                })
                .collect(),
        })
        .collect()
}

/// `small` appears inside `big` in order (not necessarily contiguously).
fn is_subsequence(small: &[String], big: &[String]) -> bool {
    let mut it = big.iter();
    small.iter().all(|x| it.any(|y| y == x))
}

/// `outer` carries `inner`: it deletes at least the same base lines and writes
/// at least the same lines.
///
/// The empty insertion is the case that needs saying out loud. A hunk that
/// deletes base lines and writes nothing says *this region should be gone*;
/// any hunk covering it vacuously "writes at least the same lines", because
/// nothing is a subsequence of everything. That reading turns modify/delete —
/// one side deletes the statement the other side edited — into a silent win
/// for the edit, when it should be a CONFLICT: the two sides disagree about
/// whether the statement should exist at all, and that disagreement is a
/// judgment call, not something this rule gets to resolve on their behalf. So
/// a deletion is carried only by a deletion: if the inner hunk writes
/// nothing, the outer must write nothing either.
fn carries(inner: &Edit, outer: &Edit) -> bool {
    if !outer.old.is_empty() || !inner.old.is_empty() {
        // ranges must nest
        if outer.old.start > inner.old.start || inner.old.end > outer.old.end {
            return false;
        }
    } else if outer.old.start != inner.old.start {
        // two insertions: they must be at the same anchor
        return false;
    }
    if inner.ins.is_empty() && !inner.old.is_empty() && !outer.ins.is_empty() {
        return false; // a deletion is carried only by a deletion
    }
    is_subsequence(&inner.ins, &outer.ins)
}

/// The side whose file already carries both edits, if there is one.
///
/// `None` when neither side subsumes the other — including when the two edits
/// are the same edit, where there is nothing to decide and the pipeline's own
/// reading (whitespace, ordering, interstitials) is the better one.
pub(crate) fn subsuming_side(base: &str, ours: &str, theirs: &str) -> Option<Superset> {
    // A line-ending conversion is a file-wide edit that the key above cannot
    // see — `trim_end` eats the `\r`. If the two sides disagree about line
    // endings, one of them made an edit this rule is blind to, and the rule
    // must not answer a file about a change it cannot read.
    if ours.contains("\r\n") != theirs.contains("\r\n") {
        return None;
    }
    let b = key_lines(base).join("\n");
    let o = key_lines(ours).join("\n");
    let t = key_lines(theirs).join("\n");
    let (eo, et) = (edits(&b, &o), edits(&b, &t));
    if eo.is_empty() || et.is_empty() {
        return None;
    }
    let o_in_t = eo.iter().all(|h| et.iter().any(|hh| carries(h, hh)));
    let t_in_o = et.iter().all(|h| eo.iter().any(|hh| carries(h, hh)));
    match (o_in_t, t_in_o) {
        (true, true) => None, // the same edit twice: nothing to choose
        (true, false) => Some(Superset::Theirs),
        (false, true) => Some(Superset::Ours),
        (false, false) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ours_side(b: &str, o: &str, t: &str) -> Option<Superset> {
        subsuming_side(b, o, t)
    }

    #[test]
    fn theirs_rewrote_the_region_ours_touched_and_kept_ours_line() {
        // ours adds one guard; theirs adds the same guard and a second one.
        let base = "fn f() {\n    work();\n}\n";
        let ours = "fn f() {\n    check();\n    work();\n}\n";
        let theirs = "fn f() {\n    check();\n    audit();\n    work();\n}\n";
        assert_eq!(ours_side(base, ours, theirs), Some(Superset::Theirs));
    }

    #[test]
    fn the_same_line_added_at_two_places_is_not_subsumption() {
        // `@Test` added to two different methods. Multiset containment calls
        // this agreement; it is two edits.
        let base = "class C {\n  void a() {}\n\n  void b() {}\n}\n";
        let ours = "class C {\n  @Test\n  void a() {}\n\n  void b() {}\n}\n";
        let theirs = "class C {\n  void a() {}\n\n  @Test\n  void b() {}\n}\n";
        assert_eq!(ours_side(base, ours, theirs), None);
    }

    #[test]
    fn a_version_bump_is_not_subsumption() {
        let base = "<version>4.0.0-beta-1-SNAPSHOT</version>\n";
        let ours = "<version>4.0.0-beta-2-SNAPSHOT</version>\n";
        let theirs = "<version>4.0.0-beta-1</version>\n";
        assert_eq!(ours_side(base, ours, theirs), None);
    }

    #[test]
    fn a_rewrite_that_spans_ours_lines_without_keeping_them_is_not_subsumption() {
        let base = "a\nb\nc\n";
        let ours = "a\nB\nc\n";
        let theirs = "a\nX\nY\nc\n";
        assert_eq!(ours_side(base, ours, theirs), None);
    }

    #[test]
    fn identical_edits_are_left_to_the_pipeline() {
        let base = "a\nb\n";
        let ours = "a\nb\nc\n";
        let theirs = "a\nb\nc\n";
        assert_eq!(ours_side(base, ours, theirs), None);
    }

    #[test]
    fn ours_can_be_the_superset() {
        let base = "x\ny\nz\n";
        let ours = "x\np\nq\ny\nz\n";
        let theirs = "x\np\ny\nz\n";
        assert_eq!(ours_side(base, ours, theirs), Some(Superset::Ours));
    }

    #[test]
    fn a_blank_line_only_edit_never_fires_the_rule() {
        // A whitespace-only edit is still an edit, and it is the pipeline's
        // to weigh — the rule must not answer the file for it.
        let base = "a\nb\n";
        let ours = "a\n\nb\n";
        let theirs = "a\nb\nc\n";
        assert_eq!(ours_side(base, ours, theirs), None);
    }

    #[test]
    fn deletions_subsume_too() {
        let base = "a\nb\nc\nd\n";
        let ours = "a\nc\nd\n"; // drops b
        let theirs = "a\nd\n"; // drops b and c
        assert_eq!(ours_side(base, ours, theirs), Some(Superset::Theirs));
    }

    #[test]
    fn a_deletion_against_an_edit_of_the_same_lines_stays_a_conflict() {
        // modify/delete. `[]` is a subsequence of `[b']`, so a rule that only
        // nested ranges and subsequenced insertions would hand the file to the
        // editing side and call the deletion carried. It is not carried.
        let base = "a\nb\nc\n";
        let ours = "a\nc\n";
        let theirs = "a\nb2\nc\n";
        assert_eq!(ours_side(base, ours, theirs), None);
    }

    #[test]
    fn a_line_ending_conversion_blocks_the_rule() {
        let base = "a\nb\n";
        let ours = "a\r\nb\r\nc\r\n";
        let theirs = "a\nb\n";
        assert_eq!(ours_side(base, ours, theirs), None);
    }

    #[test]
    fn two_unrelated_edits_are_not_subsumption() {
        let base = "a\nb\nc\nd\n";
        let ours = "a\nB\nc\nd\n";
        let theirs = "a\nb\nc\nD\n";
        assert_eq!(ours_side(base, ours, theirs), None);
    }
}
