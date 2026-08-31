//! Law witnesses for the entity-level 3-way merge (`entity_merge`).
//!
//! The structure under test: `entity_merge(base, ours, theirs, path)` is a
//! *total, deterministic* function realizing, per matched entity, the
//! **conservative three-way merge cell function**
//!
//!     cell(b, o, t) = o        if o = t
//!                   = t        if o = b        (a side that changed nothing
//!                   = o        if t = b         asserted nothing)
//!                   = CONFLICT otherwise
//!
//! lifted pointwise over an *inferred* keying of the file into entities
//! (functions / classes / JSON keys), with the conflict case reified as data
//! (an `EntityConflict` + rendered markers) rather than as failure. This is
//! diff3's "stable/maximal merge" of Khanna–Kunal–Pierce (*A Formal
//! Investigation of Diff3*, FSTTCS 2007) computed at entity rather than line
//! granularity; categorically, the clean cases are the pushout of
//! `ours ← base → theirs` in a category of keyed edits, and the conflict
//! cases are the formal colimit that exists only in the free cocompletion
//! (Mimram–Di Giusto, *A Categorical Theory of Patches*, ENTCS 2013). It is
//! NOT a CRDT: there is no associativity, no ancestor-free join, and no
//! convergence claim — the guarantees are exactly the laws below.
//!
//! Laws witnessed here, at the public boundary only (`weave_core::entity_merge`,
//! `MergeResult { content, conflicts, .. }`), never against internals:
//!
//!   L1  Identity:            merge(b, b, t) = t   and   merge(b, o, b) = o        GREEN
//!   L2  Idempotence:         merge(b, x, x) = x                                   GREEN
//!                            (RED corner: marker-bearing base, see red_l2_*)
//!   L2b Absorption:          m = merge(b,o,t) clean ⇒ merge(b, o, m) ~ m
//!                            GREEN for ~ = same surviving entity set;
//!                            RED at byte level (divergent whitespace) and at
//!                            line level (TS duplication) — see red_l2b_*, red_l8_*
//!   L3  Commutativity:       merge(b,o,t) ≅ merge(b,t,o)                          GREEN
//!                            (byte-identical on the disjoint code domain)
//!   L4  No silent loss:      every body a side wrote is visible in the output
//!                            or in the conflict set  (THE safety law)             GREEN
//!   L5  Conflict soundness:  disjoint entity edits merge clean                    GREEN (code)
//!                            RED for JSON (separator-comma leak, red_l5_json_*)
//!   L5b Conflict completeness: divergent same-name additions conflict             GREEN
//!                            (RED corner: TS rename-steal, red_l5b_*)
//!   L6  Determinism:         same inputs ⇒ identical result, repeatedly           GREEN
//!   L7  Totality:            no panic on arbitrary input triples                  GREEN
//!   L8  Linearity:           a clean merge defines each name at most once         GREEN (py/json)
//!                            RED for TS (rename inference steals one side of a
//!                            both-added pair and emits the name twice, red_l8_*)
//!
//! Domain conditions (documented boundaries, not violations):
//!   - inputs free of conflict markers (`<<<<<<<`/`>>>>>>>`): pre-conflicted
//!     inputs are refused by design before any law applies;
//!   - base non-empty: an empty base with both sides creating the file is the
//!     `BothCreated` gate (issue #51) and routes to line-level diff3, which
//!     conflicts even on disjoint additions — pinned below as a boundary test;
//!   - text under 1MB, non-binary.
//!
//! Every property carries a non-vacuity guarantee: the generators are built so
//! each side really writes something (asserted or filtered inside the
//! property), and the observation functions are themselves proven RED-capable
//! by the `control_*` tests at the bottom (a checker that cannot fail is not
//! a witness).
//!
//! Full findings: see `laws_merge_NOTES.md` next to this file.

use std::collections::{BTreeMap, BTreeSet};

use proptest::prelude::*;
use weave_core::{entity_merge, MergeResult};

// ===========================================================================
// Synthetic modules: the arbitrary domain
// ===========================================================================
//
// A module is an ordered list of (name, sentinel) entities rendered into one
// of three surfaces the registry parses: Python, TypeScript, JSON. Sentinels
// are unique 5-digit tokens, one per (entity, writer), so "this side's edit
// survived" is decidable by substring at the boundary without reading any
// internal representation:
//
//   base entity i                 : 10000 + 100*i
//   ours   modifies entity i      : 20000 + 100*i
//   theirs modifies entity i      : 30000 + 100*i
//   ours   adds (divergent) slot j: 40000 + 100*j
//   theirs adds (divergent) slot j: 50000 + 100*j
//   both   add (convergent) slot j: 60000 + 100*j   (same body on both sides)

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lang {
    Py,
    Ts,
    Json,
}

