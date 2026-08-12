//! Identity, separated from spelling.
//!
//! `v2::types` states the rule this module exists to import: identity is never a
//! string. weave-crdt cannot hold an arena index across replicas, so it holds
//! the next best thing — an opaque id that no stage is allowed to read meaning
//! out of — and keeps a *separate* index from the parser's spelling of an entity
//! to that id.
//!
//! The bug this closes: the keys of the `entities` map used to BE the parser key
//! `file::type::name`, so an entity's name was stored twice, once as a column
//! and once inside its own identity. `EntityOp::Renamed` updated the column; the
//! key went on spelling the old name; and the next `sync_from_files` looked up
//! the new key, missed, and created a second entity beside the first. The edit
//! history stayed on the row nobody would ask for again.
//!
//! Two properties are load-bearing, and they pull in opposite directions:
//!
//! 1. **An id is minted from the parser key it was first seen under**, not from
//!    a counter or a UUID. Two replicas that meet the same new entity while
//!    disconnected must agree on its id without talking, or the CRDT has two
//!    rows for one function and no join can put them back together. A random id
//!    would break exactly the convergence the old scheme got for free.
//! 2. **After the first sight, the id never moves.** Renames add an *alias*, so
//!    an entity accumulates the spellings it has had and answers to all of them.
//!
//! The consequence of (1) is that an id often looks like a parser key, and after
//! a rename it looks like a *stale* one. That is cosmetic and it is the price of
//! offline convergence: nothing may parse an id, and `entity_id_for` is the only way
//! to get from a spelling to one.

use automerge::{transaction::Transactable, ReadDoc};

use crate::error::Result;
use crate::state::EntityStateDoc;

/// The entity id a parser key names, if the document has seen that key.
///
/// Falls back to the key itself, which is what makes v3 documents readable
/// without a rekeying migration: before v4 every key *was* its entity's id, so
/// the identity mapping is the correct answer for every alias never recorded.
pub(crate) fn entity_id_for(state: &EntityStateDoc, parser_key: &str) -> String {
    let Ok(index) = state.key_index_id() else {
        return parser_key.to_string();
    };
    match state.doc.get(&index, parser_key) {
        Ok(Some((v, _))) => v.into_string().unwrap_or_else(|_| parser_key.to_string()),
        _ => parser_key.to_string(),
    }
}

/// Record that `parser_key` names `entity_id` from now on.
///
/// Idempotent and monotone: an alias is only ever added, never retargeted, so
/// two replicas that learn the same rename in either order agree. Retargeting an
/// existing alias is refused rather than overwritten — two entities claiming one
/// spelling is a fact about the file, not something this index may decide, and
/// silently moving the alias would strand whichever history got there first.
pub(crate) fn bind_alias(
    state: &mut EntityStateDoc,
    parser_key: &str,
    entity_id: &str,
) -> Result<()> {
    let index = state.key_index_id()?;
    if let Ok(Some((v, _))) = state.doc.get(&index, parser_key) {
        if let Ok(existing) = v.into_string() {
            if existing != entity_id {
                return Ok(());
            }
        }
    }
    state.doc.put(&index, parser_key, entity_id)?;
    Ok(())
}
