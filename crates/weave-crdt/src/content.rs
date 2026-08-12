//! Entity content in the CRDT: a multi-value register, not a scalar.
//!
//! Every write lands under its author's key in a `writes` map together with the
//! version vector its author had seen. Automerge unions maps by key, so two
//! agents writing concurrently produce two entries; there is no single slot for
//! a last writer to win. The entity's *content* is derived on read by
//! [`crate::join::value_of`], which folds the causally-maximal writes through
//! `weave_core::entity_merge`.
//!
//! The fields `content`, `merge_state`, `conflict_ours`, `conflict_theirs` are
//! still in [`EntityContentStatus`] because callers ask for them — but they are
//! *computed*, not stored, so no code path can put a merged body back into the
//! document and make it authoritative.

use std::collections::{BTreeMap, HashMap};

use automerge::{
    transaction::Transactable, AutoCommit, ObjId, ObjType, ReadDoc, ScalarValue, Value,
};
use serde::Serialize;

use crate::error::{Result, WeaveError};
use crate::join::{value_of, EntityValue, Write};
use crate::merge::VersionVector;
use crate::ops::{get_str, get_u64, read_version_vector, write_version_vector};
use crate::state::EntityStateDoc;

/// Full content status for an entity in the CRDT.
#[derive(Debug, Clone, Serialize)]
pub struct EntityContentStatus {
    pub entity_id: String,
    pub name: String,
    pub content: String,
    pub base_content: String,
    pub content_hash: String,
    pub version_vector: VersionVector,
    pub merge_state: String,
    pub conflict_ours: Option<String>,
    pub conflict_theirs: Option<String>,
    pub conflict_base: Option<String>,
}

// ---------------------------------------------------------------------------
// Reading the register
// ---------------------------------------------------------------------------

/// Every agent's causally-latest write to this entity.
pub(crate) fn read_writes(doc: &AutoCommit, entity_obj: &ObjId) -> BTreeMap<String, Write> {
    let mut out = BTreeMap::new();
    let Ok(Some((_, writes_obj))) = doc.get(entity_obj, "writes") else {
        return out;
    };
    for agent in doc.keys(&writes_obj) {
        let Ok(Some((_, w))) = doc.get(&writes_obj, agent.as_str()) else {
            continue;
        };
        let mut vv = VersionVector::new();
        if let Ok(Some((_, vv_obj))) = doc.get(&w, "vv") {
            let map: HashMap<String, u64> = doc
                .keys(&vv_obj)
                .filter_map(|k| get_u64(doc, &vv_obj, &k).map(|n| (k, n)))
                .collect();
            vv = VersionVector::from_map(map);
        }
        out.insert(
            agent,
            Write {
                content: get_str(doc, &w, "content").unwrap_or_default(),
                deleted: matches!(
                    doc.get(&w, "deleted"),
                    Ok(Some((Value::Scalar(ref v), _)))
                        if matches!(v.as_ref(), ScalarValue::Boolean(true))
                ),
                vv,
            },
        );
    }
    out
}

/// The joined value of an entity — the whole merge path, in one function.
pub(crate) fn entity_value(doc: &AutoCommit, entity_obj: &ObjId) -> EntityValue {
    let base = get_str(doc, entity_obj, "base_content").unwrap_or_default();
    let file_path = get_str(doc, entity_obj, "file_path").unwrap_or_default();
    let writes = read_writes(doc, entity_obj);
    value_of(&writes, &base, &file_path)
}

// ---------------------------------------------------------------------------
// Writing
// ---------------------------------------------------------------------------

/// Record `agent_id`'s write of an entity.
///
/// The write is stored under the agent's own key with the version vector the
/// agent had seen, so a later reader can tell "wrote after seeing you" from
/// "wrote without seeing you" — the distinction the old scalar `content` field
/// destroyed.
pub fn update_entity_content(
    state: &mut EntityStateDoc,
    agent_id: &str,
    entity_id: &str,
    content: &str,
    content_hash: &str,
) -> Result<()> {
    write_entity(state, agent_id, entity_id, content, content_hash, false)
}

/// Record `agent_id`'s deletion of an entity: a tombstone write, so the
/// deletion is a value that can be joined rather than an absence that cannot.
pub(crate) fn delete_entity_content(
    state: &mut EntityStateDoc,
    agent_id: &str,
    entity_id: &str,
) -> Result<()> {
    write_entity(state, agent_id, entity_id, "", "", true)
}

fn write_entity(
    state: &mut EntityStateDoc,
    agent_id: &str,
    entity_id: &str,
    content: &str,
    content_hash: &str,
    deleted: bool,
) -> Result<()> {
    // Any spelling this entity has ever had resolves to it.
    let resolved = crate::identity::entity_id_for(state, entity_id);
    let entity_id = resolved.as_str();
    let entities = state.entities_id()?;
    let Some((_, entity_obj)) = state.doc.get(&entities, entity_id)? else {
        return Err(WeaveError::EntityNotFound(entity_id.to_string()));
    };
    // The clock this write happened at: everything this replica has seen, plus
    // one of the author's own. A *version vector*, and only that — the entry
    // used to carry an `at: now_ms()` alongside, which is a second, unreliable
    // opinion about ordering sitting on the causal path.
    let mut vv = read_version_vector(&state.doc, &entity_obj);
    for w in read_writes(&state.doc, &entity_obj).values() {
        vv.merge(&w.vv);
    }
    vv.increment(agent_id);
    write_version_vector(&mut state.doc, &entity_obj, agent_id, vv.get(agent_id))?;

    // The ancestor moves forward the first time an entity that already had a
    // settled value is written again: that value is what the next concurrent
    // pair will be merged over.
    let settled = entity_value(&state.doc, &entity_obj);
    if let EntityValue::Content(c) = &settled {
        let base = get_str(&state.doc, &entity_obj, "base_content").unwrap_or_default();
        if base.is_empty() && !c.is_empty() {
            state.doc.put(&entity_obj, "base_content", c.as_str())?;
        }
    }

    let writes_obj = match state.doc.get(&entity_obj, "writes")? {
        Some((_, id)) => id,
        None => state.doc.put_object(&entity_obj, "writes", ObjType::Map)?,
    };
    // One agent id is one causal thread, so replacing that agent's own entry is
    // not a lost update — it is the agent superseding itself.
    let w = state.doc.put_object(&writes_obj, agent_id, ObjType::Map)?;
    state.doc.put(&w, "content", content)?;
    state.doc.put(&w, "deleted", deleted)?;
    state.doc.put(&w, "hash", content_hash)?;
    let vv_obj = state.doc.put_object(&w, "vv", ObjType::Map)?;
    for (agent, &count) in vv.counters() {
        state.doc.put(&vv_obj, agent.as_str(), count as i64)?;
    }

    state.doc.put(&entity_obj, "content_hash", content_hash)?;
    state.doc.put(&entity_obj, "version", vv.total() as i64)?;
    state.doc.put(&entity_obj, "last_modified_by", agent_id)?;
    Ok(())
}

