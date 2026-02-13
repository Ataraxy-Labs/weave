use std::collections::{HashMap, HashSet};

use sem_core::model::change::ChangeType;
use sem_core::model::entity::SemanticEntity;
use sem_core::model::identity::match_entities;
use sem_core::parser::plugins::create_default_registry;
use sem_core::parser::registry::ParserRegistry;

use crate::conflict::{classify_conflict, ConflictKind, EntityConflict, MergeStats};
use crate::region::{extract_regions, EntityRegion, FileRegion};
use crate::validate::SemanticWarning;
use crate::reconstruct::reconstruct;

/// Result of a merge operation.
#[derive(Debug)]
pub struct MergeResult {
    pub content: String,
    pub conflicts: Vec<EntityConflict>,
    pub warnings: Vec<SemanticWarning>,
    pub stats: MergeStats,
}

impl MergeResult {
    pub fn is_clean(&self) -> bool {
        self.conflicts.is_empty()
    }
}

/// The resolved content for a single entity after merging.
#[derive(Debug, Clone)]
pub enum ResolvedEntity {
    /// Clean resolution — use this content.
    Clean(EntityRegion),
    /// Conflict — render conflict markers.
    Conflict(EntityConflict),
    /// Entity was deleted.
    Deleted,
}

/// Perform entity-level 3-way merge.
///
/// Falls back to line-level merge (via diffy) when:
/// - No parser matches the file type
/// - Parser returns 0 entities for non-empty content
/// - File exceeds 1MB
pub fn entity_merge(
    base: &str,
    ours: &str,
    theirs: &str,
    file_path: &str,
) -> MergeResult {
    let registry = create_default_registry();
    entity_merge_with_registry(base, ours, theirs, file_path, &registry)
}

pub fn entity_merge_with_registry(
    base: &str,
    ours: &str,
    theirs: &str,
    file_path: &str,
    registry: &ParserRegistry,
) -> MergeResult {
    // Fast path: if ours == theirs, no merge needed
    if ours == theirs {
        return MergeResult {
            content: ours.to_string(),
            conflicts: vec![],
            warnings: vec![],
            stats: MergeStats::default(),
        };
    }

    // Fast path: if base == ours, take theirs entirely
    if base == ours {
        return MergeResult {
            content: theirs.to_string(),
            conflicts: vec![],
            warnings: vec![],
            stats: MergeStats {
                entities_theirs_only: 1,
                ..Default::default()
            },
        };
    }

    // Fast path: if base == theirs, take ours entirely
    if base == theirs {
        return MergeResult {
            content: ours.to_string(),
            conflicts: vec![],
            warnings: vec![],
            stats: MergeStats {
                entities_ours_only: 1,
                ..Default::default()
            },
        };
    }

    // Large file fallback
    if base.len() > 1_000_000 || ours.len() > 1_000_000 || theirs.len() > 1_000_000 {
        return line_level_fallback(base, ours, theirs);
    }

    // Try to get a parser for this file type
    let plugin = match registry.get_plugin(file_path) {
        Some(p) => p,
        None => return line_level_fallback(base, ours, theirs),
    };

    // Extract entities from all three versions, filtering out nested entities
    // (e.g. variables inside class methods — handled as part of the parent entity)
    let base_entities = filter_nested_entities(plugin.extract_entities(base, file_path));
    let ours_entities = filter_nested_entities(plugin.extract_entities(ours, file_path));
    let theirs_entities = filter_nested_entities(plugin.extract_entities(theirs, file_path));

    // Fallback if parser returns nothing for non-empty content
    if base_entities.is_empty() && !base.trim().is_empty() {
        return line_level_fallback(base, ours, theirs);
    }
    // Allow empty entities if content is actually empty
    if ours_entities.is_empty() && !ours.trim().is_empty() && theirs_entities.is_empty() && !theirs.trim().is_empty() {
        return line_level_fallback(base, ours, theirs);
    }

    // Extract regions from all three
    let base_regions = extract_regions(base, &base_entities);
    let ours_regions = extract_regions(ours, &ours_entities);
    let theirs_regions = extract_regions(theirs, &theirs_entities);

    // Build region content maps (entity_id → content from file lines, preserving
    // surrounding syntax like `export` that sem-core's entity.content may strip)
    let base_region_content = build_region_content_map(&base_regions);
    let ours_region_content = build_region_content_map(&ours_regions);
    let theirs_region_content = build_region_content_map(&theirs_regions);

    // Match entities: base↔ours and base↔theirs
    let ours_changes = match_entities(&base_entities, &ours_entities, file_path, None, None, None);
    let theirs_changes = match_entities(&base_entities, &theirs_entities, file_path, None, None, None);

    // Build lookup maps
    let base_entity_map: HashMap<&str, &SemanticEntity> =
        base_entities.iter().map(|e| (e.id.as_str(), e)).collect();
    let ours_entity_map: HashMap<&str, &SemanticEntity> =
        ours_entities.iter().map(|e| (e.id.as_str(), e)).collect();
    let theirs_entity_map: HashMap<&str, &SemanticEntity> =
        theirs_entities.iter().map(|e| (e.id.as_str(), e)).collect();

    // Classify what happened to each entity in each branch
    let mut ours_change_map: HashMap<String, ChangeType> = HashMap::new();
    for change in &ours_changes.changes {
        ours_change_map.insert(change.entity_id.clone(), change.change_type);
    }
    let mut theirs_change_map: HashMap<String, ChangeType> = HashMap::new();
    for change in &theirs_changes.changes {
        theirs_change_map.insert(change.entity_id.clone(), change.change_type);
    }

    // Detect renames using structural_hash (RefFilter / IntelliMerge-inspired).
    // When one branch renames an entity, connect the old and new IDs so the merge
    // treats it as the same entity rather than a delete+add.
    let ours_rename_to_base = build_rename_map(&base_entities, &ours_entities);
    let theirs_rename_to_base = build_rename_map(&base_entities, &theirs_entities);
    // Reverse maps: base_id → renamed_id in that branch
    let base_to_ours_rename: HashMap<String, String> = ours_rename_to_base
        .iter()
        .map(|(new, old)| (old.clone(), new.clone()))
        .collect();
    let base_to_theirs_rename: HashMap<String, String> = theirs_rename_to_base
        .iter()
        .map(|(new, old)| (old.clone(), new.clone()))
        .collect();

    // Collect all entity IDs across all versions
    let mut all_entity_ids: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    // Track renamed IDs so we don't process them twice
    let mut skip_ids: HashSet<String> = HashSet::new();
    // The "new" IDs from renames should be skipped — they'll be handled via the base ID
    for new_id in ours_rename_to_base.keys() {
        skip_ids.insert(new_id.clone());
    }
    for new_id in theirs_rename_to_base.keys() {
        skip_ids.insert(new_id.clone());
    }

    // Start with ours ordering (skeleton)
    for entity in &ours_entities {
        if skip_ids.contains(&entity.id) {
            continue;
        }
        if seen.insert(entity.id.clone()) {
            all_entity_ids.push(entity.id.clone());
        }
    }
    // Add theirs-only entities
    for entity in &theirs_entities {
        if skip_ids.contains(&entity.id) {
            continue;
        }
        if seen.insert(entity.id.clone()) {
            all_entity_ids.push(entity.id.clone());
        }
    }
    // Add base-only entities (deleted in both → skip, deleted in one → handled below)
    for entity in &base_entities {
        if seen.insert(entity.id.clone()) {
            all_entity_ids.push(entity.id.clone());
        }
    }

    let mut stats = MergeStats::default();
    let mut conflicts: Vec<EntityConflict> = Vec::new();
    let mut resolved_entities: HashMap<String, ResolvedEntity> = HashMap::new();

    // Detect rename/rename conflicts: same base entity renamed differently in both branches.
    // These must be flagged before the entity resolution loop, which would otherwise silently
    // pick ours and also include theirs as an unmatched entity.
    let mut rename_conflict_ids: HashSet<String> = HashSet::new();
    for (base_id, ours_new_id) in &base_to_ours_rename {
        if let Some(theirs_new_id) = base_to_theirs_rename.get(base_id) {
            if ours_new_id != theirs_new_id {
                rename_conflict_ids.insert(base_id.clone());
            }
        }
    }

    for entity_id in &all_entity_ids {
        // Handle rename/rename conflicts: both branches renamed this base entity differently
        if rename_conflict_ids.contains(entity_id) {
            let ours_new_id = &base_to_ours_rename[entity_id];
            let theirs_new_id = &base_to_theirs_rename[entity_id];
            let base_entity = base_entity_map.get(entity_id.as_str());
            let ours_entity = ours_entity_map.get(ours_new_id.as_str());
            let theirs_entity = theirs_entity_map.get(theirs_new_id.as_str());
            let base_name = base_entity.map(|e| e.name.as_str()).unwrap_or(entity_id);
            let ours_name = ours_entity.map(|e| e.name.as_str()).unwrap_or(ours_new_id);
            let theirs_name = theirs_entity.map(|e| e.name.as_str()).unwrap_or(theirs_new_id);

            let base_rc = base_entity.map(|e| base_region_content.get(&e.id).cloned().unwrap_or_else(|| e.content.clone()));
            let ours_rc = ours_entity.map(|e| ours_region_content.get(&e.id).cloned().unwrap_or_else(|| e.content.clone()));
            let theirs_rc = theirs_entity.map(|e| theirs_region_content.get(&e.id).cloned().unwrap_or_else(|| e.content.clone()));

            stats.entities_conflicted += 1;
            let conflict = EntityConflict {
                entity_name: base_name.to_string(),
                entity_type: base_entity.map(|e| e.entity_type.clone()).unwrap_or_default(),
                kind: ConflictKind::RenameRename {
                    base_name: base_name.to_string(),
                    ours_name: ours_name.to_string(),
                    theirs_name: theirs_name.to_string(),
                },
                complexity: crate::conflict::ConflictComplexity::Syntax,
                ours_content: ours_rc,
                theirs_content: theirs_rc,
                base_content: base_rc,
            };
            conflicts.push(conflict.clone());
            let resolution = ResolvedEntity::Conflict(conflict);
            resolved_entities.insert(entity_id.clone(), resolution.clone());
            resolved_entities.insert(ours_new_id.clone(), resolution);
            // Mark theirs renamed ID as Deleted so reconstruct doesn't emit the conflict twice
            // (once from ours skeleton, once from theirs-only insertion)
            resolved_entities.insert(theirs_new_id.clone(), ResolvedEntity::Deleted);
            continue;
        }

        let in_base = base_entity_map.get(entity_id.as_str());
        // Follow rename chains: if base entity was renamed in ours/theirs, use renamed version
        let ours_id = base_to_ours_rename.get(entity_id.as_str()).map(|s| s.as_str()).unwrap_or(entity_id.as_str());
        let theirs_id = base_to_theirs_rename.get(entity_id.as_str()).map(|s| s.as_str()).unwrap_or(entity_id.as_str());
        let in_ours = ours_entity_map.get(ours_id).or_else(|| ours_entity_map.get(entity_id.as_str()));
        let in_theirs = theirs_entity_map.get(theirs_id).or_else(|| theirs_entity_map.get(entity_id.as_str()));

        let ours_change = ours_change_map.get(entity_id);
        let theirs_change = theirs_change_map.get(entity_id);

        let resolution = resolve_entity(
            entity_id,
            in_base,
            in_ours,
            in_theirs,
            ours_change,
            theirs_change,
            &base_region_content,
            &ours_region_content,
            &theirs_region_content,
            &mut stats,
        );

        if let ResolvedEntity::Conflict(ref c) = resolution {
            conflicts.push(c.clone());
        }

        resolved_entities.insert(entity_id.clone(), resolution.clone());
        // Also store under renamed IDs so reconstruct can find them
        if let Some(ours_renamed_id) = base_to_ours_rename.get(entity_id.as_str()) {
            resolved_entities.insert(ours_renamed_id.clone(), resolution.clone());
        }
        if let Some(theirs_renamed_id) = base_to_theirs_rename.get(entity_id.as_str()) {
            resolved_entities.insert(theirs_renamed_id.clone(), resolution);
        }
    }

    // Merge interstitial regions
    let merged_interstitials = merge_interstitials(&base_regions, &ours_regions, &theirs_regions);

    // Reconstruct the file
    let content = reconstruct(
        &ours_regions,
        &theirs_regions,
        &theirs_entities,
        &ours_entity_map,
        &resolved_entities,
        &merged_interstitials,
    );

    // Post-merge parse validation: verify the merged result still parses correctly
    // (MergeBot-inspired safety check — catch syntactically broken merges)
    let mut warnings = vec![];
    if conflicts.is_empty() && stats.entities_both_changed_merged > 0 {
        let merged_entities = plugin.extract_entities(&content, file_path);
        if merged_entities.is_empty() && !content.trim().is_empty() {
            warnings.push(crate::validate::SemanticWarning {
                entity_name: "(file)".to_string(),
                entity_type: "file".to_string(),
                file_path: file_path.to_string(),
                kind: crate::validate::WarningKind::ParseFailedAfterMerge,
                related: vec![],
            });
        }
    }

    MergeResult {
        content,
        conflicts,
        warnings,
        stats,
    }
}

