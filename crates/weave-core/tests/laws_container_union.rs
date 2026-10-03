//! Law witnesses for the container insertion union (`D3 container insertion
//! union`): two sides that each insert a run of new elements at ONE point of
//! one container.
//!
//! Two families of container:
//!
//! - **sets by the language** — a Python dict literal, `__all__`, a TS object
//!   literal, a TS export list, the keys of an INI section. Merged with no
//!   declaration.
//! - **sequences** — a Python list, a function body's statements, match arms,
//!   the lines of an INI multi-line value, a YAML sequence, a TS array. Order
//!   is meaning, so they merge only where the file's `weave-set` declaration
//!   names them (granted here through [`Host::set_attribute`]); without it,
//!   or with a declaration naming another container, they conflict.
//!
//! Stated at the public boundary (`entity_merge_fmt`) only:
//!
//!   C1  Identity:        merge(b, b, t) = t   and   merge(b, o, b) = o
//!   C2  Idempotence:     merge(b, x, x) = x
//!   C3  Commutativity:   merge(b, o, t) is clean and byte-identical to
//!                        merge(b, t, o) — commutative up to the canonical
//!                        order, which is a function of the elements, not
//!                        of the side's name
//!   C4  Union:           every base element, and every element either side
//!                        inserted, is in the merge exactly once; base
//!                        elements in base order; each side's in that
//!                        side's order; the merge parses
//!   C5  Confluence:      branches that each insert one element at one point,
//!                        merged one after another, give one tree whatever
//!                        order they merge in
//!   C6  Fail closed:     a sequence without a declaration (or declared under
//!                        another name); one key inserted by both sides with
//!                        different values; an existing element edited or
//!                        deleted beside the insertion; an existing element
//!                        moved to the insertion point — a conflict
//!
//! The domain is synthetic: sentinel-tagged elements, so which side wrote
//! what is decidable by substring. Base writes 1xxxx, ours 4xxxx, theirs
//! 5xxxx, both 6xxxx.

use proptest::prelude::*;
use weave_core::host::Host;
use weave_core::{entity_merge_fmt, MarkerFormat, MergeResult, ResolutionStrategy};

// ===========================================================================
// The arbitrary domain
// ===========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Surface {
    PyDict,
    PyAll,
    TsObject,
    TsExport,
    IniKeys,
    // Sequences: merged only by declaration.
    PyList,
    PyStatements,
    PyMatch,
    IniValueLines,
    YamlSequence,
    TsArray,
}

const SETS: [Surface; 5] = [
    Surface::PyDict,
    Surface::PyAll,
    Surface::TsObject,
    Surface::TsExport,
    Surface::IniKeys,
];

const SEQUENCES: [Surface; 6] = [
    Surface::PyList,
    Surface::PyStatements,
    Surface::PyMatch,
    Surface::IniValueLines,
    Surface::YamlSequence,
    Surface::TsArray,
];

impl Surface {
    fn path(self) -> &'static str {
        match self {
            Surface::PyDict => "registry.py",
            Surface::PyAll => "package.py",
            Surface::TsObject => "handlers.ts",
            Surface::TsExport => "index.ts",
            Surface::IniKeys => "settings.ini",
            Surface::PyList => "choices.py",
            Surface::PyStatements => "commands.py",
            Surface::PyMatch => "dispatch.py",
            Surface::IniValueLines => "entry.cfg",
            Surface::YamlSequence => "hooks.yaml",
            Surface::TsArray => "list.ts",
        }
    }

    fn is_set(self) -> bool {
        SETS.contains(&self)
    }
}

/// The declaration the test host grants: every sequence surface's container
/// by name, and the YAML file whole.
fn declared(path: &str) -> Option<String> {
    if path.ends_with(".yaml") {
        Some("set".to_string())
    } else {
        Some("CHOICES,build,dispatch,console_scripts,LIST".to_string())
    }
}

/// A declaration of a container none of these files has.
fn declared_elsewhere(_: &str) -> Option<String> {
    Some("SOMETHING_ELSE".to_string())
}

fn host(declaring: bool) -> Host {
    Host {
        set_attribute: declaring.then_some(declared as fn(&str) -> Option<String>),
        ..Host::default()
    }
}

fn merge_with(h: &Host, b: &str, o: &str, t: &str, path: &str) -> MergeResult {
    entity_merge_fmt(b, o, t, path, &MarkerFormat::default(), h)
}

