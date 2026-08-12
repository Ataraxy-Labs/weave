//! Placement, decided once and deterministically.
//!
//! v1 reconstructed the file by walking *ours* as a skeleton and splicing
//! theirs-only entities in beside whatever preceded them in theirs. That is
//! peer-relative: two runs folding the same set of changes in different orders
//! can produce different bytes even when the result is multiset-equal, which
//! breaks determinism.
//!
//! Here every item's position is a function of the SHARED history: the rank of
//! the nearest base entity it follows. Additions sort after the survivor they
//! were written after; additions that share one anchor sort by the order some
//! input version wrote them in, and only where no version holds both does the
//! identity key decide ([`gap_order`]). Nothing consults "which side is ours",
//! so `plan(a, b)` and `plan(b, a)` produce the same order, and the result does
//! not depend on which side is folded first.
//!
//! Order that a side deliberately changed is still honoured: if exactly one
//! side reordered the survivors, that side's order is the one the plan follows.
//!
//! When *both* sides reordered, there is no single side to defer to, and what
//! to do depends on whether order means anything in this language
//! ([`EntityOrder`]). Where it does, the two assertions are
//! combined the way the import merge combines two import sequences — an
//! order-preserving union over the anchors they share — and a disagreement
//! about SHARED entities is an honest conflict, because no sequence satisfies
//! both. Where it does not, the shared base order is a deterministic answer
//! that loses nothing observable.

use std::collections::{HashMap, HashSet};

use super::resolve::Resolved;
use super::types::*;

/// The ordered plan, plus the interstitial text that leads each item.
#[derive(Debug)]
pub(crate) struct PlannedItem {
    pub anchor: Anchor,
    /// Index into the triples/dispositions arrays.
    pub triple: usize,
}

/// What `plan` decided: the order, and the ORDER-rule violation it could not
/// decide away. `plan` never invents a conflict marker — it reports
/// the fact and the pipeline's owner of conflicts renders it.
#[derive(Debug)]
pub(crate) struct Placement {
    pub items: Vec<PlannedItem>,
    pub order_conflict: Option<OrderConflict>,
}

/// Order-preserving union of two asserted sequences, the entity-level
/// counterpart of `order_preserving_import_union` in the import merge.
///
/// Walks both sequences at once. An item only one side has goes where that side
/// put it; an item both have goes where they agree. If both heads are *shared*
/// items in opposite order, no sequence satisfies both sides: that is reported,
/// and the walk continues so the caller still has a full sequence to render
/// alongside the markers.
fn order_preserving_union(ours: &[usize], theirs: &[usize]) -> (Vec<usize>, bool) {
    let ours_set: HashSet<usize> = ours.iter().copied().collect();
    let theirs_set: HashSet<usize> = theirs.iter().copied().collect();
    let mut out: Vec<usize> = Vec::with_capacity(ours.len() + theirs.len());
    let mut emitted: HashSet<usize> = HashSet::new();
    let mut disagreement = false;
    let (mut i, mut j) = (0usize, 0usize);

    while i < ours.len() || j < theirs.len() {
        if i < ours.len() && emitted.contains(&ours[i]) {
            i += 1;
            continue;
        }
        if j < theirs.len() && emitted.contains(&theirs[j]) {
            j += 1;
            continue;
        }
        let next = if i >= ours.len() {
            let x = theirs[j];
            j += 1;
            x
        } else if j >= theirs.len() {
            let x = ours[i];
            i += 1;
            x
        } else if ours[i] == theirs[j] {
            let x = ours[i];
            i += 1;
            j += 1;
            x
        } else if !theirs_set.contains(&ours[i]) {
            let x = ours[i];
            i += 1;
            x
        } else if !ours_set.contains(&theirs[j]) {
            let x = theirs[j];
            j += 1;
            x
        } else {
            disagreement = true;
            let x = ours[i];
            i += 1;
            x
        };
        if emitted.insert(next) {
            out.push(next);
        }
    }
    (out, disagreement)
}