impl Lang {
    fn path(self) -> &'static str {
        match self {
            Lang::Py => "m.py",
            Lang::Ts => "m.ts",
            Lang::Json => "m.json",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Entity {
    name: String,
    sentinel: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Module {
    lang: Lang,
    entities: Vec<Entity>,
}

fn render(m: &Module) -> String {
    match m.lang {
        Lang::Py => m
            .entities
            .iter()
            .map(|e| format!("def {}():\n    return {}\n", e.name, e.sentinel))
            .collect::<Vec<_>>()
            .join("\n\n"),
        Lang::Ts => m
            .entities
            .iter()
            .map(|e| {
                format!(
                    "export function {}(): number {{\n  return {};\n}}\n",
                    e.name, e.sentinel
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Lang::Json => {
            let body = m
                .entities
                .iter()
                .map(|e| format!("  \"{}\": {}", e.name, e.sentinel))
                .collect::<Vec<_>>()
                .join(",\n");
            format!("{{\n{}\n}}\n", body)
        }
    }
}

fn merge_m(b: &Module, o: &Module, t: &Module) -> MergeResult {
    entity_merge(&render(b), &render(o), &render(t), b.lang.path())
}

// --- edit plans ------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Keep,
    Modify,
    Delete,
}

#[derive(Debug, Clone, Copy)]
enum SideTag {
    Ours,
    Theirs,
}

impl SideTag {
    fn modify_base(self) -> u32 {
        match self {
            SideTag::Ours => 20000,
            SideTag::Theirs => 30000,
        }
    }
    fn add_base(self) -> u32 {
        match self {
            SideTag::Ours => 40000,
            SideTag::Theirs => 50000,
        }
    }
}

/// One side's edit of the base: per-entity ops plus appended additions
/// carrying their final sentinel (so a *shared* name can be added by both
/// sides either convergently — same sentinel — or divergently).
#[derive(Debug, Clone)]
struct Plan {
    ops: Vec<Op>,
    adds: Vec<(String, u32)>,
}

impl Plan {
    fn is_noop(&self) -> bool {
        self.adds.is_empty() && self.ops.iter().all(|o| *o == Op::Keep)
    }
}

fn apply(base: &Module, side: SideTag, plan: &Plan) -> Module {
    let mut entities = Vec::new();
    for (i, e) in base.entities.iter().enumerate() {
        match plan.ops.get(i).copied().unwrap_or(Op::Keep) {
            Op::Keep => entities.push(e.clone()),
            Op::Modify => entities.push(Entity {
                name: e.name.clone(),
                sentinel: side.modify_base() + 100 * i as u32,
            }),
            Op::Delete => {}
        }
    }
    for (name, sentinel) in &plan.adds {
        entities.push(Entity {
            name: name.clone(),
            sentinel: *sentinel,
        });
    }
    Module {
        lang: base.lang,
        entities,
    }
}

// --- strategies ------------------------------------------------------------

fn any_lang() -> impl Strategy<Value = Lang> {
    prop_oneof![Just(Lang::Py), Just(Lang::Ts), Just(Lang::Json)]
}

/// A base module with 2..=5 uniquely named entities.
fn base_strategy(lang: impl Strategy<Value = Lang>) -> impl Strategy<Value = Module> {
    (lang, 2usize..=5).prop_map(|(lang, n)| Module {
        lang,
        entities: (0..n)
            .map(|i| Entity {
                name: format!("e{}", i),
                sentinel: 10000 + 100 * i as u32,
            })
            .collect(),
    })
}

/// An unconstrained plan: any op per entity, 0..=2 additions. An addition is
/// either side-private ("ox…"/"tx…"), shared-divergent ("sx…" with a
/// side-specific body), or shared-convergent ("sx…" with the SAME body on
/// both sides) — so the generator reaches AddedOneSide, AddedBothDivergent
/// and AddedBothConvergent cells, with and without concurrent deletes.
fn general_plan_strategy(n: usize, side: SideTag) -> impl Strategy<Value = Plan> {
    let prefix = match side {
        SideTag::Ours => "ox",
        SideTag::Theirs => "tx",
    };
    let op = prop_oneof![3 => Just(Op::Keep), 2 => Just(Op::Modify), 1 => Just(Op::Delete)];
    let add = (0u8..3).prop_flat_map(move |kind| {
        (0usize..2).prop_map(move |slot| match kind {
            0 => (format!("{}{}", prefix, slot), side.add_base() + 100 * slot as u32),
            1 => (format!("sx{}", slot), side.add_base() + 100 * slot as u32),
            _ => (format!("sx{}", slot), 60000 + 100 * slot as u32),
        })
    });
    (
        proptest::collection::vec(op, n),
        proptest::collection::vec(add, 0..=2),
    )
        .prop_map(|(ops, mut adds)| {
            adds.sort();
            adds.dedup_by(|a, b| a.0 == b.0);
            Plan { ops, adds }
        })
}

/// (base, ours, theirs) with arbitrary, possibly overlapping edits, where each
/// side made at least one real change and neither side's file is empty.
fn general_triple_strategy_with(
    lang: impl Strategy<Value = Lang>,
) -> impl Strategy<Value = (Module, Module, Module)> {
    base_strategy(lang).prop_flat_map(|base| {
        let n = base.entities.len();
        (
            Just(base),
            general_plan_strategy(n, SideTag::Ours),
            general_plan_strategy(n, SideTag::Theirs),
        )
            .prop_filter_map(
                "both sides must edit, neither may empty the file",
                |(base, po, pt)| {
                    if po.is_noop() || pt.is_noop() {
                        return None;
                    }
                    let ours = apply(&base, SideTag::Ours, &po);
                    let theirs = apply(&base, SideTag::Theirs, &pt);
                    if ours.entities.is_empty() || theirs.entities.is_empty() {
                        return None;
                    }
                    Some((base, ours, theirs))
                },
            )
    })
}

fn general_triple_strategy() -> impl Strategy<Value = (Module, Module, Module)> {
    general_triple_strategy_with(any_lang())
}

/// (base, ours, theirs) where the two sides touch DISJOINT entity sets:
/// entity 0 is untouched by both; every other base entity is owned by at most
/// one side; additions come from disjoint name pools. Each side makes at
/// least one change. `lang` selects the surface; every law here now passes
/// `any_lang()` — the domain restrictions the JSON comma leak and the TS
/// rename-steal used to force are gone with them.
fn disjoint_triple_strategy(
    lang: impl Strategy<Value = Lang>,
) -> impl Strategy<Value = (Module, Module, Module)> {
    base_strategy(lang).prop_flat_map(|base| {
        let n = base.entities.len();
        // owner per entity: 0 = untouched, 1 = ours, 2 = theirs; entity 0 untouched.
        let owners = proptest::collection::vec(0u8..3, n - 1).prop_map(|mut v| {
            v.insert(0, 0);
            v
        });
        let op = prop_oneof![Just(Op::Modify), Just(Op::Delete)];
        (
            Just(base),
            owners,
            proptest::collection::vec(op.clone(), n),
            proptest::collection::vec(op, n),
            0usize..=2, // ours adds
            0usize..=2, // theirs adds
        )
            .prop_map(|(base, owners, ops_o, ops_t, ao, at)| {
                let mk = |who: u8, side: SideTag, ops: &[Op], prefix: &str, count: usize| {
                    let mut plan = Plan {
                        ops: owners
                            .iter()
                            .zip(ops.iter())
                            .map(|(w, op)| if *w == who { *op } else { Op::Keep })
                            .collect(),
                        adds: (0..count)
                            .map(|j| {
                                (
                                    format!("{}{}", prefix, j),
                                    side.add_base() + 100 * j as u32,
                                )
                            })
                            .collect(),
                    };
                    if plan.is_noop() {
                        // guarantee at least one change per side
                        plan.adds
                            .push((format!("{}9", prefix), side.add_base() + 900));
                    }
                    plan
                };
                let po = mk(1, SideTag::Ours, &ops_o, "ox", ao);
                let pt = mk(2, SideTag::Theirs, &ops_t, "tx", at);
                let ours = apply(&base, SideTag::Ours, &po);
                let theirs = apply(&base, SideTag::Theirs, &pt);
                (base, ours, theirs)
            })
    })
}

// --- observations (boundary-only) ------------------------------------------

/// Sentinels a side wrote that are not base's: new entities, or new bodies for
/// base entities. These are exactly the tokens the safety law must conserve.
fn written_sentinels(base: &Module, side: &Module) -> BTreeSet<u32> {
    let base_map: BTreeMap<&str, u32> = base
        .entities
        .iter()
        .map(|e| (e.name.as_str(), e.sentinel))
        .collect();
    side.entities
        .iter()
        .filter(|e| base_map.get(e.name.as_str()) != Some(&e.sentinel))
        .map(|e| e.sentinel)
        .collect()
}

/// A sentinel is *visible* if it survives in the merged content (which
/// includes rendered conflict markers) or in the typed conflict payloads.
fn visible(r: &MergeResult, sentinel: u32) -> bool {
    let tok = sentinel.to_string();
    if r.content.contains(&tok) {
        return true;
    }
    r.conflicts.iter().any(|c| {
        [&c.ours_content, &c.theirs_content, &c.base_content]
            .iter()
            .any(|side| side.as_deref().is_some_and(|s| s.contains(&tok)))
    })
}

/// All 5-digit sentinel tokens present in a text — the entity-content image of
/// the output, used to compare two merges up to ordering and duplication.
fn sentinel_image(text: &str) -> BTreeSet<u32> {
    let bytes = text.as_bytes();
    let mut out = BTreeSet::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() && (i == 0 || !bytes[i - 1].is_ascii_digit()) {
            let mut j = i;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j - i == 5 {
                if let Ok(v) = text[i..j].parse::<u32>() {
                    out.insert(v);
                }
            }
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

/// Content and conflict payloads together: the total surviving image.
fn full_image(r: &MergeResult) -> BTreeSet<u32> {
    let mut img = sentinel_image(&r.content);
    for c in &r.conflicts {
        for side in [&c.ours_content, &c.theirs_content, &c.base_content] {
            if let Some(s) = side.as_deref() {
                img.extend(sentinel_image(s));
            }
        }
    }
    img
}

/// How many times the output *defines* a name, read the way the language
/// reads it.
fn definition_count(lang: Lang, text: &str, name: &str) -> usize {
    let needle = match lang {
        Lang::Py => format!("def {}(", name),
        Lang::Ts => format!("function {}(", name),
        Lang::Json => format!("\"{}\"", name),
    };
    text.matches(&needle).count()
}

fn names_present(lang: Lang, text: &str, name: &str) -> bool {
    definition_count(lang, text, name) > 0
}

// ===========================================================================
// L1 — Identity: merge(b, b, t) = t and merge(b, o, b) = o
// ===========================================================================
// The unit law of the cell function: a side that changed nothing asserted
// nothing. (Discharged in part by the engine's base==side fast path — the law
// is stated at the boundary, and the boundary includes the fast path. The
// pipeline-level identity, per entity rather than per file, is L5.)

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn l1_identity_unchanged_side_yields_other_side(
        (base, _ours, theirs) in general_triple_strategy()
    ) {
        let b = render(&base);
        let t = render(&theirs);
        prop_assume!(b != t); // non-vacuity: theirs really differs

        let left = entity_merge(&b, &b, &t, base.lang.path());
        prop_assert!(left.is_clean(), "identity merge must be clean");
        prop_assert_eq!(&left.content, &t, "merge(b, b, t) must be byte-identical to t");

        let right = entity_merge(&b, &t, &b, base.lang.path());
        prop_assert!(right.is_clean());
        prop_assert_eq!(&right.content, &t, "merge(b, o, b) must be byte-identical to o");
    }
}

// ===========================================================================
// L2 — Idempotence: merge(b, x, x) = x
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn l2_idempotence_agreeing_sides_win(
        (base, ours, _theirs) in general_triple_strategy()
    ) {
        let b = render(&base);
        let x = render(&ours);
        prop_assume!(b != x); // non-vacuity: x is a real change from b
        let r = entity_merge(&b, &x, &x, base.lang.path());
        prop_assert!(r.is_clean(), "agreeing sides can never conflict");
        prop_assert_eq!(&r.content, &x, "merge(b, x, x) must be byte-identical to x");
    }
}

// ===========================================================================
// L2b — Absorption: m = merge(b, o, t) clean  ⇒  merge(b, o, m) ~ m
// ===========================================================================
// The semilattice absorption law x ∨ (x ∨ y) = x ∨ y, restricted to the clean
// domain: the merged file already carries ours' whole edit, so re-merging
// ours against it must change nothing. This is the law the subsumption rule
// (subsumption.rs) exists to realize.
//
// GREEN at the entity-set level (~ = same surviving sentinel image, still
// clean) for PYTHON. Anything stronger — and TypeScript at any level — is
// RED today, in three independent ways, all pinned below:
//   * bytes: the blank-line gap merge is a SUM, not a join — each re-merge
//     widens the gap by two newlines and iteration diverges
//     (`red_l2b_absorption_bytes_diverge`);
//   * lines: TS rename inference can pair a deleted base entity with the
//     re-merged copy of an addition and emit the definition twice
//     (`red_l8_ts_rename_steal_duplicates_convergent_add`);
//   * verdicts: on TS the re-merge can even FABRICATE a RenameRename
//     conflict between two adds neither side renamed
//     (`red_l5_ts_delete_add_fabricates_rename_rename_conflict` — a
//     first-order triple, not only a re-merge shape).

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn l2b_absorption_remerge_is_clean_and_preserves_the_entity_image(
        (base, ours, theirs) in disjoint_triple_strategy(Just(Lang::Py))
    ) {
        let b = render(&base);
        let o = render(&ours);
        let t = render(&theirs);
        let m = entity_merge(&b, &o, &t, base.lang.path());
        prop_assume!(m.is_clean());
        prop_assume!(o != m.content); // non-vacuity: theirs contributed something

        let again = entity_merge(&b, &o, &m.content, base.lang.path());
        prop_assert!(again.is_clean(),
            "re-merging an input against the clean merge must not conflict\nm:\n{}\nagain conflicts: {:?}",
            m.content, again.conflicts);
        prop_assert_eq!(sentinel_image(&again.content), sentinel_image(&m.content),
            "re-merging an input must not add or drop entity bodies\nm:\n{}\nagain:\n{}",
            &m.content, &again.content);
    }

    /// The strict absorption law, preserved as a standing RED property: byte
    /// (and even line-multiset) idempotence of re-merge does not hold. Kept
    /// ignored so the suite stays green while the violation stays executable;
    /// un-ignore when the gap-join and the rename-steal are fixed.
    #[test]
    fn l2b_red_absorption_strict(
        (base, ours, theirs) in disjoint_triple_strategy(any_lang())
    ) {
        let b = render(&base);
        let o = render(&ours);
        let t = render(&theirs);
        let m = entity_merge(&b, &o, &t, base.lang.path());
        prop_assume!(m.is_clean());
        prop_assume!(o != m.content);
        let again = entity_merge(&b, &o, &m.content, base.lang.path());
        prop_assert!(again.is_clean());
        prop_assert_eq!(&again.content, &m.content,
            "merge(b, o, merge(b,o,t)) must be byte-identical to merge(b,o,t)");
    }
}

// ===========================================================================
// L3 — Commutativity of sides: merge(b, o, t) ≅ merge(b, t, o)
// ===========================================================================
// Two components with different strengths:
//   (a) over ARBITRARY (overlapping) triples, all three surfaces: swapping
//       sides may not change whether the merge is clean, nor the total set of
//       surviving entity bodies (content + conflict payloads);
//   (b) over the DISJOINT code domain the merge is clean both ways and the
//       output is BYTE-IDENTICAL: placement canonicalizes additions (base
//       order first, then additions in a side-independent order), so the
//       output imposes no ours-first bias. This was probed, not assumed.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn l3a_side_swap_preserves_cleanliness_and_surviving_content(
        (base, ours, theirs) in general_triple_strategy()
    ) {
        let b = render(&base);
        let o = render(&ours);
        let t = render(&theirs);
        let fwd = entity_merge(&b, &o, &t, base.lang.path());
        let rev = entity_merge(&b, &t, &o, base.lang.path());
        prop_assert_eq!(fwd.is_clean(), rev.is_clean(),
            "whether a merge conflicts must not depend on which side is 'ours'");
        prop_assert_eq!(full_image(&fwd), full_image(&rev),
            "the set of surviving entity bodies must not depend on side order");
    }

    #[test]
    fn l3b_disjoint_commutativity_is_byte_identical_on_code(
        (base, ours, theirs) in disjoint_triple_strategy(any_lang())
    ) {
        let b = render(&base);
        let o = render(&ours);
        let t = render(&theirs);
        let fwd = entity_merge(&b, &o, &t, base.lang.path());
        let rev = entity_merge(&b, &t, &o, base.lang.path());
        prop_assert!(fwd.is_clean() && rev.is_clean());
        prop_assert_eq!(&fwd.content, &rev.content,
            "disjoint edits: output must not depend on which side is 'ours'");
    }
}

// ===========================================================================
// L4 — NO SILENT LOSS (the safety law)
// ===========================================================================
// Every body a side wrote (a modified base entity or a new entity) either
// survives into the merged content or is carried by the conflict set. Over
// the FULL generator — overlapping edits, deletes against edits, colliding
// same-name additions, all three surfaces — not just the polite disjoint
// domain.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    #[test]
    fn l4_no_silent_loss(
        (base, ours, theirs) in general_triple_strategy()
    ) {
        let r = merge_m(&base, &ours, &theirs);
        let wo = written_sentinels(&base, &ours);
        let wt = written_sentinels(&base, &theirs);
        for s in wo.iter().chain(wt.iter()) {
            prop_assert!(
                visible(&r, *s),
                "sentinel {} written by a side is in neither content nor conflicts\n\
                 base:\n{}\nours:\n{}\ntheirs:\n{}\nmerged:\n{}\nconflicts: {:?}",
                s, render(&base), render(&ours), render(&theirs), r.content, r.conflicts
            );
        }
        // Non-vacuity: discard the (rare) pure-deletion cases where the law
        // is vacuous, so the reported case count is a count of real checks.
        prop_assume!(!wo.is_empty() || !wt.is_empty());
    }
}

// ===========================================================================
// L5 — Conflict soundness on the disjoint code domain
// ===========================================================================
// Within the supported domain (non-empty base, parseable code file, unique
// names), edits touching disjoint entity sets merge CLEAN: no false
// conflicts, both sides' writes present, both sides' deletions honored.
//
// Domain note: JSON is excluded — semantically disjoint JSON edits can
// collide on the separator comma and falsely conflict; that violation is
// preserved RED in `red_l5_json_disjoint_add_forces_comma_conflict`. The
// empty-base boundary is pinned in
// `boundary_both_created_conflicts_but_loses_nothing`.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    #[test]
    fn l5_disjoint_edits_merge_clean_and_complete(
        (base, ours, theirs) in disjoint_triple_strategy(any_lang())
    ) {
        let r = merge_m(&base, &ours, &theirs);
        prop_assert!(
            r.is_clean(),
            "disjoint edits must not conflict\nbase:\n{}\nours:\n{}\ntheirs:\n{}\nout:\n{}\nconflicts: {:?}",
            render(&base), render(&ours), render(&theirs), r.content, r.conflicts
        );
        prop_assert!(!r.content.contains("<<<<<<<"),
            "clean merge must not contain conflict markers");

        // every write from both sides survives in the content itself
        let wo = written_sentinels(&base, &ours);
        let wt = written_sentinels(&base, &theirs);
        prop_assert!(!wo.is_empty() || !wt.is_empty() ||
            ours.entities.len() < base.entities.len() ||
            theirs.entities.len() < base.entities.len(),
            "non-vacuity: generator produced a change-free side");
        for s in wo.iter().chain(wt.iter()) {
            prop_assert!(r.content.contains(&s.to_string()),
                "clean merge dropped sentinel {}\nout:\n{}", s, r.content);
        }

        // every deletion is honored: an entity deleted by its owning side and
        // untouched by the other must not reappear
        let ours_names: BTreeSet<&str> =
            ours.entities.iter().map(|e| e.name.as_str()).collect();
        let theirs_names: BTreeSet<&str> =
            theirs.entities.iter().map(|e| e.name.as_str()).collect();
        for e in &base.entities {
            let deleted_ours = !ours_names.contains(e.name.as_str());
            let deleted_theirs = !theirs_names.contains(e.name.as_str());
            if deleted_ours || deleted_theirs {
                prop_assert!(
                    !names_present(base.lang, &r.content, &e.name),
                    "entity {} was deleted by one side (untouched by the other) \
                     but survives in the output:\n{}",
                    e.name, r.content
                );
            }
        }
    }
}

