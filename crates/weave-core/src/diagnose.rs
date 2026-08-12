//! Diagnose, don't restate.
//!
//! An agent resolving a real conflicted merge with weave's typed findings in
//! front of it gets little from a `both_modified` finding alone — it reads as
//! nearly useless without per-hunk detail, and the reason is visible in the
//! document. A `CONFLICT` finding carried the
//! entity's name, a complexity label, a sentence of advice, and the *complete
//! text* of base, ours and theirs. All of that except the label is something the
//! reader already had: the file is in the working tree and the two branches are
//! in the repository. Restating three versions of a method is not analysis, it
//! is a copy with extra steps — and on a 769-line Java class it is 450KB of one.
//!
//! What an agent cannot cheaply get is the *alignment*: which lines of base each
//! side rewrote, where those rewrites overlap, and what the overlap consists of.
//! That is one three-way diff away, and weave has already computed it. This
//! module hands it over.
//!
//! **Every field here is READ OFF the two diffs. Nothing is inferred and nothing
//! is guessed.** A hunk's ranges are diffy's own `old_range`s with a context
//! length of zero, so a hunk is exactly one edit and its range is exactly the
//! base lines it replaces. The tokens are set differences over the identifiers
//! of the lines each side wrote and deleted — not a summary of intent, which is
//! the thing a merge tool must never claim to know. `collision` is the
//! intersection of the two sides' deleted base lines: the lines both developers
//! had opinions about, which is the whole question the marker is asking.

use std::collections::BTreeSet;

use crate::binding::word_tokens;

/// What one side did to one hunk of base.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SideAction {
    /// This side left this hunk of base alone; only the other side touched it.
    Untouched,
    /// Wrote lines where base had none.
    Added,
    /// Deleted base lines and wrote nothing in their place.
    Removed,
    /// Deleted base lines and wrote different ones.
    Modified,
}

impl SideAction {
    /// The wire tag. Exhaustive, so a new action cannot compile until it has a
    /// name on the wire — the discipline [`crate::conflict::ConflictKind::wire_kind`]
    /// already sets.
    pub fn wire(&self) -> &'static str {
        match self {
            SideAction::Untouched => "untouched",
            SideAction::Added => "added",
            SideAction::Removed => "removed",
            SideAction::Modified => "modified",
        }
    }
}

/// One side's edit to one hunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SideEdit {
    pub action: SideAction,
    /// Base lines this side deleted, verbatim and in order.
    pub removed: Vec<String>,
    /// Lines this side wrote in their place, verbatim and in order.
    pub added: Vec<String>,
    /// Identifiers present in what this side wrote and absent from what it
    /// deleted. A *set* difference over tokens, so it names what appeared —
    /// never why.
    pub tokens_added: Vec<String>,
    /// Identifiers present in what this side deleted and absent from what it
    /// wrote.
    pub tokens_removed: Vec<String>,
}

impl SideEdit {
    fn untouched() -> Self {
        SideEdit {
            action: SideAction::Untouched,
            removed: Vec::new(),
            added: Vec::new(),
            tokens_added: Vec::new(),
            tokens_removed: Vec::new(),
        }
    }

    fn of(removed: Vec<String>, added: Vec<String>) -> Self {
        let action = match (removed.is_empty(), added.is_empty()) {
            (true, true) => SideAction::Untouched,
            (true, false) => SideAction::Added,
            (false, true) => SideAction::Removed,
            (false, false) => SideAction::Modified,
        };
        let tok = |ls: &[String]| -> BTreeSet<String> {
            ls.iter()
                .flat_map(|l| word_tokens(l))
                .map(str::to_string)
                .collect()
        };
        let (ta, tr) = (tok(&added), tok(&removed));
        SideEdit {
            action,
            tokens_added: ta.difference(&tr).cloned().collect(),
            tokens_removed: tr.difference(&ta).cloned().collect(),
            removed,
            added,
        }
    }
}

/// One place inside a conflict box where the two sides' edits meet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    /// The base lines this hunk replaces, 1-based and inclusive, relative to the
    /// entity's own text. `end < start` means a pure insertion *before* `start`,
    /// which is what diffy's zero-length old range says and what it must keep
    /// saying: an insertion has a position, not an extent.
    pub base_start: usize,
    pub base_end: usize,
    pub ours: SideEdit,
    pub theirs: SideEdit,
    /// Base lines BOTH sides deleted or rewrote. Empty means the two edits are
    /// merely adjacent — they are in one hunk because nothing separates them,
    /// not because they contradict.
    pub collision: Vec<String>,
}

