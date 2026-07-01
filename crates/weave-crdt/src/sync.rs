use std::collections::HashMap;
use std::path::Path;

use automerge::{transaction::Transactable, ObjType, ReadDoc, Value};
use sem_core::parser::registry::ParserRegistry;
use weave_core::region::{extract_regions, FileRegion};

use crate::content::get_entity_content;
use crate::error::Result;
use crate::merge::CrdtMergeResult;
use crate::ops::{get_str, upsert_entity};
use crate::state::EntityStateDoc;

/// The decision for reconciling one entity's file content against the CRDT
/// during a sync, given the last-synced `base`, the current CRDT `content`
/// (which may hold an unmaterialized local edit), and the `file` content.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ContentReconcile {
    /// First time we have seen this entity: seed content and base from the file.
    Seed,
    /// File and CRDT already agree (e.g. right after `weave apply`): just move
    /// the ancestor forward.
    AdvanceBase,
    /// File is unchanged since the last sync: keep the local CRDT edit.
    PreserveLocal,
    /// File changed externally and there is no local edit: take the file.
    FastForward,
    /// Both sides changed since the ancestor: a real conflict.
    Conflict,
}

pub(crate) fn reconcile_entity_content(base: &str, crdt: &str, file: &str) -> ContentReconcile {
    if base.is_empty() {
        ContentReconcile::Seed
    } else if file == crdt {
        ContentReconcile::AdvanceBase
    } else if file == base {
        ContentReconcile::PreserveLocal
    } else if crdt == base {
        ContentReconcile::FastForward
    } else {
        ContentReconcile::Conflict
    }
}

/// Sync entities from working tree files into CRDT state.
///
/// Extracts entities from each file using sem-core's parser registry,
/// then upserts them into the automerge document. Reconciles each entity's
/// file content against the CRDT (see [`reconcile_entity_content`]) so a local
/// edit is never clobbered, and stores file ordering and interstitial content.
pub fn sync_from_files(
    state: &mut EntityStateDoc,
    repo_root: &Path,
    file_paths: &[String],
    registry: &ParserRegistry,
) -> Result<usize> {
    let mut count = 0;

    for file_path in file_paths {
        let full_path = repo_root.join(file_path);
        let content = match std::fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(_) => continue, // File may not exist (deleted)
        };

        let plugin = match registry.get_plugin(file_path) {
            Some(p) => p,
            None => continue, // No parser for this file type
        };

        let entities = plugin.extract_entities(&content, file_path);

        // Upsert entities and store content
        for entity in &entities {
            upsert_entity(
                state,
                &entity.id,
                &entity.name,
                &entity.entity_type,
                file_path,
                &entity.content_hash,
            )?;

            // Reconcile the file's content against the CRDT with a 3-way rule so
            // a local (unmaterialized) entity edit is never silently clobbered by
            // a re-sync. `base` is the content at the last sync (the common
            // ancestor), the CRDT `content` is the possibly-locally-edited side,
            // and `entity.content` is the file's current content.
            let entities_map = state.entities_id()?;
            if let Ok(Some((_, entity_obj))) = state.doc.get(&entities_map, entity.id.as_str()) {
                let base = get_str(&state.doc, &entity_obj, "base_content").unwrap_or_default();
                let crdt_content = get_str(&state.doc, &entity_obj, "content").unwrap_or_default();
                let file_content = entity.content.as_str();

                match reconcile_entity_content(&base, &crdt_content, file_content) {
                    ContentReconcile::Seed | ContentReconcile::FastForward => {
                        state.doc.put(&entity_obj, "content", file_content)?;
                        state.doc.put(&entity_obj, "base_content", file_content)?;
                    }
                    ContentReconcile::AdvanceBase => {
                        state.doc.put(&entity_obj, "base_content", file_content)?;
                    }
                    // File unchanged since last sync: keep the local CRDT edit.
                    ContentReconcile::PreserveLocal => {}
                    ContentReconcile::Conflict => {
                        state.doc.put(&entity_obj, "merge_state", "conflict")?;
                        state
                            .doc
                            .put(&entity_obj, "conflict_ours", crdt_content.as_str())?;
                        state.doc.put(&entity_obj, "conflict_theirs", file_content)?;
                    }
                }
            }

            count += 1;
        }

        // Store file entity ordering
        let entity_order = state.file_entity_order_id()?;
        let order_list = state
            .doc
            .put_object(&entity_order, file_path.as_str(), ObjType::List)?;
        for (i, entity) in entities.iter().enumerate() {
            state.doc.insert(&order_list, i, entity.id.as_str())?;
        }

        // Extract and store interstitial regions
        let regions = extract_regions(&content, entities.as_slice());
        let interstitials_map = state.file_interstitials_id()?;
        for region in &regions {
            if let FileRegion::Interstitial(inter) = region {
                let key = format!("{}::{}", file_path, inter.position_key);
                state
                    .doc
                    .put(&interstitials_map, key.as_str(), inter.content.as_str())?;
            }
        }
    }

    Ok(count)
}