/// Rank the additions that share a gap, by the order some input version put
/// them in.
///
/// Two additions anchored to the same survivor are, to the shared coordinate,
/// simultaneous — and the identity key was breaking the tie, which sorts a
/// `const` and the function beneath it by entity type. But simultaneity is
/// often only apparent: if any one version contains both entities, that version
/// already says which comes first, and that statement is a fact about the
/// content rather than about a peer.
///
/// So: an edge `u -> v` whenever some version has both and puts `u` first, then
/// place items by repeatedly taking the smallest key whose predecessors are all
/// placed. Pairs no version orders fall back to the key exactly as before, so
/// the result does not depend on the order the versions are folded in.
/// Cyclic evidence (two versions disagreeing) is dropped and the key decides.
fn gap_order(
    arena: &Arena,
    triples: &[Triple],
    staged: &[(usize, Key, u32, u8)],
) -> HashMap<usize, u32> {
    let mut out: HashMap<usize, u32> = HashMap::new();
    let mut gaps: HashMap<u32, Vec<usize>> = HashMap::new();
    for (i, _, after_base, tier) in staged {
        if *tier == 1 {
            gaps.entry(*after_base).or_default().push(*i);
        }
    }
    let key_of: HashMap<usize, &Key> = staged.iter().map(|(i, k, _, _)| (*i, k)).collect();
    for (_, members) in gaps {
        if members.len() < 2 {
            continue;
        }
        // Evidence: for each side, the positions of the members it holds.
        let mut before: Vec<(usize, usize)> = Vec::new();
        for side in [Side::Ours, Side::Theirs] {
            let mut seen: Vec<(u32, usize)> = members
                .iter()
                .filter_map(|i| {
                    triples[*i]
                        .side_idx(side)
                        .map(|idx| (arena.get(idx).position, *i))
                })
                .collect();
            seen.sort_unstable();
            for a in 0..seen.len() {
                for b in a + 1..seen.len() {
                    before.push((seen[a].1, seen[b].1));
                }
            }
        }
        // A pair both versions order, in opposite directions, is not evidence.
        let contradicted: HashSet<(usize, usize)> = before
            .iter()
            .filter(|(u, v)| before.contains(&(*v, *u)))
            .copied()
            .collect();
        let mut indeg: HashMap<usize, usize> = members.iter().map(|i| (*i, 0)).collect();
        let mut succ: HashMap<usize, Vec<usize>> = HashMap::new();
        let mut edges: HashSet<(usize, usize)> = HashSet::new();
        for (u, v) in &before {
            if contradicted.contains(&(*u, *v)) || !edges.insert((*u, *v)) {
                continue;
            }
            succ.entry(*u).or_default().push(*v);
            *indeg
                .get_mut(v)
                .expect("every ordering edge names a member: `before` is built only from `seen`, which is `members` filtered, and `indeg` is seeded with one entry per member") += 1;
        }
        let mut placed: Vec<usize> = Vec::with_capacity(members.len());
        let mut ready: Vec<usize> = members.iter().copied().filter(|i| indeg[i] == 0).collect();
        while !ready.is_empty() {
            ready.sort_by(|a, b| key_of[a].cmp(key_of[b]));
            let next = ready.remove(0);
            placed.push(next);
            for v in succ.get(&next).into_iter().flatten() {
                let d = indeg.get_mut(v).expect(
                    "`succ` and `indeg` are filled from the same edge list, so every successor is a member and has an in-degree",
                );
                *d -= 1;
                if *d == 0 {
                    ready.push(*v);
                }
            }
        }
        // A cycle would leave members unplaced; the key finishes the job.
        let mut rest: Vec<usize> = members
            .iter()
            .copied()
            .filter(|i| !placed.contains(i))
            .collect();
        rest.sort_by(|a, b| key_of[a].cmp(key_of[b]));
        placed.extend(rest);
        for (r, i) in placed.into_iter().enumerate() {
            out.insert(i, r as u32);
        }
    }
    out
}