// ===========================================================================
// L5b — Conflict completeness: divergent same-name additions conflict
// ===========================================================================
// The dual of L5: when both sides ADD the same name with different bodies and
// touch nothing else, the contradiction must be reported, not resolved. Holds
// on all three surfaces when no concurrent delete is present; the TS corner
// where a concurrent delete lets rename inference swallow one side is RED in
// `red_l5b_ts_divergent_add_conflict_suppressed`.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn l5b_divergent_same_name_additions_conflict(
        base in base_strategy(any_lang()),
        slot in 0usize..2
    ) {
        let add_o = Plan { ops: vec![Op::Keep; base.entities.len()],
            adds: vec![(format!("sx{}", slot), 40000 + 100 * slot as u32)] };
        let add_t = Plan { ops: vec![Op::Keep; base.entities.len()],
            adds: vec![(format!("sx{}", slot), 50000 + 100 * slot as u32)] };
        let ours = apply(&base, SideTag::Ours, &add_o);
        let theirs = apply(&base, SideTag::Theirs, &add_t);
        let r = merge_m(&base, &ours, &theirs);
        prop_assert!(!r.is_clean(),
            "both sides adding '{}' with different bodies is a contradiction and must conflict\nout:\n{}",
            format!("sx{}", slot), r.content);
        // and the safety law still holds across the conflict
        prop_assert!(visible(&r, 40000 + 100 * slot as u32));
        prop_assert!(visible(&r, 50000 + 100 * slot as u32));
    }
}

