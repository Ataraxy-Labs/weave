//! The frame and the hole: what a conflicted file says when nobody is arbitrating.
//!
//! A conflicted merge output is not one document, it is a *family* of them —
//! one per way of resolving the boxes. The two a user can reach with no thought
//! at all are "keep ours everywhere" and "keep theirs everywhere", and git's own
//! tooling (`git checkout --ours`, every merge UI's two big buttons) makes them
//! the default moves.
//!
//! Every check weave runs used to stop at the marker. `is_clean()` is a fact
//! about the dispositions, and every checker opened with "an honest conflict is
//! not a defect" and returned — so the text weave authored *around* the boxes,
//! which is most of a conflicted file, went unchecked entirely. That is not a
//! gap in coverage, it is a gap in what the checks range over: the same mistake
//! made earlier about the member level and the statement level. A check that
//! does not range over a region cannot rule anything out there, and exactly
//! that kind of region turned up a duplicate `verifyPost` OUTSIDE the
//! markers — text no marker-aware reader would look for, and which would not
//! have compiled.
//!
//! So this module gives the conflicted output the same vocabulary the clean one
//! has:
//!
//!   FRAME   every line outside a conflict box. Text weave placed, on its own
//!           authority, with no marker telling the reader to check it.
//!   HOLE    one conflict box, per side. Text weave refused to place, handed
//!           back to the reader with both claims intact.
//!
//! and one derived object, [`resolve_conflicts`], which fills every hole from one side and
//! hands back an ordinary file — the thing the checks can then range over.
//!
//! The rule this module and [`crate::conflict`] follow: a conflict is a hole
//! cut from the file. The frame is the file minus the hole, and each side's
//! marker content is exactly that side's bytes of the hole, cut at the SAME
//! offsets the frame was cut at. Renderers that cut the sides with a second,
//! independent slicer produce a frame and a hole that do not tile the input —
//! and text that belongs to no piece has to end up either duplicated or lost.

use std::collections::HashMap;

/// Which claim to keep when filling every hole in a conflicted file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    /// `git checkout --ours`: keep the first half of every box.
    Ours,
    /// `git checkout --theirs`: keep the second half of every box.
    Theirs,
}

/// Where a line of a conflicted file sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Zone {
    Frame,
    Ours,
    Base,
    Theirs,
}

/// Walk a conflicted file, telling each line which zone it is in.
///
/// Marker lines themselves belong to no zone and are not yielded. Two other
/// lines are marker furniture on the same argument — weave writes them, no
/// version did — and are dropped with the markers rather than counted as
/// anyone's claim: the box's `refused_by:` line, and the file's single trailing
/// teach line. Counting either would make weave's own annotations look like
/// content it invented, which is precisely what the frame check exists to catch.
fn zones(content: &str) -> impl Iterator<Item = (Zone, &str)> {
    let mut zone = Zone::Frame;
    content.lines().filter_map(move |line| {
        if line.starts_with("<<<<<<<") {
            zone = Zone::Ours;
            return None;
        }
        if zone != Zone::Frame && line.starts_with("|||||||") {
            zone = Zone::Base;
            return None;
        }
        if zone != Zone::Frame && line.starts_with("=======") {
            zone = Zone::Theirs;
            return None;
        }
        if line.starts_with(">>>>>>>") {
            zone = Zone::Frame;
            return None;
        }
        if zone == Zone::Ours && crate::conflict::refusal_body(line).is_some() {
            return None;
        }
        if crate::conflict::is_teach_line(line) {
            return None;
        }
        Some((zone, line))
    })
}

/// How many marker lines a text still carries.
///
/// One owner for "does this look resolved". `weave check` needs the count and
/// used to spell the three patterns itself — a second place that decides what a
/// marker looks like, which is the duplication the marker-scan check
/// exists to catch. The scan is legitimate here and only here: the subject is
/// TEXT SOMEONE EDITED, not a merge weave performed, so there is no typed
/// decision to consult. `is_clean()` is the case where scanning is wrong and it
/// does not scan.
pub fn marker_line_count(content: &str) -> usize {
    content
        .lines()
        .filter(|l| {
            l.starts_with("<<<<<<<") || l.starts_with("=======") || l.starts_with(">>>>>>>")
        })
        .count()
}