fn resolve_entity(
    _entity_id: &str,
    in_base: Option<&&SemanticEntity>,
    in_ours: Option<&&SemanticEntity>,
    in_theirs: Option<&&SemanticEntity>,
    _ours_change: Option<&ChangeType>,
    _theirs_change: Option<&ChangeType>,
    base_region_content: &HashMap<String, String>,
    ours_region_content: &HashMap<String, String>,
    theirs_region_content: &HashMap<String, String>,
    stats: &mut MergeStats,
) -> ResolvedEntity {
    // Helper: get region content (from file lines) for an entity, falling back to entity.content
    let region_content = |entity: &SemanticEntity, map: &HashMap<String, String>| -> String {
        map.get(&entity.id).cloned().unwrap_or_else(|| entity.content.clone())
    };

    match (in_base, in_ours, in_theirs) {
        // Entity exists in all three versions
        (Some(base), Some(ours), Some(theirs)) => {
            // Check modification status via structural hash AND region content.
            // Region content may differ even when structural hash is the same
            // (e.g., doc comment added/changed but function body unchanged).
            let base_rc_lazy = || region_content(base, base_region_content);
            let ours_rc_lazy = || region_content(ours, ours_region_content);
            let theirs_rc_lazy = || region_content(theirs, theirs_region_content);

            let ours_modified = ours.content_hash != base.content_hash
                || ours_rc_lazy() != base_rc_lazy();
            let theirs_modified = theirs.content_hash != base.content_hash
                || theirs_rc_lazy() != base_rc_lazy();

            match (ours_modified, theirs_modified) {
                (false, false) => {
                    // Neither changed
                    stats.entities_unchanged += 1;
                    ResolvedEntity::Clean(entity_to_region_with_content(ours, &region_content(ours, ours_region_content)))
                }
                (true, false) => {
                    // Only ours changed
                    stats.entities_ours_only += 1;
                    ResolvedEntity::Clean(entity_to_region_with_content(ours, &region_content(ours, ours_region_content)))
                }
                (false, true) => {
                    // Only theirs changed
                    stats.entities_theirs_only += 1;
                    ResolvedEntity::Clean(entity_to_region_with_content(theirs, &region_content(theirs, theirs_region_content)))
                }
                (true, true) => {
                    // Both changed — try intra-entity merge
                    if ours.content_hash == theirs.content_hash {
                        // Same change in both — take ours
                        stats.entities_both_changed_merged += 1;
                        ResolvedEntity::Clean(entity_to_region_with_content(ours, &region_content(ours, ours_region_content)))
                    } else {
                        // Try diffy 3-way merge on region content (preserves full syntax)
                        let base_rc = region_content(base, base_region_content);
                        let ours_rc = region_content(ours, ours_region_content);
                        let theirs_rc = region_content(theirs, theirs_region_content);

                        // Whitespace-aware shortcut: if one side only changed
                        // whitespace/formatting, take the other side's content changes.
                        // This handles the common case where one agent reformats while
                        // another makes semantic changes.
                        if is_whitespace_only_diff(&base_rc, &ours_rc) {
                            stats.entities_theirs_only += 1;
                            return ResolvedEntity::Clean(entity_to_region_with_content(theirs, &theirs_rc));
                        }
                        if is_whitespace_only_diff(&base_rc, &theirs_rc) {
                            stats.entities_ours_only += 1;
                            return ResolvedEntity::Clean(entity_to_region_with_content(ours, &ours_rc));
                        }

                        match diffy_merge(&base_rc, &ours_rc, &theirs_rc) {
                            Some(merged) => {
                                stats.entities_both_changed_merged += 1;
                                stats.resolved_via_diffy += 1;
                                ResolvedEntity::Clean(EntityRegion {
                                    entity_id: ours.id.clone(),
                                    entity_name: ours.name.clone(),
                                    entity_type: ours.entity_type.clone(),
                                    content: merged,
                                    start_line: ours.start_line,
                                    end_line: ours.end_line,
                                })
                            }
                            None => {
                                // Strategy 1: decorator/annotation-aware merge
                                // Decorators are unordered annotations — merge them commutatively
                                if let Some(merged) = try_decorator_aware_merge(&base_rc, &ours_rc, &theirs_rc) {
                                    stats.entities_both_changed_merged += 1;
                                    stats.resolved_via_diffy += 1;
                                    return ResolvedEntity::Clean(EntityRegion {
                                        entity_id: ours.id.clone(),
                                        entity_name: ours.name.clone(),
                                        entity_type: ours.entity_type.clone(),
                                        content: merged,
                                        start_line: ours.start_line,
                                        end_line: ours.end_line,
                                    });
                                }

                                // Strategy 2: inner entity merge for container types
                                // (LastMerge insight: class members are unordered children)
                                if is_container_entity_type(&ours.entity_type) {
                                    if let Some(merged) = try_inner_entity_merge(&base_rc, &ours_rc, &theirs_rc) {
                                        stats.entities_both_changed_merged += 1;
                                        stats.resolved_via_inner_merge += 1;
                                        return ResolvedEntity::Clean(EntityRegion {
                                            entity_id: ours.id.clone(),
                                            entity_name: ours.name.clone(),
                                            entity_type: ours.entity_type.clone(),
                                            content: merged,
                                            start_line: ours.start_line,
                                            end_line: ours.end_line,
                                        });
                                    }
                                }
                                stats.entities_conflicted += 1;
                                let complexity = classify_conflict(Some(&base_rc), Some(&ours_rc), Some(&theirs_rc));
                                ResolvedEntity::Conflict(EntityConflict {
                                    entity_name: ours.name.clone(),
                                    entity_type: ours.entity_type.clone(),
                                    kind: ConflictKind::BothModified,
                                    complexity,
                                    ours_content: Some(ours_rc),
                                    theirs_content: Some(theirs_rc),
                                    base_content: Some(base_rc),
                                })
                            }
                        }
                    }
                }
            }
        }

        // Entity in base and ours, but not theirs → theirs deleted it
        (Some(_base), Some(ours), None) => {
            let ours_modified = ours.content_hash != _base.content_hash;
            if ours_modified {
                // Modify/delete conflict
                stats.entities_conflicted += 1;
                let ours_rc = region_content(ours, ours_region_content);
                let base_rc = region_content(_base, base_region_content);
                let complexity = classify_conflict(Some(&base_rc), Some(&ours_rc), None);
                ResolvedEntity::Conflict(EntityConflict {
                    entity_name: ours.name.clone(),
                    entity_type: ours.entity_type.clone(),
                    kind: ConflictKind::ModifyDelete {
                        modified_in_ours: true,
                    },
                    complexity,
                    ours_content: Some(ours_rc),
                    theirs_content: None,
                    base_content: Some(base_rc),
                })
            } else {
                // Theirs deleted, ours unchanged → accept deletion
                stats.entities_deleted += 1;
                ResolvedEntity::Deleted
            }
        }

        // Entity in base and theirs, but not ours → ours deleted it
        (Some(_base), None, Some(theirs)) => {
            let theirs_modified = theirs.content_hash != _base.content_hash;
            if theirs_modified {
                // Modify/delete conflict
                stats.entities_conflicted += 1;
                let theirs_rc = region_content(theirs, theirs_region_content);
                let base_rc = region_content(_base, base_region_content);
                let complexity = classify_conflict(Some(&base_rc), None, Some(&theirs_rc));
                ResolvedEntity::Conflict(EntityConflict {
                    entity_name: theirs.name.clone(),
                    entity_type: theirs.entity_type.clone(),
                    kind: ConflictKind::ModifyDelete {
                        modified_in_ours: false,
                    },
                    complexity,
                    ours_content: None,
                    theirs_content: Some(theirs_rc),
                    base_content: Some(base_rc),
                })
            } else {
                // Ours deleted, theirs unchanged → accept deletion
                stats.entities_deleted += 1;
                ResolvedEntity::Deleted
            }
        }

        // Entity only in ours (added by ours)
        (None, Some(ours), None) => {
            stats.entities_added_ours += 1;
            ResolvedEntity::Clean(entity_to_region_with_content(ours, &region_content(ours, ours_region_content)))
        }

        // Entity only in theirs (added by theirs)
        (None, None, Some(theirs)) => {
            stats.entities_added_theirs += 1;
            ResolvedEntity::Clean(entity_to_region_with_content(theirs, &region_content(theirs, theirs_region_content)))
        }

        // Entity in both ours and theirs but not base (both added)
        (None, Some(ours), Some(theirs)) => {
            if ours.content_hash == theirs.content_hash {
                // Same content added by both → take ours
                stats.entities_added_ours += 1;
                ResolvedEntity::Clean(entity_to_region_with_content(ours, &region_content(ours, ours_region_content)))
            } else {
                // Different content → conflict
                stats.entities_conflicted += 1;
                let ours_rc = region_content(ours, ours_region_content);
                let theirs_rc = region_content(theirs, theirs_region_content);
                let complexity = classify_conflict(None, Some(&ours_rc), Some(&theirs_rc));
                ResolvedEntity::Conflict(EntityConflict {
                    entity_name: ours.name.clone(),
                    entity_type: ours.entity_type.clone(),
                    kind: ConflictKind::BothAdded,
                    complexity,
                    ours_content: Some(ours_rc),
                    theirs_content: Some(theirs_rc),
                    base_content: None,
                })
            }
        }

        // Entity only in base (deleted by both)
        (Some(_), None, None) => {
            stats.entities_deleted += 1;
            ResolvedEntity::Deleted
        }

        // Should not happen
        (None, None, None) => ResolvedEntity::Deleted,
    }
}

