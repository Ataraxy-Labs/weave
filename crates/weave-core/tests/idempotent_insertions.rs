//! Agreement is idempotent: a change both sides made is made once.
//!
//!   D0  merge(b, x, x) = x — and, for an insertion both sides made
//!       identically, the merge states it once whether or not the two
//!       insertions landed at one place, and whatever else each side did.
//!
//! A line merge of disjoint hunks reads the same block inserted at two places
//! as two edits and writes it twice, clean; so do the rules that settle what
//! it conflicts on, since they splice its clean regions verbatim. The witness
//! is the answer: a clean merge states a jointly inserted block exactly once,
//! or the merge is a conflict. Stated at the public boundary (`entity_merge`)
//! over three paths — a statement block inside one function (the statement
//! and line paths), a whole new function (the entity path), and the entries
//! of a container literal (the container path) — each with a disjoint edit on
//! each side, so the two sides are never the same file.
//!
//! Also here, the other shapes of the same audit:
//! - two arms for one pattern of one match, one from each side;
//! - two creations (no base) where one states a block of the other's twice;
//! - a blank line one side deleted beside a line both deleted;
//! - an import one side's refactor deleted and the other moved to a new
//!   module path.
//!
//! The domain is synthetic: sentinel-tagged items. Base writes 1xxxx, the
//! block both insert 7xxxx.

use proptest::prelude::*;
use weave_core::entity_merge;

// ===========================================================================
// D0 over three paths
// ===========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Path {
    /// Statements of one Python function body.
    Statements,
    /// Top-level Python functions.
    Entities,
    /// Entries of a Python dict literal.
    Container,
    /// Statements of one Rust function body.
    RustStatements,
}

impl Path {
    fn file(self) -> &'static str {
        match self {
            Path::RustStatements => "steps.rs",
            _ => "steps.py",
        }
    }
}

/// Base item `i`, as the path writes it (one unit of the collection).
fn item(p: Path, n: u32, edited: bool) -> String {
    let v = if edited { n + 5 } else { n };
    match p {
        Path::Statements => format!("    record_step(\"step{n}\", {v})\n"),
        Path::Entities => format!("def step{n}():\n    return run_step(\"step{n}\", {v})\n\n\n"),
        Path::Container => format!("    \"step{n}\": {v},\n"),
        Path::RustStatements => format!("    record_step(\"step{n}\", {v});\n"),
    }
}

/// The block both sides insert: three significant lines, one sentinel.
fn block(p: Path, s: u32) -> String {
    match p {
        Path::Statements => format!(
            "    if config.enabled(\"feature{s}\"):\n        log_event(\"feature{s} enabled\")\n        register_feature(\"feature{s}\", {s})\n"
        ),
        Path::Entities => format!(
            "def feature{s}(config):\n    log_event(\"feature{s} enabled\")\n    return register_feature(\"feature{s}\", {s})\n\n\n"
        ),
        Path::Container => format!(
            "    \"feature{s}_name\": \"feature{s}\",\n    \"feature{s}_flag\": {s},\n    \"feature{s}_note\": \"added\",\n"
        ),
        Path::RustStatements => format!(
            "    if config.enabled(\"feature{s}\") {{\n        log_event(\"feature{s} enabled\");\n        register_feature(\"feature{s}\", {s});\n    }}\n"
        ),
    }
}

fn render(p: Path, units: &[String]) -> String {
    let body: String = units.concat();
    match p {
        Path::Statements => format!("def build(config):\n{body}    return config\n"),
        Path::Entities => body,
        Path::Container => format!("STEPS = {{\n{body}}}\n"),
        Path::RustStatements => {
            format!("pub fn build(config: &Config) {{\n{body}    finish(config);\n}}\n")
        }
    }
}

#[derive(Debug, Clone)]
struct Case {
    path: Path,
    n: usize,
    /// Where each side inserts the block (before base unit `at`).
    at_ours: usize,
    at_theirs: usize,
    /// The base unit each side edits (distinct, and never next to where
    /// either side inserts, so git has no reason to conflict on them).
    edit_ours: usize,
    edit_theirs: usize,
    sentinel: u32,
}

