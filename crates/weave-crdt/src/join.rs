//! Merging entity state across replicas: `weave_core::entity_merge` decides an
//! entity's value from its concurrent writes, and the per-side actions map onto
//! the op vocabulary.
//!
//! The old schema was a concurrent-write detector: it never
//! called entity_merge, content was a last-writer-wins automerge scalar
//! under true concurrency, and its op vocabulary (locks/heartbeats) could
//! not express renames or deletes. All three are structural facts about the
//! old schema, so all three are fixed in the schema rather than in a code
//! path:
//!
//! * **No LWW.** An entity's content is not a scalar any replica overwrites.
//!   It is a map `agent_id → write`, and each write carries the version vector
//!   its author saw. Automerge unions the map by key, so two agents writing
//!   concurrently produce two entries rather than one survivor — there is no
//!   register for a last writer to win. The *value* of the entity is then a
//!   pure function of the entries, computed on read.
//! * **entity_merge decides the value.** When more than one write is causally
//!   maximal, the value is the fold of those writes through
//!   `weave_core::entity_merge` over the common ancestor.
//! * **A conflict is a value.** Not a flag: it carries the ancestor and the set
//!   of sides, and merging two conflicts unions the sides.
//!
//! **Order-independence.** The value is computed from the *set* of
//! causally-maximal writes, and the fold walks that set in a canonical (sorted)
//! order, so the order the ops arrived in cannot reach the fold. Two replicas
//! that hold the same set — which automerge's map union gives them — compute the
//! same value.

use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};

use automerge::{transaction::Transactable, ObjType, ReadDoc};
use serde::Serialize;

use crate::error::Result;
use crate::merge::VersionVector;
use crate::state::EntityStateDoc;

/// The per-side actions, mapped onto the merge-relevant op vocabulary. Claims
/// and heartbeats remain in `ops.rs`
/// because agents still coordinate — but they are advisory, and nothing in the
/// merge path reads them any more.
///
/// ABSENT and UNCHANGED are not ops: they are the absence of one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum EntityOp {
    /// ADDED — a key that was in no ancestor.
    Added {
        entity_id: String,
        name: String,
        entity_type: String,
        file_path: String,
        content: String,
    },
    /// EDITED — same key, new body.
    Edited { entity_id: String, content: String },
    /// RENAMED — new name, body otherwise untouched. The identity is the entity
    /// id, so a rename is one op, not a delete plus an add — the distinction a
    /// lock/heartbeat vocabulary could not make at all. `content` is the entity's
    /// text under the new name, because that is where the name really lives.
    ///
    /// `to_key` is the entity's new *parser key* — the spelling a later
    /// `sync_from_files` will look the entity up under. It is supplied by the
    /// producer, which has the parser, and never synthesized here: weave-crdt
    /// building `file::type::name` strings for itself would couple it to one
    /// parser's key format. Without it the rename lands on the name column
    /// and the next sync creates a second entity instead of finding the one
    /// that already exists.
    Renamed {
        entity_id: String,
        to: String,
        to_key: String,
        content: String,
    },
    /// RENAME+EDITED — a case a per-key matcher tends to lose (it looks like
    /// a delete of the old key plus an add of a new one), and the one a
    /// delete/add op vocabulary cannot express at all.
    RenameEdited {
        entity_id: String,
        to: String,
        to_key: String,
        content: String,
    },
    /// DELETED — a tombstone, never a removal: a key that is simply absent
    /// cannot be told apart from one that never arrived.
    Deleted { entity_id: String },
    /// Relational evidence: this entity's body moved into other keys. Not a
    /// per-key action — it is a fact a per-key alphabet cannot hold, carried
    /// alongside so a re-home is not read as a deletion.
    Rehomed {
        entity_id: String,
        homes: Vec<String>,
    },
}