fn entity_to_region_with_content(entity: &SemanticEntity, content: &str) -> EntityRegion {
    EntityRegion {
        entity_id: entity.id.clone(),
        entity_name: entity.name.clone(),
        entity_type: entity.entity_type.clone(),
        content: content.to_string(),
        start_line: entity.start_line,
        end_line: entity.end_line,
    }
}

/// Build a map from entity_id to region content (from file lines).
/// This preserves surrounding syntax (like `export`) that sem-core's entity.content may strip.
fn build_region_content_map(regions: &[FileRegion]) -> HashMap<String, String> {
    regions
        .iter()
        .filter_map(|r| match r {
            FileRegion::Entity(e) => Some((e.entity_id.clone(), e.content.clone())),
            _ => None,
        })
        .collect()
}

/// Check if the only differences between two strings are whitespace changes.
/// This includes: indentation changes, trailing whitespace, blank line additions/removals.
fn is_whitespace_only_diff(a: &str, b: &str) -> bool {
    if a == b {
        return true; // identical, not really a "whitespace-only diff" but safe
    }
    let a_normalized: Vec<&str> = a.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
    let b_normalized: Vec<&str> = b.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
    a_normalized == b_normalized
}

/// Check if a line is a decorator or annotation.
/// Covers Python (@decorator), Java/TS (@Annotation), and comment-style annotations.
fn is_decorator_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('@')
        && !trimmed.starts_with("@param")
        && !trimmed.starts_with("@return")
        && !trimmed.starts_with("@type")
        && !trimmed.starts_with("@see")
}

/// Split content into (decorators, body) where decorators are leading @-prefixed lines.
fn split_decorators(content: &str) -> (Vec<&str>, &str) {
    let mut decorator_end = 0;
    let mut byte_offset = 0;
    for line in content.lines() {
        if is_decorator_line(line) || line.trim().is_empty() {
            decorator_end += 1;
            byte_offset += line.len() + 1; // +1 for newline
        } else {
            break;
        }
    }
    // Trim trailing empty lines from decorator section
    let lines: Vec<&str> = content.lines().collect();
    while decorator_end > 0 && lines.get(decorator_end - 1).map_or(false, |l| l.trim().is_empty()) {
        byte_offset -= lines[decorator_end - 1].len() + 1;
        decorator_end -= 1;
    }
    let decorators: Vec<&str> = lines[..decorator_end]
        .iter()
        .filter(|l| is_decorator_line(l))
        .copied()
        .collect();
    let body = &content[byte_offset.min(content.len())..];
    (decorators, body)
}

/// Try decorator-aware merge: when both sides add different decorators/annotations,
/// merge them commutatively (like imports). Also try merging the bodies separately.
///
/// This handles the common pattern where one agent adds @cache and another adds @deprecated
/// to the same function — they should both be preserved.
fn try_decorator_aware_merge(base: &str, ours: &str, theirs: &str) -> Option<String> {
    let (base_decorators, base_body) = split_decorators(base);
    let (ours_decorators, ours_body) = split_decorators(ours);
    let (theirs_decorators, theirs_body) = split_decorators(theirs);

    // Only useful if at least one side has decorators
    if ours_decorators.is_empty() && theirs_decorators.is_empty() {
        return None;
    }

    // Merge bodies using diffy (or take unchanged side)
    let merged_body = if base_body == ours_body && base_body == theirs_body {
        base_body.to_string()
    } else if base_body == ours_body {
        theirs_body.to_string()
    } else if base_body == theirs_body {
        ours_body.to_string()
    } else {
        // Both changed body — try diffy on just the body
        diffy_merge(base_body, ours_body, theirs_body)?
    };

    // Merge decorators commutatively (set union)
    let base_set: HashSet<&str> = base_decorators.iter().copied().collect();
    let ours_set: HashSet<&str> = ours_decorators.iter().copied().collect();
    let theirs_set: HashSet<&str> = theirs_decorators.iter().copied().collect();

    // Deletions
    let ours_deleted: HashSet<&str> = base_set.difference(&ours_set).copied().collect();
    let theirs_deleted: HashSet<&str> = base_set.difference(&theirs_set).copied().collect();

    // Start with base decorators, remove deletions
    let mut merged_decorators: Vec<&str> = base_decorators
        .iter()
        .filter(|d| !ours_deleted.contains(**d) && !theirs_deleted.contains(**d))
        .copied()
        .collect();

    // Add new decorators from ours (not in base)
    for d in &ours_decorators {
        if !base_set.contains(d) && !merged_decorators.contains(d) {
            merged_decorators.push(d);
        }
    }
    // Add new decorators from theirs (not in base, not already added)
    for d in &theirs_decorators {
        if !base_set.contains(d) && !merged_decorators.contains(d) {
            merged_decorators.push(d);
        }
    }

    // Reconstruct
    let mut result = String::new();
    for d in &merged_decorators {
        result.push_str(d);
        result.push('\n');
    }
    result.push_str(&merged_body);

    Some(result)
}

/// Try 3-way merge on text using diffy. Returns None if there are conflicts.
fn diffy_merge(base: &str, ours: &str, theirs: &str) -> Option<String> {
    let result = diffy::merge(base, ours, theirs);
    match result {
        Ok(merged) => Some(merged),
        Err(_conflicted) => None,
    }
}

/// Merge interstitial regions from all three versions.
/// Uses commutative (set-based) merge for import blocks — inspired by
/// LastMerge/Mergiraf's "unordered children" concept.
/// Falls back to line-level 3-way merge for non-import content.
fn merge_interstitials(
    base_regions: &[FileRegion],
    ours_regions: &[FileRegion],
    theirs_regions: &[FileRegion],
) -> HashMap<String, String> {
    let base_map: HashMap<&str, &str> = base_regions
        .iter()
        .filter_map(|r| match r {
            FileRegion::Interstitial(i) => Some((i.position_key.as_str(), i.content.as_str())),
            _ => None,
        })
        .collect();

    let ours_map: HashMap<&str, &str> = ours_regions
        .iter()
        .filter_map(|r| match r {
            FileRegion::Interstitial(i) => Some((i.position_key.as_str(), i.content.as_str())),
            _ => None,
        })
        .collect();

    let theirs_map: HashMap<&str, &str> = theirs_regions
        .iter()
        .filter_map(|r| match r {
            FileRegion::Interstitial(i) => Some((i.position_key.as_str(), i.content.as_str())),
            _ => None,
        })
        .collect();

    let mut all_keys: HashSet<&str> = HashSet::new();
    all_keys.extend(base_map.keys());
    all_keys.extend(ours_map.keys());
    all_keys.extend(theirs_map.keys());

    let mut merged: HashMap<String, String> = HashMap::new();

    for key in all_keys {
        let base_content = base_map.get(key).copied().unwrap_or("");
        let ours_content = ours_map.get(key).copied().unwrap_or("");
        let theirs_content = theirs_map.get(key).copied().unwrap_or("");

        // If all same, no merge needed
        if ours_content == theirs_content {
            merged.insert(key.to_string(), ours_content.to_string());
        } else if base_content == ours_content {
            merged.insert(key.to_string(), theirs_content.to_string());
        } else if base_content == theirs_content {
            merged.insert(key.to_string(), ours_content.to_string());
        } else {
            // Both changed — check if this is an import-heavy region
            if is_import_region(base_content)
                || is_import_region(ours_content)
                || is_import_region(theirs_content)
            {
                // Commutative merge: treat import lines as a set
                let result = merge_imports_commutatively(base_content, ours_content, theirs_content);
                merged.insert(key.to_string(), result);
            } else {
                // Regular line-level merge
                match diffy::merge(base_content, ours_content, theirs_content) {
                    Ok(m) => {
                        merged.insert(key.to_string(), m);
                    }
                    Err(conflicted) => {
                        merged.insert(key.to_string(), conflicted);
                    }
                }
            }
        }
    }

    merged
}

/// Check if a region is predominantly import/use statements.
fn is_import_region(content: &str) -> bool {
    let lines: Vec<&str> = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .collect();
    if lines.is_empty() {
        return false;
    }
    let import_count = lines.iter().filter(|l| is_import_line(l)).count();
    // If >50% of non-empty lines are imports, treat as import region
    import_count * 2 > lines.len()
}

/// Check if a line is an import/use/require statement.
fn is_import_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("import ")
        || trimmed.starts_with("from ")
        || trimmed.starts_with("use ")
        || trimmed.starts_with("require(")
        || trimmed.starts_with("const ") && trimmed.contains("require(")
        || trimmed.starts_with("package ")
        || trimmed.starts_with("#include ")
        || trimmed.starts_with("using ")
}