fn texts(c: &Case) -> (String, String, String) {
    let unit = |i: usize, edited: bool| item(c.path, 10000 + 100 * i as u32, edited);
    let side = |at: usize, edit: usize| {
        let mut v = Vec::new();
        for i in 0..c.n {
            if i == at {
                v.push(block(c.path, c.sentinel));
            }
            v.push(unit(i, i == edit));
        }
        if at == c.n {
            v.push(block(c.path, c.sentinel));
        }
        render(c.path, &v)
    };
    let base: Vec<String> = (0..c.n).map(|i| unit(i, false)).collect();
    (
        render(c.path, &base),
        side(c.at_ours, c.edit_ours),
        side(c.at_theirs, c.edit_theirs),
    )
}

fn any_case() -> impl Strategy<Value = Case> {
    (
        prop_oneof![
            Just(Path::Statements),
            Just(Path::Entities),
            Just(Path::Container),
            Just(Path::RustStatements),
        ],
        8usize..14,
    )
        .prop_flat_map(|(path, n)| {
            (
                Just(path),
                Just(n),
                0..=n,
                0..=n,
                0..n,
                0..n,
                70000u32..79999,
            )
        })
        .prop_map(
            |(path, n, at_ours, at_theirs, edit_ours, edit_theirs, sentinel)| Case {
                path,
                n,
                at_ours,
                at_theirs,
                edit_ours,
                edit_theirs,
                sentinel,
            },
        )
        .prop_filter("the edits are distinct and away from the insertions", |c| {
            let far = |e: usize| {
                [c.at_ours, c.at_theirs]
                    .iter()
                    .all(|a| e + 1 < *a || e > *a + 1)
            };
            c.edit_ours != c.edit_theirs
                && c.edit_ours.abs_diff(c.edit_theirs) > 1
                && far(c.edit_ours)
                && far(c.edit_theirs)
        })
}

fn stated(text: &str, needle: &str) -> usize {
    text.matches(needle).count()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// A block both sides inserted is stated once in a clean merge, wherever
    /// each side put it; each side's own edit survives.
    #[test]
    fn d0_a_jointly_inserted_block_is_stated_once(c in any_case()) {
        let (b, o, t) = texts(&c);
        let needle = format!("\"feature{} enabled\"", c.sentinel);
        let needle = match c.path {
            Path::Container => format!("\"feature{}_flag\"", c.sentinel),
            _ => needle,
        };
        for (x, y) in [(&o, &t), (&t, &o)] {
            let r = entity_merge(&b, x, y, c.path.file());
            if r.is_clean() {
                prop_assert_eq!(
                    stated(&r.content, &needle), 1,
                    "a block both inserted is stated once:\n{}", r.content
                );
                for e in [c.edit_ours, c.edit_theirs] {
                    let edited = item(c.path, 10000 + 100 * e as u32, true);
                    prop_assert!(r.content.contains(edited.trim_end()), "an edit was lost:\n{}", r.content);
                }
            }
        }
    }

    /// At one point, the joint insertion is agreement: the merge is clean
    /// and the block is there once.
    #[test]
    fn d0_a_block_inserted_at_one_point_settles(c in any_case()) {
        let c = Case { at_theirs: c.at_ours, ..c };
        let (b, o, t) = texts(&c);
        let r = entity_merge(&b, &o, &t, c.path.file());
        prop_assert!(r.is_clean(), "{}", r.content);
        let needle = match c.path {
            Path::Container => format!("\"feature{}_flag\"", c.sentinel),
            _ => format!("\"feature{} enabled\"", c.sentinel),
        };
        prop_assert_eq!(stated(&r.content, &needle), 1, "{}", r.content);
    }

    /// merge(b, x, x) = x.
    #[test]
    fn d0_agreement_is_the_answer(c in any_case()) {
        let (b, o, _) = texts(&c);
        let r = entity_merge(&b, &o, &o, c.path.file());
        prop_assert!(r.is_clean());
        prop_assert_eq!(r.content, o);
    }
}

// ===========================================================================
// The audit's shapes, one fixture each
// ===========================================================================

