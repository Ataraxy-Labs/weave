use automerge::{ReadDoc, transaction::Transactable};
use serde::Serialize;

use crate::error::{Result, WeaveError};
use crate::merge::VersionVector;
use crate::ops::{get_str, read_version_vector, write_version_vector};
use crate::state::{now_ms, EntityStateDoc};

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
    pub conflict_ours_agent: Option<String>,
    pub conflict_theirs_agent: Option<String>,
}

/// Update an entity's content in the CRDT.
///
/// Increments the agent's counter in the version vector,
/// stores the content and hash, and clears any conflict state.
pub fn update_entity_content(
    state: &mut EntityStateDoc,
    agent_id: &str,
    entity_id: &str,
    content: &str,
    content_hash: &str,
) -> Result<()> {
    let entities = state.entities_id()?;

    let entity_obj = match state.doc.get(&entities, entity_id)? {
        Some((_, id)) => id,
        None => return Err(WeaveError::EntityNotFound(entity_id.to_string())),
    };

    let ts = now_ms();

    // Read current version vector and increment
    let mut vv = read_version_vector(&state.doc, &entity_obj);
    vv.increment(agent_id);
    write_version_vector(&mut state.doc, &entity_obj, &vv)?;

    // If this is the first write since last sync and there's existing content,
    // save current content as base_content
    let current_content = get_str(&state.doc, &entity_obj, "content").unwrap_or_default();
    if !current_content.is_empty() {
        let base = get_str(&state.doc, &entity_obj, "base_content").unwrap_or_default();
        if base.is_empty() {
            state.doc.put(&entity_obj, "base_content", current_content.as_str())?;
        }
    }

    // Store new content
    state.doc.put(&entity_obj, "content", content)?;
    state.doc.put(&entity_obj, "content_hash", content_hash)?;

    // Keep scalar version in sync
    state.doc.put(&entity_obj, "version", vv.total() as i64)?;
    state.doc.put(&entity_obj, "last_modified_by", agent_id)?;
    state.doc.put(&entity_obj, "last_modified_at", ts as i64)?;

    // Clear conflict state if present
    state.doc.put(&entity_obj, "merge_state", "clean")?;

    Ok(())
}

/// Get the full content status of an entity.
pub fn get_entity_content(
    state: &EntityStateDoc,
    entity_id: &str,
) -> Result<EntityContentStatus> {
    let entities = state.entities_id()?;

    let entity_obj = match state.doc.get(&entities, entity_id)? {
        Some((_, id)) => id,
        None => return Err(WeaveError::EntityNotFound(entity_id.to_string())),
    };

    let vv = read_version_vector(&state.doc, &entity_obj);
    let merge_state = get_str(&state.doc, &entity_obj, "merge_state")
        .unwrap_or_else(|| "clean".to_string());

    Ok(EntityContentStatus {
        entity_id: entity_id.to_string(),
        name: get_str(&state.doc, &entity_obj, "name").unwrap_or_default(),
        content: get_str(&state.doc, &entity_obj, "content").unwrap_or_default(),
        base_content: get_str(&state.doc, &entity_obj, "base_content").unwrap_or_default(),
        content_hash: get_str(&state.doc, &entity_obj, "content_hash").unwrap_or_default(),
        version_vector: vv,
        merge_state,
        conflict_ours: get_str(&state.doc, &entity_obj, "conflict_ours"),
        conflict_theirs: get_str(&state.doc, &entity_obj, "conflict_theirs"),
        conflict_base: get_str(&state.doc, &entity_obj, "conflict_base"),
        conflict_ours_agent: get_str(&state.doc, &entity_obj, "conflict_ours_agent"),
        conflict_theirs_agent: get_str(&state.doc, &entity_obj, "conflict_theirs_agent"),
    })
}

/// Resolve a conflict on an entity by providing the resolved content.
///
/// Merges the version vectors from both sides, increments the resolving
/// agent's counter, sets merge_state back to clean.
pub fn resolve_entity_conflict(
    state: &mut EntityStateDoc,
    agent_id: &str,
    entity_id: &str,
    resolved_content: &str,
    content_hash: &str,
) -> Result<()> {
    let entities = state.entities_id()?;

    let entity_obj = match state.doc.get(&entities, entity_id)? {
        Some((_, id)) => id,
        None => return Err(WeaveError::EntityNotFound(entity_id.to_string())),
    };

    let merge_state = get_str(&state.doc, &entity_obj, "merge_state")
        .unwrap_or_else(|| "clean".to_string());
    if merge_state != "conflict" {
        return Err(WeaveError::NotInConflict(entity_id.to_string()));
    }

    let ts = now_ms();

    // Merge version vectors from both sides and increment resolver's counter
    let mut vv = read_version_vector(&state.doc, &entity_obj);
    vv.increment(agent_id);
    write_version_vector(&mut state.doc, &entity_obj, &vv)?;

    // Store resolved content
    state.doc.put(&entity_obj, "content", resolved_content)?;
    state.doc.put(&entity_obj, "content_hash", content_hash)?;
    state.doc.put(&entity_obj, "base_content", resolved_content)?;

    // Keep scalar version in sync
    state.doc.put(&entity_obj, "version", vv.total() as i64)?;
    state.doc.put(&entity_obj, "last_modified_by", agent_id)?;
    state.doc.put(&entity_obj, "last_modified_at", ts as i64)?;

    // Clear conflict state
    state.doc.put(&entity_obj, "merge_state", "clean")?;

    // Clean up conflict fields
    if state.doc.get(&entity_obj, "conflict_ours")?.is_some() {
        state.doc.delete(&entity_obj, "conflict_ours")?;
    }
    if state.doc.get(&entity_obj, "conflict_theirs")?.is_some() {
        state.doc.delete(&entity_obj, "conflict_theirs")?;
    }
    if state.doc.get(&entity_obj, "conflict_base")?.is_some() {
        state.doc.delete(&entity_obj, "conflict_base")?;
    }
    if state.doc.get(&entity_obj, "conflict_ours_agent")?.is_some() {
        state.doc.delete(&entity_obj, "conflict_ours_agent")?;
    }
    if state.doc.get(&entity_obj, "conflict_theirs_agent")?.is_some() {
        state.doc.delete(&entity_obj, "conflict_theirs_agent")?;
    }

    Ok(())
}

/// Set an entity into conflict state in the CRDT.
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
    let entities = state.entities_id()?;

    let entity_obj = match state.doc.get(&entities, entity_id)? {
        Some((_, id)) => id,
        None => return Err(WeaveError::EntityNotFound(entity_id.to_string())),
    };

    state.doc.put(&entity_obj, "merge_state", "conflict")?;
    state.doc.put(&entity_obj, "conflict_ours", ours)?;
    state.doc.put(&entity_obj, "conflict_theirs", theirs)?;
    state.doc.put(&entity_obj, "conflict_base", base)?;
    state.doc.put(&entity_obj, "conflict_ours_agent", ours_agent)?;
    state.doc.put(&entity_obj, "conflict_theirs_agent", theirs_agent)?;

    Ok(())
}