/// Apply one op on behalf of one agent.
///
/// Every merge-relevant op bottoms out in a write to the register, so there is
/// exactly one door into the content of an entity and it is the one the join
/// reads. The *name* is written too, but as an index rather than as truth: an
/// entity's name lives inside its own text (`def f():`, `static int f(void)`),
/// which is what goes through `entity_merge`, so RENAME and RENAME+EDIT are
/// content writes that happen to change the definition line.
///
/// Honest limit, stated where it bites: the `name` field itself is a plain
/// automerge register. Two replicas renaming concurrently to different names
/// converge on automerge's deterministic tiebreak, not on a join — they agree,
/// but they agree on one of the two names rather than on a conflict. The
/// conflict is not lost: it is in the entity text, where the definition lines
/// differ, and the register reports it.
pub fn apply_op(state: &mut EntityStateDoc, agent_id: &str, op: &EntityOp) -> Result<()> {
    let hash = |s: &str| {
        let mut h = DefaultHasher::new();
        s.hash(&mut h);
        format!("{:x}", h.finish())
    };
    match op {
        EntityOp::Added {
            entity_id,
            name,
            entity_type,
            file_path,
            content,
        } => {
            crate::ops::upsert_entity(
                state,
                entity_id,
                name,
                entity_type,
                file_path,
                &hash(content),
            )?;
            crate::content::update_entity_content(
                state,
                agent_id,
                entity_id,
                content,
                &hash(content),
            )?;
        }
        EntityOp::Edited { entity_id, content } => {
            crate::content::update_entity_content(
                state,
                agent_id,
                entity_id,
                content,
                &hash(content),
            )?;
        }
        EntityOp::Renamed {
            entity_id,
            to,
            to_key,
            content,
        }
        | EntityOp::RenameEdited {
            entity_id,
            to,
            to_key,
            content,
        } => {
            crate::content::update_entity_content(
                state,
                agent_id,
                entity_id,
                content,
                &hash(content),
            )?;
            // The id the caller named may itself be an older spelling.
            let resolved = crate::identity::entity_id_for(state, entity_id);
            let entities = state.entities_id()?;
            if let Some((_, obj)) = state.doc.get(&entities, resolved.as_str())? {
                state.doc.put(&obj, "name", to.as_str())?;
            }
            // The rename's real work: the new spelling becomes another alias of
            // the SAME entity, so the next `sync_from_files` — which sees only
            // the new key — resolves to the row that holds the history instead
            // of minting a fresh one beside it.
            crate::identity::bind_alias(state, to_key, resolved.as_str())?;
        }
        EntityOp::Deleted { entity_id } => {
            crate::content::delete_entity_content(state, agent_id, entity_id)?;
        }
        EntityOp::Rehomed { entity_id, homes } => {
            let entities = state.entities_id()?;
            if let Some((_, obj)) = state.doc.get(&entities, entity_id.as_str())? {
                let list = state.doc.put_object(&obj, "rehomed_into", ObjType::List)?;
                for (i, h) in homes.iter().enumerate() {
                    state.doc.insert(&list, i, h.as_str())?;
                }
            }
        }
    }
    Ok(())
}

/// One agent's causally-latest write to one entity.
///
/// The author is the key this write is filed under in the register, and is
/// deliberately *not* repeated inside the value: it used to be, and nothing
/// ever read the inner copy — the same "stored twice, once as a column and
/// once inside its own identity" shape this schema avoids everywhere else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Write {
    pub content: String,
    pub deleted: bool,
    /// The version vector the author had seen, including their own increment.
    pub vv: VersionVector,
}

/// The value of an entity, computed from its writes.
///
/// `Conflict` is a first-class value, not an error and not a flag on a value.
/// That is what makes it *mergeable*: two replicas that both learned about the
/// same conflict join to the same conflict, and a replica that learns about a
/// conflict it already has does not change state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntityValue {
    /// No write has ever been seen.
    Absent,
    /// Everyone who wrote agrees, or their writes merged cleanly.
    Content(String),
    /// The entity is deleted, and nobody contested it.
    Tombstone,
    /// Irreconcilable sides over a shared ancestor. A deletion is carried as
    /// the empty side, so modify/delete is the same value shape as
    /// modify/modify rather than a special case.
    ///
    /// Honest limit, since the side is just a string: the empty side stands for
    /// a deletion, so a (degenerate) write whose content is genuinely empty
    /// cannot be told apart from a tombstone here, and [`Self::materialize`]
    /// labels any empty side "deleted". Real code entities always have a
    /// definition line, so an empty *live* side does not arise in practice —
    /// but the label is an inference from emptiness, not a recorded fact, and
    /// that is the bound.
    Conflict {
        base: String,
        sides: BTreeSet<String>,
    },
}