pub(crate) fn plan(
    arena: &Arena,
    triples: &[Triple],
    resolved: &[Resolved],
    order: EntityOrder,
) -> Placement {
    // -- 1. Choose the coordinate system for survivors -------------------
    let survivor_order = |version_of: fn(&Triple) -> Option<Idx>| -> Vec<usize> {
        let mut with_pos: Vec<(u32, usize)> = triples
            .iter()
            .enumerate()
            .filter(|(_, t)| t.base_idx().is_some())
            .filter_map(|(i, t)| version_of(t).map(|idx| (arena.get(idx).position, i)))
            .collect();
        with_pos.sort_unstable();
        with_pos.into_iter().map(|(_, i)| i).collect()
    };
    let base_seq = survivor_order(|t| t.base_idx());
    let ours_seq = survivor_order(|t| t.ours_idx());
    let theirs_seq = survivor_order(|t| t.theirs_idx());

    // A side that left the base order alone has nothing to say about order, so
    // the other side's order wins; if both moved things, the shared order is
    // the only one neither side can claim was overridden.
    //
    // "Moved" is asked of the entities the side STILL HAS, against base's order
    // of that same set. Restricting to the other side's set instead made a
    // plain deletion look like a reorder — the two sequences then differed in
    // LENGTH, never in order — and a spurious `moved` verdict is not cosmetic:
    // it changes the coordinate system every later anchor is expressed in.
    let restrict = |seq: &[usize], to: &[usize]| -> Vec<usize> {
        seq.iter().copied().filter(|i| to.contains(i)).collect()
    };
    let ours_moved = ours_seq != restrict(&base_seq, &ours_seq);
    let theirs_moved = theirs_seq != restrict(&base_seq, &theirs_seq);
    let mut order_conflict: Option<OrderConflict> = None;
    let chosen: Vec<usize> = match (ours_moved, theirs_moved) {
        (false, false) => base_seq.clone(),
        (false, true) => theirs_seq.clone(),
        (true, false) => ours_seq.clone(),
        // Both sides asserted an order. Which answer is honest depends on
        // whether the language gives the assertion any meaning.
        (true, true) => match order {
            EntityOrder::Effectful => {
                let (union, disagree) = order_preserving_union(&ours_seq, &theirs_seq);
                if disagree {
                    let names = |seq: &[usize]| -> Vec<String> {
                        seq.iter()
                            .map(|i| arena.get(triples[*i].representative()).name().to_string())
                            .collect()
                    };
                    order_conflict = Some(OrderConflict {
                        base: names(&base_seq),
                        ours: names(&ours_seq),
                        theirs: names(&theirs_seq),
                    });
                }
                union
            }
            // Nothing observable depends on definition order here, so the
            // shared coordinate is a deterministic answer that discards no
            // semantics — only a cosmetic preference neither side can claim
            // the other overrode.
            EntityOrder::Unordered => base_seq.clone(),
        },
    };
    // Survivors the chosen order does not mention — deleted on the side whose
    // order was chosen, but still emitted here because the other side's edit
    // turned the deletion into a conflict — go back into their BASE
    // neighbourhood, not onto the end of the file. Appending them was
    // observable twice over: the entity itself moved, and every later addition
    // anchored after it inherited a rank from the tail of the file.
    let survivors = order_preserving_union(&chosen, &base_seq).0;
    let mut rank: HashMap<usize, u32> = HashMap::new();
    for (r, i) in survivors.iter().enumerate() {
        rank.insert(*i, r as u32);
    }

    // -- 2. Anchor every item -------------------------------------------
    // Survivor with rank r sits at 2(r+1); an addition written after it sits at
    // 2(r+1)+1; an addition before every survivor sits at 1.
    let mut items: Vec<PlannedItem> = Vec::new();
    let dropped: Vec<bool> = resolved
        .iter()
        .map(|r| matches!(r.disposition, Disposition::Drop))
        .collect();
    // Staged, because a run is a property of a *set* of additions: which gap
    // they share and which side wrote them. Pass one fills this; pass two
    // reads the runs off it.
    let mut staged: Vec<(usize, Key, u32, u8)> = Vec::new();
    for (i, triple) in triples.iter().enumerate() {
        if dropped[i] {
            continue;
        }
        let key = arena.get(triple.representative()).key.clone();
        let after_base = match rank.get(&i) {
            Some(r) => (r + 1) * 2,
            None => {
                // An addition: anchor it to the NEAREST survivor it follows in
                // the side that wrote it. Both sides express the answer in the
                // same shared coordinate, so a both-added entity gets the same
                // number from either.
                //
                // Nearest, not highest-ranked: "the entity this was written
                // after" is a fact about the author's file, and reading it as
                // `max(rank)` silently substitutes the plan's coordinate for
                // the author's. The two agree only while rank order matches the
                // side's order — which is exactly what a deletion or a one-
                // sided reorder breaks. A predecessor
                // that this merge drops is not an anchor anyone can see, so the
                // walk continues past it to the nearest one that survives.
                let mut best: Option<u32> = None;
                for side in [Side::Ours, Side::Theirs] {
                    let Some(idx) = triple.side_idx(side) else {
                        continue;
                    };
                    let pos = arena.get(idx).position;
                    let mut nearest: Option<(u32, u32)> = None;
                    for (j, other) in triples.iter().enumerate() {
                        if dropped[j] {
                            continue;
                        }
                        let Some(r) = rank.get(&j) else { continue };
                        let Some(other_idx) = other.side_idx(side) else {
                            continue;
                        };
                        let other_pos = arena.get(other_idx).position;
                        if other_pos < pos && nearest.is_none_or(|(p, _)| other_pos > p) {
                            nearest = Some((other_pos, *r));
                        }
                    }
                    let anchor = nearest.map_or(1, |(_, r)| (r + 1) * 2 + 1);
                    best = Some(match best {
                        Some(b) => b.min(anchor),
                        None => anchor,
                    });
                }
                best.unwrap_or(1)
            }
        };
        let tier = u8::from(!rank.contains_key(&i));
        staged.push((i, key, after_base, tier));
    }

    // -- 3. Inside one gap, honour the order some version wrote down --------
    let within = gap_order(arena, triples, &staged);
    for (i, key, after_base, tier) in staged {
        items.push(PlannedItem {
            anchor: Anchor {
                after_base,
                tier,
                within: *within.get(&i).unwrap_or(&0),
                key,
            },
            triple: i,
        });
    }
    items.sort_by(|a, b| a.anchor.cmp(&b.anchor));
    Placement {
        items,
        order_conflict,
    }
}