/// The host a surface merges under when its union is expected.
fn merge(s: Surface, b: &str, o: &str, t: &str) -> MergeResult {
    merge_with(&host(!s.is_set()), b, o, t, s.path())
}

/// An element: its sentinel, and whether it takes its long form (several
/// lines, or a comment above it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Item {
    sentinel: u32,
    long: bool,
}

#[derive(Debug, Clone)]
struct Case {
    surface: Surface,
    base: Vec<Item>,
    at: usize,
    ours: Vec<Item>,
    theirs: Vec<Item>,
}

fn item_text(s: Surface, it: Item) -> String {
    let n = it.sentinel;
    match (s, it.long) {
        (Surface::PyDict, false) => format!("    \"k{n}\": {n},\n"),
        (Surface::PyDict, true) => format!("    \"k{n}\": [\n        {n},\n    ],\n"),
        (Surface::PyAll, false) => format!("    \"n{n}\",\n"),
        (Surface::PyAll, true) => format!("    # exported for {n}\n    \"n{n}\",\n"),
        (Surface::TsObject, false) => format!("  k{n}: {n},\n"),
        (Surface::TsObject, true) => format!("  k{n}: {{\n    id: {n},\n  }},\n"),
        (Surface::TsExport, false) => format!("  n{n},\n"),
        (Surface::TsExport, true) => format!("  // re-exported for {n}\n  n{n},\n"),
        (Surface::IniKeys, false) => format!("k{n} = {n}\n"),
        (Surface::IniKeys, true) => format!("k{n} =\n    v{n}\n"),
        (Surface::PyList, false) => format!("    \"v{n}\",\n"),
        (Surface::PyList, true) => format!("    (\n        \"v{n}\",\n        {n},\n    ),\n"),
        (Surface::PyStatements, false) => format!("    sub.add(\"s{n}\")\n"),
        (Surface::PyStatements, true) => format!("    sub.add(\n        \"s{n}\",\n    )\n"),
        (Surface::PyMatch, false) => format!("        case \"c{n}\":\n            return {n}\n"),
        (Surface::PyMatch, true) => {
            format!("        case \"c{n}\":\n            x = {n}\n            return x\n")
        }
        (Surface::IniValueLines, _) => format!("    e{n} = pkg.e{n}:main\n"),
        (Surface::YamlSequence, false) => format!("- id: h{n}\n  entry: h{n}\n"),
        (Surface::YamlSequence, true) => format!("- id: h{n}\n  entry: h{n}\n  args: [\"{n}\"]\n"),
        (Surface::TsArray, false) => format!("  \"v{n}\",\n"),
        (Surface::TsArray, true) => format!("  {{\n    v: {n},\n  }},\n"),
    }
}

fn render(s: Surface, items: &[Item]) -> String {
    let body: String = items.iter().map(|i| item_text(s, *i)).collect();
    match s {
        Surface::PyDict => format!(
            "HANDLERS = {{\n{body}}}\n\n\ndef lookup(name):\n    return HANDLERS[name]\n"
        ),
        Surface::PyAll => format!("__all__ = [\n{body}]\n"),
        Surface::TsObject => format!(
            "export const HANDLERS = {{\n{body}}};\n\nexport function lookup(name: string) {{\n  return name;\n}}\n"
        ),
        Surface::TsExport => format!("export {{\n{body}}} from \"./names\";\n"),
        Surface::IniKeys => {
            format!("[metadata]\nname = demo\n\n[tool]\n{body}\n[other]\nx = 1\n")
        }
        Surface::PyList => format!("CHOICES = [\n{body}]\n"),
        Surface::PyStatements => format!("def build(sub):\n{body}    return sub\n"),
        Surface::PyMatch => format!(
            "def dispatch(x):\n    match x:\n{body}        case _:\n            return 0\n"
        ),
        Surface::IniValueLines => format!(
            "[metadata]\nname = demo\n\n[options.entry_points]\nconsole_scripts =\n{body}\n[other]\nx = 1\n"
        ),
        Surface::YamlSequence => format!("# hooks\n{body}"),
        Surface::TsArray => format!("export const LIST = [\n{body}];\n"),
    }
}

fn side(case: &Case, run: &[Item]) -> Vec<Item> {
    let mut v = case.base.clone();
    for (k, it) in run.iter().enumerate() {
        v.insert(case.at + k, *it);
    }
    v
}

fn texts(case: &Case) -> (String, String, String) {
    let s = case.surface;
    (
        render(s, &case.base),
        render(s, &side(case, &case.ours)),
        render(s, &side(case, &case.theirs)),
    )
}