// ===========================================================================
// L6 — Determinism: same three inputs ⇒ identical result
// ===========================================================================
// Repeated in-process calls construct fresh HashMaps with fresh RandomState,
// so a dependence on hash iteration order WOULD show up here.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn l6_determinism(
        (base, ours, theirs) in general_triple_strategy()
    ) {
        let r1 = merge_m(&base, &ours, &theirs);
        let r2 = merge_m(&base, &ours, &theirs);
        let r3 = merge_m(&base, &ours, &theirs);
        prop_assert_eq!(&r1.content, &r2.content, "content must be byte-identical across calls");
        prop_assert_eq!(&r2.content, &r3.content);
        prop_assert_eq!(format!("{:?}", r1.conflicts), format!("{:?}", r2.conflicts),
            "conflict records must be identical across calls");
        prop_assert_eq!(format!("{:?}", r1.audit), format!("{:?}", r2.audit),
            "audit must be identical across calls");
    }
}

// ===========================================================================
// L7 — Totality: never panics
// ===========================================================================
// (a) hostile free-form strings, including unicode, partial markers, braces;
// (b) generated modules with random byte-level corruption.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn l7a_totality_arbitrary_strings(
        base in ".{0,200}",
        ours in ".{0,200}",
        theirs in ".{0,200}",
        path_idx in 0usize..5
    ) {
        let path = ["m.py", "m.ts", "m.json", "m.rs", "m.txt"][path_idx];
        // the law is precisely that this call returns
        let r = entity_merge(&base, &ours, &theirs, path);
        // and its verdict is internally coherent
        prop_assert_eq!(r.is_clean(), r.conflicts.is_empty());
    }

    #[test]
    fn l7b_totality_corrupted_modules(
        (base, ours, theirs) in general_triple_strategy(),
        pos in 0usize..1000,
        junk in prop::sample::select(vec!["}", "{", "def ", "\"", "<<<<<<<", "\u{feff}", "\r\n", "):", "]"])
    ) {
        let mut o = render(&ours);
        let cut = o
            .char_indices()
            .map(|(i, _)| i)
            .chain(std::iter::once(o.len()))
            .nth(pos % (o.chars().count() + 1))
            .unwrap_or(o.len());
        o.insert_str(cut, junk);
        let _ = entity_merge(&render(&base), &o, &render(&theirs), base.lang.path());
    }
}

