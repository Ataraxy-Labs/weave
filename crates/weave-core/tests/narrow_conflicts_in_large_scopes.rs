//! A conflict inside one large scope boxes only the lines that overlap.
//!
//! Field shape: a very long body (here one JSX `return (…)` statement, which
//! the statement fold cannot split) where the two sides edited different,
//! non-overlapping lines, plus ONE line both sides changed differently. The
//! whole span between the first and the last edit came back as a single box,
//! swallowing every non-overlapping edit. The reader had to take one side and
//! re-apply the other side's hunks by hand. A line-level three-way merge of
//! the same texts (git's) applies those hunks and boxes only the overlap.
//!
//! The merge laws this must keep, checked here at the boundary:
//! * still a conflict: the overlap is real, and exit status must say so;
//! * no silent loss: every edit either side made is in the output, in the
//!   frame or in a box;
//! * symmetry: swapping the sides mirrors the boxes and leaves the frame alone;
//! * narrow: the box holds only the overlapping lines.

use proptest::prelude::*;
use weave_core::host::Host;
use weave_core::{entity_merge, entity_merge_fmt, MarkerFormat};

fn panel(rows: usize, edits: &[(usize, &str)]) -> String {
    let mut out = String::from(
        "import * as Kit from \"./kit\"\n\nexport function Panel(props: Props) {\n  const value = Kit.read(props)\n  return (\n    <Kit.Frame>\n",
    );
    for i in 0..rows {
        let extra = edits
            .iter()
            .find(|(r, _)| *r == i)
            .map(|(_, e)| format!(" {e}"))
            .unwrap_or_default();
        out.push_str(&format!(
            "      <Kit.Row key=\"row{i}\">\n        <Kit.Cell value={{value.item{i}}}{extra} />\n      </Kit.Row>\n"
        ));
    }
    out.push_str("    </Kit.Frame>\n  )\n}\n");
    out
}

/// One box: (ours lines, theirs lines).
type Boxed<'a> = (Vec<&'a str>, Vec<&'a str>);

/// (frame lines, boxes).
fn split(text: &str) -> (Vec<&str>, Vec<Boxed<'_>>) {
    let mut frame = Vec::new();
    let mut boxes = Vec::new();
    let mut zone = 0; // 0 frame, 1 ours, 2 base, 3 theirs
    let (mut o, mut t) = (Vec::new(), Vec::new());
    for line in text.lines() {
        if line.starts_with("<<<<<<<") {
            zone = 1;
        } else if line.starts_with("|||||||") && zone == 1 {
            zone = 2;
        } else if line.starts_with("=======") && zone != 0 {
            zone = 3;
        } else if line.starts_with(">>>>>>>") {
            boxes.push((std::mem::take(&mut o), std::mem::take(&mut t)));
            zone = 0;
        } else {
            match zone {
                0 => frame.push(line),
                1 if !line.contains("refused_by: ") => o.push(line),
                3 => t.push(line),
                _ => {}
            }
        }
    }
    (frame, boxes)
}

fn check_shape(
    rows: usize,
    ours_edits: &[(usize, &str)],
    theirs_edits: &[(usize, &str)],
    clash: usize,
) {
    let base = panel(rows, &[]);
    let ours = panel(rows, ours_edits);
    let theirs = panel(rows, theirs_edits);
    let r = entity_merge(&base, &ours, &theirs, "panel.tsx");
    assert!(!r.is_clean(), "the overlap is real:\n{}", r.content);
    let (frame, boxes) = split(&r.content);
    assert_eq!(boxes.len(), 1, "one overlap, one box:\n{}", r.content);
    let (bo, bt) = &boxes[0];
    assert_eq!(
        bo.len(),
        1,
        "box holds only the clashing line:\n{}",
        r.content
    );
    assert_eq!(
        bt.len(),
        1,
        "box holds only the clashing line:\n{}",
        r.content
    );
    assert!(bo[0].contains(&format!("item{clash}}}")), "{}", r.content);
    // Every non-overlapping edit is applied in the frame.
    for (row, e) in ours_edits.iter().chain(theirs_edits) {
        if *row == clash {
            continue;
        }
        let line = format!("        <Kit.Cell value={{value.item{row}}} {e} />");
        assert!(
            frame.contains(&line.as_str()),
            "`{line}` lost:\n{}",
            r.content
        );
    }
    // Symmetry: swapping sides mirrors the box and keeps the frame.
    let s = entity_merge(&base, &theirs, &ours, "panel.tsx");
    let (frame2, boxes2) = split(&s.content);
    assert_eq!(frame, frame2);
    assert_eq!(boxes2.len(), 1);
    assert_eq!(&boxes2[0].0, bt);
    assert_eq!(&boxes2[0].1, bo);
}