/// A contiguous run of base lines one side replaced.
struct Edit {
    start: usize,
    len: usize,
    removed: Vec<String>,
    added: Vec<String>,
}

/// The edits `side` makes to `base`, one per contiguous run.
///
/// Context length zero, so a hunk is exactly one edit and `old_range` is exactly
/// the base lines it replaces — the same reading `subsumption::edits` takes, for
/// the same reason.
fn edits(base: &str, side: &str) -> Vec<Edit> {
    diffy::DiffOptions::new()
        .set_context_len(0)
        .create_patch(base, side)
        .hunks()
        .iter()
        .map(|h| {
            let mut removed = Vec::new();
            let mut added = Vec::new();
            for l in h.lines() {
                match l {
                    diffy::Line::Delete(s) => removed.push(s.trim_end_matches('\n').to_string()),
                    diffy::Line::Insert(s) => added.push(s.trim_end_matches('\n').to_string()),
                    diffy::Line::Context(_) => {}
                }
            }
            Edit {
                start: h.old_range().start(),
                len: h.old_range().len(),
                removed,
                added,
            }
        })
        .collect()
}

/// Do two base spans overlap, or sit with nothing between them?
///
/// Adjacent edits are grouped because a reader resolving one has to look at the
/// other: there is no line between them to stop at. Insertions (`len == 0`) are
/// points, and a point inside or on the boundary of a span belongs to it.
fn touches(a: &Edit, b: &Edit) -> bool {
    let (a_lo, a_hi) = (a.start, a.start + a.len);
    let (b_lo, b_hi) = (b.start, b.start + b.len);
    a_lo <= b_hi && b_lo <= a_hi
}