// ===========================================================================
// L8 — Linearity: a clean merge defines each name at most once
// ===========================================================================
// The pipeline's own documented invariant ("claims are linear: an entity
// cannot be emitted twice", v2/mod.rs). Witnessed over the general domain —
// including convergent and divergent same-name additions racing concurrent
// deletes — for Python and JSON. TypeScript VIOLATES it (rename inference
// steals one side of a both-added pair; the name is emitted twice in a clean
// merge): pinned RED below.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    #[test]
    fn l8_linearity_clean_output_defines_each_name_once(
        (base, ours, theirs) in general_triple_strategy_with(any_lang())
    ) {
        let r = merge_m(&base, &ours, &theirs);
        prop_assume!(r.is_clean());
        let mut names: BTreeSet<&str> = BTreeSet::new();
        for e in base.entities.iter().chain(ours.entities.iter()).chain(theirs.entities.iter()) {
            names.insert(e.name.as_str());
        }
        for name in names {
            let c = definition_count(base.lang, &r.content, name);
            prop_assert!(c <= 1,
                "clean merge defines '{}' {} times\nbase:\n{}\nours:\n{}\ntheirs:\n{}\nout:\n{}",
                name, c, render(&base), render(&ours), render(&theirs), r.content);
        }
    }
}

// ===========================================================================
// Documented boundaries (domain conditions, pinned so they cannot drift
// silently into either "fixed" or "worse")
// ===========================================================================