/// Merge import blocks commutatively (as unordered sets).
///
/// Algorithm (from Mergiraf's unordered merge):
/// 1. Compute imports deleted by ours (in base but not ours)
/// 2. Compute imports deleted by theirs (in base but not theirs)
/// 3. Compute imports added by ours (in ours but not base)
/// 4. Compute imports added by theirs (in theirs but not base)
/// 5. Start with base imports, remove both deletions, add both additions
/// 6. Preserve non-import lines from ours version
fn merge_imports_commutatively(base: &str, ours: &str, theirs: &str) -> String {
    let base_imports: Vec<&str> = base.lines().filter(|l| is_import_line(l)).collect();
    let ours_imports: Vec<&str> = ours.lines().filter(|l| is_import_line(l)).collect();
    let theirs_imports: Vec<&str> = theirs.lines().filter(|l| is_import_line(l)).collect();

    let base_set: HashSet<&str> = base_imports.iter().copied().collect();
    let ours_set: HashSet<&str> = ours_imports.iter().copied().collect();
    let theirs_set: HashSet<&str> = theirs_imports.iter().copied().collect();

    // Deletions: in base but removed by a branch
    let ours_deleted: HashSet<&str> = base_set.difference(&ours_set).copied().collect();
    let theirs_deleted: HashSet<&str> = base_set.difference(&theirs_set).copied().collect();

    // Additions: in branch but not in base
    let ours_added: Vec<&str> = ours_imports
        .iter()
        .filter(|i| !base_set.contains(**i))
        .copied()
        .collect();
    let theirs_added: Vec<&str> = theirs_imports
        .iter()
        .filter(|i| !base_set.contains(**i) && !ours_set.contains(**i))
        .copied()
        .collect();

    // Build merged import list: base - deletions + additions
    let mut merged_imports: Vec<&str> = base_imports
        .iter()
        .filter(|i| !ours_deleted.contains(**i) && !theirs_deleted.contains(**i))
        .copied()
        .collect();
    merged_imports.extend(ours_added);
    merged_imports.extend(theirs_added);

    // Collect non-import lines from ours (preserve comments, blank lines, etc.)
    let ours_non_imports: Vec<&str> = ours
        .lines()
        .filter(|l| !is_import_line(l))
        .collect();

    // Reconstruct: non-import preamble lines + merged imports
    let mut result_lines: Vec<&str> = Vec::new();

    // Add non-import lines that come before first import in ours
    let first_import_idx = ours
        .lines()
        .position(|l| is_import_line(l));

    if let Some(idx) = first_import_idx {
        for (i, line) in ours.lines().enumerate() {
            if i < idx {
                result_lines.push(line);
            }
        }
    }

    // Add merged imports
    result_lines.extend(&merged_imports);

    // Add non-import lines that come after imports in ours
    if let Some(idx) = first_import_idx {
        for (i, line) in ours.lines().enumerate() {
            if i <= idx {
                continue;
            }
            if is_import_line(line) {
                continue;
            }
            result_lines.push(line);
        }
    } else {
        // No imports in ours, just add non-import lines
        result_lines.extend(&ours_non_imports);
    }

    let mut result = result_lines.join("\n");
    // Preserve trailing newline
    if ours.ends_with('\n') || theirs.ends_with('\n') {
        if !result.ends_with('\n') {
            result.push('\n');
        }
    }
    result
}

/// Fallback to line-level 3-way merge when entity extraction isn't possible.
///
/// Uses Sesame-inspired separator preprocessing (arXiv:2407.18888) to get
/// finer-grained alignment before line-level merge. Inserts newlines around
/// syntactic separators ({, }, ;) so that changes in different code blocks
/// align independently, reducing spurious conflicts.
fn line_level_fallback(base: &str, ours: &str, theirs: &str) -> MergeResult {
    let mut stats = MergeStats::default();
    stats.used_fallback = true;

    // Preprocess: expand separators into separate lines for finer alignment
    let base_expanded = expand_separators(base);
    let ours_expanded = expand_separators(ours);
    let theirs_expanded = expand_separators(theirs);

    match diffy::merge(&base_expanded, &ours_expanded, &theirs_expanded) {
        Ok(merged) => {
            // Collapse back: remove the newlines we inserted
            let content = collapse_separators(&merged, base);
            MergeResult {
                content,
                conflicts: vec![],
                warnings: vec![],
                stats,
            }
        }
        Err(_) => {
            // If separator-expanded merge still conflicts, try plain merge
            // for cleaner conflict markers
            match diffy::merge(base, ours, theirs) {
                Ok(merged) => MergeResult {
                    content: merged,
                    conflicts: vec![],
                    warnings: vec![],
                    stats,
                },
                Err(conflicted_plain) => {
                    stats.entities_conflicted = 1;
                    MergeResult {
                        content: conflicted_plain,
                        conflicts: vec![EntityConflict {
                            entity_name: "(file)".to_string(),
                            entity_type: "file".to_string(),
                            kind: ConflictKind::BothModified,
                            complexity: classify_conflict(Some(base), Some(ours), Some(theirs)),
                            ours_content: Some(ours.to_string()),
                            theirs_content: Some(theirs.to_string()),
                            base_content: Some(base.to_string()),
                        }],
                        warnings: vec![],
                        stats,
                    }
                }
            }
        }
    }
}

/// Filter out entities that are nested inside other entities.
///
/// When a class contains methods which contain local variables, sem-core may extract
/// all of them as entities. But for merge purposes, nested entities are part of their
/// parent — we handle them via inner entity merge. Keeping them causes false conflicts
/// (e.g. two methods both declaring `const user` would appear as BothAdded).
fn filter_nested_entities(entities: Vec<SemanticEntity>) -> Vec<SemanticEntity> {
    if entities.len() <= 1 {
        return entities;
    }

    let mut keep: Vec<bool> = vec![true; entities.len()];

    for (i, entity) in entities.iter().enumerate() {
        if !keep[i] {
            continue;
        }
        // Check if this entity is fully contained within another entity
        for (j, other) in entities.iter().enumerate() {
            if i == j {
                continue;
            }
            if entity.start_line >= other.start_line
                && entity.end_line <= other.end_line
                && !(entity.start_line == other.start_line && entity.end_line == other.end_line)
            {
                // entity is strictly nested inside other
                keep[i] = false;
                break;
            }
        }
    }

    entities
        .into_iter()
        .enumerate()
        .filter(|(i, _)| keep[*i])
        .map(|(_, e)| e)
        .collect()
}

/// Compute a body hash for rename detection: the entity content with the entity
/// name replaced at word boundaries by a placeholder, so entities with identical
/// bodies but different names produce the same hash.
///
/// Uses word-boundary matching to avoid partial replacements (e.g. replacing
/// "get" inside "getAll"). Works across all languages since it operates on
/// the content string, not language-specific AST features.
fn body_hash(entity: &SemanticEntity) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let normalized = replace_at_word_boundaries(&entity.content, &entity.name, "__ENTITY__");
    let mut hasher = DefaultHasher::new();
    normalized.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Replace `needle` with `replacement` only at word boundaries.