impl EntityValue {
    /// The wire name for this value's merge state.
    ///
    /// The join owns it, because the join is what decides it. `ops.rs` and
    /// `content.rs` each used to spell the same two-armed match out for
    /// themselves, and `merge.rs` carried a third spelling in a `MergeState`
    /// enum nothing constructed.
    pub fn merge_state(&self) -> &'static str {
        if self.is_conflict() {
            "conflict"
        } else {
            "clean"
        }
    }

    pub(crate) fn is_conflict(&self) -> bool {
        matches!(self, EntityValue::Conflict { .. })
    }

    /// The text a replica materializes for this value.
    pub(crate) fn materialize(&self) -> String {
        match self {
            EntityValue::Absent | EntityValue::Tombstone => String::new(),
            EntityValue::Content(c) => c.clone(),
            EntityValue::Conflict { sides, .. } => {
                let mut out = String::new();
                let live: Vec<&String> = sides.iter().filter(|s| !s.is_empty()).collect();
                let deleted = sides.iter().any(|s| s.is_empty());
                for (i, s) in live.iter().enumerate() {
                    out.push_str(if i == 0 { "<<<<<<< " } else { "======= " });
                    out.push_str(&format!("side {}\n", i + 1));
                    out.push_str(s);
                    if !s.ends_with('\n') {
                        out.push('\n');
                    }
                }
                if deleted {
                    out.push_str("======= deleted\n");
                }
                out.push_str(">>>>>>> end\n");
                out
            }
        }
    }
}

/// Merge two entity values into one.
///
/// The operands are put in a canonical order by [`rank`] before the match, so a
/// pairing of two DIFFERENT ranks (Absent/Tombstone/Content/Conflict) does not
/// depend on argument order; the conflict cases use set unions and `min` on the
/// base, which do not either. The one case rank does NOT canonicalize is
/// `Content` vs `Content` (equal rank, so the caller's order survives): there
/// the value is `entity_merge(base, x, y)`, and swapping the call to
/// `entity_merge(base, y, x)` washes out only because `weave_core::entity_merge`
/// returns the same bytes for a clean merge with its ours/theirs operands
/// swapped — that is relied on, not established here. The conflicting sub-case
/// is order-free regardless: the sides are collected into a `BTreeSet`.
///
/// Equal operands short-circuit at the top, and the conflict case is a set
/// union.
pub fn join(a: &EntityValue, b: &EntityValue, base: &str, file_path: &str) -> EntityValue {
    use EntityValue as V;
    if a == b {
        return a.clone();
    }
    // Canonical operand order — see the note above.
    let (a, b) = if rank(a) <= rank(b) { (a, b) } else { (b, a) };
    match (a, b) {
        (V::Absent, other) => other.clone(),
        (V::Tombstone, V::Content(c)) => {
            // Modify vs. delete. If the survivor never changed the body, the
            // deletion is uncontested; otherwise nobody may pick.
            if c == base {
                V::Tombstone
            } else {
                V::Conflict {
                    base: base.to_string(),
                    sides: [String::new(), c.clone()].into_iter().collect(),
                }
            }
        }
        (V::Content(x), V::Content(y)) => {
            let merged = weave_core::entity_merge(base, x, y, file_path);
            if merged.is_clean() {
                V::Content(merged.content)
            } else {
                V::Conflict {
                    base: base.to_string(),
                    sides: [x.clone(), y.clone()].into_iter().collect(),
                }
            }
        }
        (V::Tombstone, V::Conflict { base: b0, sides }) => {
            let mut sides = sides.clone();
            sides.insert(String::new());
            V::Conflict {
                base: b0.clone(),
                sides,
            }
        }
        (V::Content(c), V::Conflict { base: b0, sides }) => {
            let mut sides = sides.clone();
            sides.insert(c.clone());
            V::Conflict {
                base: b0.clone(),
                sides,
            }
        }
        (
            V::Conflict {
                base: b0,
                sides: s0,
            },
            V::Conflict {
                base: b1,
                sides: s1,
            },
        ) => V::Conflict {
            // The ancestor is part of the value, so joining two conflicts that
            // disagree about it has to be deterministic as well.
            base: b0.min(b1).clone(),
            sides: s0.union(s1).cloned().collect(),
        },
        // Every remaining pair is `(x, y)` with rank(x) <= rank(y) already
        // covered above, or the equal case returned at the top.
        (x, _) => x.clone(),
    }
}