/// The standard resolution: fill every hole from one side, keep the frame.
///
/// This is the file a user gets from the two-button move, and it is the object
/// the merge checks range over in the test suite. Nothing here is weave's
/// opinion about which side is right — it is the reader's, applied uniformly.
pub fn resolve_conflicts(content: &str, take: Resolution) -> String {
    let keep = match take {
        Resolution::Ours => Zone::Ours,
        Resolution::Theirs => Zone::Theirs,
    };
    let mut out = String::new();
    for (zone, line) in zones(content) {
        if zone == Zone::Frame || zone == keep {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// Every line weave placed on its own authority — outside every box.
pub fn frame(content: &str) -> Vec<&str> {
    zones(content)
        .filter(|(z, _)| *z == Zone::Frame)
        .map(|(_, l)| l)
        .collect()
}

/// Does this file contain a conflict box?
///
/// `pub(crate)`, deliberately: outside this crate the question "did this merge
/// conflict" is answered by `MergeResult::is_clean()`, which reads the
/// DISPOSITIONS. Publishing a marker scan next to it would hand callers a second
/// answer to a question that already has one — and a worse one, since a source
/// line quoting `<<<<<<<` is not a conflict. In here it is asked of
/// text this pipeline just wrote, where there is no disposition to consult.
pub(crate) fn is_conflicted(content: &str) -> bool {
    content.lines().any(|l| l.starts_with("<<<<<<<"))
}

/// A line the frame states more times than any version of the file stated it.
///
/// The frame is weave's own placement, so a line that appears there more times
/// than any honest union of the two sides could put there is text this merge
/// manufactured — a duplicate `verifyPost` outside the markers is one
/// instance of it, the duplicate extracted body the scenario table now names
/// is another.
///
/// **The bound is the union bound, and additions from the two sides are
/// additive** — the mirror of the argument
/// [`crate::container::drops_unanimous_lines`] makes about deletions. Of the `n`
/// copies base wrote, ours added `o - n` and theirs added `t - n`, and nothing
/// says those are the same copies, so a merge that keeps both sides' additions
/// legitimately holds `o + t - n`. Two sides independently adding one copy of a
/// line is the commonest shape of it, and a `max` bound would score that as a
/// duplication. It is floored at `max(o, t)` because a frame that carries all of
/// one side's copies has invented nothing, whatever the other side deleted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameDuplicate {
    /// The line, trailing whitespace stripped.
    pub line: String,
    /// How many times the frame states it.
    pub found: usize,
    /// The most times any one version states it.
    pub allowed: usize,
}

/// Structural punctuation carries no identity: a bare `}` or `end` appearing
/// more often than any single version wrote it is what happens when two blocks
/// from two sides land next to each other, not a manufactured line.
fn is_structural(line: &str) -> bool {
    let t = line.trim();
    t.len() <= 2 || t.chars().all(|c| !c.is_alphanumeric() && c != '_')
}

fn line_counts(lines: impl IntoIterator<Item = impl AsRef<str>>) -> HashMap<String, usize> {
    let mut m: HashMap<String, usize> = HashMap::new();
    for l in lines {
        let t = l.as_ref().trim_end();
        if !t.trim().is_empty() {
            *m.entry(t.to_string()).or_default() += 1;
        }
    }
    m
}

/// The runtime self-check: what did this merge state in the frame that no
/// version of the file states?
///
/// Answered on the FINISHED bytes rather than on the derivation, for the same
/// reason [`crate::container::drops_unanimous_lines`] is: the derivation is only
/// as good as the parse behind it, and this is the question a reader would ask.
/// It is advisory — a duplicate in a conflicted file is a warning, never a
/// verdict change, because the file already needs a human.
///
/// Deterministic order: sorted by line, so the warning list is a function of the
/// merge and not of a hash seed.
pub fn frame_duplicates(base: &str, ours: &str, theirs: &str, merged: &str) -> Vec<FrameDuplicate> {
    let cf = line_counts(frame(merged));
    let (cb, co, ct) = (
        line_counts(base.lines()),
        line_counts(ours.lines()),
        line_counts(theirs.lines()),
    );
    let mut out: Vec<FrameDuplicate> = cf
        .into_iter()
        .filter(|(line, _)| !is_structural(line))
        .filter_map(|(line, found)| {
            let get = |m: &HashMap<String, usize>| m.get(&line).copied().unwrap_or(0);
            let (n, o, t) = (get(&cb), get(&co), get(&ct));
            let allowed = (o + t).saturating_sub(n).max(o).max(t);
            (found > allowed).then_some(FrameDuplicate {
                line,
                found,
                allowed,
            })
        })
        .collect();
    out.sort_by(|a, b| a.line.cmp(&b.line));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFLICTED: &str = "\
before()
<<<<<<< ours — function `f` (S, confidence: low)
// refused_by: statement_fold · collision: `body()`
ours_body()
=======
theirs_body()
>>>>>>> theirs — function `f`
after()
";

    #[test]
    fn the_two_resolutions_keep_the_frame_and_one_claim_each() {
        assert_eq!(
            resolve_conflicts(CONFLICTED, Resolution::Ours),
            "before()\nours_body()\nafter()\n"
        );
        assert_eq!(
            resolve_conflicts(CONFLICTED, Resolution::Theirs),
            "before()\ntheirs_body()\nafter()\n"
        );
    }

    #[test]
    fn the_refusal_line_is_marker_furniture_and_belongs_to_neither_side() {
        assert!(!resolve_conflicts(CONFLICTED, Resolution::Ours).contains("refused_by:"));
        assert!(!frame(CONFLICTED).iter().any(|l| l.contains("refused_by:")));
    }

    #[test]
    fn the_frame_is_what_sits_outside_every_box() {
        assert_eq!(frame(CONFLICTED), vec!["before()", "after()"]);
    }

    #[test]
    fn the_base_section_of_a_diff3_box_is_nobody_s_claim() {
        let diff3 = "<<<<<<< ours\na()\n||||||| base\nb()\n=======\nc()\n>>>>>>> theirs\n";
        assert_eq!(resolve_conflicts(diff3, Resolution::Ours), "a()\n");
        assert_eq!(resolve_conflicts(diff3, Resolution::Theirs), "c()\n");
        assert!(frame(diff3).is_empty());
    }

    #[test]
    fn a_frame_line_no_version_wrote_twice_is_reported_once() {
        let merged = "def alpha():\n    return 1\ndef alpha():\n    return 1\n";
        let one = "def alpha():\n    return 1\n";
        let dups = frame_duplicates(one, one, one, merged);
        assert_eq!(dups.len(), 2, "both body lines are duplicated: {dups:?}");
        assert_eq!(dups[1].line, "def alpha():");
        assert_eq!((dups[1].found, dups[1].allowed), (2, 1));
    }

    #[test]
    fn a_line_one_version_really_does_write_twice_is_not_a_duplicate() {
        let twice = "log_it()\nlog_it()\n";
        assert!(frame_duplicates("log_it()\n", twice, "log_it()\n", twice).is_empty());
    }

    #[test]
    fn two_sides_independently_adding_one_copy_may_both_be_kept() {
        // base has none, each side wrote one, the union has two. A `max` bound
        // would call that a duplication; the union bound is what a merge that
        // keeps both additions is entitled to.
        let both = "import a\nimport a\n";
        assert!(frame_duplicates("", "import a\n", "import a\n", both).is_empty());
        // Three, though, is one more than anybody wrote.
        assert_eq!(
            frame_duplicates(
                "",
                "import a\n",
                "import a\n",
                "import a\nimport a\nimport a\n"
            )
            .len(),
            1
        );
    }
}