/// A word boundary means the character before/after the match is not
/// alphanumeric or underscore (i.e. not an identifier character).
fn replace_at_word_boundaries(content: &str, needle: &str, replacement: &str) -> String {
    if needle.is_empty() {
        return content.to_string();
    }
    let bytes = content.as_bytes();
    let mut result = String::with_capacity(content.len());
    let mut i = 0;
    while i < content.len() {
        if content.is_char_boundary(i) && content[i..].starts_with(needle) {
            let before_ok = i == 0 || {
                let prev_idx = content[..i]
                    .char_indices()
                    .next_back()
                    .map(|(idx, _)| idx)
                    .unwrap_or(0);
                !is_ident_char(bytes[prev_idx])
            };
            let after_idx = i + needle.len();
            let after_ok = after_idx >= content.len()
                || (content.is_char_boundary(after_idx)
                    && !is_ident_char(bytes[after_idx]));
            if before_ok && after_ok {
                result.push_str(replacement);
                i += needle.len();
                continue;
            }
        }
        if content.is_char_boundary(i) {
            let ch = content[i..].chars().next().unwrap();
            result.push(ch);
            i += ch.len_utf8();
        } else {
            i += 1;
        }
    }
    result
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Build a rename map from new_id → base_id using body hash matching.
///
/// Detects when an entity in the branch has the same body as an entity
/// in base but a different name/ID, indicating it was renamed.
/// Uses body_hash (name-stripped content hash) instead of structural_hash
/// so that pure renames (same body, different name) are detected.
fn build_rename_map(
    base_entities: &[SemanticEntity],
    branch_entities: &[SemanticEntity],
) -> HashMap<String, String> {
    let mut rename_map: HashMap<String, String> = HashMap::new();

    // Build body_hash → entity map for base
    let mut base_by_body: HashMap<String, &SemanticEntity> = HashMap::new();
    let base_ids: HashSet<&str> = base_entities.iter().map(|e| e.id.as_str()).collect();
    for entity in base_entities {
        base_by_body.insert(body_hash(entity), entity);
    }

    // Also keep structural_hash index as fallback (for cases where name doesn't
    // appear literally in content, e.g. some generated entities)
    let mut base_by_structural: HashMap<&str, &SemanticEntity> = HashMap::new();
    for entity in base_entities {
        if let Some(ref sh) = entity.structural_hash {
            base_by_structural.insert(sh.as_str(), entity);
        }
    }

    // Find branch entities that aren't in base by ID but match by body hash or structural_hash
    let mut used_base_ids: HashSet<String> = HashSet::new();
    for branch_entity in branch_entities {
        // Skip entities that exist in base by ID (not renamed)
        if base_ids.contains(branch_entity.id.as_str()) {
            continue;
        }

        // Try body hash match first (handles pure renames)
        let bh = body_hash(branch_entity);
        let matched_base = if let Some(base_entity) = base_by_body.get(&bh) {
            Some(*base_entity)
        } else if let Some(ref sh) = branch_entity.structural_hash {
            // Fallback to structural_hash (original behavior)
            base_by_structural.get(sh.as_str()).copied()
        } else {
            None
        };

        if let Some(base_entity) = matched_base {
            if !used_base_ids.contains(&base_entity.id) {
                let base_id_in_branch = branch_entities.iter().any(|e| e.id == base_entity.id);
                if !base_id_in_branch {
                    rename_map.insert(branch_entity.id.clone(), base_entity.id.clone());
                    used_base_ids.insert(base_entity.id.clone());
                }
            }
        }
    }

    rename_map
}

/// Check if an entity type is a container that may benefit from inner entity merge.
fn is_container_entity_type(entity_type: &str) -> bool {
    matches!(
        entity_type,
        "class" | "interface" | "enum" | "impl" | "trait" | "module" | "impl_item" | "trait_item"
    )
}

/// A named member chunk extracted from a class/container body.
#[derive(Debug, Clone)]
struct MemberChunk {
    /// The member name (method name, field name, etc.)
    name: String,
    /// Full content of the member including its body
    content: String,
}

/// Try recursive inner entity merge for container types (classes, impls, etc.).
///
/// Inspired by LastMerge (arXiv:2507.19687): class members are "unordered children" —
/// reordering them is not a conflict. We chunk the class body into members, match by
/// name, and merge each member independently.
///
/// Returns Some(merged_content) if successful, None if there are unresolvable conflicts.
fn try_inner_entity_merge(base: &str, ours: &str, theirs: &str) -> Option<String> {
    let base_chunks = extract_member_chunks(base)?;
    let ours_chunks = extract_member_chunks(ours)?;
    let theirs_chunks = extract_member_chunks(theirs)?;

    // Need at least 1 member to attempt inner merge
    // (Even single-member containers benefit from decorator-aware merge)
    if base_chunks.is_empty() && ours_chunks.is_empty() && theirs_chunks.is_empty() {
        return None;
    }

    // Build name → content maps
    let base_map: HashMap<&str, &str> = base_chunks
        .iter()
        .map(|c| (c.name.as_str(), c.content.as_str()))
        .collect();
    let ours_map: HashMap<&str, &str> = ours_chunks
        .iter()
        .map(|c| (c.name.as_str(), c.content.as_str()))
        .collect();
    let theirs_map: HashMap<&str, &str> = theirs_chunks
        .iter()
        .map(|c| (c.name.as_str(), c.content.as_str()))
        .collect();

    // Collect all member names
    let mut all_names: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    // Use ours ordering as skeleton
    for chunk in &ours_chunks {
        if seen.insert(chunk.name.clone()) {
            all_names.push(chunk.name.clone());
        }
    }
    // Add theirs-only members
    for chunk in &theirs_chunks {
        if seen.insert(chunk.name.clone()) {
            all_names.push(chunk.name.clone());
        }
    }

    // Extract header/footer (class declaration line and closing brace)
    let (ours_header, ours_footer) = extract_container_wrapper(ours)?;

    let mut merged_members: Vec<String> = Vec::new();
    let mut has_conflict = false;

    for name in &all_names {
        let in_base = base_map.get(name.as_str());
        let in_ours = ours_map.get(name.as_str());
        let in_theirs = theirs_map.get(name.as_str());

        match (in_base, in_ours, in_theirs) {
            // In all three
            (Some(b), Some(o), Some(t)) => {
                if o == t {
                    // Both same (or both unchanged)
                    merged_members.push(o.to_string());
                } else if b == o {
                    // Only theirs changed
                    merged_members.push(t.to_string());
                } else if b == t {
                    // Only ours changed
                    merged_members.push(o.to_string());
                } else {
                    // Both changed differently — try diffy on this member
                    if let Some(merged) = diffy_merge(b, o, t) {
                        merged_members.push(merged);
                    } else if let Some(merged) = try_decorator_aware_merge(b, o, t) {
                        // Try decorator-aware merge on the member
                        merged_members.push(merged);
                    } else {
                        has_conflict = true;
                        break;
                    }
                }
            }
            // Deleted by theirs, ours unchanged or not in base
            (Some(b), Some(o), None) => {
                if *b == *o {
                    // Ours unchanged, theirs deleted → accept deletion
                } else {
                    // Ours modified, theirs deleted → conflict
                    has_conflict = true;
                    break;
                }
            }
            // Deleted by ours, theirs unchanged or not in base
            (Some(b), None, Some(t)) => {
                if *b == *t {
                    // Theirs unchanged, ours deleted → accept deletion
                } else {
                    // Theirs modified, ours deleted → conflict
                    has_conflict = true;
                    break;
                }
            }
            // Added by ours only
            (None, Some(o), None) => {
                merged_members.push(o.to_string());
            }
            // Added by theirs only
            (None, None, Some(t)) => {
                merged_members.push(t.to_string());
            }
            // Added by both
            (None, Some(o), Some(t)) => {
                if o == t {
                    merged_members.push(o.to_string());
                } else {
                    // Both added different content with same name → conflict
                    has_conflict = true;
                    break;
                }
            }
            // Deleted by both
            (Some(_), None, None) => {
                // Both deleted → accept
            }
            (None, None, None) => {}
        }
    }

    if has_conflict {
        return None;
    }

    // Reconstruct: header + merged members + footer
    let mut result = String::new();
    result.push_str(ours_header);
    if !ours_header.ends_with('\n') {
        result.push('\n');
    }

    // Detect if members are single-line (fields, variants) vs multi-line (methods)
    let has_multiline_members = merged_members.iter().any(|m| m.contains('\n'));

    for (i, member) in merged_members.iter().enumerate() {
        result.push_str(member);
        if !member.ends_with('\n') {
            result.push('\n');
        }
        // Add blank line between multi-line members (methods) but not single-line (fields, variants)
        if i < merged_members.len() - 1 && has_multiline_members && !member.ends_with("\n\n") {
            result.push('\n');
        }
    }

    result.push_str(ours_footer);
    if !ours_footer.ends_with('\n') && ours.ends_with('\n') {
        result.push('\n');
    }

    Some(result)
}

/// Extract the header (class declaration) and footer (closing brace) from a container.
/// Supports both brace-delimited (JS/TS/Java/Rust/C) and indentation-based (Python) containers.
fn extract_container_wrapper(content: &str) -> Option<(&str, &str)> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() < 2 {
        return None;
    }

    // Check if this is a Python-style container (ends with `:` instead of `{`)
    let is_python_style = lines.iter().any(|l| {
        let trimmed = l.trim();
        (trimmed.starts_with("class ") || trimmed.starts_with("def "))
            && trimmed.ends_with(':')
    }) && !lines.iter().any(|l| l.contains('{'));

    if is_python_style {
        // Python: header is the `class Foo:` line, no footer
        let header_end = lines.iter().position(|l| l.trim().ends_with(':'))?;
        let header_byte_end: usize = lines[..=header_end]
            .iter()
            .map(|l| l.len() + 1)
            .sum();
        let header = &content[..header_byte_end.min(content.len())];
        // No closing brace in Python — footer is empty
        let footer = &content[content.len()..];
        Some((header, footer))
    } else {
        // Brace-delimited: header up to `{`, footer from last `}`
        let header_end = lines.iter().position(|l| l.contains('{'))?;
        let header_byte_end = lines[..=header_end]
            .iter()
            .map(|l| l.len() + 1)
            .sum::<usize>();
        let header = &content[..header_byte_end.min(content.len())];

        let footer_start = lines.iter().rposition(|l| {
            let trimmed = l.trim();
            trimmed == "}" || trimmed == "};"
        })?;

        let footer_byte_start: usize = lines[..footer_start]
            .iter()
            .map(|l| l.len() + 1)
            .sum();
        let footer = &content[footer_byte_start.min(content.len())..];

        Some((header, footer))
    }
}

/// Extract named member chunks from a container body.
///
/// Identifies member boundaries by indentation: members start at the first
/// indentation level inside the container. Each member extends until the next
/// member starts or the container closes.
fn extract_member_chunks(content: &str) -> Option<Vec<MemberChunk>> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() < 2 {
        return None;
    }

    // Check if Python-style (indentation-based)
    let is_python_style = lines.iter().any(|l| {
        let trimmed = l.trim();
        (trimmed.starts_with("class ") || trimmed.starts_with("def "))
            && trimmed.ends_with(':')
    }) && !lines.iter().any(|l| l.contains('{'));

    // Find the body range
    let body_start = if is_python_style {
        lines.iter().position(|l| l.trim().ends_with(':'))? + 1
    } else {
        lines.iter().position(|l| l.contains('{'))? + 1
    };
    let body_end = if is_python_style {
        // Python: body extends to end of content
        lines.len()
    } else {
        lines.iter().rposition(|l| {
            let trimmed = l.trim();
            trimmed == "}" || trimmed == "};"
        })?
    };

    if body_start >= body_end {
        return None;
    }

    // Determine member indentation level by looking at first non-empty body line
    let member_indent = lines[body_start..body_end]
        .iter()
        .find(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())?;

    let mut chunks: Vec<MemberChunk> = Vec::new();
    let mut current_chunk_lines: Vec<&str> = Vec::new();
    let mut current_name: Option<String> = None;

    for line in &lines[body_start..body_end] {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            // Blank lines: if we have a current chunk, include them
            if current_name.is_some() {
                // Only include if not trailing blanks
                current_chunk_lines.push(line);
            }
            continue;
        }

        let indent = line.len() - line.trim_start().len();

        // Is this a new member declaration at the member indent level?
        // Exclude closing braces, comments, and decorators/annotations
        if indent == member_indent
            && !trimmed.starts_with("//")
            && !trimmed.starts_with("/*")
            && !trimmed.starts_with("*")
            && !trimmed.starts_with("#")
            && !trimmed.starts_with("@")
            && trimmed != "}"
            && trimmed != "};"
            && trimmed != ","
        {
            // Save previous chunk
            if let Some(name) = current_name.take() {
                // Trim trailing blank lines
                while current_chunk_lines.last().map_or(false, |l| l.trim().is_empty()) {
                    current_chunk_lines.pop();
                }
                if !current_chunk_lines.is_empty() {
                    chunks.push(MemberChunk {
                        name,
                        content: current_chunk_lines.join("\n"),
                    });
                }
                current_chunk_lines.clear();
            }

            // Start new chunk — extract member name
            let name = extract_member_name(trimmed);
            current_name = Some(name);
            current_chunk_lines.push(line);
        } else if current_name.is_some() {
            // Continuation of current member (body lines, nested blocks)
            current_chunk_lines.push(line);
        } else {
            // Content before first member (decorators, comments for first member)
            // Attach to next member
            current_chunk_lines.push(line);
        }
    }

    // Save last chunk
    if let Some(name) = current_name {
        while current_chunk_lines.last().map_or(false, |l| l.trim().is_empty()) {
            current_chunk_lines.pop();
        }
        if !current_chunk_lines.is_empty() {
            chunks.push(MemberChunk {
                name,
                content: current_chunk_lines.join("\n"),
            });
        }
    }

    if chunks.is_empty() {
        None
    } else {
        Some(chunks)
    }
}