#[cfg(test)]
mod tests {
    use super::super::match_phase::{build_arena, match_phase, RawEntity};
    use super::*;
    use crate::conflict::MarkerFormat;
    use crate::v2::classify::classify;
    use crate::v2::resolve::{resolve, ResolveCtx};

    fn raw(name: &str) -> RawEntity {
        RawEntity {
            name: name.to_string(),
            entity_type: "function".into(),
            parent: None,
            content: format!("def {name}():\n    return \"{name}\"\n"),
            src_id: format!("f::{name}"),
        }
    }
    fn raws(names: &[&str]) -> Vec<RawEntity> {
        names.iter().map(|n| raw(n)).collect()
    }

    fn order(b: &[&str], o: &[&str], t: &[&str]) -> Vec<String> {
        placement(b, o, t, EntityOrder::Unordered).0
    }

    /// (names in output order, order conflict?)
    fn placement(
        b: &[&str],
        o: &[&str],
        t: &[&str],
        order: EntityOrder,
    ) -> (Vec<String>, Option<OrderConflict>) {
        let (arena, claims) = build_arena(raws(b), raws(o), raws(t));
        let m = match_phase(arena, claims);
        let cells: Vec<_> = m.triples.iter().map(|tr| classify(&m.arena, tr)).collect();
        let fmt = MarkerFormat::default();
        let ctx = ResolveCtx {
            marker_format: &fmt,
            indent_sensitive: true,
            decorators_compose: true,
            base_all: &[],
            ours_all: &[],
            theirs_all: &[],
            host: &crate::host::Host::default(),
        };
        let resolved: Vec<_> = m
            .triples
            .iter()
            .zip(cells.iter())
            .map(|(tr, c)| resolve(&m.arena, tr, *c, &ctx))
            .collect();
        let placed = plan(&m.arena, &m.triples, &resolved, order);
        (
            placed
                .items
                .iter()
                .map(|p| p.anchor.key.name.clone())
                .collect(),
            placed.order_conflict,
        )
    }