/// Get the full content status of an entity. Everything merge-relevant here is
/// derived from the register, so two replicas that hold the same writes report
/// the same status without having exchanged a decision.
pub fn get_entity_content(state: &EntityStateDoc, entity_id: &str) -> Result<EntityContentStatus> {
    // Any spelling this entity has ever had resolves to it.
    let resolved = crate::identity::entity_id_for(state, entity_id);
    let entity_id = resolved.as_str();
    let entities = state.entities_id()?;
    let Some((_, entity_obj)) = state.doc.get(&entities, entity_id)? else {
        return Err(WeaveError::EntityNotFound(entity_id.to_string()));
    };

    let base_content = get_str(&state.doc, &entity_obj, "base_content").unwrap_or_default();
    let writes = read_writes(&state.doc, &entity_obj);
    let value = entity_value(&state.doc, &entity_obj);

    // The version vector a caller sees is the join of every write's clock, not
    // a separately maintained field that could disagree with them.
    let mut vv = read_version_vector(&state.doc, &entity_obj);
    for w in writes.values() {
        vv.merge(&w.vv);
    }

    // The two sides of a conflict, in the register's own order.
    //
    // There used to be `conflict_ours_agent` / `conflict_theirs_agent`
    // alongside these, taken from `frontier(&writes)` by index. That paired two
    // differently-ordered sequences: `sides` is a BTreeSet ordered by CONTENT,
    // the frontier is a BTreeMap ordered by AGENT ID, so side[0] and agent[0]
    // named the same writer only by coincidence — and for a modify/delete
    // conflict, whose empty side sorts first, reliably did not. Nothing read
    // them, so the fix is to stop claiming to know.
    let (ours, theirs) = match &value {
        EntityValue::Conflict { sides, .. } => {
            let mut it = sides.iter();
            (it.next().cloned(), it.next().cloned())
        }
        _ => (None, None),
    };

    Ok(EntityContentStatus {
        entity_id: entity_id.to_string(),
        name: get_str(&state.doc, &entity_obj, "name").unwrap_or_default(),
        content: value.materialize(),
        base_content: base_content.clone(),
        content_hash: get_str(&state.doc, &entity_obj, "content_hash").unwrap_or_default(),
        version_vector: vv,
        merge_state: value.merge_state().to_string(),
        conflict_base: value.is_conflict().then_some(base_content),
        conflict_ours: ours,
        conflict_theirs: theirs,
    })
}

/// Resolve a conflict on an entity by providing the resolved content.
///
/// The resolution is an ordinary write — its clock dominates every contender,
/// so the frontier collapses to one and the value becomes `Content` again. It
/// also moves the ancestor forward, which is what stops the same conflict from
/// being rediscovered by the next join.
pub fn resolve_entity_conflict(
    state: &mut EntityStateDoc,
    agent_id: &str,
    entity_id: &str,
    resolved_content: &str,
    content_hash: &str,
) -> Result<()> {
    // Any spelling this entity has ever had resolves to it.
    let resolved = crate::identity::entity_id_for(state, entity_id);
    let entity_id = resolved.as_str();
    let entities = state.entities_id()?;
    let Some((_, entity_obj)) = state.doc.get(&entities, entity_id)? else {
        return Err(WeaveError::EntityNotFound(entity_id.to_string()));
    };
    if !entity_value(&state.doc, &entity_obj).is_conflict() {
        return Err(WeaveError::NotInConflict(entity_id.to_string()));
    }
    write_entity(
        state,
        agent_id,
        entity_id,
        resolved_content,
        content_hash,
        false,
    )?;
    let entities = state.entities_id()?;
    if let Some((_, entity_obj)) = state.doc.get(&entities, entity_id)? {
        state
            .doc
            .put(&entity_obj, "base_content", resolved_content)?;
    }
    Ok(())
}

/// Put an entity into conflict, by writing the two sides as genuinely
/// concurrent writes. Used by tests and by `sync` when the working tree and the
/// CRDT diverged from the same ancestor.
#[cfg(any(test, feature = "test-helpers"))]
pub fn set_entity_conflict(
    state: &mut EntityStateDoc,
    entity_id: &str,
    ours: &str,
    theirs: &str,
    base: &str,
    ours_agent: &str,
    theirs_agent: &str,
) -> Result<()> {
    crate::sync::seed_concurrent_writes(
        state,
        entity_id,
        base,
        &[(ours_agent, ours), (theirs_agent, theirs)],
    )
}