/// The per-hunk alignment of two sides' edits to one base text.
///
/// Hunks come out ordered by base position, which makes the output a function of
/// the three texts and nothing else — no hash iteration anywhere.
pub fn diagnose(base: &str, ours: &str, theirs: &str) -> Vec<Hunk> {
    let (oe, te) = (edits(base, ours), edits(base, theirs));
    if oe.is_empty() && te.is_empty() {
        return Vec::new();
    }
    // Group edits from both sides into runs that touch. `used` is per side, so
    // an edit joins exactly one group — the same linearity `v2::types::Claim`
    // enforces for entities, here enforced by a sorted sweep.
    let mut all: Vec<(bool, &Edit)> = oe
        .iter()
        .map(|e| (true, e))
        .chain(te.iter().map(|e| (false, e)))
        .collect();
    all.sort_by_key(|(is_ours, e)| (e.start, e.len, !*is_ours));

    let mut hunks: Vec<Hunk> = Vec::new();
    let mut group: Vec<(bool, &Edit)> = Vec::new();
    let flush = |group: &mut Vec<(bool, &Edit)>, hunks: &mut Vec<Hunk>| {
        if group.is_empty() {
            return;
        }
        let lo = group.iter().map(|(_, e)| e.start).min().unwrap_or(1);
        let hi = group
            .iter()
            .map(|(_, e)| e.start + e.len)
            .max()
            .unwrap_or(lo);
        let collect = |want: bool| -> SideEdit {
            let mut removed = Vec::new();
            let mut added = Vec::new();
            for (is_ours, e) in group.iter().filter(|(s, _)| *s == want) {
                let _ = is_ours;
                removed.extend(e.removed.iter().cloned());
                added.extend(e.added.iter().cloned());
            }
            if removed.is_empty() && added.is_empty() {
                SideEdit::untouched()
            } else {
                SideEdit::of(removed, added)
            }
        };
        let o = collect(true);
        let t = collect(false);
        // The lines both developers had an opinion about. Order follows ours'
        // deletion order, which is base order, so this is deterministic.
        let theirs_removed: BTreeSet<&String> = t.removed.iter().collect();
        let collision: Vec<String> = o
            .removed
            .iter()
            .filter(|l| theirs_removed.contains(*l))
            .cloned()
            .collect();
        // `hi` is one past the last base line the group replaces, so the
        // inclusive end is `hi - 1` — and `hi == 0` is a real input, not an
        // underflow to repair: diffy reports a zero-length old range at position
        // 0 for an insertion before the first line, and `base_end < base_start`
        // is exactly what this type says an insertion looks like. Written out
        // rather than clamped, so a static analyzer and a reader are told the
        // same true thing (the discipline `container::drops_unanimous_lines`
        // set).
        let base_end = if hi > 0 { hi - 1 } else { 0 };
        hunks.push(Hunk {
            base_start: lo,
            base_end,
            ours: o,
            theirs: t,
            collision,
        });
        group.clear();
    };

    for item in all {
        let joins = group.iter().any(|(_, g)| touches(g, item.1));
        if !joins {
            flush(&mut group, &mut hunks);
        }
        group.push(item);
    }
    flush(&mut group, &mut hunks);
    hunks
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "fn f() {\n    a();\n    b();\n    c();\n}\n";

    #[test]
    fn two_sides_rewriting_the_same_line_collide_in_one_hunk() {
        let ours = "fn f() {\n    a();\n    b_ours();\n    c();\n}\n";
        let theirs = "fn f() {\n    a();\n    b_theirs();\n    c();\n}\n";
        let h = diagnose(BASE, ours, theirs);
        assert_eq!(h.len(), 1, "{h:#?}");
        assert_eq!((h[0].base_start, h[0].base_end), (3, 3));
        assert_eq!(h[0].ours.action, SideAction::Modified);
        assert_eq!(h[0].theirs.action, SideAction::Modified);
        assert_eq!(h[0].collision, vec!["    b();".to_string()]);
        assert_eq!(h[0].ours.tokens_added, vec!["b_ours".to_string()]);
        assert_eq!(h[0].theirs.tokens_added, vec!["b_theirs".to_string()]);
        assert_eq!(h[0].ours.tokens_removed, vec!["b".to_string()]);
    }

    #[test]
    fn edits_far_apart_are_two_hunks_and_neither_collides() {
        let ours = "fn f() {\n    a_ours();\n    b();\n    c();\n}\n";
        let theirs = "fn f() {\n    a();\n    b();\n    c_theirs();\n}\n";
        let h = diagnose(BASE, ours, theirs);
        assert_eq!(h.len(), 2, "{h:#?}");
        assert_eq!(h[0].ours.action, SideAction::Modified);
        assert_eq!(h[0].theirs.action, SideAction::Untouched);
        assert!(h[0].collision.is_empty());
        assert_eq!(h[1].ours.action, SideAction::Untouched);
        assert_eq!(h[1].theirs.action, SideAction::Modified);
        assert!(h[1].collision.is_empty());
    }

    #[test]
    fn a_deletion_and_an_insertion_are_named_as_what_they_are() {
        let ours = "fn f() {\n    a();\n    c();\n}\n"; // deleted b
        let theirs = "fn f() {\n    a();\n    b();\n    new();\n    c();\n}\n"; // inserted
        let h = diagnose(BASE, ours, theirs);
        assert_eq!(h.len(), 1, "adjacent edits are one hunk: {h:#?}");
        assert_eq!(h[0].ours.action, SideAction::Removed);
        assert_eq!(h[0].theirs.action, SideAction::Added);
        assert!(h[0].collision.is_empty(), "theirs deleted nothing");
        assert_eq!(h[0].theirs.tokens_added, vec!["new".to_string()]);
    }

    #[test]
    fn an_unedited_body_has_no_hunks() {
        assert!(diagnose(BASE, BASE, BASE).is_empty());
    }

    /// The output is a function of the three texts: same inputs, same hunks,
    /// every time and in base order.
    #[test]
    fn hunks_come_out_in_base_order() {
        let ours = "fn f() {\n    a_o();\n    b();\n    c_o();\n}\n";
        let theirs = "fn f() {\n    a_t();\n    b();\n    c_t();\n}\n";
        let h = diagnose(BASE, ours, theirs);
        assert!(
            h.windows(2).all(|w| w[0].base_start <= w[1].base_start),
            "{h:#?}"
        );
        assert_eq!(h, diagnose(BASE, ours, theirs));
    }
}