/// Extract a member name from a declaration line.
fn extract_member_name(line: &str) -> String {
    let trimmed = line.trim();

    // Try common patterns:
    // method(...)  or  async method(...)  or  public method(...)
    // fn name(...)
    // def name(...)
    // name: type  (field)
    // get name() / set name()

    // Remove leading keywords
    let mut s = trimmed;
    for keyword in &[
        "export ", "public ", "private ", "protected ", "static ",
        "abstract ", "async ", "override ", "readonly ",
        "pub ", "pub(crate) ", "fn ", "def ", "get ", "set ",
    ] {
        if s.starts_with(keyword) {
            s = &s[keyword.len()..];
        }
    }
    // Handle `fn ` after `pub`
    if s.starts_with("fn ") {
        s = &s[3..];
    }

    // Extract identifier (alphanumeric + underscore until non-identifier char)
    let name: String = s
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();

    if name.is_empty() {
        // Fallback: use first few chars as pseudo-name
        trimmed.chars().take(20).collect()
    } else {
        name
    }
}

/// Expand syntactic separators into separate lines for finer merge alignment.
/// Inspired by Sesame (arXiv:2407.18888): isolating separators lets line-based
/// merge tools see block boundaries as independent change units.
fn expand_separators(content: &str) -> String {
    let mut result = String::with_capacity(content.len() * 2);
    let mut in_string = false;
    let mut escape_next = false;
    let mut string_char = '"';

    for ch in content.chars() {
        if escape_next {
            result.push(ch);
            escape_next = false;
            continue;
        }
        if ch == '\\' && in_string {
            result.push(ch);
            escape_next = true;
            continue;
        }
        if !in_string && (ch == '"' || ch == '\'' || ch == '`') {
            in_string = true;
            string_char = ch;
            result.push(ch);
            continue;
        }
        if in_string && ch == string_char {
            in_string = false;
            result.push(ch);
            continue;
        }

        if !in_string && (ch == '{' || ch == '}' || ch == ';') {
            // Ensure separator is on its own line
            if !result.ends_with('\n') && !result.is_empty() {
                result.push('\n');
            }
            result.push(ch);
            result.push('\n');
        } else {
            result.push(ch);
        }
    }

    result
}

/// Collapse separator expansion back to original formatting.
/// Uses the base formatting as a guide where possible.
fn collapse_separators(merged: &str, _base: &str) -> String {
    // Simple approach: join lines that contain only a separator with adjacent lines
    let lines: Vec<&str> = merged.lines().collect();
    let mut result = String::new();
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim();
        if (trimmed == "{" || trimmed == "}" || trimmed == ";") && trimmed.len() == 1 {
            // This is a separator-only line we may have created
            // Try to join with previous line if it doesn't end with a separator
            if !result.is_empty() && !result.ends_with('\n') {
                // Peek: if it's an opening brace, join with previous
                if trimmed == "{" {
                    result.push(' ');
                    result.push_str(trimmed);
                    result.push('\n');
                } else if trimmed == "}" {
                    result.push('\n');
                    result.push_str(trimmed);
                    result.push('\n');
                } else {
                    result.push_str(trimmed);
                    result.push('\n');
                }
            } else {
                result.push_str(lines[i]);
                result.push('\n');
            }
        } else {
            result.push_str(lines[i]);
            result.push('\n');
        }
        i += 1;
    }

    // Trim any trailing extra newlines to match original style
    while result.ends_with("\n\n") {
        result.pop();
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replace_at_word_boundaries() {
        // Should replace standalone occurrences
        assert_eq!(replace_at_word_boundaries("fn get() {}", "get", "__E__"), "fn __E__() {}");
        // Should NOT replace inside longer identifiers
        assert_eq!(replace_at_word_boundaries("fn getAll() {}", "get", "__E__"), "fn getAll() {}");
        assert_eq!(replace_at_word_boundaries("fn _get() {}", "get", "__E__"), "fn _get() {}");
        // Should replace multiple standalone occurrences
        assert_eq!(
            replace_at_word_boundaries("pub enum Source { Source }", "Source", "__E__"),
            "pub enum __E__ { __E__ }"
        );
        // Should not replace substring at start/end of identifiers
        assert_eq!(
            replace_at_word_boundaries("SourceManager isSource", "Source", "__E__"),
            "SourceManager isSource"
        );
        // Should handle multi-byte UTF-8 characters (emojis) without panicking
        assert_eq!(
            replace_at_word_boundaries("❌ get ✅", "get", "__E__"),
            "❌ __E__ ✅"
        );
        assert_eq!(
            replace_at_word_boundaries("fn 名前() { get }", "get", "__E__"),
            "fn 名前() { __E__ }"
        );
        // Emoji-only content with no needle match should pass through unchanged
        assert_eq!(
            replace_at_word_boundaries("🎉🚀✨", "get", "__E__"),
            "🎉🚀✨"
        );
    }

    #[test]
    fn test_fast_path_identical() {
        let content = "hello world";
        let result = entity_merge(content, content, content, "test.ts");
        assert!(result.is_clean());
        assert_eq!(result.content, content);
    }

    #[test]
    fn test_fast_path_only_ours_changed() {
        let base = "hello";
        let ours = "hello world";
        let result = entity_merge(base, ours, base, "test.ts");
        assert!(result.is_clean());
        assert_eq!(result.content, ours);
    }

    #[test]
    fn test_fast_path_only_theirs_changed() {
        let base = "hello";
        let theirs = "hello world";
        let result = entity_merge(base, base, theirs, "test.ts");
        assert!(result.is_clean());
        assert_eq!(result.content, theirs);
    }

    #[test]
    fn test_different_functions_no_conflict() {
        // Core value prop: two agents add different functions to the same file
        let base = r#"export function existing() {
    return 1;
}
"#;
        let ours = r#"export function existing() {
    return 1;
}

export function agentA() {
    return "added by agent A";
}
"#;
        let theirs = r#"export function existing() {
    return 1;
}