fn rank(v: &EntityValue) -> u8 {
    match v {
        EntityValue::Absent => 0,
        EntityValue::Tombstone => 1,
        EntityValue::Content(_) => 2,
        EntityValue::Conflict { .. } => 3,
    }
}

/// The value of an entity, given every write any replica made and the shared
/// ancestor.
///
/// Two steps, and the order of neither depends on delivery:
///   1. the *frontier* — writes no other write causally dominates. A write the
///      author made after seeing another is not concurrent with it, and folding
///      it in as if it were would resurrect content its author had replaced.
///   2. the fold, over the frontier taken in agent-id order.
pub fn value_of(writes: &BTreeMap<String, Write>, base: &str, file_path: &str) -> EntityValue {
    let frontier = frontier(writes);
    let mut acc = EntityValue::Absent;
    for w in &frontier {
        let v = if w.deleted {
            EntityValue::Tombstone
        } else {
            EntityValue::Content(w.content.clone())
        };
        acc = join(&acc, &v, base, file_path);
    }
    acc
}

/// The causally-maximal writes, in agent-id order.
pub(crate) fn frontier(writes: &BTreeMap<String, Write>) -> Vec<&Write> {
    writes
        .values()
        .filter(|w| {
            !writes.values().any(|other| {
                !std::ptr::eq(*w, other)
                    && other.vv.partial_cmp(&w.vv) == Some(std::cmp::Ordering::Greater)
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vv(pairs: &[(&str, u64)]) -> VersionVector {
        let mut v = VersionVector::new();
        for (a, n) in pairs {
            for _ in 0..*n {
                v.increment(a);
            }
        }
        v
    }

    fn w(content: &str, clock: &[(&str, u64)]) -> Write {
        Write {
            content: content.to_string(),
            deleted: false,
            vv: vv(clock),
        }
    }

    #[test]
    fn a_write_that_saw_another_is_not_concurrent_with_it() {
        let writes: BTreeMap<String, Write> = [
            ("a".to_string(), w("first", &[("a", 1)])),
            ("b".to_string(), w("second", &[("a", 1), ("b", 1)])),
        ]
        .into_iter()
        .collect();
        // b wrote after seeing a, so there is nothing to merge — and picking b
        // here is causality, not last-writer-wins.
        assert_eq!(
            value_of(&writes, "", "x.py"),
            EntityValue::Content("second".into())
        );
    }

    #[test]
    fn two_concurrent_writes_go_through_entity_merge() {
        let base = "def f():\n    a()\n    mid1()\n    mid2()\n    b()\n";
        let ours = "def f():\n    a2()\n    mid1()\n    mid2()\n    b()\n";
        let theirs = "def f():\n    a()\n    mid1()\n    mid2()\n    b2()\n";
        let writes: BTreeMap<String, Write> = [
            ("a".to_string(), w(ours, &[("a", 1)])),
            ("b".to_string(), w(theirs, &[("b", 1)])),
        ]
        .into_iter()
        .collect();
        let v = value_of(&writes, base, "x.py");
        match v {
            EntityValue::Content(c) => {
                assert!(c.contains("a2()") && c.contains("b2()"), "{c}");
            }
            other => panic!("disjoint hunks should merge cleanly, got {other:?}"),
        }
    }

    #[test]
    fn a_deletion_against_an_untouched_body_is_uncontested() {
        let base = "def f():\n    a()\n";
        let mut del = w("", &[("a", 1)]);
        del.deleted = true;
        let writes: BTreeMap<String, Write> = [
            ("a".to_string(), del),
            ("b".to_string(), w(base, &[("b", 1)])),
        ]
        .into_iter()
        .collect();
        assert_eq!(value_of(&writes, base, "x.py"), EntityValue::Tombstone);
    }

    #[test]
    fn a_deletion_against_an_edit_is_a_conflict_value() {
        let base = "def f():\n    a()\n";
        let mut del = w("", &[("a", 1)]);
        del.deleted = true;
        let writes: BTreeMap<String, Write> = [
            ("a".to_string(), del),
            ("b".to_string(), w("def f():\n    a2()\n", &[("b", 1)])),
        ]
        .into_iter()
        .collect();
        assert!(value_of(&writes, base, "x.py").is_conflict());
    }
}