fn run_strategy(offset: u32) -> impl Strategy<Value = Vec<Item>> {
    prop::collection::btree_set(0u32..90, 1..4).prop_flat_map(move |ids| {
        let n = ids.len();
        (Just(ids), prop::collection::vec(any::<bool>(), n)).prop_map(move |(ids, long)| {
            ids.into_iter()
                .zip(long)
                .map(|(i, long)| Item {
                    sentinel: offset + 100 * i,
                    long,
                })
                .collect::<Vec<_>>()
        })
    })
}

/// A base of 0–3 elements and one insertion point; each side's run, with one
/// element both wrote spliced into both at arbitrary places when `shared` is
/// set.
fn case_strategy(surface: Surface) -> impl Strategy<Value = Case> {
    // A YAML sequence and the match arms need a base element to insert
    // beside (an empty sequence is `[]`; a match needs a case).
    let min = match surface {
        Surface::YamlSequence | Surface::PyMatch => 1,
        _ => 0,
    };
    (min..4usize)
        .prop_flat_map(move |k| {
            (
                Just(k),
                0..=k,
                run_strategy(40000),
                run_strategy(50000),
                any::<bool>(),
                any::<prop::sample::Index>(),
                any::<prop::sample::Index>(),
            )
        })
        .prop_map(move |(k, at, mut ours, mut theirs, shared, i, j)| {
            let base = (0..k as u32)
                .map(|n| Item {
                    sentinel: 10000 + 100 * n,
                    long: false,
                })
                .collect();
            // The YAML sequence goes through the data union, which refuses
            // one line written by both beside distinct ones.
            if shared && surface != Surface::YamlSequence {
                let both = Item {
                    sentinel: 60000,
                    long: false,
                };
                ours.insert(i.index(ours.len() + 1), both);
                theirs.insert(j.index(theirs.len() + 1), both);
            }
            Case {
                surface,
                base,
                at,
                ours,
                theirs,
            }
        })
}

fn any_set_case() -> impl Strategy<Value = Case> {
    prop::sample::select(SETS.to_vec()).prop_flat_map(case_strategy)
}

fn any_sequence_case() -> impl Strategy<Value = Case> {
    prop::sample::select(SEQUENCES.to_vec()).prop_flat_map(case_strategy)
}

fn any_case() -> impl Strategy<Value = Case> {
    prop_oneof![any_set_case(), any_sequence_case()]
}

/// The sentinels of `text`, in order of first appearance.
fn sentinels(text: &str) -> Vec<u32> {
    let mut out = Vec::new();
    let b = text.as_bytes();
    let mut i = 0;
    while i + 5 <= b.len() {
        let w = &b[i..i + 5];
        let bounded = (i == 0 || !b[i - 1].is_ascii_digit())
            && (i + 5 == b.len() || !b[i + 5].is_ascii_digit());
        if bounded && w.iter().all(u8::is_ascii_digit) {
            let n: u32 = std::str::from_utf8(w).unwrap().parse().unwrap();
            if n >= 10000 && !out.contains(&n) {
                out.push(n);
            }
            i += 5;
        } else {
            i += 1;
        }
    }
    out
}

/// How many times the element for `sentinel` is stated.
fn stated(s: Surface, text: &str, n: u32) -> usize {
    let needle = match s {
        Surface::PyDict => format!("\"k{n}\":"),
        Surface::PyAll => format!("\"n{n}\""),
        Surface::TsExport => format!("n{n},"),
        Surface::TsObject => format!("k{n}:"),
        Surface::IniKeys => format!("k{n} ="),
        Surface::PyList | Surface::TsArray => {
            return text.matches(&format!("\"v{n}\"")).count()
                + text.matches(&format!("v: {n},")).count()
        }
        Surface::PyStatements => format!("\"s{n}\""),
        Surface::PyMatch => format!("case \"c{n}\":"),
        Surface::IniValueLines => format!("e{n} ="),
        Surface::YamlSequence => format!("- id: h{n}\n"),
    };
    text.matches(&needle).count()
}

/// The merge still reads as its format: YAML loads; the others are checked
/// by the rule's own parse (a Python/TS answer that does not parse is
/// refused before it gets here), so only line structure is asked of them.
fn loads(s: Surface, text: &str) -> bool {
    match s {
        Surface::YamlSequence => serde_yaml::from_str::<serde_yaml::Value>(text).is_ok(),
        _ => !text.contains("<<<<<<<"),
    }
}