/// A SwiftUI section both sides added, at two places of one view body.
#[test]
fn a_view_section_both_sides_added_at_two_places_is_not_written_twice() {
    let base = "import SwiftUI\n\nstruct ItemListView: View {\n    @State private var name = \"\"\n\n    var body: some View {\n        Form {\n            Section(header: Text(\"Title\")) {\n                TextField(\"Title\", text: $name)\n            }\n            Section(header: Text(\"Details\")) {\n                Text(\"details\")\n            }\n        }\n    }\n}\n";
    let section = "            Section(header: Text(\"Attachments\")) {\n                ForEach(attachments, id: \\.id) { attachment in\n                    Text(attachment.fileName)\n                }\n                Button(\"Add Attachment...\") {\n                    showPicker = true\n                }\n            }\n";
    let first = "                TextField(\"Title\", text: $name)\n            }\n";
    let second = "                Text(\"details\")\n            }\n";
    let ours = base.replace(first, &format!("{first}{section}"));
    let theirs = base.replace(second, &format!("{second}{section}"));
    for (x, y) in [(&ours, &theirs), (&theirs, &ours)] {
        let r = entity_merge(base, x, y, "ItemListView.swift");
        assert!(
            !r.is_clean() || r.content.matches("Text(\"Attachments\")").count() == 1,
            "{}",
            r.content
        );
    }
}

/// Two arms for one pattern of one match, one from each side, each with its
/// own body: the second is unreachable.
#[test]
fn two_arms_for_one_pattern_are_a_conflict() {
    let base = "pub fn describe(shape: &Shape) -> String {\n    match shape {\n        Shape::Circle(r) => format!(\"circle {r}\"),\n        Shape::Square(s) => format!(\"square {s}\"),\n        Shape::Line(l) => format!(\"line {l}\"),\n    }\n}\n";
    let top = "        Shape::Circle(r) => format!(\"circle {r}\"),\n";
    let bottom = "        Shape::Line(l) => format!(\"line {l}\"),\n";
    let ours = base.replace(
        top,
        &format!(
            "        Shape::Mesh(m) => format!(\"mesh with {{}} triangles\", m.len()),\n{top}"
        ),
    );
    let theirs = base.replace(
        bottom,
        &format!("{bottom}        Shape::Mesh(m) => format!(\"mesh {{}}\", m.name),\n"),
    );
    assert!(!entity_merge(base, &ours, &theirs, "shapes.rs").is_clean());
    assert!(!entity_merge(base, &theirs, &ours, "shapes.rs").is_clean());
    // Control: two arms for two patterns are two answers to two questions.
    let theirs = base.replace(
        bottom,
        &format!("{bottom}        Shape::Hexagon(h) => format!(\"hexagon {{h}}\"),\n"),
    );
    let r = entity_merge(base, &ours, &theirs, "shapes.rs");
    assert!(r.is_clean(), "{}", r.content);
    assert!(r.content.contains("Shape::Mesh") && r.content.contains("Shape::Hexagon"));
}

/// The same arm pattern in two different functions' matches is two keys.
#[test]
fn one_arm_pattern_in_two_functions_is_not_a_duplicate() {
    let base = "pub fn a(x: u8) -> u8 {\n    match x {\n        0 => 1,\n        _ => 0,\n    }\n}\n\npub fn b(x: u8) -> u8 {\n    match x {\n        0 => 2,\n        _ => 0,\n    }\n}\n";
    let ours = base.replace("        0 => 1,\n", "        0 => 1,\n        7 => 70,\n");
    let theirs = base.replace("        0 => 2,\n", "        0 => 2,\n        7 => 71,\n");
    let r = entity_merge(base, &ours, &theirs, "m.rs");
    assert!(r.is_clean(), "{}", r.content);
    assert_eq!(r.content.matches("7 => ").count(), 2);
}

const HEADER_TOP: &str = "#pragma once\n#include <cstdint>\n\nclass Manager {\npublic:\n    Manager();\n    void update();\n";
const HEADER_DECLS: &str = "    const Metrics& metrics() const;\n    uint64_t globalTick() const;\n    uint32_t patternCount(Tier tier) const;\n    float fragmentation(Tier tier) const;\n";
const HEADER_BOTTOM: &str = "\nprivate:\n    int state_;\n};\n";

