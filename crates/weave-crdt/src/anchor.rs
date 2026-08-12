//! The anchor rule: **one** rule for where an entity sits in its file, owned
//! here and written nowhere else.
//!
//! What this replaces: the crate had *three* competing rules —
//! `ops::next_anchor_for_file` appended after a replica-local maximum over
//! every row including other additions, `sync::assign_anchors` re-derived
//! positions from the file and overwrote that answer, and v2 `plan` had a
//! third. Which one governed depended on which door you came in by: `sync`
//! overwrote `ops`, while `join::apply_op`, `weave claim` and the MCP server
//! called `upsert_entity` and never went near `assign_anchors` at all. Two
//! replicas reaching the same state by different doors anchored the same
//! entity differently, which is divergence between replicas that made the
//! exact same edits.
//!
//! ## The rule
//!
//! An anchor is a pair `(after_base, tier)`, and a file is laid out by sorting
//! `(after_base, tier, entity_id)`. This is the same shape as weave-core's v2
//! `Anchor` (`v2::types`, built in `v2::plan`), transplanted rather than
//! reinvented — with one field left behind: v2 also carries a `within` rank
//! that orders co-anchored additions by where a version wrote them. That needs
//! a version's text to read the order off, which a CRDT door does not have, so
//! here the same ties break on entity id.
//!
//! * **tier 0 — the shared coordinate.** The entity's rank `r` in the first
//!   whole-file observation, as the even slot `2(r+1)`. Both replicas read that
//!   observation from the same bytes, so both compute the same evens.
//! * **tier 1 — an addition with positional evidence.** The odd slot `s + 1`,
//!   where `s` is the tier-0 slot the addition was written after (`0`, hence
//!   slot 1, for an addition before every shared entity). Two replicas adding
//!   concurrently after the same shared entity therefore land on the *same*
//!   slot and break the tie on entity id — never on "who arrived first".
//! * **tier 2 — an addition with no positional evidence.** A door like `weave
//!   claim` or the MCP server sees one entity and no file, so the only honest
//!   answer is "after everything shared": slot `max(tier-0 slots) + 1`, sorting
//!   after tier 1 at the same slot, ties on entity id.
//!
//! ## The single writer, and how a tier is settled
//!
//! [`place`] is the only function in the crate that writes `anchor_after` /
//! `anchor_tier` — a test in the suite greps the crate to keep it that
//! way — and an anchor only ever moves from a higher tier number to a lower one
//! (`2 → 1 → 0`), i.e. from less evidence to more; a tier-0 or tier-1 anchor is
//! never rewritten. So a door that arrives with weaker evidence cannot undo a
//! door that arrived with stronger evidence, whatever order the doors are used
//! in.

use automerge::{transaction::Transactable, ReadDoc};

use crate::error::Result;
use crate::ops::{get_str, get_u64};
use crate::state::EntityStateDoc;

/// A position in the file's shared coordinate system. Ordered: this is the
/// output order, and `entity_id` is the final tie-break.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Anchor {
    pub after_base: u64,
    /// 0 shared, 1 addition with positional evidence, 2 addition without.
    /// Lower is more evidence, and evidence only ever accumulates.
    pub tier: u8,
}

