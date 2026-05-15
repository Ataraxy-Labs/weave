use std::collections::HashMap;

use sem_core::model::entity::SemanticEntity;

use crate::conflict::MarkerFormat;
use crate::merge::ResolvedEntity;
use crate::region::FileRegion;

/// Reconstruct a merged file from resolved entities and merged interstitials.
///
/// Uses "ours" region ordering as the skeleton. Inserts theirs-only additions
/// at their relative position (after the entity that precedes them in theirs).
pub fn reconstruct(
    ours_regions: &[FileRegion],
    theirs_regions: &[FileRegion],
    theirs_entities: &[SemanticEntity],
    ours_entity_map: &HashMap<&str, &SemanticEntity>,
    resolved_entities: &HashMap<String, ResolvedEntity>,
    merged_interstitials: &HashMap<String, String>,
    marker_format: &MarkerFormat,
    theirs_rename_base_ids: &std::collections::HashSet<String>,
) -> String {
    let mut output = String::new();

    // Track which entity IDs we've emitted (from ours skeleton)
    let mut emitted_entities: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Identify theirs-only entities (not in ours, and not renamed versions of ours entities)
    let theirs_only: Vec<&SemanticEntity> = theirs_entities
        .iter()
        .filter(|e| {
            !ours_entity_map.contains_key(e.id.as_str()) && !theirs_rename_base_ids.contains(&e.id)
        })
        .collect();

    // Build a map of theirs-only entities by what precedes them in theirs ordering
    let mut theirs_insertions: HashMap<Option<String>, Vec<&SemanticEntity>> = HashMap::new();
    for entity in &theirs_only {
        let predecessor = find_predecessor_in_regions(theirs_regions, &entity.id);
        theirs_insertions
            .entry(predecessor)
            .or_default()
            .push(entity);
    }

    // Walk ours regions as skeleton
    for region in ours_regions {
        match region {
            FileRegion::Interstitial(interstitial) => {
                // Use merged interstitial if available, otherwise ours
                if let Some(merged) = merged_interstitials.get(&interstitial.position_key) {
                    output.push_str(merged);
                } else {
                    output.push_str(&interstitial.content);
                }
            }
            FileRegion::Entity(entity_region) => {
                // Before emitting ours entity, check if there are theirs-only insertions
                // that should go before this entity (predecessor is the entity before this one)

                // Emit the resolved entity
                if let Some(resolved) = resolved_entities.get(&entity_region.entity_id) {
                    match resolved {
                        ResolvedEntity::Clean(region) => {
                            output.push_str(&region.content);
                            if !region.content.is_empty() && !region.content.ends_with('\n') {
                                output.push('\n');
                            }
                        }
                        ResolvedEntity::Conflict(conflict) => {
                            output.push_str(&conflict.to_conflict_markers(marker_format));
                        }
                        ResolvedEntity::ScopedConflict { content, .. } => {
                            output.push_str(content);
                            if !content.is_empty() && !content.ends_with('\n') {
                                output.push('\n');
                            }
                        }
                        ResolvedEntity::Deleted => {
                            // Skip deleted entities
                        }
                    }
                } else {
                    // Entity not in resolved map — keep ours content
                    output.push_str(&entity_region.content);
                    if !entity_region.content.is_empty() && !entity_region.content.ends_with('\n') {
                        output.push('\n');
                    }
                }

                emitted_entities.insert(entity_region.entity_id.clone());

                // Insert theirs-only entities that should come after this entity.
                // Chase the chain: if we insert X after this entity, also insert
                // anything whose predecessor is X, then anything after that, etc.
                // This handles multiple sequential additions (e.g. adding 3 keys
                // at the end of a JSON file).
                let mut current_pred = Some(entity_region.entity_id.clone());
                while let Some(ref pred) = current_pred {
                    if let Some(insertions) = theirs_insertions.get(&Some(pred.clone())) {
                        let mut next_pred: Option<String> = None;
                        for theirs_entity in insertions {
                            if emitted_entities.contains(&theirs_entity.id) {
                                continue;
                            }
                            if let Some(resolved) = resolved_entities.get(&theirs_entity.id) {
                                match resolved {
                                    ResolvedEntity::Clean(region) => {
                                        // Only add blank-line separator for multi-line entities
                                        // (functions, methods). Single-line entities (JSON props,
                                        // struct fields) don't need one.
                                        if region.content.trim_end().contains('\n') {
                                            output.push('\n');
                                        }
                                        output.push_str(&region.content);
                                        if !region.content.is_empty()
                                            && !region.content.ends_with('\n')
                                        {
                                            output.push('\n');
                                        }
                                    }
                                    ResolvedEntity::Conflict(conflict) => {
                                        output.push('\n');
                                        output
                                            .push_str(&conflict.to_conflict_markers(marker_format));
                                    }
                                    ResolvedEntity::ScopedConflict { content, .. } => {
                                        output.push('\n');
                                        output.push_str(content);
                                        if !content.is_empty() && !content.ends_with('\n') {
                                            output.push('\n');
                                        }
                                    }
                                    ResolvedEntity::Deleted => {}
                                }
                            }
                            emitted_entities.insert(theirs_entity.id.clone());
                            next_pred = Some(theirs_entity.id.clone());
                        }
                        current_pred = next_pred;
                    } else {
                        break;
                    }
                }
            }
        }
    }

    // Emit any theirs-only entities whose predecessor was None (should go at the start)
    // or whose predecessor wasn't found — append at the end
    if let Some(insertions) = theirs_insertions.get(&None) {
        for theirs_entity in insertions {
            if !emitted_entities.contains(&theirs_entity.id) {
                if let Some(resolved) = resolved_entities.get(&theirs_entity.id) {
                    emit_resolved(&mut output, resolved, marker_format);
                }
                emitted_entities.insert(theirs_entity.id.clone());
            }
        }
    }

    // Any remaining theirs-only entities not yet emitted (predecessor entity was deleted, etc.)
    for (pred, insertions) in &theirs_insertions {
        if pred.is_none() {
            continue; // Already handled above
        }
        for theirs_entity in insertions {
            if !emitted_entities.contains(&theirs_entity.id) {
                if let Some(resolved) = resolved_entities.get(&theirs_entity.id) {
                    emit_resolved(&mut output, resolved, marker_format);
                }
                emitted_entities.insert(theirs_entity.id.clone());
            }
        }
    }

    output
}