    /// Same shape as `raw`, with the entity type chosen by the caller — the
    /// identity key is `(parent, entity_type, name)`, so a `const` and a
    /// function are not interchangeable in the tie-break.
    fn raw_typed(name: &str, ty: &str) -> RawEntity {
        RawEntity {
            name: name.to_string(),
            entity_type: ty.into(),
            parent: None,
            content: format!("{ty} {name}\n"),
            src_id: format!("{ty}::{name}"),
        }
    }

    fn order_typed(b: &[(&str, &str)], o: &[(&str, &str)], t: &[(&str, &str)]) -> Vec<String> {
        let mk = |v: &[(&str, &str)]| v.iter().map(|(n, ty)| raw_typed(n, ty)).collect();
        let (arena, claims) = build_arena(mk(b), mk(o), mk(t));
        let m = match_phase(arena, claims);
        let cells: Vec<_> = m.triples.iter().map(|tr| classify(&m.arena, tr)).collect();
        let fmt = MarkerFormat::default();
        let ctx = ResolveCtx {
            marker_format: &fmt,
            indent_sensitive: false,
            decorators_compose: true,
            base_all: &[],
            ours_all: &[],
            theirs_all: &[],
            host: &crate::host::Host::default(),
        };
        let resolved: Vec<_> = m
            .triples
            .iter()
            .zip(cells.iter())
            .map(|(tr, c)| resolve(&m.arena, tr, *c, &ctx))
            .collect();
        plan(&m.arena, &m.triples, &resolved, EntityOrder::Unordered)
            .items
            .iter()
            .map(|p| p.anchor.key.name.clone())
            .collect()
    }

    #[test]
    fn two_additions_one_side_wrote_together_keep_that_side_s_order() {
        // The scenario this guards: one side adds a const and the function
        // under it. Sorted by identity key they come back function-first,
        // because "function" < "variable" — an order no version of the file
        // contains.
        let out = order_typed(
            &[("head", "function"), ("tail", "function")],
            &[
                ("head", "function"),
                ("WORKTREE", "variable"),
                ("isWorktree", "function"),
                ("tail", "function"),
            ],
            &[("head", "function"), ("tail", "function")],
        );
        assert_eq!(out, vec!["head", "WORKTREE", "isWorktree", "tail"]);
    }

    #[test]
    fn additions_no_version_orders_still_fall_back_to_the_key() {
        // Nothing has been written down about zed-vs-mid: no version holds
        // both. The key decides, from either direction — the property the
        // n-way fold rests on.
        let fwd = order(&["a"], &["a", "zed"], &["a", "mid"]);
        let rev = order(&["a"], &["a", "mid"], &["a", "zed"]);
        assert_eq!(fwd, rev);
        assert_eq!(fwd, vec!["a", "mid", "zed"]);
    }