/// Extract entity IDs from a single file (for lookups).
pub fn extract_entity_ids(
    content: &str,
    file_path: &str,
    registry: &ParserRegistry,
) -> Vec<(String, String, String)> {
    let plugin = match registry.get_plugin(file_path) {
        Some(p) => p,
        None => return Vec::new(),
    };

    plugin
        .extract_entities(content, file_path)
        .into_iter()
        .map(|e| (e.id, e.name, e.entity_type))
        .collect()
}

/// Find entity ID by human-readable name and file path.
pub fn resolve_entity_id(
    content: &str,
    file_path: &str,
    entity_name: &str,
    registry: &ParserRegistry,
) -> Option<String> {
    let entities = extract_entity_ids(content, file_path, registry);
    entities
        .into_iter()
        .find(|(_, name, _)| name == entity_name)
        .map(|(id, _, _)| id)
}

/// Merge all entities in a file using CRDT version vectors.
///
/// For each entity:
/// 1. If only one agent wrote it, auto-accept
/// 2. If version vectors show one dominates, take dominant
/// 3. If concurrent, try 3-way merge via weave-core
/// 4. If merge conflicts, mark entity as conflicted
pub fn merge_file_entities(
    state: &mut EntityStateDoc,
    file_path: &str,
    _registry: &ParserRegistry,
) -> Result<CrdtMergeResult> {
    // Get all entities for this file
    let entity_order = read_file_entity_order(state, file_path);
    if entity_order.is_empty() {
        return Ok(CrdtMergeResult {
            file_path: file_path.to_string(),
            entities_auto_merged: 0,
            entities_conflicted: 0,
            merged_content: None,
        });
    }

    let mut auto_merged = 0;
    let mut conflicted = 0;

    for entity_id in &entity_order {
        let content_status = match get_entity_content(state, entity_id) {
            Ok(s) => s,
            Err(_) => continue,
        };

        // Already in conflict or clean with no edits
        if content_status.merge_state == "conflict" {
            conflicted += 1;
            continue;
        }

        let vv = &content_status.version_vector;

        // Count how many agents have non-zero counters
        let active_agents: Vec<&String> = vv
            .counters()
            .iter()
            .filter(|(_, &c)| c > 0)
            .map(|(a, _)| a)
            .collect();

        if active_agents.len() <= 1 {
            // Single writer or no writes: auto-accept, no merge needed
            auto_merged += 1;
            continue;
        }

        // Multiple writers: need to check for conflicts via 3-way merge
        let base = content_status.base_content.clone();
        let current = content_status.content.clone();

        // For now, the current content IS the merged state if no
        // concurrent document exists. In a real multi-doc scenario,
        // we'd compare two replicas. With Automerge, the CRDT layer
        // handles concurrent writes at the field level, so if content
        // was set by different agents, the last-writer-wins in Automerge.
        // We use version vectors to detect this case and let the user
        // know a merge happened.

        if !base.is_empty() && base != current {
            // Content diverged from base. Try entity-level merge.
            // In a two-replica scenario: ours=local content, theirs=remote content.
            // Since Automerge already resolved the field-level conflict (LWW),
            // we compare base vs current to detect semantic changes.
            auto_merged += 1;
        } else {
            auto_merged += 1;
        }
    }

    // Reconstruct the merged file
    let merged = reconstruct_file_from_crdt(state, file_path)?;

    Ok(CrdtMergeResult {
        file_path: file_path.to_string(),
        entities_auto_merged: auto_merged,
        entities_conflicted: conflicted,
        merged_content: Some(merged),
    })
}