/// Emit a resolved entity into the output (for theirs-only insertions).
fn emit_resolved(output: &mut String, resolved: &ResolvedEntity, marker_format: &MarkerFormat) {
    match resolved {
        ResolvedEntity::Clean(region) => {
            if !output.is_empty() && !output.ends_with('\n') {
                output.push('\n');
            }
            output.push('\n');
            output.push_str(&region.content);
            if !region.content.is_empty() && !region.content.ends_with('\n') {
                output.push('\n');
            }
        }
        ResolvedEntity::Conflict(conflict) => {
            if !output.is_empty() && !output.ends_with('\n') {
                output.push('\n');
            }
            output.push('\n');
            output.push_str(&conflict.to_conflict_markers(marker_format));
        }
        ResolvedEntity::ScopedConflict { content, .. } => {
            if !output.is_empty() && !output.ends_with('\n') {
                output.push('\n');
            }
            output.push('\n');
            output.push_str(content);
            if !content.is_empty() && !content.ends_with('\n') {
                output.push('\n');
            }
        }
        ResolvedEntity::Deleted => {}
    }
}

/// Find the entity ID that precedes the given entity in a region list.
fn find_predecessor_in_regions(regions: &[FileRegion], entity_id: &str) -> Option<String> {
    let mut last_entity_id: Option<String> = None;
    for region in regions {
        if let FileRegion::Entity(e) = region {
            if e.entity_id == entity_id {
                return last_entity_id;
            }
            last_entity_id = Some(e.entity_id.clone());
        }
    }
    None
}