#[test]
fn one_overlap_in_a_long_statement_boxes_only_that_line() {
    check_shape(
        120,
        &[(3, "bold"), (10, "wide"), (60, "ours")],
        &[(40, "dim"), (100, "tall"), (60, "theirs")],
        60,
    );
}

#[test]
fn standard_markers_refine_too_and_carry_base() {
    let rows = 80;
    let base = panel(rows, &[]);
    let ours = panel(rows, &[(5, "bold"), (30, "ours")]);
    let theirs = panel(rows, &[(70, "dim"), (30, "theirs")]);
    let r = entity_merge_fmt(
        &base,
        &ours,
        &theirs,
        "panel.tsx",
        &MarkerFormat::standard(7),
        &Host::default(),
    );
    assert!(!r.is_clean());
    assert_eq!(r.content.matches("<<<<<<<").count(), 1, "{}", r.content);
    assert_eq!(
        r.content.matches("||||||| base").count(),
        1,
        "{}",
        r.content
    );
    // Taking ours in the box gives ours plus theirs' non-overlapping edit.
    let mut take_ours = String::new();
    let mut zone = 0;
    for line in r.content.lines() {
        if line.starts_with("<<<<<<<") {
            zone = 1;
            continue;
        }
        if line.starts_with("|||||||") || line.starts_with("=======") {
            zone = 2;
            continue;
        }
        if line.starts_with(">>>>>>>") {
            zone = 0;
            continue;
        }
        if zone <= 1 {
            take_ours.push_str(line);
            take_ours.push('\n');
        }
    }
    assert_eq!(
        take_ours,
        panel(rows, &[(5, "bold"), (30, "ours"), (70, "dim")])
    );
}

#[test]
fn a_small_conflict_keeps_its_single_box() {
    let base = "function f() {\n  return 1\n}\n";
    let ours = "function f() {\n  return 2\n}\n";
    let theirs = "function f() {\n  return 3\n}\n";
    let r = entity_merge(base, ours, theirs, "f.ts");
    assert!(!r.is_clean());
    assert_eq!(r.content.matches("<<<<<<<").count(), 1, "{}", r.content);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    /// Any spread of non-overlapping edits around one clash: one narrow box,
    /// nothing lost, symmetric.
    #[test]
    fn scattered_edits_around_one_clash_stay_out_of_the_box(
        rows in 40usize..90,
        clash_at in 0.0f64..1.0,
        picks in prop::collection::btree_set(0usize..90, 2..10),
    ) {
        let clash = ((rows as f64 - 1.0) * clash_at) as usize;
        // Keep edits at least 2 rows (6 lines) from the clash and from each
        // other, so no two edits touch adjacent lines.
        let mut chosen: Vec<usize> = Vec::new();
        for p in picks.into_iter().filter(|p| *p < rows) {
            if p.abs_diff(clash) >= 2 && chosen.iter().all(|c| c.abs_diff(p) >= 2) {
                chosen.push(p);
            }
        }
        let mut ours_edits: Vec<(usize, &str)> = vec![(clash, "ours")];
        let mut theirs_edits: Vec<(usize, &str)> = vec![(clash, "theirs")];
        for (k, row) in chosen.iter().enumerate() {
            if k % 2 == 0 {
                ours_edits.push((*row, "left"));
            } else {
                theirs_edits.push((*row, "right"));
            }
        }
        check_shape(rows, &ours_edits, &theirs_edits, clash);
    }
}