/// Issue #51 / `Unsupported::BothCreated`: an empty base with both sides
/// creating the file routes to line-level diff3, which CONFLICTS even on
/// disjoint additions. This is the documented boundary of L5 (conflict
/// soundness), chosen deliberately: entity reconstruction of a structured
/// format from no base can place content after a closing delimiter, and
/// "coarse and safe" beats "precise and lossy". L4 (no silent loss) still
/// holds on this path — both sides' content is in the marker payload.
#[test]
fn boundary_both_created_conflicts_but_loses_nothing() {
    let ours = "{\n  \"a\": 40000\n}\n";
    let theirs = "{\n  \"b\": 50000\n}\n";
    let r = entity_merge("", ours, theirs, "config.json");
    assert!(
        !r.is_clean(),
        "BothCreated gate: disjoint creations from an empty base conflict by design (issue #51)"
    );
    // the safety law survives the boundary
    assert!(visible(&r, 40000), "ours' creation must not be silently lost");
    assert!(visible(&r, 50000), "theirs' creation must not be silently lost");
}

/// Inputs that already contain conflict markers are refused before any law
/// applies: the result is a single whole-file conflict carrying all three
/// inputs. L4 holds (nothing is lost — the payload is the entire input);
/// L1/L2 byte-equations do NOT apply on this domain. See the RED test below
/// for the corner where this refusal overrides an agreement.
#[test]
fn boundary_pre_conflicted_inputs_are_refused_not_merged() {
    let base = "def e0():\n    return 10000\n";
    let ours = "<<<<<<< ours\ndef e0():\n    return 20000\n=======\ndef e0():\n    return 30000\n>>>>>>> theirs\n";
    let theirs = "def e0():\n    return 30000\n";
    let r = entity_merge(base, ours, theirs, "m.py");
    assert!(!r.is_clean());
    assert!(visible(&r, 20000) && visible(&r, 30000));
}

// ===========================================================================
// RED — preserved counterexamples (production is read-only here; these are
// the minimized witnesses of the violations, ignored so the law suite stays
// green while the violations stay visible and executable. Un-ignoring one is
// the acceptance test of its fix.)
// ===========================================================================

/// BUG-CLASS: idempotence override by the pre-conflict guard.
/// LAW VIOLATED: L2 (idempotence), merge(b, x, x) = x — and with it the
///   stronger agreement law "ours == theirs decides the merge".
/// WHERE: `entity_merge_with_registry` (merge.rs): the `has_conflict_markers`
///   guard runs BEFORE the `ours == theirs` fast path. When only BASE
///   contains marker text, the guard fires, picks `content = base` (the
///   marker-bearing input), and reports a whole-file conflict — even though
///   ours and theirs BYTE-AGREE on marker-free content. The two sides'
///   agreement is discarded in favor of re-serving the pre-conflicted
///   ancestor.
/// WHY IT MATTERS: git can hand a driver a stage-1 blob that legitimately
///   quotes markers (a fixture, docs about merging) while both branches
///   resolved it identically; the driver then "conflicts" a merge both sides
///   already agree on AND proposes the ancestor's bytes as the content.
///   Content is not lost (the conflict record carries ours/theirs), so L4
///   holds; but the verdict is wrong and the proposed content is the one
///   text all parties agree is stale.
/// STRUCTURE: the guard order inverts the cell function's o = t ⇒ o axiom;
///   the fix is a reordering (agreement check before the refusal guard),
///   which preserves the refusal for every case where the sides disagree.
#[test]
fn red_l2_idempotence_defeated_by_marker_bearing_base() {
    let base = "<<<<<<< ours\ndef e0():\n    return 10000\n=======\ndef e0():\n    return 10001\n>>>>>>> theirs\n";
    let x = "def e0():\n    return 20000\n";
    let r = entity_merge(base, x, x, "m.py");
    assert!(
        r.is_clean(),
        "ours == theirs must decide the merge regardless of base's bytes; got conflicts: {:?}",
        r.conflicts
    );
    assert_eq!(
        r.content, x,
        "merge(b, x, x) must be x, not the pre-conflicted base"
    );
}

/// BUG-CLASS: sequential-encoding leak into the entity carrier (JSON).
/// LAW VIOLATED: L5 (conflict soundness): a conflict may be emitted only when
///   both sides changed the same entity incompatibly. Here the two edits are
///   disjoint in JSON's own semantics (one side changes the VALUE of "e1",
///   the other ADDS a new key "tx9"), yet the merge conflicts.
/// WHERE: the JSON entity carrier is the raw source line INCLUDING the object
///   separator comma. Adding a key after the last key rewrites the previous
///   key's line ("e1": 10100 → "e1": 10100,), so the adder is classified as
///   having EDITED "e1" too, and the cell becomes EDIT×EDIT divergent →
///   `merge_ladder_exhausted` conflict on an entity only one side changed.
/// DENOTATION: the intended semantic domain for a JSON object is the partial
///   map key ⇀ value; the comma belongs to the *sequence encoding*, not to
///   any key's value. The carrier fails to quotient out the encoding, so the
///   pointwise cell function is applied to a text that is not a function of
///   the entity alone. The fix is a carrier change (normalize separators out
///   of the compared text, re-derive them at render), not a rule change.
/// WHY IT MATTERS: value-edit + key-add is the most common pair of concurrent
///   edits to package.json/config files; every such pair that lands at an
///   add boundary conflicts falsely.
/// NOTE: L4 (no silent loss) still holds on this path — both bodies are in
///   the marker payload. The failure is a FALSE conflict, not a loss.
#[test]
fn red_l5_json_disjoint_add_forces_comma_conflict() {
    let base = "{\n  \"e0\": 10000,\n  \"e1\": 10100\n}\n";
    let ours = "{\n  \"e0\": 10000,\n  \"e1\": 20100\n}\n"; // edits e1's value
    let theirs = "{\n  \"e0\": 10000,\n  \"e1\": 10100,\n  \"tx9\": 50900\n}\n"; // adds tx9
    let r = entity_merge(base, ours, theirs, "config.json");
    assert!(
        r.is_clean(),
        "semantically disjoint JSON edits (value edit vs key add) must not conflict; got: {:?}",
        r.conflicts
    );
    assert!(r.content.contains("20100") && r.content.contains("50900"));
}

