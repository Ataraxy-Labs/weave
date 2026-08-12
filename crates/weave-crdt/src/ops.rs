use std::collections::HashMap;

use automerge::{
    transaction::Transactable, AutoCommit, ObjId, ObjType, ReadDoc, ScalarValue, Value,
};
use serde::Serialize;

use crate::anchor::Observation;
use crate::error::{Result, WeaveError};
use crate::merge::VersionVector;
use crate::state::{now_ms, EntityStateDoc};

// ── Result types ──

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ClaimResult {
    Claimed,
    AlreadyOwnedBySelf,
    AlreadyClaimed { by: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct EntityStatus {
    pub entity_id: String,
    pub name: String,
    pub entity_type: String,
    pub file_path: String,
    pub content_hash: String,
    pub claimed_by: Option<String>,
    pub claimed_at: Option<u64>,
    pub last_modified_by: Option<String>,
    pub version: u64,
    pub version_vector: VersionVector,
    pub merge_state: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentStatus {
    pub agent_id: String,
    pub name: String,
    pub status: String,
    pub branch: String,
    pub last_seen: u64,
    pub working_on: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PotentialConflict {
    pub entity_id: String,
    pub entity_name: String,
    pub file_path: String,
    pub agents: Vec<String>,
}

// ── Helper to read a string field from an automerge map ──

pub(crate) fn get_str(doc: &AutoCommit, obj: &ObjId, key: &str) -> Option<String> {
    match doc.get(obj, key) {
        Ok(Some((Value::Scalar(v), _))) => {
            if let ScalarValue::Str(s) = v.as_ref() {
                Some(s.to_string())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Every concurrent string value automerge holds for `key`, not just the one
/// `get` picks.
///
/// A scalar `put` under true concurrency does not overwrite — automerge keeps
/// both values and `get` returns an arbitrary winner. `get_str` therefore
/// destroys the evidence of a contest; `get_all_str` preserves it, so a field
/// two replicas wrote concurrently (a `claimed_by` both agents were told they
/// held) reads as the set it actually is.
pub(crate) fn get_all_str(doc: &AutoCommit, obj: &ObjId, key: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(values) = doc.get_all(obj, key) {
        for (v, _) in values {
            if let Value::Scalar(s) = v {
                if let ScalarValue::Str(s) = s.as_ref() {
                    let s = s.to_string();
                    if !out.contains(&s) {
                        out.push(s);
                    }
                }
            }
        }
    }
    out
}

pub(crate) fn get_u64(doc: &AutoCommit, obj: &ObjId, key: &str) -> Option<u64> {
    match doc.get(obj, key) {
        Ok(Some((Value::Scalar(v), _))) => match v.as_ref() {
            ScalarValue::Uint(n) => Some(*n),
            ScalarValue::Int(n) => Some(*n as u64),
            _ => None,
        },
        _ => None,
    }
}

// ── Operations ──

/// Claim an entity for an agent. Advisory lock — doesn't prevent edits.
pub fn claim_entity(
    state: &mut EntityStateDoc,
    agent_id: &str,
    entity_id: &str,
) -> Result<ClaimResult> {
    let entities = state.entities_id()?;

    // Check if entity exists in state
    let Some((_, entity_obj)) = state.doc.get(&entities, entity_id)? else {
        return Err(WeaveError::EntityNotFound(entity_id.to_string()));
    };

    // Check current claim
    let current_claim = get_str(&state.doc, &entity_obj, "claimed_by");
    if let Some(ref owner) = current_claim {
        if owner == agent_id {
            return Ok(ClaimResult::AlreadyOwnedBySelf);
        }
        return Ok(ClaimResult::AlreadyClaimed { by: owner.clone() });
    }

    // Set claim
    let ts = now_ms();
    state.doc.put(&entity_obj, "claimed_by", agent_id)?;
    state.doc.put(&entity_obj, "claimed_at", ts as i64)?;

    // Log operation
    log_operation(state, agent_id, entity_id, "claim")?;

    Ok(ClaimResult::Claimed)
}

/// Release an entity claim.
pub fn release_entity(state: &mut EntityStateDoc, agent_id: &str, entity_id: &str) -> Result<()> {
    let entities = state.entities_id()?;

    let Some((_, entity_obj)) = state.doc.get(&entities, entity_id)? else {
        return Err(WeaveError::EntityNotFound(entity_id.to_string()));
    };

    // Only release if this agent owns it
    let current_claim = get_str(&state.doc, &entity_obj, "claimed_by");
    if current_claim.as_deref() == Some(agent_id) {
        state.doc.delete(&entity_obj, "claimed_by")?;
        state.doc.delete(&entity_obj, "claimed_at")?;
        log_operation(state, agent_id, entity_id, "release")?;
    }

    Ok(())
}

/// The status of one entity, read off its object. One spelling: the two public
/// readers below used to build this struct field-by-field independently, which
/// is two places for the same fact to drift.
fn status_of(doc: &AutoCommit, entity_id: &str, entity_obj: &ObjId) -> EntityStatus {
    let vv = read_version_vector(doc, entity_obj);
    EntityStatus {
        entity_id: entity_id.to_string(),
        name: get_str(doc, entity_obj, "name").unwrap_or_default(),
        entity_type: get_str(doc, entity_obj, "type").unwrap_or_default(),
        file_path: get_str(doc, entity_obj, "file_path").unwrap_or_default(),
        content_hash: get_str(doc, entity_obj, "content_hash").unwrap_or_default(),
        claimed_by: get_str(doc, entity_obj, "claimed_by"),
        claimed_at: get_u64(doc, entity_obj, "claimed_at"),
        last_modified_by: get_str(doc, entity_obj, "last_modified_by"),
        // Whichever is larger, for backward compat with the pre-VV scalar.
        version: vv
            .total()
            .max(get_u64(doc, entity_obj, "version").unwrap_or(0)),
        version_vector: vv,
        // Derived, never stored: a merge state that lived in a field could
        // disagree with the writes it is supposed to summarise.
        merge_state: crate::content::entity_value(doc, entity_obj)
            .merge_state()
            .to_string(),
    }
}

/// Get the status of an entity.
pub fn get_entity_status(state: &EntityStateDoc, entity_id: &str) -> Result<EntityStatus> {
    let entities = state.entities_id()?;
    let (_, obj) = state
        .doc
        .get(&entities, entity_id)?
        .ok_or_else(|| WeaveError::EntityNotFound(entity_id.to_string()))?;
    Ok(status_of(&state.doc, entity_id, &obj))
}

/// Get all entities for a given file path.
pub fn get_entities_for_file(state: &EntityStateDoc, file_path: &str) -> Result<Vec<EntityStatus>> {
    let entities = state.entities_id()?;
    let mut result = Vec::new();
    for key in state.doc.keys(&entities) {
        let Some((_, obj)) = state.doc.get(&entities, key.as_str())? else {
            continue;
        };
        if get_str(&state.doc, &obj, "file_path").unwrap_or_default() == file_path {
            result.push(status_of(&state.doc, &key, &obj));
        }
    }
    Ok(result)
}

/// Get the status of an agent.
pub fn get_agent_status(state: &EntityStateDoc, agent_id: &str) -> Result<AgentStatus> {
    let agents = state.agents_id()?;

    let Some((_, agent_obj)) = state.doc.get(&agents, agent_id)? else {
        return Err(WeaveError::AgentNotFound(agent_id.to_string()));
    };

    // Read working_on list
    let working_on = match state.doc.get(&agent_obj, "working_on")? {
        Some((_, list_id)) => {
            let len = state.doc.length(&list_id);
            let mut items = Vec::new();
            for i in 0..len {
                if let Ok(Some((Value::Scalar(v), _))) = state.doc.get(&list_id, i) {
                    if let ScalarValue::Str(s) = v.as_ref() {
                        items.push(s.to_string());
                    }
                }
            }
            items
        }
        None => Vec::new(),
    };

    Ok(AgentStatus {
        agent_id: agent_id.to_string(),
        name: get_str(&state.doc, &agent_obj, "name").unwrap_or_default(),
        status: get_str(&state.doc, &agent_obj, "status").unwrap_or("unknown".to_string()),
        branch: get_str(&state.doc, &agent_obj, "branch").unwrap_or_default(),
        last_seen: get_u64(&state.doc, &agent_obj, "last_seen").unwrap_or(0),
        working_on,
    })
}

/// Register an agent in the state.
pub fn register_agent(
    state: &mut EntityStateDoc,
    agent_id: &str,
    name: &str,
    branch: &str,
) -> Result<()> {
    let agents = state.agents_id()?;

    let agent_obj = state.doc.put_object(&agents, agent_id, ObjType::Map)?;
    state.doc.put(&agent_obj, "name", name)?;
    state.doc.put(&agent_obj, "status", "active")?;
    state.doc.put(&agent_obj, "branch", branch)?;
    state.doc.put(&agent_obj, "last_seen", now_ms() as i64)?;
    state
        .doc
        .put_object(&agent_obj, "working_on", ObjType::List)?;

    Ok(())
}

/// Update agent heartbeat and working_on list.
pub fn agent_heartbeat(
    state: &mut EntityStateDoc,
    agent_id: &str,
    working_on: &[String],
) -> Result<()> {
    let agents = state.agents_id()?;

    let Some((_, agent_obj)) = state.doc.get(&agents, agent_id)? else {
        return Err(WeaveError::AgentNotFound(agent_id.to_string()));
    };

    state.doc.put(&agent_obj, "last_seen", now_ms() as i64)?;
    state.doc.put(&agent_obj, "status", "active")?;

    // Replace working_on list
    let list_id = state
        .doc
        .put_object(&agent_obj, "working_on", ObjType::List)?;
    for (i, entity_id) in working_on.iter().enumerate() {
        state.doc.insert(&list_id, i, entity_id.as_str())?;
    }

    Ok(())
}

/// Clean up stale agents: release their claims and mark them inactive.
///
/// **Nothing in weave calls this.** It is the only writer of
/// `agent.status = "stale"`, and [`detect_potential_conflicts`] is the only
/// reader — so on every shipped path that filter is a no-op and a crashed
/// agent's claims are held forever. It is kept, rather than deleted, because
/// deleting it would leave the reader checking for a status no code can ever
/// produce, which reads like a bug the next person has to rediscover. Giving it
/// a caller is a product decision (which door reaps, on what timeout) and not a
/// cleanup; until then this comment and the one on the reader are the honest
/// statement of where the gap is.
pub fn cleanup_stale_agents(state: &mut EntityStateDoc, timeout_ms: u64) -> Result<Vec<String>> {
    let now = now_ms();
    let agents = state.agents_id()?;
    let mut stale = Vec::new();

    // Collect stale agent IDs
    let agent_keys: Vec<String> = state.doc.keys(&agents).collect();
    for key in &agent_keys {
        let Some((_, agent_obj)) = state.doc.get(&agents, key.as_str())? else {
            continue;
        };
        let last_seen = get_u64(&state.doc, &agent_obj, "last_seen").unwrap_or(0);
        // `now` is THIS replica's wall clock; `last_seen` was stamped from the
        // agent's OWN machine and arrived here through a merge. Two
        // unsynchronized clocks are not a total order, so `now - last_seen` is
        // not a duration: a peer one second fast makes `last_seen > now`, which
        // in debug panics ("subtract with overflow") — taking the whole reaper
        // down before it reaps ANY agent — and in release wraps to ~2^64,
        // declaring the freshest possible agent stale and releasing its live
        // claims.
        //
        // `saturating_sub` reads a future `last_seen` as "zero elapsed,
        // therefore not stale", the only answer a clock this replica cannot
        // verify honestly supports. LIMIT (documented, not fixed): clock skew
        // is thereby indistinguishable from genuine recency — an agent whose
        // clock is far enough ahead is never reaped. Distinguishing the two
        // needs causal evidence (a VersionVector), not a wall-clock difference;
        // `last_seen` carries none, so this is the honest bound until a
        // heartbeat does. No malice is required: an NTP step or a VM resume is
        // enough, which is why one skewed peer must not deny the reaper to
        // every other agent.
        if now.saturating_sub(last_seen) > timeout_ms {
            stale.push(key.clone());
        }
    }

    // Release claims and mark inactive
    for agent_id in &stale {
        // Mark agent as stale
        let Some((_, agent_obj)) = state.doc.get(&agents, agent_id.as_str())? else {
            continue;
        };
        state.doc.put(&agent_obj, "status", "stale")?;

        // Release all entity claims held by this agent
        let entities = state.entities_id()?;
        let entity_keys: Vec<String> = state.doc.keys(&entities).collect();
        for ek in &entity_keys {
            let Some((_, entity_obj)) = state.doc.get(&entities, ek.as_str())? else {
                continue;
            };
            if get_str(&state.doc, &entity_obj, "claimed_by").as_deref() == Some(agent_id.as_str())
            {
                state.doc.delete(&entity_obj, "claimed_by")?;
                state.doc.delete(&entity_obj, "claimed_at")?;
            }
        }
    }

    Ok(stale)
}

/// Detect entities being touched/claimed by multiple agents.
pub fn detect_potential_conflicts(state: &EntityStateDoc) -> Result<Vec<PotentialConflict>> {
    let entities = state.entities_id()?;
    let agents = state.agents_id()?;
    let mut conflicts = Vec::new();

    // Build map: entity_id → set of agents working on it
    let mut entity_agents: HashMap<String, Vec<String>> = HashMap::new();

    // From agent working_on lists
    let agent_keys: Vec<String> = state.doc.keys(&agents).collect();
    for ak in &agent_keys {
        let Some((_, agent_obj)) = state.doc.get(&agents, ak.as_str())? else {
            continue;
        };
        // Only [`cleanup_stale_agents`] ever writes this status, and nothing
        // calls it, so today this filter never fires: a crashed agent's claims
        // still show up as live collisions. See that function.
        let agent_status = get_str(&state.doc, &agent_obj, "status").unwrap_or_default();
        if agent_status == "stale" {
            continue;
        }
        if let Ok(Some((_, list_id))) = state.doc.get(&agent_obj, "working_on") {
            let len = state.doc.length(&list_id);
            for i in 0..len {
                if let Ok(Some((Value::Scalar(v), _))) = state.doc.get(&list_id, i) {
                    if let ScalarValue::Str(s) = v.as_ref() {
                        entity_agents
                            .entry(s.to_string())
                            .or_default()
                            .push(ak.clone());
                    }
                }
            }
        }
    }

    // Also check claimed_by
    let entity_keys: Vec<String> = state.doc.keys(&entities).collect();
    for ek in &entity_keys {
        let Some((_, entity_obj)) = state.doc.get(&entities, ek.as_str())? else {
            continue;
        };
        // Read EVERY claimant automerge holds, not the single one `get` picks:
        // two agents on two replicas each told `Claimed` for one entity leave
        // two concurrent `claimed_by` values, and reporting only the tiebreak
        // winner is how the loser's contest became invisible (weave-dir). With
        // all values, a two-claimant entity surfaces here as the collision the
        // claim layer exists to catch, and the agent that lost has a door to
        // learn it through.
        for claimed_by in get_all_str(&state.doc, &entity_obj, "claimed_by") {
            let agents_list = entity_agents.entry(ek.clone()).or_default();
            if !agents_list.contains(&claimed_by) {
                agents_list.push(claimed_by);
            }
        }
    }

    // Report entities with multiple agents
    for (entity_id, agent_list) in &entity_agents {
        if agent_list.len() > 1 {
            // Look up entity details
            let Some((_, entity_obj)) = state.doc.get(&entities, entity_id.as_str())? else {
                continue;
            };
            conflicts.push(PotentialConflict {
                entity_id: entity_id.clone(),
                entity_name: get_str(&state.doc, &entity_obj, "name").unwrap_or_default(),
                file_path: get_str(&state.doc, &entity_obj, "file_path").unwrap_or_default(),
                agents: agent_list.clone(),
            });
        }
    }

    Ok(conflicts)
}

/// Upsert an entity into the CRDT state (used during sync).
pub fn upsert_entity(
    state: &mut EntityStateDoc,
    entity_id: &str,
    name: &str,
    entity_type: &str,
    file_path: &str,
    content_hash: &str,
) -> Result<()> {
    // `entity_id` arrives as the parser's spelling. Identity is decided here,
    // once, and everything below this line works on the resolved id.
    // On first sight the two coincide — the id IS the first key — which is what
    // keeps two disconnected replicas minting the same id for the same new
    // entity.
    let resolved = crate::identity::entity_id_for(state, entity_id);
    let entity_id = resolved.as_str();
    let entities = state.entities_id()?;

    match state.doc.get(&entities, entity_id)? {
        Some((_, id)) => {
            // Update existing: only update mutable fields, preserve claims + content
            state.doc.put(&id, "name", name)?;
            state.doc.put(&id, "type", entity_type)?;
            state.doc.put(&id, "file_path", file_path)?;
            state.doc.put(&id, "content_hash", content_hash)?;
        }
        None => {
            // Create new with all v3 fields
            let id = state.doc.put_object(&entities, entity_id, ObjType::Map)?;
            state.doc.put(&id, "name", name)?;
            state.doc.put(&id, "type", entity_type)?;
            state.doc.put(&id, "file_path", file_path)?;
            state.doc.put(&id, "content_hash", content_hash)?;
            state.doc.put(&id, "version", 0_i64)?;
            state.doc.put_object(&id, "version_vector", ObjType::Map)?;
            // No `content` scalar and no `merge_state` flag: both are derived
            // from `writes` on read, so there is nothing here for a concurrent
            // put to overwrite.
            state.doc.put_object(&id, "writes", ObjType::Map)?;
            state.doc.put(&id, "base_content", "")?;
            // First sight: the spelling that introduced this entity is its
            // first alias, so a later rename has something to add beside.
            crate::identity::bind_alias(state, entity_id, entity_id)?;
        }
    };

    // Placement, through the one anchor rule. This is the door every
    // non-sync caller comes in by — `weave claim`, the MCP server,
    // `join::apply_op` — so routing it here is what makes "which door you used"
    // stop being an input to where the entity lands. `place` is monotone, so a
    // later whole-file sync settles this evidence-free anchor rather than
    // fighting it.
    crate::anchor::place(state, file_path, Observation::Entity(entity_id))?;

    Ok(())
}

/// Set an agent's last_seen timestamp (for testing stale cleanup).
#[cfg(any(test, feature = "test-helpers"))]
pub fn set_agent_last_seen(
    state: &mut EntityStateDoc,
    agent_id: &str,
    last_seen: u64,
) -> Result<()> {
    let agents = state.agents_id()?;
    let Some((_, agent_obj)) = state.doc.get(&agents, agent_id)? else {
        return Err(WeaveError::AgentNotFound(agent_id.to_string()));
    };
    state.doc.put(&agent_obj, "last_seen", last_seen as i64)?;
    Ok(())
}

// ── Version vector helpers ──

/// Read a version vector from an entity's version_vector map.
pub(crate) fn read_version_vector(doc: &AutoCommit, entity_obj: &ObjId) -> VersionVector {
    let Ok(Some((_, vv_obj))) = doc.get(entity_obj, "version_vector") else {
        return VersionVector::new();
    };

    let map: HashMap<String, u64> = doc
        .keys(&vv_obj)
        .filter_map(|key| get_u64(doc, &vv_obj, &key).map(|val| (key, val)))
        .collect();
    VersionVector::from_map(map)
}

/// Record `agent_id`'s own counter in an entity's version_vector map.
///
/// One key, and only ever the author's own. The previous version replaced the
/// whole map with `put_object`, which is a last-writer-wins overwrite of the
/// entire clock: two replicas each rewriting the map concurrently kept one map
/// and dropped the other's counters, so the very structure that is supposed to
/// detect concurrency was itself resolved by LWW. A vector clock entry belongs
/// to exactly one writer, so writing exactly that one key is both correct and
/// concurrency-free.
pub(crate) fn write_version_vector(
    doc: &mut AutoCommit,
    entity_obj: &ObjId,
    agent_id: &str,
    count: u64,
) -> Result<()> {
    let vv_obj = match doc.get(entity_obj, "version_vector")? {
        Some((_, id)) => id,
        None => doc.put_object(entity_obj, "version_vector", ObjType::Map)?,
    };
    doc.put(&vv_obj, agent_id, count as i64)?;
    Ok(())
}

// ── Internal helpers ──

fn log_operation(
    state: &mut EntityStateDoc,
    agent_id: &str,
    entity_id: &str,
    op: &str,
) -> Result<()> {
    let operations = state.operations_id()?;
    let len = state.doc.length(&operations);
    let entry = state.doc.insert_object(&operations, len, ObjType::Map)?;
    state.doc.put(&entry, "agent", agent_id)?;
    state.doc.put(&entry, "entity_id", entity_id)?;
    state.doc.put(&entry, "op", op)?;
    state.doc.put(&entry, "timestamp", now_ms() as i64)?;
    Ok(())
}
