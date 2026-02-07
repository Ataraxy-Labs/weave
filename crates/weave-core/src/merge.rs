use std::collections::{HashMap, HashSet};

use sem_core::model::change::ChangeType;
use sem_core::model::entity::SemanticEntity;
use sem_core::model::identity::match_entities;
use sem_core::parser::plugins::create_default_registry;
use sem_core::parser::registry::ParserRegistry;

use crate::conflict::{ConflictKind, EntityConflict, MergeStats};
use crate::region::{extract_regions, EntityRegion, FileRegion};
use crate::reconstruct::reconstruct;

/// Result of a merge operation.
#[derive(Debug)]
pub struct MergeResult {
    pub content: String,
    pub conflicts: Vec<EntityConflict>,
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
            stats: MergeStats::default(),
        };
    }

    // Fast path: if base == ours, take theirs entirely
    if base == ours {
        return MergeResult {
            content: theirs.to_string(),
            conflicts: vec![],
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

    // Extract entities from all three versions
    let base_entities = plugin.extract_entities(base, file_path);
    let ours_entities = plugin.extract_entities(ours, file_path);
    let theirs_entities = plugin.extract_entities(theirs, file_path);

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

    // Collect all entity IDs across all versions
    let mut all_entity_ids: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    // Start with ours ordering (skeleton)
    for entity in &ours_entities {
        if seen.insert(entity.id.clone()) {
            all_entity_ids.push(entity.id.clone());
        }
    }
    // Add theirs-only entities
    for entity in &theirs_entities {
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

    for entity_id in &all_entity_ids {
        let in_base = base_entity_map.get(entity_id.as_str());
        let in_ours = ours_entity_map.get(entity_id.as_str());
        let in_theirs = theirs_entity_map.get(entity_id.as_str());

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

        resolved_entities.insert(entity_id.clone(), resolution);
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

    MergeResult {
        content,
        conflicts,
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
            let ours_modified = ours.content_hash != base.content_hash;
            let theirs_modified = theirs.content_hash != base.content_hash;

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
                        match diffy_merge(&base_rc, &ours_rc, &theirs_rc) {
                            Some(merged) => {
                                stats.entities_both_changed_merged += 1;
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
                                stats.entities_conflicted += 1;
                                ResolvedEntity::Conflict(EntityConflict {
                                    entity_name: ours.name.clone(),
                                    entity_type: ours.entity_type.clone(),
                                    kind: ConflictKind::BothModified,
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
                ResolvedEntity::Conflict(EntityConflict {
                    entity_name: ours.name.clone(),
                    entity_type: ours.entity_type.clone(),
                    kind: ConflictKind::ModifyDelete {
                        modified_in_ours: true,
                    },
                    ours_content: Some(region_content(ours, ours_region_content)),
                    theirs_content: None,
                    base_content: Some(region_content(_base, base_region_content)),
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
                ResolvedEntity::Conflict(EntityConflict {
                    entity_name: theirs.name.clone(),
                    entity_type: theirs.entity_type.clone(),
                    kind: ConflictKind::ModifyDelete {
                        modified_in_ours: false,
                    },
                    ours_content: None,
                    theirs_content: Some(region_content(theirs, theirs_region_content)),
                    base_content: Some(region_content(_base, base_region_content)),
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
                ResolvedEntity::Conflict(EntityConflict {
                    entity_name: ours.name.clone(),
                    entity_type: ours.entity_type.clone(),
                    kind: ConflictKind::BothAdded,
                    ours_content: Some(region_content(ours, ours_region_content)),
                    theirs_content: Some(region_content(theirs, theirs_region_content)),
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
fn line_level_fallback(base: &str, ours: &str, theirs: &str) -> MergeResult {
    let mut stats = MergeStats::default();
    stats.used_fallback = true;

    match diffy::merge(base, ours, theirs) {
        Ok(merged) => MergeResult {
            content: merged,
            conflicts: vec![],
            stats,
        },
        Err(conflicted) => {
            stats.entities_conflicted = 1;
            MergeResult {
                content: conflicted,
                conflicts: vec![EntityConflict {
                    entity_name: "(file)".to_string(),
                    entity_type: "file".to_string(),
                    kind: ConflictKind::BothModified,
                    ours_content: Some(ours.to_string()),
                    theirs_content: Some(theirs.to_string()),
                    base_content: Some(base.to_string()),
                }],
                stats,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