/// What a door saw. The rule is one function; the doors differ only in how much
/// of the file they were looking at.
pub(crate) enum Observation<'a> {
    /// The whole file, in file order. On first sight this is TAKEN AS the shared
    /// coordinate — tier 0, the even slots. That is exact when both replicas
    /// first see the file from the same bytes (the common case: a file synced
    /// once before it forks). It is NOT exact across branches: two replicas that
    /// each first-see the file on their own branch, with different entities or a
    /// different order, each write tier-0 evens from their own view, so an
    /// entity only one branch had lands on a tier-0 slot the other never
    /// assigned. Convergence still holds — the layout sorts on
    /// `(after_base, tier, entity_id)`, and the `entity_id` tie-break makes two
    /// entities that independently claimed the same even slot deterministic on
    /// both replicas — but the tier-0 label then means "each replica's own first
    /// sight", not "a coordinate both read from one byte string". Both replicas
    /// still reach the same layout, resting on the id tie-break; what they do not
    /// share in that case is tier 0 meaning a coordinate both read from one byte
    /// string.
    File(&'a [String]),
    /// One entity, with no positional information — `weave claim`, the MCP
    /// server, `join::apply_op`.
    Entity(&'a str),
}

/// Place the entities of `file_path` according to the observation.
///
/// The only writer of an entity's anchor. An existing tier-0 or tier-1 anchor
/// is never touched, and a tier-2 anchor is settled only by a whole-file
/// observation.
pub(crate) fn place(
    state: &mut EntityStateDoc,
    file_path: &str,
    observed: Observation<'_>,
) -> Result<()> {
    let existing = anchors_of_file(state, file_path);
    let anchor_of = |id: &str| existing.iter().find(|(k, _)| k == id).and_then(|(_, a)| *a);
    let shared_max = existing
        .iter()
        .filter_map(|(_, a)| a.filter(|a| a.tier == 0))
        .map(|a| a.after_base)
        .max();

    match observed {
        // First sight of the file: its own order is the shared coordinate.
        Observation::File(ids) if shared_max.is_none() => {
            for (r, id) in ids.iter().enumerate() {
                write_anchor(
                    state,
                    id,
                    Anchor {
                        after_base: 2 * (r as u64 + 1),
                        tier: 0,
                    },
                )?;
            }
        }
        // A file we already share a coordinate for: everything new in it is an
        // addition, anchored to the nearest shared entity it follows.
        Observation::File(ids) => {
            let mut last_shared = 0u64;
            for id in ids {
                match anchor_of(id) {
                    Some(a) if a.tier == 0 => last_shared = a.after_base,
                    // tier 1 is settled; tier 2 was a guess, and this is evidence.
                    Some(a) if a.tier == 1 => {}
                    _ => write_anchor(
                        state,
                        id,
                        Anchor {
                            after_base: last_shared + 1,
                            tier: 1,
                        },
                    )?,
                }
            }
        }
        Observation::Entity(id) => {
            if anchor_of(id).is_some() {
                return Ok(());
            }
            write_anchor(
                state,
                id,
                Anchor {
                    after_base: shared_max.unwrap_or(0) + 1,
                    tier: 2,
                },
            )?;
        }
    }
    Ok(())
}

/// The anchor an entity carries, if it carries one.
///
/// Read-only, and public on purpose: the layout is derived everywhere else in
/// the crate, so this is how a caller (or a convergence fixture) sees the
/// *coordinate* rather than the order it produces. There is deliberately no
/// public writer — [`place`] is crate-private, so a door cannot invent an
/// anchor without going through the rule.
pub fn anchor_of(state: &EntityStateDoc, entity_id: &str) -> Option<Anchor> {
    let entities = state.entities_id().ok()?;
    let (_, obj) = state.doc.get(&entities, entity_id).ok()??;
    Some(Anchor {
        after_base: get_u64(&state.doc, &obj, "anchor_after")?,
        tier: get_u64(&state.doc, &obj, "anchor_tier").unwrap_or(2) as u8,
    })
}

/// The entities of a file, in the order the anchor rule puts them.
pub fn ordered_entity_ids(state: &EntityStateDoc, file_path: &str) -> Vec<String> {
    let mut anchored: Vec<(Anchor, String)> = anchors_of_file(state, file_path)
        .into_iter()
        // An entity from a pre-anchor document has no anchor at all; tier 3 is
        // "no evidence whatsoever", which is exactly what it is, and the id
        // tie-break still makes the layout deterministic.
        .map(|(id, a)| {
            (
                a.unwrap_or(Anchor {
                    after_base: 0,
                    tier: 3,
                }),
                id,
            )
        })
        .collect();
    anchored.sort();
    anchored.into_iter().map(|(_, id)| id).collect()
}

/// Every entity of a file with the anchor it carries, if it carries one.
fn anchors_of_file(state: &EntityStateDoc, file_path: &str) -> Vec<(String, Option<Anchor>)> {
    let Ok(entities) = state.entities_id() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for key in state.doc.keys(&entities) {
        let Ok(Some((_, obj))) = state.doc.get(&entities, key.as_str()) else {
            continue;
        };
        if get_str(&state.doc, &obj, "file_path").unwrap_or_default() != file_path {
            continue;
        }
        let anchor = get_u64(&state.doc, &obj, "anchor_after").map(|after_base| Anchor {
            after_base,
            tier: get_u64(&state.doc, &obj, "anchor_tier").unwrap_or(2) as u8,
        });
        out.push((key, anchor));
    }
    out
}

fn write_anchor(state: &mut EntityStateDoc, entity_id: &str, anchor: Anchor) -> Result<()> {
    let entities = state.entities_id()?;
    let Some((_, obj)) = state.doc.get(&entities, entity_id)? else {
        return Ok(());
    };
    state
        .doc
        .put(&obj, "anchor_after", anchor.after_base as i64)?;
    state.doc.put(&obj, "anchor_tier", anchor.tier as i64)?;
    Ok(())
}