export function agentB() {
    return "added by agent B";
}
"#;
        let result = entity_merge(base, ours, theirs, "test.ts");
        assert!(
            result.is_clean(),
            "Should auto-resolve: different functions added. Conflicts: {:?}",
            result.conflicts
        );
        assert!(
            result.content.contains("agentA"),
            "Should contain agentA function"
        );
        assert!(
            result.content.contains("agentB"),
            "Should contain agentB function"
        );
    }

    #[test]
    fn test_same_function_modified_by_both_conflict() {
        let base = r#"export function shared() {
    return "original";
}
"#;
        let ours = r#"export function shared() {
    return "modified by ours";
}
"#;
        let theirs = r#"export function shared() {
    return "modified by theirs";
}
"#;
        let result = entity_merge(base, ours, theirs, "test.ts");
        // This should be a conflict since both modified the same function incompatibly
        assert!(
            !result.is_clean(),
            "Should conflict when both modify same function differently"
        );
        assert_eq!(result.conflicts.len(), 1);
        assert_eq!(result.conflicts[0].entity_name, "shared");
    }

    #[test]
    fn test_fallback_for_unknown_filetype() {
        // Non-adjacent changes should merge cleanly with line-level merge
        let base = "line 1\nline 2\nline 3\nline 4\nline 5\n";
        let ours = "line 1 modified\nline 2\nline 3\nline 4\nline 5\n";
        let theirs = "line 1\nline 2\nline 3\nline 4\nline 5 modified\n";
        let result = entity_merge(base, ours, theirs, "test.xyz");
        assert!(
            result.is_clean(),
            "Non-adjacent changes should merge cleanly. Conflicts: {:?}",
            result.conflicts,
        );
    }

    #[test]
    fn test_line_level_fallback() {
        // Non-adjacent changes merge cleanly in 3-way merge
        let base = "a\nb\nc\nd\ne\n";
        let ours = "A\nb\nc\nd\ne\n";
        let theirs = "a\nb\nc\nd\nE\n";
        let result = line_level_fallback(base, ours, theirs);
        assert!(result.is_clean());
        assert!(result.stats.used_fallback);
        assert_eq!(result.content, "A\nb\nc\nd\nE\n");
    }

    #[test]
    fn test_line_level_fallback_conflict() {
        // Same line changed differently → conflict
        let base = "a\nb\nc\n";
        let ours = "X\nb\nc\n";
        let theirs = "Y\nb\nc\n";
        let result = line_level_fallback(base, ours, theirs);
        assert!(!result.is_clean());
        assert!(result.stats.used_fallback);
    }

    #[test]
    fn test_expand_separators() {
        let code = "function foo() { return 1; }";
        let expanded = expand_separators(code);
        // Separators should be on their own lines
        assert!(expanded.contains("{\n"), "Opening brace should have newline after");
        assert!(expanded.contains(";\n"), "Semicolons should have newline after");
        assert!(expanded.contains("\n}"), "Closing brace should have newline before");
    }

    #[test]
    fn test_expand_separators_preserves_strings() {
        let code = r#"let x = "hello { world };";"#;
        let expanded = expand_separators(code);
        // Separators inside strings should NOT be expanded
        assert!(
            expanded.contains("\"hello { world };\""),
            "Separators in strings should be preserved: {}",
            expanded
        );
    }

    #[test]
    fn test_is_import_region() {
        assert!(is_import_region("import foo from 'foo';\nimport bar from 'bar';\n"));
        assert!(is_import_region("use std::io;\nuse std::fs;\n"));
        assert!(!is_import_region("let x = 1;\nlet y = 2;\n"));
        // Mixed: 1 import + 2 non-imports → not import region
        assert!(!is_import_region("import foo from 'foo';\nlet x = 1;\nlet y = 2;\n"));
        // Empty → not import region
        assert!(!is_import_region(""));
    }

    #[test]
    fn test_is_import_line() {
        // JS/TS
        assert!(is_import_line("import foo from 'foo';"));
        assert!(is_import_line("import { bar } from 'bar';"));
        assert!(is_import_line("from typing import List"));
        // Rust
        assert!(is_import_line("use std::io::Read;"));
        // C/C++
        assert!(is_import_line("#include <stdio.h>"));
        // Node require
        assert!(is_import_line("const fs = require('fs');"));
        // Not imports
        assert!(!is_import_line("let x = 1;"));
        assert!(!is_import_line("function foo() {}"));
    }

    #[test]
    fn test_commutative_import_merge_both_add_different() {
        // The key scenario: both branches add different imports
        let base = "import a from 'a';\nimport b from 'b';\n";
        let ours = "import a from 'a';\nimport b from 'b';\nimport c from 'c';\n";
        let theirs = "import a from 'a';\nimport b from 'b';\nimport d from 'd';\n";
        let result = merge_imports_commutatively(base, ours, theirs);
        assert!(result.contains("import a from 'a';"));
        assert!(result.contains("import b from 'b';"));
        assert!(result.contains("import c from 'c';"));
        assert!(result.contains("import d from 'd';"));
    }

    #[test]
    fn test_commutative_import_merge_one_removes() {
        // Ours removes an import, theirs keeps it → removed
        let base = "import a from 'a';\nimport b from 'b';\nimport c from 'c';\n";
        let ours = "import a from 'a';\nimport c from 'c';\n";
        let theirs = "import a from 'a';\nimport b from 'b';\nimport c from 'c';\n";
        let result = merge_imports_commutatively(base, ours, theirs);
        assert!(result.contains("import a from 'a';"));
        assert!(!result.contains("import b from 'b';"), "Removed import should stay removed");
        assert!(result.contains("import c from 'c';"));
    }

    #[test]
    fn test_commutative_import_merge_both_add_same() {
        // Both add the same import → should appear only once
        let base = "import a from 'a';\n";
        let ours = "import a from 'a';\nimport b from 'b';\n";
        let theirs = "import a from 'a';\nimport b from 'b';\n";
        let result = merge_imports_commutatively(base, ours, theirs);
        let count = result.matches("import b from 'b';").count();
        assert_eq!(count, 1, "Duplicate import should be deduplicated");
    }

    #[test]
    fn test_inner_entity_merge_different_methods() {
        // Two agents modify different methods in the same class
        // This would normally conflict with diffy because the changes are adjacent
        let base = r#"export class Calculator {
    add(a: number, b: number): number {
        return a + b;
    }

    subtract(a: number, b: number): number {
        return a - b;
    }
}
"#;
        let ours = r#"export class Calculator {
    add(a: number, b: number): number {
        // Added logging
        console.log("adding", a, b);
        return a + b;
    }

    subtract(a: number, b: number): number {
        return a - b;
    }
}
"#;
        let theirs = r#"export class Calculator {
    add(a: number, b: number): number {
        return a + b;
    }

    subtract(a: number, b: number): number {
        // Added validation
        if (b > a) throw new Error("negative");
        return a - b;
    }
}
"#;
        let result = entity_merge(base, ours, theirs, "test.ts");
        assert!(
            result.is_clean(),
            "Different methods modified should auto-merge via inner entity merge. Conflicts: {:?}",
            result.conflicts,
        );
        assert!(result.content.contains("console.log"), "Should contain ours changes");
        assert!(result.content.contains("negative"), "Should contain theirs changes");
    }

    #[test]
    fn test_inner_entity_merge_both_add_different_methods() {
        // Both branches add different methods to the same class
        let base = r#"export class Calculator {
    add(a: number, b: number): number {
        return a + b;
    }
}
"#;
        let ours = r#"export class Calculator {
    add(a: number, b: number): number {
        return a + b;
    }

    multiply(a: number, b: number): number {
        return a * b;
    }
}
"#;
        let theirs = r#"export class Calculator {
    add(a: number, b: number): number {
        return a + b;
    }

    divide(a: number, b: number): number {
        return a / b;
    }
}
"#;
        let result = entity_merge(base, ours, theirs, "test.ts");
        assert!(
            result.is_clean(),
            "Both adding different methods should auto-merge. Conflicts: {:?}",
            result.conflicts,
        );
        assert!(result.content.contains("multiply"), "Should contain ours's new method");
        assert!(result.content.contains("divide"), "Should contain theirs's new method");
    }

    #[test]
    fn test_inner_entity_merge_same_method_modified_still_conflicts() {
        // Both modify the same method differently → should still conflict
        let base = r#"export class Calculator {
    add(a: number, b: number): number {
        return a + b;
    }

    subtract(a: number, b: number): number {
        return a - b;
    }
}
"#;
        let ours = r#"export class Calculator {
    add(a: number, b: number): number {
        return a + b + 1;
    }

    subtract(a: number, b: number): number {
        return a - b;
    }
}
"#;
        let theirs = r#"export class Calculator {
    add(a: number, b: number): number {
        return a + b + 2;
    }

    subtract(a: number, b: number): number {
        return a - b;
    }
}
"#;
        let result = entity_merge(base, ours, theirs, "test.ts");
        assert!(
            !result.is_clean(),
            "Both modifying same method differently should still conflict"
        );
    }

    #[test]
    fn test_extract_member_chunks() {
        let class_body = r#"export class Foo {
    bar() {
        return 1;
    }

    baz() {
        return 2;
    }
}
"#;
        let chunks = extract_member_chunks(class_body).unwrap();
        assert_eq!(chunks.len(), 2, "Should find 2 members, found {:?}", chunks.iter().map(|c| &c.name).collect::<Vec<_>>());
        assert_eq!(chunks[0].name, "bar");
        assert_eq!(chunks[1].name, "baz");
    }

    #[test]
    fn test_extract_member_name() {
        assert_eq!(extract_member_name("add(a, b) {"), "add");
        assert_eq!(extract_member_name("fn add(&self, a: i32) -> i32 {"), "add");
        assert_eq!(extract_member_name("def add(self, a, b):"), "add");
        assert_eq!(extract_member_name("public static getValue(): number {"), "getValue");
        assert_eq!(extract_member_name("async fetchData() {"), "fetchData");
    }

    #[test]
    fn test_commutative_import_merge_rust_use() {
        let base = "use std::io;\nuse std::fs;\n";
        let ours = "use std::io;\nuse std::fs;\nuse std::path::Path;\n";
        let theirs = "use std::io;\nuse std::fs;\nuse std::collections::HashMap;\n";
        let result = merge_imports_commutatively(base, ours, theirs);
        assert!(result.contains("use std::path::Path;"));
        assert!(result.contains("use std::collections::HashMap;"));
        assert!(result.contains("use std::io;"));
        assert!(result.contains("use std::fs;"));
    }

    #[test]
    fn test_is_whitespace_only_diff_true() {
        // Same content, different indentation
        assert!(is_whitespace_only_diff(
            "    return 1;\n    return 2;\n",
            "      return 1;\n      return 2;\n"
        ));
        // Same content, extra blank lines
        assert!(is_whitespace_only_diff(
            "return 1;\nreturn 2;\n",
            "return 1;\n\nreturn 2;\n"
        ));
    }

    #[test]
    fn test_is_whitespace_only_diff_false() {
        // Different content
        assert!(!is_whitespace_only_diff(
            "    return 1;\n",
            "    return 2;\n"
        ));
        // Added code
        assert!(!is_whitespace_only_diff(
            "return 1;\n",
            "return 1;\nconsole.log('x');\n"
        ));
    }

    #[test]
    fn test_ts_interface_both_add_different_fields() {
        let base = "interface Config {\n    name: string;\n}\n";
        let ours = "interface Config {\n    name: string;\n    age: number;\n}\n";
        let theirs = "interface Config {\n    name: string;\n    email: string;\n}\n";
        let result = entity_merge(base, ours, theirs, "test.ts");
        eprintln!("TS interface: clean={}, conflicts={:?}", result.is_clean(), result.conflicts);
        eprintln!("Content: {:?}", result.content);
        assert!(
            result.is_clean(),
            "Both adding different fields to TS interface should merge. Conflicts: {:?}",
            result.conflicts,
        );
        assert!(result.content.contains("age"));
        assert!(result.content.contains("email"));
    }

    #[test]
    fn test_rust_enum_both_add_different_variants() {
        let base = "enum Color {\n    Red,\n    Blue,\n}\n";
        let ours = "enum Color {\n    Red,\n    Blue,\n    Green,\n}\n";
        let theirs = "enum Color {\n    Red,\n    Blue,\n    Yellow,\n}\n";
        let result = entity_merge(base, ours, theirs, "test.rs");
        eprintln!("Rust enum: clean={}, conflicts={:?}", result.is_clean(), result.conflicts);
        eprintln!("Content: {:?}", result.content);
        assert!(
            result.is_clean(),
            "Both adding different enum variants should merge. Conflicts: {:?}",
            result.conflicts,
        );
        assert!(result.content.contains("Green"));
        assert!(result.content.contains("Yellow"));
    }

    #[test]
    fn test_python_both_add_different_decorators() {
        // Both add different decorators to the same function
        let base = "def foo():\n    return 1\n\ndef bar():\n    return 2\n";
        let ours = "@cache\ndef foo():\n    return 1\n\ndef bar():\n    return 2\n";
        let theirs = "@deprecated\ndef foo():\n    return 1\n\ndef bar():\n    return 2\n";
        let result = entity_merge(base, ours, theirs, "test.py");
        assert!(
            result.is_clean(),
            "Both adding different decorators should merge. Conflicts: {:?}",
            result.conflicts,
        );
        assert!(result.content.contains("@cache"));
        assert!(result.content.contains("@deprecated"));
        assert!(result.content.contains("def foo()"));
    }

    #[test]
    fn test_decorator_plus_body_change() {
        // One adds decorator, other modifies body — should merge both
        let base = "def foo():\n    return 1\n";
        let ours = "@cache\ndef foo():\n    return 1\n";
        let theirs = "def foo():\n    return 42\n";
        let result = entity_merge(base, ours, theirs, "test.py");
        assert!(
            result.is_clean(),
            "Decorator + body change should merge. Conflicts: {:?}",
            result.conflicts,
        );
        assert!(result.content.contains("@cache"));
        assert!(result.content.contains("return 42"));
    }

    #[test]
    fn test_ts_class_decorator_merge() {
        // TypeScript decorators on class methods — both add different decorators
        let base = "class Foo {\n    bar() {\n        return 1;\n    }\n}\n";
        let ours = "class Foo {\n    @Injectable()\n    bar() {\n        return 1;\n    }\n}\n";
        let theirs = "class Foo {\n    @Deprecated()\n    bar() {\n        return 1;\n    }\n}\n";
        let result = entity_merge(base, ours, theirs, "test.ts");
        assert!(
            result.is_clean(),
            "Both adding different decorators to same method should merge. Conflicts: {:?}",
            result.conflicts,
        );
        assert!(result.content.contains("@Injectable()"));
        assert!(result.content.contains("@Deprecated()"));
        assert!(result.content.contains("bar()"));
    }

    #[test]
    fn test_non_adjacent_intra_function_changes() {
        let base = r#"export function process(data: any) {
    const validated = validate(data);
    const transformed = transform(validated);
    const saved = save(transformed);
    return saved;
}
"#;
        let ours = r#"export function process(data: any) {
    const validated = validate(data);
    const transformed = transform(validated);
    const saved = save(transformed);
    console.log("saved", saved);
    return saved;
}
"#;
        let theirs = r#"export function process(data: any) {
    console.log("input", data);
    const validated = validate(data);
    const transformed = transform(validated);
    const saved = save(transformed);
    return saved;
}
"#;
        let result = entity_merge(base, ours, theirs, "test.ts");
        assert!(
            result.is_clean(),
            "Non-adjacent changes within same function should merge via diffy. Conflicts: {:?}",
            result.conflicts,
        );
        assert!(result.content.contains("console.log(\"saved\""));
        assert!(result.content.contains("console.log(\"input\""));
    }

    #[test]
    fn test_method_reordering_with_modification() {
        // Agent A reorders methods in class, Agent B modifies one method
        // Inner entity merge matches by name, so reordering should be transparent
        let base = r#"class Service {
    getUser(id: string) {
        return db.find(id);
    }

    createUser(data: any) {
        return db.create(data);
    }

    deleteUser(id: string) {
        return db.delete(id);
    }
}
"#;
        // Ours: reorder methods (move deleteUser before createUser)
        let ours = r#"class Service {
    getUser(id: string) {
        return db.find(id);
    }

    deleteUser(id: string) {
        return db.delete(id);
    }

    createUser(data: any) {
        return db.create(data);
    }
}
"#;
        // Theirs: modify getUser
        let theirs = r#"class Service {
    getUser(id: string) {
        console.log("fetching", id);
        return db.find(id);
    }

    createUser(data: any) {
        return db.create(data);
    }

    deleteUser(id: string) {
        return db.delete(id);
    }
}
"#;
        let result = entity_merge(base, ours, theirs, "test.ts");
        eprintln!("Method reorder: clean={}, conflicts={:?}", result.is_clean(), result.conflicts);
        eprintln!("Content:\n{}", result.content);
        assert!(
            result.is_clean(),
            "Method reordering + modification should merge. Conflicts: {:?}",
            result.conflicts,
        );
        assert!(result.content.contains("console.log(\"fetching\""), "Should contain theirs modification");
        assert!(result.content.contains("deleteUser"), "Should have deleteUser");
        assert!(result.content.contains("createUser"), "Should have createUser");
    }

    #[test]
    fn test_doc_comment_plus_body_change() {
        // One side adds JSDoc comment, other modifies function body
        // Doc comments are part of the entity region — they should merge with body changes
        let base = r#"export function calculate(a: number, b: number): number {
    return a + b;
}
"#;
        let ours = r#"/**
 * Calculate the sum of two numbers.
 * @param a - First number
 * @param b - Second number
 */