/// The container insertion union settled this merge.
fn settled_by_the_union(r: &MergeResult) -> bool {
    r.is_clean()
        && r.audit.iter().any(|a| {
            matches!(&a.resolution, ResolutionStrategy::RuleSettled { rules }
                if rules.iter().any(|x| x == "D3 container insertion union"))
        })
}

/// A guard's case: never settled by the union, and a conflict — except in a
/// TS object literal or export list, which the entity pipeline already
/// merges member by member before any rule runs (an edited member and an
/// added one are two members; a moved one is the same member). That was so
/// before this rule, and is not its answer.
fn refused(s: Surface, r: &MergeResult) -> bool {
    let member_wise = matches!(s, Surface::TsObject | Surface::TsExport);
    !settled_by_the_union(r) && (member_wise || !r.is_clean())
}

fn is_subsequence(needle: &[u32], hay: &[u32]) -> bool {
    let mut it = hay.iter();
    needle.iter().all(|n| it.any(|h| h == n))
}

// ===========================================================================
// C1 — Identity
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn c1_identity(case in any_case()) {
        let (b, o, t) = texts(&case);
        let s = case.surface;
        let left = merge(s, &b, &b, &t);
        prop_assert!(left.is_clean());
        prop_assert_eq!(&left.content, &t);
        let right = merge(s, &b, &o, &b);
        prop_assert!(right.is_clean());
        prop_assert_eq!(&right.content, &o);
    }
}

// ===========================================================================
// C2 — Idempotence
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn c2_idempotence(case in any_case()) {
        let (b, o, _) = texts(&case);
        let r = merge(case.surface, &b, &o, &o);
        prop_assert!(r.is_clean());
        prop_assert_eq!(&r.content, &o);
    }
}

// ===========================================================================
// C3 + C4 — Commutativity and union
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn c3_c4_same_point_insertions_commute_to_the_union(case in any_case()) {
        let (b, o, t) = texts(&case);
        prop_assume!(o != t);
        let s = case.surface;
        let r = merge(s, &b, &o, &t);
        prop_assert!(r.is_clean(), "{:?}: same-point insertions of distinct elements must settle:\n{}", s, r.content);
        let swapped = merge(s, &b, &t, &o);
        prop_assert!(swapped.is_clean());
        prop_assert_eq!(&swapped.content, &r.content, "the answer must not depend on which side is ours");
        prop_assert!(loads(s, &r.content), "the merge must load:\n{}", r.content);

        let order = sentinels(&r.content);
        let mut every: Vec<u32> = case.base.iter().chain(&case.ours).chain(&case.theirs).map(|i| i.sentinel).collect();
        every.sort_unstable();
        every.dedup();
        for n in &every {
            prop_assert_eq!(stated(s, &r.content, *n), 1, "element {} must be stated once:\n{}", n, r.content);
        }
        let base: Vec<u32> = case.base.iter().map(|i| i.sentinel).collect();
        prop_assert!(is_subsequence(&base, &order), "base order:\n{}", r.content);
        for run in [&case.ours, &case.theirs] {
            let own: Vec<u32> = run.iter().map(|i| i.sentinel).filter(|n| *n != 60000).collect();
            prop_assert!(is_subsequence(&own, &order), "a side's order:\n{}", r.content);
        }
    }
}

// ===========================================================================
// C5 — Confluence of a fleet
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    /// Four branches each insert one element after the same base element;
    /// merged one after another (each merge against the base), two merge
    /// orders give one tree.
    #[test]
    fn c5_a_fleet_lands_on_one_tree(
        s in prop::sample::select(SETS.iter().copied().chain([Surface::PyList, Surface::PyStatements, Surface::PyMatch, Surface::IniValueLines, Surface::TsArray]).collect::<Vec<_>>()),
        ids in prop::collection::btree_set(0u32..90, 4..5),
        perm in Just(vec![0usize, 1, 2, 3]).prop_shuffle(),
    ) {
        let base = vec![Item { sentinel: 10000, long: false }];
        let lanes: Vec<String> = ids.iter().map(|i| {
            let mut v = base.clone();
            v.push(Item { sentinel: 40000 + 100 * i, long: false });
            render(s, &v)
        }).collect();
        let b = render(s, &base);
        let fold = |order: &[usize]| -> String {
            let mut state = b.clone();
            for &k in order {
                let r = merge(s, &b, &state, &lanes[k]);
                assert!(r.is_clean(), "{s:?}:\n{}", r.content);
                state = r.content;
            }
            state
        };
        prop_assert_eq!(fold(&[0, 1, 2, 3]), fold(&perm));
    }
}