/// Two creations, no base: one states a block of member declarations the
/// other states once, twice. Whether one side pasted it again or the other
/// removed the repetition, the base would say; there is none.
#[test]
fn two_creations_disagreeing_on_how_often_a_block_is_stated_conflict() {
    let once = format!("{HEADER_TOP}{HEADER_DECLS}{HEADER_BOTTOM}");
    let twice = format!("{HEADER_TOP}{HEADER_DECLS}\n{HEADER_DECLS}{HEADER_BOTTOM}");
    assert!(!entity_merge("", &once, &twice, "manager.h").is_clean());
    assert!(!entity_merge("", &twice, &once, "manager.h").is_clean());
    // Control: a creation that is the other's plus new lines is still the
    // other's plus new lines.
    let more = format!(
        "{HEADER_TOP}{HEADER_DECLS}    void reset();\n    bool ready() const;\n    int count() const;\n{HEADER_BOTTOM}"
    );
    let r = entity_merge("", &once, &more, "manager.h");
    assert!(r.is_clean(), "{}", r.content);
    assert_eq!(r.content, more);
}

/// Both deleted a line; one also deleted the blank line under it. The sides
/// differ only in layout, but only one of them changed the layout there, and
/// a one-sided edit is carried — blank line or not.
#[test]
fn a_blank_line_one_side_deleted_stays_deleted() {
    let base = "#![feature(let_chains)]\n\nuse std::fmt;\n\npub fn f() -> u32 {\n    1\n}\n";
    let ours = "use std::fmt;\n\npub fn f() -> u32 {\n    1\n}\n";
    let theirs = "\nuse std::fmt;\n\npub fn f() -> u32 {\n    1\n}\n";
    for (x, y) in [(ours, theirs), (theirs, ours)] {
        let r = entity_merge(base, x, y, "lib.rs");
        assert!(r.is_clean(), "{}", r.content);
        assert_eq!(r.content, ours);
    }
    // Two sides that laid one change out differently, each at its own
    // place, still settle on one of the two (the smaller).
    let ours = "use std::fmt;\n\n\npub fn f() -> u32 {\n    1\n}\n";
    let r = entity_merge(base, ours, theirs, "lib.rs");
    let s = entity_merge(base, theirs, ours, "lib.rs");
    assert!(r.is_clean() && s.is_clean());
    assert_eq!(r.content, s.content);
}

const PRIOR_BASE: &str = "import os\nfrom pkg.old.base import Distribution\nfrom pkg.transforms import (\n    Scale,\n    Offset,\n)\n\n\nclass Prior(Distribution):\n    def names(self):\n        return os.sep\n";

fn prior_sides() -> (String, String) {
    let ours = PRIOR_BASE
        .replace(
            "from pkg.old.base import Distribution\n",
            "import equinox as eqx\n",
        )
        .replace("    Offset,\n", "    Offset,\n    Rayleigh,\n")
        .replace("class Prior(Distribution):", "class Prior(eqx.Module):");
    let theirs = PRIOR_BASE
        .replace("pkg.old.base", "pkg.new.base")
        .replace("    Offset,\n", "    Offset,\n    Sine,\n");
    (ours, theirs)
}

/// One side's refactor stopped using a name and deleted its import; the
/// other moved the import to a new module path. No code of the merge uses the
/// name, so the deletion stands.
#[test]
fn an_import_a_refactor_deleted_is_not_brought_back_by_a_path_change() {
    let (ours, theirs) = prior_sides();
    for (x, y) in [(&ours, &theirs), (&theirs, &ours)] {
        let r = entity_merge(PRIOR_BASE, x, y, "prior.py");
        if r.is_clean() {
            assert!(!r.content.contains("import Distribution"), "{}", r.content);
            assert!(
                r.content.contains("class Prior(eqx.Module):"),
                "{}",
                r.content
            );
            assert!(r.content.contains("Rayleigh") && r.content.contains("Sine"));
        }
    }
}

/// …and where the other side's code does use it, the import is needed.
#[test]
fn an_import_the_other_side_still_uses_is_kept() {
    let (ours, theirs) = prior_sides();
    let theirs = format!("{theirs}\n\ndef make():\n    return Distribution()\n");
    for (x, y) in [(&ours, &theirs), (&theirs, &ours)] {
        let r = entity_merge(PRIOR_BASE, x, y, "prior.py");
        if r.is_clean() {
            assert!(
                r.content.contains("from pkg.new.base import Distribution"),
                "{}",
                r.content
            );
        }
    }
}
