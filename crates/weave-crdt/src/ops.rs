use std::collections::HashMap;

use automerge::{ObjType, ReadDoc, Value, transaction::Transactable};
use serde::Serialize;

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
    pub last_modified_at: Option<u64>,
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

pub(crate) fn get_str(doc: &automerge::AutoCommit, obj: &automerge::ObjId, key: &str) -> Option<String> {
    match doc.get(obj, key) {
        Ok(Some((Value::Scalar(v), _))) => {
            if let automerge::ScalarValue::Str(s) = v.as_ref() {
                Some(s.to_string())
            } else {
                None
            }
        }
        _ => None,
    }
}

pub(crate) fn get_u64(doc: &automerge::AutoCommit, obj: &automerge::ObjId, key: &str) -> Option<u64> {
    match doc.get(obj, key) {
        Ok(Some((Value::Scalar(v), _))) => match v.as_ref() {
            automerge::ScalarValue::Uint(n) => Some(*n),
            automerge::ScalarValue::Int(n) => Some(*n as u64),
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
    let entity_obj = match state.doc.get(&entities, entity_id)? {
        Some((_, id)) => id,
        None => return Err(WeaveError::EntityNotFound(entity_id.to_string())),
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
pub fn release_entity(
    state: &mut EntityStateDoc,
    agent_id: &str,
    entity_id: &str,
) -> Result<()> {
    let entities = state.entities_id()?;

    let entity_obj = match state.doc.get(&entities, entity_id)? {
        Some((_, id)) => id,
        None => return Err(WeaveError::EntityNotFound(entity_id.to_string())),
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

/// Record that an agent modified an entity.
pub fn record_modification(
    state: &mut EntityStateDoc,
    agent_id: &str,
    entity_id: &str,
    content_hash: &str,
) -> Result<()> {
    let entities = state.entities_id()?;

    let entity_obj = match state.doc.get(&entities, entity_id)? {
        Some((_, id)) => id,
        None => return Err(WeaveError::EntityNotFound(entity_id.to_string())),
    };

    let ts = now_ms();

    // Read current version vector, increment agent's counter
    let vv = read_version_vector(&state.doc, &entity_obj);
    let mut new_vv = vv;
    new_vv.increment(agent_id);

    // Write version vector back
    write_version_vector(&mut state.doc, &entity_obj, &new_vv)?;

    // Keep scalar version in sync (total of VV)
    let version = new_vv.total();

    state.doc.put(&entity_obj, "content_hash", content_hash)?;
    state
        .doc
        .put(&entity_obj, "last_modified_by", agent_id)?;
    state
        .doc
        .put(&entity_obj, "last_modified_at", ts as i64)?;
    state.doc.put(&entity_obj, "version", version as i64)?;

    log_operation(state, agent_id, entity_id, "modify")?;

    Ok(())
}

/// Get the status of an entity.
pub fn get_entity_status(state: &EntityStateDoc, entity_id: &str) -> Result<EntityStatus> {
    let entities = state.entities_id()?;

    let entity_obj = match state.doc.get(&entities, entity_id)? {
        Some((_, id)) => id,
        None => return Err(WeaveError::EntityNotFound(entity_id.to_string())),
    };

    let vv = read_version_vector(&state.doc, &entity_obj);
    let version = {
        let vv_total = vv.total();
        let stored = get_u64(&state.doc, &entity_obj, "version").unwrap_or(0);
        // Use whichever is larger for backward compat
        vv_total.max(stored)
    };

    Ok(EntityStatus {
        entity_id: entity_id.to_string(),
        name: get_str(&state.doc, &entity_obj, "name").unwrap_or_default(),
        entity_type: get_str(&state.doc, &entity_obj, "type").unwrap_or_default(),
        file_path: get_str(&state.doc, &entity_obj, "file_path").unwrap_or_default(),
        content_hash: get_str(&state.doc, &entity_obj, "content_hash").unwrap_or_default(),
        claimed_by: get_str(&state.doc, &entity_obj, "claimed_by"),
        claimed_at: get_u64(&state.doc, &entity_obj, "claimed_at"),
        last_modified_by: get_str(&state.doc, &entity_obj, "last_modified_by"),
        last_modified_at: get_u64(&state.doc, &entity_obj, "last_modified_at"),
        version,
        version_vector: vv,
        merge_state: get_str(&state.doc, &entity_obj, "merge_state")
            .unwrap_or_else(|| "clean".to_string()),
    })
}

/// Get all entities for a given file path.
pub fn get_entities_for_file(state: &EntityStateDoc, file_path: &str) -> Result<Vec<EntityStatus>> {
    let entities = state.entities_id()?;
    let mut result = Vec::new();

    for key in state.doc.keys(&entities) {
        let entity_obj = match state.doc.get(&entities, key.as_str())? {
            Some((_, id)) => id,
            None => continue,
        };
        let fp = get_str(&state.doc, &entity_obj, "file_path").unwrap_or_default();
        if fp == file_path {
            let vv = read_version_vector(&state.doc, &entity_obj);
            let version = {
                let vv_total = vv.total();
                let stored = get_u64(&state.doc, &entity_obj, "version").unwrap_or(0);
                vv_total.max(stored)
            };
            result.push(EntityStatus {
                entity_id: key.clone(),
                name: get_str(&state.doc, &entity_obj, "name").unwrap_or_default(),
                entity_type: get_str(&state.doc, &entity_obj, "type").unwrap_or_default(),
                file_path: fp,
                content_hash: get_str(&state.doc, &entity_obj, "content_hash").unwrap_or_default(),
                claimed_by: get_str(&state.doc, &entity_obj, "claimed_by"),
                claimed_at: get_u64(&state.doc, &entity_obj, "claimed_at"),
                last_modified_by: get_str(&state.doc, &entity_obj, "last_modified_by"),
                last_modified_at: get_u64(&state.doc, &entity_obj, "last_modified_at"),
                version,
                version_vector: vv,
                merge_state: get_str(&state.doc, &entity_obj, "merge_state")
                    .unwrap_or_else(|| "clean".to_string()),
            });
        }
    }

    Ok(result)
}

/// Get the status of an agent.
pub fn get_agent_status(state: &EntityStateDoc, agent_id: &str) -> Result<AgentStatus> {
    let agents = state.agents_id()?;

    let agent_obj = match state.doc.get(&agents, agent_id)? {
        Some((_, id)) => id,
        None => return Err(WeaveError::AgentNotFound(agent_id.to_string())),
    };

    // Read working_on list
    let working_on = match state.doc.get(&agent_obj, "working_on")? {
        Some((_, list_id)) => {
            let len = state.doc.length(&list_id);
            let mut items = Vec::new();
            for i in 0..len {
                if let Ok(Some((Value::Scalar(v), _))) = state.doc.get(&list_id, i) {
                    if let automerge::ScalarValue::Str(s) = v.as_ref() {
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

    let agent_obj = match state.doc.get(&agents, agent_id)? {
        Some((_, id)) => id,
        None => return Err(WeaveError::AgentNotFound(agent_id.to_string())),
    };

    state.doc.put(&agent_obj, "last_seen", now_ms() as i64)?;
    state.doc.put(&agent_obj, "status", "active")?;

    // Replace working_on list
    let list_id = state
        .doc
        .put_object(&agent_obj, "working_on", ObjType::List)?;
    for (i, entity_id) in working_on.iter().enumerate() {
        state
            .doc
            .insert(&list_id, i, entity_id.as_str())?;
    }

    Ok(())
}

/// Clean up stale agents: release their claims and mark inactive.
pub fn cleanup_stale_agents(state: &mut EntityStateDoc, timeout_ms: u64) -> Result<Vec<String>> {
    let now = now_ms();
    let agents = state.agents_id()?;
    let mut stale = Vec::new();

    // Collect stale agent IDs
    let agent_keys: Vec<String> = state.doc.keys(&agents).collect();
    for key in &agent_keys {
        let agent_obj = match state.doc.get(&agents, key.as_str())? {
            Some((_, id)) => id,
            None => continue,
        };
        let last_seen = get_u64(&state.doc, &agent_obj, "last_seen").unwrap_or(0);
        if now - last_seen > timeout_ms {
            stale.push(key.clone());
        }
    }

    // Release claims and mark inactive
    for agent_id in &stale {
        // Mark agent as stale
        let agent_obj = match state.doc.get(&agents, agent_id.as_str())? {
            Some((_, id)) => id,
            None => continue,
        };
        state.doc.put(&agent_obj, "status", "stale")?;

        // Release all entity claims held by this agent
        let entities = state.entities_id()?;
        let entity_keys: Vec<String> = state.doc.keys(&entities).collect();
        for ek in &entity_keys {
            let entity_obj = match state.doc.get(&entities, ek.as_str())? {
                Some((_, id)) => id,
                None => continue,
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
    let mut entity_agents: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();

    // From agent working_on lists
    let agent_keys: Vec<String> = state.doc.keys(&agents).collect();
    for ak in &agent_keys {
        let agent_obj = match state.doc.get(&agents, ak.as_str())? {
            Some((_, id)) => id,
            None => continue,
        };
        let agent_status = get_str(&state.doc, &agent_obj, "status").unwrap_or_default();
        if agent_status == "stale" {
            continue;
        }
        if let Ok(Some((_, list_id))) = state.doc.get(&agent_obj, "working_on") {
            let len = state.doc.length(&list_id);
            for i in 0..len {
                if let Ok(Some((Value::Scalar(v), _))) = state.doc.get(&list_id, i) {
                    if let automerge::ScalarValue::Str(s) = v.as_ref() {
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
        let entity_obj = match state.doc.get(&entities, ek.as_str())? {
            Some((_, id)) => id,
            None => continue,
        };
        if let Some(claimed_by) = get_str(&state.doc, &entity_obj, "claimed_by") {
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
            let entity_obj = match state.doc.get(&entities, entity_id.as_str())? {
                Some((_, id)) => id,
                None => continue,
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
            // Create new with all v2 fields
            let id = state.doc.put_object(&entities, entity_id, ObjType::Map)?;
            state.doc.put(&id, "name", name)?;
            state.doc.put(&id, "type", entity_type)?;
            state.doc.put(&id, "file_path", file_path)?;
            state.doc.put(&id, "content_hash", content_hash)?;
            state.doc.put(&id, "version", 0_i64)?;
            state.doc.put(&id, "last_modified_at", now_ms() as i64)?;
            state.doc.put_object(&id, "version_vector", ObjType::Map)?;
            state.doc.put(&id, "content", "")?;
            state.doc.put(&id, "base_content", "")?;
            state.doc.put(&id, "merge_state", "clean")?;
        }
    };

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
    let agent_obj = match state.doc.get(&agents, agent_id)? {
        Some((_, id)) => id,
        None => return Err(WeaveError::AgentNotFound(agent_id.to_string())),
    };
    state.doc.put(&agent_obj, "last_seen", last_seen as i64)?;
    Ok(())
}

// ── Version vector helpers ──

/// Read a version vector from an entity's version_vector map.
pub(crate) fn read_version_vector(doc: &automerge::AutoCommit, entity_obj: &automerge::ObjId) -> VersionVector {
    let vv_obj = match doc.get(entity_obj, "version_vector") {
        Ok(Some((_, id))) => id,
        _ => return VersionVector::new(),
    };

    let mut map = HashMap::new();
    for key in doc.keys(&vv_obj) {
        if let Some(val) = get_u64(doc, &vv_obj, &key) {
            map.insert(key, val);
        }
    }
    VersionVector::from_map(map)
}

/// Write a version vector to an entity's version_vector map.
pub(crate) fn write_version_vector(
    doc: &mut automerge::AutoCommit,
    entity_obj: &automerge::ObjId,
    vv: &VersionVector,
) -> Result<()> {
    let vv_obj = doc.put_object(entity_obj, "version_vector", ObjType::Map)?;
    for (agent_id, &count) in vv.counters() {
        doc.put(&vv_obj, agent_id.as_str(), count as i64)?;
    }
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
    let entry = state
        .doc
        .insert_object(&operations, len, ObjType::Map)?;
    state.doc.put(&entry, "agent", agent_id)?;
    state.doc.put(&entry, "entity_id", entity_id)?;
    state.doc.put(&entry, "op", op)?;
    state.doc.put(&entry, "timestamp", now_ms() as i64)?;
    Ok(())
}