    #[test]
    fn placement_does_not_depend_on_which_side_is_ours() {
        // Concurrent appends: the peer-relative rule put ours' first; the
        // anchor rule puts them in key order, from either direction.
        let fwd = order(&["a", "b"], &["a", "b", "zed"], &["a", "b", "mid"]);
        let rev = order(&["a", "b"], &["a", "b", "mid"], &["a", "b", "zed"]);
        assert_eq!(fwd, rev, "placement is peer-relative");
        assert_eq!(fwd, vec!["a", "b", "mid", "zed"]);
    }

    #[test]
    fn an_addition_stays_next_to_the_entity_it_was_written_after() {
        let out = order(
            &["a", "b", "c"],
            &["a", "helper", "b", "c"],
            &["a", "b", "c"],
        );
        assert_eq!(out, vec!["a", "helper", "b", "c"]);
    }

    #[test]
    fn a_one_sided_reorder_is_honoured() {
        let out = order(&["a", "b", "c"], &["c", "b", "a"], &["a", "b", "c"]);
        assert_eq!(out, vec!["c", "b", "a"]);
    }

    // -- both sides asserted an order ------------------------------------

    #[test]
    fn a_one_sided_reorder_is_honoured_in_an_order_effectful_language_too() {
        // The flag must not change the cell where only one side spoke: that
        // side's move is preserved either way, which is what keeps the
        // MoveEnd x Edit scenario cells green in both languages.
        let (out, conflict) = placement(
            &["a", "b", "c"],
            &["a", "c", "b"],
            &["a", "b", "c"],
            EntityOrder::Effectful,
        );
        assert_eq!(out, vec!["a", "c", "b"]);
        assert!(conflict.is_none());
    }

    #[test]
    fn two_compatible_reorders_are_unioned_where_order_is_semantics() {
        // ours moves `a` to the end; theirs inserts nothing and moves nothing
        // that contradicts it — the two assertions have a common linearisation
        // and the union preserves BOTH, rather than reverting to base.
        let (out, conflict) = placement(
            &["a", "b", "c", "d"],
            &["b", "c", "d", "a"],
            &["a", "c", "b", "d"],
            EntityOrder::Effectful,
        );
        assert!(conflict.is_some(), "b/c disagree, so this cell conflicts");
        // Even conflicted, every survivor is placed exactly once.
        let mut sorted = out.clone();
        sorted.sort();
        assert_eq!(sorted, vec!["a", "b", "c", "d"]);
    }

    #[test]
    fn contradictory_order_of_shared_entities_conflicts_where_order_is_semantics() {
        let (_, conflict) = placement(
            &["a", "b", "c"],
            &["a", "c", "b"],
            &["b", "a", "c"],
            EntityOrder::Effectful,
        );
        let c = conflict.expect("contradictory shared order must be reported");
        assert_eq!(c.ours, vec!["a", "c", "b"]);
        assert_eq!(c.theirs, vec!["b", "a", "c"]);
    }

    #[test]
    fn the_same_contradiction_is_deterministic_and_silent_where_order_is_not_semantics() {
        let (out, conflict) = placement(
            &["a", "b", "c"],
            &["a", "c", "b"],
            &["b", "a", "c"],
            EntityOrder::Unordered,
        );
        assert!(conflict.is_none());
        assert_eq!(out, vec!["a", "b", "c"], "falls back to the shared order");
    }

    #[test]
    fn placement_is_still_direction_free_when_both_sides_reorder() {
        for order in [EntityOrder::Unordered, EntityOrder::Effectful] {
            let (fwd, cf) = placement(&["a", "b", "c"], &["a", "c", "b"], &["b", "a", "c"], order);
            let (rev, cr) = placement(&["a", "b", "c"], &["b", "a", "c"], &["a", "c", "b"], order);
            assert_eq!(cf.is_some(), cr.is_some(), "{order:?}: verdict flipped");
            let (mut f, mut r) = (fwd, rev);
            f.sort();
            r.sort();
            assert_eq!(f, r, "{order:?}: entity multiset depends on direction");
        }
    }