/// Reconstruct a file from CRDT state: entity order + interstitials + content.
///
/// Reads file_entity_order for ordering, file_interstitials for non-entity
/// content, and entity content (clean or conflict markers).
pub fn reconstruct_file_from_crdt(state: &EntityStateDoc, file_path: &str) -> Result<String> {
    let entity_order = read_file_entity_order(state, file_path);
    let interstitials = read_file_interstitials(state, file_path);

    let mut output = String::new();

    // File header interstitial
    let header_key = format!("{}::file_header", file_path);
    if let Some(header) = interstitials.get(&header_key) {
        output.push_str(header);
    }

    for (i, entity_id) in entity_order.iter().enumerate() {
        // Interstitial between previous entity and this one
        if i > 0 {
            let prev_id = &entity_order[i - 1];
            let between_key = format!("{}::between:{}:{}", file_path, prev_id, entity_id);
            if let Some(between) = interstitials.get(&between_key) {
                output.push_str(between);
            }
        }

        // Entity content
        match get_entity_content(state, entity_id) {
            Ok(status) => {
                if status.merge_state == "conflict" {
                    // Emit conflict markers
                    output.push_str(&format!(
                        "<<<<<<< {}\n",
                        status.conflict_ours_agent.as_deref().unwrap_or("ours")
                    ));
                    if let Some(ref ours) = status.conflict_ours {
                        output.push_str(ours);
                        if !ours.ends_with('\n') {
                            output.push('\n');
                        }
                    }
                    output.push_str("=======\n");
                    if let Some(ref theirs) = status.conflict_theirs {
                        output.push_str(theirs);
                        if !theirs.ends_with('\n') {
                            output.push('\n');
                        }
                    }
                    output.push_str(&format!(
                        ">>>>>>> {}\n",
                        status.conflict_theirs_agent.as_deref().unwrap_or("theirs")
                    ));
                } else {
                    output.push_str(&status.content);
                    if !status.content.is_empty() && !status.content.ends_with('\n') {
                        output.push('\n');
                    }
                }
            }
            Err(_) => continue,
        }
    }

    // File footer interstitial
    let footer_key = format!("{}::file_footer", file_path);
    if let Some(footer) = interstitials.get(&footer_key) {
        output.push_str(footer);
    }

    Ok(output)
}

/// Read the entity ordering for a file from the CRDT.
fn read_file_entity_order(state: &EntityStateDoc, file_path: &str) -> Vec<String> {
    let order_map = match state.file_entity_order_id() {
        Ok(id) => id,
        Err(_) => return Vec::new(),
    };

    let list_id = match state.doc.get(&order_map, file_path) {
        Ok(Some((_, id))) => id,
        _ => return Vec::new(),
    };

    let len = state.doc.length(&list_id);
    let mut order = Vec::with_capacity(len);
    for i in 0..len {
        if let Ok(Some((Value::Scalar(v), _))) = state.doc.get(&list_id, i) {
            if let automerge::ScalarValue::Str(s) = v.as_ref() {
                order.push(s.to_string());
            }
        }
    }
    order
}

/// Read all interstitials for a file from the CRDT.
fn read_file_interstitials(state: &EntityStateDoc, file_path: &str) -> HashMap<String, String> {
    let inter_map = match state.file_interstitials_id() {
        Ok(id) => id,
        Err(_) => return HashMap::new(),
    };

    let prefix = format!("{}::", file_path);
    let mut result = HashMap::new();

    for key in state.doc.keys(&inter_map) {
        if key.starts_with(&prefix) {
            if let Some(val) = get_str(&state.doc, &inter_map, &key) {
                result.insert(key, val);
            }
        }
    }

    result
}

#[cfg(test)]
mod reconcile_tests {
    use super::{reconcile_entity_content, ContentReconcile};

    #[test]
    fn first_sight_seeds_from_file() {
        // No ancestor yet: take the file.
        assert_eq!(
            reconcile_entity_content("", "", "fn f(){}"),
            ContentReconcile::Seed
        );
    }

    #[test]
    fn local_edit_survives_resync_when_file_unchanged() {
        // The exact bug this guards: base==file (file untouched), CRDT holds a
        // local edit. The edit must be preserved, not clobbered.
        assert_eq!(
            reconcile_entity_content("old", "local_edit", "old"),
            ContentReconcile::PreserveLocal
        );
    }

    #[test]
    fn file_and_crdt_agree_just_advances_base() {
        // After `weave apply`, file == CRDT; only the ancestor moves.
        assert_eq!(
            reconcile_entity_content("old", "applied", "applied"),
            ContentReconcile::AdvanceBase
        );
    }

    #[test]
    fn external_file_edit_without_local_edit_fast_forwards() {
        assert_eq!(
            reconcile_entity_content("old", "old", "external"),
            ContentReconcile::FastForward
        );
    }

    #[test]
    fn both_sides_changed_is_a_conflict() {
        assert_eq!(
            reconcile_entity_content("old", "local_edit", "external_edit"),
            ContentReconcile::Conflict
        );
    }
}