export function calculate(a: number, b: number): number {
    return a + b;
}
"#;
        let theirs = r#"export function calculate(a: number, b: number): number {
    const result = a + b;
    console.log("result:", result);
    return result;
}
"#;
        let result = entity_merge(base, ours, theirs, "test.ts");
        eprintln!("Doc comment + body: clean={}, conflicts={:?}", result.is_clean(), result.conflicts);
        eprintln!("Content:\n{}", result.content);
        // This tests whether weave can merge doc comment additions with body changes
    }

    #[test]
    fn test_both_add_different_guard_clauses() {
        // Both add different guard clauses at the start of a function
        let base = r#"export function processOrder(order: Order): Result {
    const total = calculateTotal(order);
    return { success: true, total };
}
"#;
        let ours = r#"export function processOrder(order: Order): Result {
    if (!order) throw new Error("Order required");
    const total = calculateTotal(order);
    return { success: true, total };
}
"#;
        let theirs = r#"export function processOrder(order: Order): Result {
    if (order.items.length === 0) throw new Error("Empty order");
    const total = calculateTotal(order);
    return { success: true, total };
}
"#;
        let result = entity_merge(base, ours, theirs, "test.ts");
        eprintln!("Guard clauses: clean={}, conflicts={:?}", result.is_clean(), result.conflicts);
        eprintln!("Content:\n{}", result.content);
        // Both add at same position — diffy may struggle since they're at the same insertion point
    }

    #[test]
    fn test_both_modify_different_enum_variants() {
        // One modifies a variant's value, other adds new variants
        let base = r#"enum Status {
    Active = "active",
    Inactive = "inactive",
    Pending = "pending",
}
"#;
        let ours = r#"enum Status {
    Active = "active",
    Inactive = "disabled",
    Pending = "pending",
}
"#;
        let theirs = r#"enum Status {
    Active = "active",
    Inactive = "inactive",
    Pending = "pending",
    Deleted = "deleted",
}
"#;
        let result = entity_merge(base, ours, theirs, "test.ts");
        eprintln!("Enum modify+add: clean={}, conflicts={:?}", result.is_clean(), result.conflicts);
        eprintln!("Content:\n{}", result.content);
        assert!(
            result.is_clean(),
            "Modify variant + add new variant should merge. Conflicts: {:?}",
            result.conflicts,
        );
        assert!(result.content.contains("\"disabled\""), "Should have modified Inactive");
        assert!(result.content.contains("Deleted"), "Should have new Deleted variant");
    }

    #[test]
    fn test_config_object_field_additions() {
        // Both add different fields to a config object (exported const)
        let base = r#"export const config = {
    timeout: 5000,
    retries: 3,
};
"#;
        let ours = r#"export const config = {
    timeout: 5000,
    retries: 3,
    maxConnections: 10,
};
"#;
        let theirs = r#"export const config = {
    timeout: 5000,
    retries: 3,
    logLevel: "info",
};
"#;
        let result = entity_merge(base, ours, theirs, "test.ts");
        eprintln!("Config fields: clean={}, conflicts={:?}", result.is_clean(), result.conflicts);
        eprintln!("Content:\n{}", result.content);
        // This tests whether inner entity merge handles object literals
        // (it probably won't since object fields aren't extracted as members the same way)
    }

    #[test]
    fn test_rust_impl_block_both_add_methods() {
        // Both add different methods to a Rust impl block
        let base = r#"impl Calculator {
    fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}
"#;
        let ours = r#"impl Calculator {
    fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    fn multiply(&self, a: i32, b: i32) -> i32 {
        a * b
    }
}
"#;
        let theirs = r#"impl Calculator {
    fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    fn divide(&self, a: i32, b: i32) -> i32 {
        a / b
    }
}
"#;
        let result = entity_merge(base, ours, theirs, "test.rs");
        eprintln!("Rust impl: clean={}, conflicts={:?}", result.is_clean(), result.conflicts);
        eprintln!("Content:\n{}", result.content);
        assert!(
            result.is_clean(),
            "Both adding methods to Rust impl should merge. Conflicts: {:?}",
            result.conflicts,
        );
        assert!(result.content.contains("multiply"), "Should have multiply");
        assert!(result.content.contains("divide"), "Should have divide");
    }

    #[test]
    fn test_rust_doc_comment_plus_body_change() {
        // One side adds Rust doc comment, other modifies body
        // Comment bundling ensures the doc comment is part of the entity
        let base = r#"fn add(a: i32, b: i32) -> i32 {
    a + b
}

fn subtract(a: i32, b: i32) -> i32 {
    a - b
}
"#;
        let ours = r#"/// Adds two numbers together.
fn add(a: i32, b: i32) -> i32 {
    a + b
}

fn subtract(a: i32, b: i32) -> i32 {
    a - b
}
"#;
        let theirs = r#"fn add(a: i32, b: i32) -> i32 {
    a + b
}

fn subtract(a: i32, b: i32) -> i32 {
    a - b - 1
}
"#;
        let result = entity_merge(base, ours, theirs, "test.rs");
        assert!(
            result.is_clean(),
            "Rust doc comment + body change should merge. Conflicts: {:?}",
            result.conflicts,
        );
        assert!(result.content.contains("/// Adds two numbers"), "Should have ours doc comment");
        assert!(result.content.contains("a - b - 1"), "Should have theirs body change");
    }

    #[test]
    fn test_both_add_different_doc_comments() {
        // Both add doc comments to different functions — should merge cleanly
        let base = r#"fn add(a: i32, b: i32) -> i32 {
    a + b
}

fn subtract(a: i32, b: i32) -> i32 {
    a - b
}
"#;
        let ours = r#"/// Adds two numbers.
fn add(a: i32, b: i32) -> i32 {
    a + b
}

fn subtract(a: i32, b: i32) -> i32 {
    a - b
}
"#;
        let theirs = r#"fn add(a: i32, b: i32) -> i32 {
    a + b
}

/// Subtracts b from a.
fn subtract(a: i32, b: i32) -> i32 {
    a - b
}
"#;
        let result = entity_merge(base, ours, theirs, "test.rs");
        assert!(
            result.is_clean(),
            "Both adding doc comments to different functions should merge. Conflicts: {:?}",
            result.conflicts,
        );
        assert!(result.content.contains("/// Adds two numbers"), "Should have add's doc comment");
        assert!(result.content.contains("/// Subtracts b from a"), "Should have subtract's doc comment");
    }

    #[test]
    fn test_go_import_block_both_add_different() {
        // Go uses import (...) blocks — both add different imports
        let base = "package main\n\nimport (\n\t\"fmt\"\n\t\"os\"\n)\n\nfunc main() {\n\tfmt.Println(\"hello\")\n}\n";
        let ours = "package main\n\nimport (\n\t\"fmt\"\n\t\"os\"\n\t\"strings\"\n)\n\nfunc main() {\n\tfmt.Println(\"hello\")\n}\n";
        let theirs = "package main\n\nimport (\n\t\"fmt\"\n\t\"os\"\n\t\"io\"\n)\n\nfunc main() {\n\tfmt.Println(\"hello\")\n}\n";
        let result = entity_merge(base, ours, theirs, "main.go");
        eprintln!("Go import block: clean={}, conflicts={:?}", result.is_clean(), result.conflicts);
        eprintln!("Content:\n{}", result.content);
        // This tests whether Go import blocks (a single entity) get inner-merged
    }

    #[test]
    fn test_python_class_both_add_methods() {
        // Python class — both add different methods
        let base = "class Calculator:\n    def add(self, a, b):\n        return a + b\n";
        let ours = "class Calculator:\n    def add(self, a, b):\n        return a + b\n\n    def multiply(self, a, b):\n        return a * b\n";
        let theirs = "class Calculator:\n    def add(self, a, b):\n        return a + b\n\n    def divide(self, a, b):\n        return a / b\n";
        let result = entity_merge(base, ours, theirs, "test.py");
        eprintln!("Python class: clean={}, conflicts={:?}", result.is_clean(), result.conflicts);
        eprintln!("Content:\n{}", result.content);
        assert!(
            result.is_clean(),
            "Both adding methods to Python class should merge. Conflicts: {:?}",
            result.conflicts,
        );
        assert!(result.content.contains("multiply"), "Should have multiply");
        assert!(result.content.contains("divide"), "Should have divide");
    }
}