/// BUG-CLASS: the merge output is not a normal form; iterated merging
///   DIVERGES on whitespace.
/// LAW VIOLATED: L2b at byte level (absorption, x ∨ (x ∨ y) = x ∨ y). The
///   result modulo entity content is correct (see the GREEN L2b property),
///   but the bytes drift: re-merging an input against the merged result
///   widens an interstitial gap by two newlines, and does so AGAIN on every
///   further iteration — merge(b, o, ·) has no fixpoint on its own image:
///     m1 = "…10000\n\n\ndef ox9…"
///     m2 = "…10000\n\n\n\n\ndef ox9…"
///     m3 = "…10000\n\n\n\n\n\n\ndef ox9…"   (observed)
/// WHERE: the blank-line/gap handling around a deleted entity (`widest_gap`
///   in merge.rs and the interstitial rebuild): the gaps flanking the
///   deletion are re-derived per merge and COMPOUND with the gap already
///   present in the previous output.
/// WHY IT MATTERS: any workflow that re-runs the merge over its own output
///   (drift re-checks, weave-mcp's update loop, a driver invoked twice)
///   accretes blank lines without bound; a μ-iteration "merge until stable"
///   never terminates by content equality.
/// STRUCTURE: the gap merge is intended as a JOIN in the max-lattice of gap
///   widths (idempotent by definition); the realization behaves as a SUM
///   across the deletion seam, and a sum is not idempotent. Restoring the
///   join restores the fixpoint.
#[test]
fn red_l2b_absorption_bytes_diverge() {
    let b = "def e0():\n    return 10000\n\n\ndef e1():\n    return 10100\n";
    let o = "def e0():\n    return 10000\n\n\ndef e1():\n    return 10100\n\n\ndef ox9():\n    return 40900\n";
    let t = "def e0():\n    return 10000\n"; // deleted e1
    let m1 = entity_merge(b, o, t, "m.py");
    assert!(m1.is_clean());
    let m2 = entity_merge(b, o, &m1.content, "m.py");
    assert!(m2.is_clean());
    assert_eq!(
        m2.content, m1.content,
        "merge(b, o, merge(b,o,t)) must be byte-identical to merge(b,o,t)"
    );
}

/// BUG-CLASS: rename inference steals one side of a both-added pair (TS).
/// LAWS VIOLATED: L8 (linearity — "an entity cannot be emitted twice",
///   v2/mod.rs's own table) and, in the divergent variant below, L5b
///   (conflict completeness).
/// THE TRIPLE (first-order; no re-merge needed):
///     base   = { e0, e1 }
///     ours   = { e0, e1, ox9 }          (keeps e1, adds ox9)
///     theirs = { e0, ox9 }              (deletes e1, adds the SAME ox9)
///   Expected: e1 deleted (delete vs unchanged), ox9 added once (both-added
///   convergent). Audit observed instead:
///     e1: Renamed { from: "e1", to: "ox9" }   ← theirs' ox9 captured as a
///     ox9: AddedOurs                            rename of deleted e1
///   and the output DEFINES `ox9` TWICE in a merge reported clean.
/// WHERE: v2 match_phase rename-candidate generation. The deleted base
///   entity is paired with the same side's addition as a rename candidate
///   even though that addition has an exact-name (here exact-body) partner
///   on the OTHER side. Claims are linear per arena entry — no entry is
///   emitted twice — but two DISTINCT entries carry the same name, so
///   name-linearity fails. TS-specific under sampling (12/597 clean merges
///   with a duplicate definition; Python 0/597, JSON 0/597): TS bodies share
///   a long token prefix (`export function …(): number { return`), which
///   feeds the prefix-filtered token index a false rename candidate; Python
///   bodies diverge at the second token.
/// WHY IT MATTERS: a clean merge is the case nobody reviews. A file with two
///   definitions of one function is silently wrong — in TS the later
///   definition wins at runtime — and the audit calls it a rename that never
///   happened.
/// STRUCTURE: matching must prefer the exact-name cross-side pairing (a
///   both-added triple) over base-side rename inference; equivalently, the
///   matcher should be a maximum-weight matching in which name-equality
///   strictly dominates body-shape similarity.
#[test]
fn red_l8_ts_rename_steal_duplicates_convergent_add() {
    let b = "export function e0(): number {\n  return 10000;\n}\n\nexport function e1(): number {\n  return 10100;\n}\n";
    let o = "export function e0(): number {\n  return 10000;\n}\n\nexport function e1(): number {\n  return 10100;\n}\n\nexport function ox9(): number {\n  return 40900;\n}\n";
    let t = "export function e0(): number {\n  return 10000;\n}\n\nexport function ox9(): number {\n  return 40900;\n}\n";
    let r = entity_merge(b, o, t, "m.ts");
    assert!(r.is_clean(), "this triple merges clean today; the bug is in the bytes");
    assert_eq!(
        r.content.matches("function ox9(").count(),
        1,
        "clean merge must define ox9 exactly once, got:\n{}",
        r.content
    );
}