// ===========================================================================
// C6 — Fail closed
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    /// A sequence is a set only by declaration: with none, or with one that
    /// names another container, two insertions at one point conflict.
    #[test]
    fn c6_an_undeclared_sequence_conflicts(case in any_sequence_case()) {
        let (b, o, t) = texts(&case);
        prop_assume!(o != t);
        let path = case.surface.path();
        prop_assert!(!merge_with(&Host::default(), &b, &o, &t, path).is_clean());
        let elsewhere = Host { set_attribute: Some(declared_elsewhere), ..Host::default() };
        prop_assert!(!merge_with(&elsewhere, &b, &o, &t, path).is_clean());
    }

    /// One key inserted by both sides with different values.
    #[test]
    fn c6_one_key_two_values_conflicts(case in any_set_case()) {
        let mut case = case;
        let k = Item { sentinel: 70000, long: false };
        case.ours.push(k);
        case.theirs.push(k);
        let (b, o, t) = texts(&case);
        let t = match case.surface {
            Surface::PyDict => t.replace("\"k70000\": 70000", "\"k70000\": 71111"),
            Surface::TsObject => t.replace("k70000: 70000", "k70000: 71111"),
            Surface::IniKeys => t.replace("k70000 = 70000", "k70000 = 71111"),
            // A name list has no value: the same name twice is one name, and
            // a different spelling of it is a different key.
            Surface::PyAll => t.replace("    \"n70000\",\n", "    \"n70000\",  # again\n"),
            _ => t.replace("  n70000,\n", "  n70000, // again\n"),
        };
        let s = case.surface;
        prop_assert!(!merge(s, &b, &o, &t).is_clean());
        prop_assert!(!merge(s, &b, &t, &o).is_clean());
    }

    /// An existing element edited, or deleted, beside the insertion point.
    #[test]
    fn c6_a_neighbour_edited_or_deleted_conflicts(case in any_set_case(), delete in any::<bool>()) {
        prop_assume!(case.at > 0);
        let (b, o, t) = texts(&case);
        let s = case.surface;
        let before = item_text(s, case.base[case.at - 1]);
        let n = case.base[case.at - 1].sentinel;
        let o = if delete {
            o.replacen(&before, "", 1)
        } else {
            o.replacen(&before, &before.replacen(&format!("{n}"), &format!("{}", n + 1), 1), 1)
        };
        prop_assert!(refused(s, &merge(s, &b, &o, &t)));
        prop_assert!(refused(s, &merge(s, &b, &t, &o)));
    }

    /// An existing element moved to the insertion point by one side while
    /// the other inserts there: a move is not an insertion.
    #[test]
    fn c6_an_element_moved_to_the_insertion_point_conflicts(
        s in prop::sample::select(SETS.to_vec()),
        theirs in run_strategy(50000),
    ) {
        let base: Vec<Item> = (0..3).map(|n| Item { sentinel: 10000 + 100 * n, long: false }).collect();
        let mut moved = base.clone();
        let first = moved.remove(0);
        moved.push(first);
        let mut added = base.clone();
        added.extend(theirs);
        let (b, o, t) = (render(s, &base), render(s, &moved), render(s, &added));
        prop_assert!(refused(s, &merge(s, &b, &o, &t)));
        prop_assert!(refused(s, &merge(s, &b, &t, &o)));
    }
}

// ===========================================================================
// Controls: the observations can fail
// ===========================================================================

#[test]
fn control_stated_counts_a_duplicate() {
    let text = "    \"k40000\": 40000,\n    \"k40000\": 40000,\n";
    assert_eq!(stated(Surface::PyDict, text, 40000), 2);
    assert!(!is_subsequence(&[2, 1], &[1, 2]));
}

#[test]
fn control_every_surface_renders_a_file_that_merges_its_own_edits() {
    // One side only: no union needed, the merge is that side.
    for s in SETS.iter().chain(&SEQUENCES) {
        let base = vec![Item {
            sentinel: 10000,
            long: false,
        }];
        let mut one = base.clone();
        one.push(Item {
            sentinel: 40000,
            long: true,
        });
        let (b, o) = (render(*s, &base), render(*s, &one));
        let r = merge(*s, &b, &o, &b);
        assert!(r.is_clean(), "{s:?}");
        assert_eq!(r.content, o);
    }
}