    // -- relocation with nobody reordering -------------------------------

    /// Regression case: an addition anchored to a deleted neighbour used to
    /// get exiled to the end of the file even though neither side touched
    /// the order.
    ///
    /// ours deletes `bravo`; theirs adds `inserted` directly after `bravo`.
    /// The addition's anchor is "after bravo" — bravo is gone, so the honest
    /// answer is the nearest survivor before it, `alpha`. Anchoring on
    /// `max(rank)` instead put bravo's tail-appended rank on the addition and
    /// exiled it to the end of the file.
    #[test]
    fn an_addition_after_a_deleted_neighbour_stays_where_it_was_written() {
        let out = order(
            &["alpha", "bravo", "charlie", "delta"],
            &["alpha", "charlie", "delta"],
            &["alpha", "bravo", "inserted", "charlie", "delta"],
        );
        assert_eq!(out, vec!["alpha", "inserted", "charlie", "delta"]);
    }

    /// Deleting an entity is not a claim about order. Reading it as one made
    /// the plan switch coordinate systems for the whole file.
    #[test]
    fn a_deletion_is_not_a_reorder() {
        // theirs deletes `b` and says nothing else; ours says nothing at all.
        // Every survivor must keep its base position.
        let out = order(
            &["a", "b", "c", "d"],
            &["a", "b", "c", "d"],
            &["a", "c", "d"],
        );
        assert_eq!(out, vec!["a", "c", "d"]);
    }

    /// A survivor the chosen side deleted — kept alive here because the other
    /// side edited it, so the cell is a modify/delete conflict rather than a
    /// drop — belongs in its base neighbourhood, not at the end of the file.
    #[test]
    fn a_survivor_missing_from_the_chosen_order_keeps_its_base_neighbourhood() {
        // ours reorders (so ours' order is chosen) AND drops `b` from its own
        // sequence; `b` must still land between `a` and `c`.
        let (arena, claims) = build_arena(
            raws(&["a", "b", "c", "d"]),
            raws(&["d", "a", "c"]),
            raws(&["a", "b", "c", "d"]),
        );
        let m = match_phase(arena, claims);
        let cells: Vec<_> = m.triples.iter().map(|tr| classify(&m.arena, tr)).collect();
        let fmt = MarkerFormat::default();
        let ctx = ResolveCtx {
            marker_format: &fmt,
            indent_sensitive: true,
            decorators_compose: true,
            base_all: &[],
            ours_all: &[],
            theirs_all: &[],
            host: &crate::host::Host::default(),
        };
        let mut resolved: Vec<_> = m
            .triples
            .iter()
            .zip(cells.iter())
            .map(|(tr, c)| resolve(&m.arena, tr, *c, &ctx))
            .collect();
        // Force `b` to be emitted rather than dropped, as a modify/delete
        // conflict would.
        for (t, r) in m.triples.iter().zip(resolved.iter_mut()) {
            if m.arena.get(t.representative()).name() == "b" {
                r.disposition = Disposition::Emit {
                    text: "def b():\n".into(),
                    name: "b".into(),
                };
            }
        }
        let placed = plan(&m.arena, &m.triples, &resolved, EntityOrder::Unordered);
        let names: Vec<String> = placed
            .items
            .iter()
            .map(|p| p.anchor.key.name.clone())
            .collect();
        assert_eq!(names, vec!["d", "a", "b", "c"]);
    }

    #[test]
    fn the_language_table_names_the_families_it_claims() {
        for p in ["a.c", "a.h", "lib.cpp", "m.ml", "m.mli", "s.sh", "q.sql"] {
            assert_eq!(EntityOrder::of_path(p), EntityOrder::Effectful, "{p}");
        }
        for p in ["a.py", "a.rs", "a.ts", "a.go", "a.java"] {
            assert_eq!(EntityOrder::of_path(p), EntityOrder::Unordered, "{p}");
        }
    }
}