/// BUG-CLASS: same rename-steal, divergent variant — a REAL contradiction
///   silently resolved.
/// LAW VIOLATED: L5b (conflict completeness). Both sides add `ox9` with
///   DIFFERENT bodies (40900 vs 50900) while theirs also deletes `e1`. This
///   is AddedBothDivergent — the cell the engine itself defines as a
///   conflict — but the rename-steal splits the pair into two triples and
///   the merge comes back CLEAN with both contradictory definitions in the
///   file, where TS semantics silently makes the later one win. The user is
///   never asked the one question the engine exists to ask.
/// SEVERITY: highest of the four REDs — this is the silent-wrong-answer
///   class on the clean path, i.e. the exact failure mode weave's product
///   promise ("never resolve a contradiction silently") rules out.
/// BUG-CLASS: same rename-steal, third facet — a FABRICATED conflict.
/// LAWS VIOLATED: L5 (conflict soundness: neither side renamed anything, yet
///   the verdict is RenameRename), L8 (ox0 is emitted twice), and audit
///   coherence (e1 is reported "Renamed to ox0" while e2's conflict claims
///   ours renamed e2 to the same ox0 — two triples claim one entity).
/// THE TRIPLE (first-order):
///     base   = { e0, e1, e2 }
///     ours   = { e0, e1, ox0 }          (deletes e2, adds ox0)
///     theirs = { e0, ox0, tx0 }         (deletes e1 and e2, adds the SAME
///                                        ox0 and its own tx0)
///   Expected: e1 delete-vs-unchanged → gone; e2 deleted-both → gone; ox0
///   added-both-convergent → once; tx0 added-theirs. Clean.
///   Observed: conflict `e2: RenameRename { ours_name: "ox0", theirs_name:
///   "tx0" }` — a rename NEITHER side made — plus `function ox0(` twice in
///   the output.
/// WHY IT MATTERS: this is the shape any absorption/re-merge workflow feeds
///   back into the engine (merge(b, o, m) where m carries both sides' adds),
///   so iterated merging on TS manufactures conflicts out of agreement.
/// STRUCTURE: same root as the other rename-steal REDs — rename inference
///   must not outrank exact-name, exact-body cross-side agreement.
#[test]
fn red_l5_ts_delete_add_fabricates_rename_rename_conflict() {
    let ts = |ents: &[(&str, u32)]| -> String {
        ents.iter()
            .map(|(n, s)| format!("export function {}(): number {{\n  return {};\n}}\n", n, s))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let b = ts(&[("e0", 10000), ("e1", 10100), ("e2", 10200)]);
    let o = ts(&[("e0", 10000), ("e1", 10100), ("ox0", 40000)]);
    let t = ts(&[("e0", 10000), ("ox0", 40000), ("tx0", 50000)]);
    let r = entity_merge(&b, &o, &t, "m.ts");
    assert!(
        r.is_clean(),
        "no side renamed anything; deletes + agreeing adds must merge clean, got: {:?}",
        r.conflicts
    );
    assert_eq!(
        r.content.matches("function ox0(").count(),
        1,
        "ox0 must be defined exactly once:\n{}",
        r.content
    );
}

#[test]
fn red_l5b_ts_divergent_add_conflict_suppressed() {
    let b = "export function e0(): number {\n  return 10000;\n}\n\nexport function e1(): number {\n  return 10100;\n}\n";
    let o = "export function e0(): number {\n  return 10000;\n}\n\nexport function e1(): number {\n  return 10100;\n}\n\nexport function ox9(): number {\n  return 40900;\n}\n";
    let t = "export function e0(): number {\n  return 10000;\n}\n\nexport function ox9(): number {\n  return 50900;\n}\n";
    let r = entity_merge(b, o, t, "m.ts");
    assert!(
        !r.is_clean(),
        "two different bodies for the same added name is a contradiction; it must conflict, got clean:\n{}",
        r.content
    );
}

// ===========================================================================
// Positive controls: the observation functions can fail.
// A checker that cannot go RED is not a witness (non-vacuity of the harness).
// ===========================================================================

/// The loss-checker flags a fabricated lossy result.
#[test]
fn control_loss_checker_detects_a_dropped_sentinel() {
    let r = MergeResult {
        content: "def e0():\n    return 10000\n".to_string(),
        conflicts: vec![],
        warnings: vec![],
        stats: Default::default(),
        audit: vec![],
    };
    assert!(!visible(&r, 20000), "checker must flag an absent sentinel");
    assert!(visible(&r, 10000), "checker must accept a present sentinel");
}

/// The soundness law is not vacuous: a genuinely divergent same-entity edit
/// does conflict, so `is_clean()` is a real observable, not a constant.
#[test]
fn control_divergent_same_entity_edit_conflicts() {
    let base = "def e0():\n    return 10000\n";
    let ours = "def e0():\n    return 20000\n";
    let theirs = "def e0():\n    return 30000\n";
    let r = entity_merge(base, ours, theirs, "m.py");
    assert!(!r.is_clean(), "a true contradiction must be reported");
    assert!(visible(&r, 20000) && visible(&r, 30000));
}

/// The sentinel-image reader actually reads: distinct texts with distinct
/// sentinels produce distinct images.
#[test]
fn control_sentinel_image_distinguishes() {
    let a = sentinel_image("def f():\n    return 20000\n");
    let b = sentinel_image("def f():\n    return 30000\n");
    assert_ne!(a, b);
    assert_eq!(a, BTreeSet::from([20000]));
}

/// The duplicate-definition counter actually counts.
#[test]
fn control_definition_count_counts() {
    let text = "def a():\n    return 1\n\ndef a():\n    return 2\n";
    assert_eq!(definition_count(Lang::Py, text, "a"), 2);
    assert_eq!(definition_count(Lang::Py, text, "b"), 0);
}

/// The generators generate what the laws need: a disjoint triple really has
/// per-side writes, and the general generator can produce overlapping edits
/// and both convergent and divergent shared additions.
#[test]
fn control_generators_are_not_degenerate() {
    use proptest::strategy::ValueTree;
    use proptest::test_runner::TestRunner;
    let mut runner = TestRunner::deterministic();
    let mut saw_side_writes = 0u32;
    let mut saw_overlap = 0u32;
    let mut saw_convergent_add = 0u32;
    for _ in 0..200 {
        let (b, o, t) = disjoint_triple_strategy(any_lang())
            .new_tree(&mut runner)
            .unwrap()
            .current();
        if !written_sentinels(&b, &o).is_empty() && !written_sentinels(&b, &t).is_empty() {
            saw_side_writes += 1;
        }
        let (b, o, t) = general_triple_strategy()
            .new_tree(&mut runner)
            .unwrap()
            .current();
        // overlap: both sides wrote a body for the same entity name
        let on: BTreeSet<&str> = o
            .entities
            .iter()
            .filter(|e| written_sentinels(&b, &o).contains(&e.sentinel))
            .map(|e| e.name.as_str())
            .collect();
        let tn: BTreeSet<&str> = t
            .entities
            .iter()
            .filter(|e| written_sentinels(&b, &t).contains(&e.sentinel))
            .map(|e| e.name.as_str())
            .collect();
        if on.intersection(&tn).next().is_some() {
            saw_overlap += 1;
        }
        // convergent shared adds: same (name, sentinel) added on both sides
        if o.entities
            .iter()
            .any(|e| e.sentinel >= 60000 && t.entities.contains(e))
        {
            saw_convergent_add += 1;
        }
    }
    assert!(
        saw_side_writes > 60,
        "disjoint generator: both-sides-write coverage floor not met ({saw_side_writes}/200)"
    );
    assert!(
        saw_overlap > 20,
        "general generator: overlapping-edit coverage floor not met ({saw_overlap}/200)"
    );
    assert!(
        saw_convergent_add > 5,
        "general generator: convergent-add coverage floor not met ({saw_convergent_add}/200)"
    );
}
