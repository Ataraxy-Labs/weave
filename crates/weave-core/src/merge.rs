use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::process::Command;
use std::sync::{mpsc, LazyLock};
use std::time::Duration;

use sem_core::model::change::ChangeType;
use sem_core::model::entity::SemanticEntity;
use sem_core::model::identity::match_entities;
use sem_core::parser::plugins::create_default_registry;
use sem_core::parser::registry::ParserRegistry;
use serde::Serialize;

/// Static parser registry shared across all merge operations.
/// Avoids recreating 11 tree-sitter language parsers per merge call.
static PARSER_REGISTRY: LazyLock<ParserRegistry> = LazyLock::new(create_default_registry);

use crate::conflict::{
    classify_conflict, ConflictComplexity, ConflictKind, EntityConflict, MarkerFormat, MergeStats,
};
use crate::reconstruct::reconstruct;
use crate::region::{extract_regions, EntityRegion, FileRegion};
use crate::validate::SemanticWarning;

/// How an individual entity was resolved during merge.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionStrategy {
    Unchanged,
    OursOnly,
    TheirsOnly,
    ContentEqual,
    DiffyMerged,
    DecoratorMerged,
    InnerMerged,
    ConflictBothModified,
    ConflictModifyDelete,
    ConflictBothAdded,
    ConflictRenameRename,
    ConflictRenameModify,
    AddedOurs,
    AddedTheirs,
    Deleted,
    Renamed { from: String, to: String },
    Fallback,
}

/// Audit record for a single entity's merge resolution.
#[derive(Debug, Clone, Serialize)]
pub struct EntityAudit {
    pub name: String,
    #[serde(rename = "type")]
    pub entity_type: String,
    pub resolution: ResolutionStrategy,
}

/// Result of a merge operation.
#[derive(Debug)]
pub struct MergeResult {
    pub content: String,
    pub conflicts: Vec<EntityConflict>,
    pub warnings: Vec<SemanticWarning>,
    pub stats: MergeStats,
    pub audit: Vec<EntityAudit>,
}

impl MergeResult {
    pub fn is_clean(&self) -> bool {
        self.conflicts.is_empty() && !self.content.lines().any(|l| l.starts_with("<<<<<<< ours"))
    }
}

/// The resolved content for a single entity after merging.
#[derive(Debug, Clone)]
pub enum ResolvedEntity {
    /// Clean resolution — use this content.
    Clean(EntityRegion),
    /// Conflict — render conflict markers.
    Conflict(EntityConflict),
    /// Inner merge with per-member scoped conflicts.
    /// Content already contains per-member conflict markers; emit as-is.
    ScopedConflict {
        content: String,
        conflict: EntityConflict,
    },
    /// Entity was deleted.
    Deleted,
}

/// Perform entity-level 3-way merge.
///
/// Falls back to line-level merge (via diffy) when:
/// - No parser matches the file type
/// - Parser returns 0 entities for non-empty content
/// - File exceeds 1MB
pub fn entity_merge(base: &str, ours: &str, theirs: &str, file_path: &str) -> MergeResult {
    entity_merge_fmt(base, ours, theirs, file_path, &MarkerFormat::default())
}

/// Perform entity-level 3-way merge with configurable marker format.
pub fn entity_merge_fmt(
    base: &str,
    ours: &str,
    theirs: &str,
    file_path: &str,
    marker_format: &MarkerFormat,
) -> MergeResult {
    let timeout_secs = std::env::var("WEAVE_TIMEOUT")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(5);

    // Timeout: if entity merge takes too long, diffy is likely hitting
    // pathological input. Fall back to git merge-file which always terminates.
    let base_owned = base.to_string();
    let ours_owned = ours.to_string();
    let theirs_owned = theirs.to_string();
    let path_owned = file_path.to_string();
    let fmt_owned = marker_format.clone();

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = entity_merge_with_registry(
            &base_owned,
            &ours_owned,
            &theirs_owned,
            &path_owned,
            &PARSER_REGISTRY,
            &fmt_owned,
        );
        let _ = tx.send(result);
    });

    match rx.recv_timeout(Duration::from_secs(timeout_secs)) {
        Ok(result) => result,
        Err(_) => {
            eprintln!(
                "weave: merge timed out after {}s for {}, falling back to git merge-file",
                timeout_secs, file_path
            );
            let mut stats = MergeStats::default();
            stats.used_fallback = true;
            git_merge_file(base, ours, theirs, &mut stats)
        }
    }
}

pub fn entity_merge_with_registry(
    base: &str,
    ours: &str,
    theirs: &str,
    file_path: &str,
    registry: &ParserRegistry,
    marker_format: &MarkerFormat,
) -> MergeResult {
    // Guard: if any input already contains conflict markers (e.g. AU/AA conflicts
    // where git bakes markers into stage blobs), report as conflict immediately.
    // We can't do a meaningful 3-way merge on pre-conflicted content.
    if has_conflict_markers(base) || has_conflict_markers(ours) || has_conflict_markers(theirs) {
        let mut stats = MergeStats::default();
        stats.entities_conflicted = 1;
        stats.used_fallback = true;
        // Use whichever input has markers as the merged content (preserves
        // the conflict for the user to resolve manually).
        let content = if has_conflict_markers(ours) {
            ours
        } else if has_conflict_markers(theirs) {
            theirs
        } else {
            base
        };
        let complexity = classify_conflict(Some(base), Some(ours), Some(theirs));
        return MergeResult {
            content: content.to_string(),
            conflicts: vec![EntityConflict {
                entity_name: "(file)".to_string(),
                entity_type: "file".to_string(),
                kind: ConflictKind::BothModified,
                complexity,
                ours_content: Some(ours.to_string()),
                theirs_content: Some(theirs.to_string()),
                base_content: Some(base.to_string()),
            }],
            warnings: vec![],
            stats,
            audit: vec![],
        };
    }

    // Fast path: if ours == theirs, no merge needed
    if ours == theirs {
        return MergeResult {
            content: ours.to_string(),
            conflicts: vec![],
            warnings: vec![],
            stats: MergeStats::default(),
            audit: vec![],
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
            audit: vec![],
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
            audit: vec![],
        };
    }

    // Binary file detection: if any version has null bytes, use git merge-file directly
    if is_binary(base) || is_binary(ours) || is_binary(theirs) {
        let mut stats = MergeStats::default();
        stats.used_fallback = true;
        return git_merge_file(base, ours, theirs, &mut stats);
    }

    // Large file fallback
    if base.len() > 1_000_000 || ours.len() > 1_000_000 || theirs.len() > 1_000_000 {
        return line_level_fallback(base, ours, theirs, file_path);
    }

    // If the file type isn't natively supported, the registry returns the fallback
    // plugin (20-line chunks). Entity merge on arbitrary chunks produces WORSE
    // results than line-level merge (confirmed on GitButler's .svelte files where
    // chunk boundaries don't align with structural boundaries). So we skip entity
    // merge entirely for fallback-plugin files and go straight to line-level merge.
    let plugin = match registry.get_plugin(file_path) {
        Some(p) if p.id() != "fallback" => p,
        _ => return line_level_fallback(base, ours, theirs, file_path),
    };

    // Extract entities from all three versions. Keep unfiltered lists for inner merge
    // (child entities provide tree-sitter-based method decomposition for classes).
    let base_all = plugin.extract_entities(base, file_path);
    let ours_all = plugin.extract_entities(ours, file_path);
    let theirs_all = plugin.extract_entities(theirs, file_path);

    // Filter out nested entities for top-level matching and region extraction
    let base_entities = filter_nested_entities(base_all.clone());
    let ours_entities = filter_nested_entities(ours_all.clone());
    let theirs_entities = filter_nested_entities(theirs_all.clone());

    // Fallback if parser returns nothing for non-empty content
    if base_entities.is_empty() && !base.trim().is_empty() {
        return line_level_fallback(base, ours, theirs, file_path);
    }
    // When base is empty (file newly added in both branches), entity-level
    // reconstruction can produce invalid output for structured formats like JSON
    // (e.g. content appended after closing delimiter). Fall back to line-level
    // merge which handles this case correctly.
    if base.trim().is_empty() && !ours.trim().is_empty() && !theirs.trim().is_empty() {
        return line_level_fallback(base, ours, theirs, file_path);
    }
    // Allow empty entities if content is actually empty
    if ours_entities.is_empty()
        && !ours.trim().is_empty()
        && theirs_entities.is_empty()
        && !theirs.trim().is_empty()
    {
        return line_level_fallback(base, ours, theirs, file_path);
    }

    // Fallback if too many duplicate entity names. Entity matching is O(n*m) on
    // same-named entities which can hang on files with many `var app = ...` etc.
    if has_excessive_duplicates(&base_entities)
        || has_excessive_duplicates(&ours_entities)
        || has_excessive_duplicates(&theirs_entities)
    {
        return line_level_fallback(base, ours, theirs, file_path);
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
    let theirs_changes = match_entities(
        &base_entities,
        &theirs_entities,
        file_path,
        None,
        None,
        None,
    );

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
    let mut audit: Vec<EntityAudit> = Vec::new();
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
            let theirs_name = theirs_entity
                .map(|e| e.name.as_str())
                .unwrap_or(theirs_new_id);

            let base_rc = base_entity.map(|e| {
                base_region_content
                    .get(e.id.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| e.content.clone())
            });
            let ours_rc = ours_entity.map(|e| {
                ours_region_content
                    .get(e.id.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| e.content.clone())
            });
            let theirs_rc = theirs_entity.map(|e| {
                theirs_region_content
                    .get(e.id.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| e.content.clone())
            });

            stats.entities_conflicted += 1;
            let conflict = EntityConflict {
                entity_name: base_name.to_string(),
                entity_type: base_entity
                    .map(|e| e.entity_type.clone())
                    .unwrap_or_default(),
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
            audit.push(EntityAudit {
                name: base_name.to_string(),
                entity_type: base_entity
                    .map(|e| e.entity_type.clone())
                    .unwrap_or_default(),
                resolution: ResolutionStrategy::ConflictRenameRename,
            });
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
        let ours_id = base_to_ours_rename
            .get(entity_id.as_str())
            .map(|s| s.as_str())
            .unwrap_or(entity_id.as_str());
        let theirs_id = base_to_theirs_rename
            .get(entity_id.as_str())
            .map(|s| s.as_str())
            .unwrap_or(entity_id.as_str());
        let in_ours = ours_entity_map
            .get(ours_id)
            .or_else(|| ours_entity_map.get(entity_id.as_str()));
        let in_theirs = theirs_entity_map
            .get(theirs_id)
            .or_else(|| theirs_entity_map.get(entity_id.as_str()));

        let ours_change = ours_change_map.get(entity_id);
        let theirs_change = theirs_change_map.get(entity_id);

        let (resolution, strategy) = resolve_entity(
            entity_id,
            in_base,
            in_ours,
            in_theirs,
            ours_change,
            theirs_change,
            &base_region_content,
            &ours_region_content,
            &theirs_region_content,
            &base_all,
            &ours_all,
            &theirs_all,
            &mut stats,
            marker_format,
        );

        // Build audit entry from entity info
        let entity_name = in_ours
            .map(|e| e.name.as_str())
            .or_else(|| in_theirs.map(|e| e.name.as_str()))
            .or_else(|| in_base.map(|e| e.name.as_str()))
            .unwrap_or(entity_id)
            .to_string();
        let entity_type = in_ours
            .map(|e| e.entity_type.as_str())
            .or_else(|| in_theirs.map(|e| e.entity_type.as_str()))
            .or_else(|| in_base.map(|e| e.entity_type.as_str()))
            .unwrap_or("")
            .to_string();
        audit.push(EntityAudit {
            name: entity_name,
            entity_type,
            resolution: strategy,
        });

        match &resolution {
            ResolvedEntity::Conflict(ref c) => conflicts.push(c.clone()),
            ResolvedEntity::ScopedConflict { conflict, .. } => conflicts.push(conflict.clone()),
            _ => {}
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
    let (merged_interstitials, interstitial_conflicts) =
        merge_interstitials(&base_regions, &ours_regions, &theirs_regions, marker_format);
    stats.entities_conflicted += interstitial_conflicts.len();
    conflicts.extend(interstitial_conflicts);

    // Collect base IDs that were renamed in ours — these should not be treated
    // as "theirs-only additions" during reconstruction.
    let theirs_rename_base_ids: HashSet<String> = base_to_ours_rename.keys().cloned().collect();

    // Reconstruct the file
    let content = reconstruct(
        &ours_regions,
        &theirs_regions,
        &theirs_entities,
        &ours_entity_map,
        &resolved_entities,
        &merged_interstitials,
        marker_format,
        &theirs_rename_base_ids,
    );

    // Post-merge cleanup: remove duplicate lines and normalize blank lines
    let content = post_merge_cleanup(&content);

    // Post-merge validation: verify the merged result is structurally sound.
    // Catches silent data loss from entity merge / reconstruction bugs.
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

        // Entity coverage check: every resolved-clean entity's content should
        // appear in the merged output. If it doesn't, reconstruct dropped it.
        if conflicts.is_empty() {
            for resolved in resolved_entities.values() {
                if let ResolvedEntity::Clean(region) = resolved {
                    let trimmed = region.content.trim();
                    if !trimmed.is_empty() && trimmed.len() > 20 && !content.contains(trimmed) {
                        // Entity resolved cleanly but its content is missing from output.
                        // Fall back to git merge-file to avoid silent data loss.
                        return git_merge_file(base, ours, theirs, &mut stats);
                    }
                }
            }
        }

        // Entity count check: re-parsed merged output should have at least as many
        // entities as the minimum of ours/theirs (minus deletions). A significant
        // drop means entities were silently lost.
        if conflicts.is_empty() && !merged_entities.is_empty() {
            let merged_top = filter_nested_entities(merged_entities);
            let deleted_count = resolved_entities
                .values()
                .filter(|r| matches!(r, ResolvedEntity::Deleted))
                .count();
            let expected_min = ours_entities
                .len()
                .min(theirs_entities.len())
                .saturating_sub(deleted_count);
            if expected_min > 3 && merged_top.len() < expected_min * 80 / 100 {
                return git_merge_file(base, ours, theirs, &mut stats);
            }
        }
    }

    // Line-level drop detection: lines present in all 3 inputs (unchanged by
    // either branch) must appear in the output. Dropping them is always wrong.
    // This runs on ALL clean merges, not just those with both-changed entities.
    if conflicts.is_empty() {
        let base_lines: HashSet<&str> = base
            .lines()
            .map(|l| l.trim())
            .filter(|l| l.len() > 15 && !is_import_line_trimmed(l))
            .collect();
        let ours_lines: HashSet<&str> = ours
            .lines()
            .map(|l| l.trim())
            .filter(|l| l.len() > 15 && !is_import_line_trimmed(l))
            .collect();
        let theirs_lines: HashSet<&str> = theirs
            .lines()
            .map(|l| l.trim())
            .filter(|l| l.len() > 15 && !is_import_line_trimmed(l))
            .collect();
        let output_lines: HashSet<&str> = content.lines().map(|l| l.trim()).collect();

        // Unchanged lines (in all 3) must be in output
        let missing_unchanged = base_lines
            .iter()
            .filter(|l| ours_lines.contains(*l) && theirs_lines.contains(*l))
            .filter(|l| !output_lines.contains(*l))
            .count();

        if missing_unchanged > 0 {
            return git_merge_file(base, ours, theirs, &mut stats);
        }

        // Lines added by either branch (not in base) shouldn't be dropped either.
        // Split into two tiers:
        // - "Significant" lines (> 40 chars): even 1 missing is likely a bug, since
        //   long lines are unique enough that reformatting won't change them completely
        // - Shorter lines (16-40 chars): use threshold > 3, since these could be
        //   structural lines reformatted by the import merger or brace placement
        let mut missing_added_significant = 0;
        let mut missing_added_short = 0;
        for l in ours_lines.iter().chain(theirs_lines.iter()) {
            if base_lines.contains(l) || output_lines.contains(l) {
                continue;
            }
            // Also check if the core content appears as a substring in any output line
            // (handles reformatting like `Foo {` becoming `Foo\n{`)
            if l.len() > 25 && content.contains(*l) {
                continue;
            }
            if l.len() > 40 {
                missing_added_significant += 1;
            } else {
                missing_added_short += 1;
            }
        }

        if missing_added_significant > 0 || missing_added_short > 3 {
            return git_merge_file(base, ours, theirs, &mut stats);
        }
    }

    let entity_result = MergeResult {
        content,
        conflicts,
        warnings,
        stats: stats.clone(),
        audit,
    };

    // Floor: never produce more conflict markers than git merge-file.
    // Entity merge can split one git conflict into multiple per-entity conflicts,
    // or interstitial merges can produce conflicts not tracked in the conflicts vec.
    let entity_markers = entity_result
        .content
        .lines()
        .filter(|l| l.starts_with("<<<<<<<"))
        .count();
    if entity_markers > 0 {
        let git_result = git_merge_file(base, ours, theirs, &mut stats);
        let git_markers = git_result
            .content
            .lines()
            .filter(|l| l.starts_with("<<<<<<<"))
            .count();
        if entity_markers > git_markers {
            return git_result;
        }
    }

    // Safety net: detect silent data loss from entity merge.
    // If the merged result is significantly shorter than expected, fall back to git.
    if entity_markers == 0 {
        let merged_len = entity_result.content.len();
        let max_input_len = ours.len().max(theirs.len());
        let min_input_len = ours.len().min(theirs.len());
        // Expected length: at least 90% of the shorter input (both branches
        // contribute content, so the merge should be at least as long as the
        // shorter one minus some deletions).
        if min_input_len > 200 && merged_len < min_input_len * 90 / 100 {
            return git_merge_file(base, ours, theirs, &mut stats);
        }
        // Also check: merged shouldn't be much shorter than max input unless
        // there were intentional deletions from one branch
        if max_input_len > 500 && merged_len < max_input_len * 70 / 100 {
            // Check if the length reduction is explained by one branch deleting content
            let base_len = base.len();
            let ours_deleted =
                base_len > ours.len() && (base_len - ours.len()) > max_input_len * 20 / 100;
            let theirs_deleted =
                base_len > theirs.len() && (base_len - theirs.len()) > max_input_len * 20 / 100;
            if !ours_deleted && !theirs_deleted {
                return git_merge_file(base, ours, theirs, &mut stats);
            }
        }
    }

    entity_result
}

fn resolve_entity(
    _entity_id: &str,
    in_base: Option<&&SemanticEntity>,
    in_ours: Option<&&SemanticEntity>,
    in_theirs: Option<&&SemanticEntity>,
    _ours_change: Option<&ChangeType>,
    _theirs_change: Option<&ChangeType>,
    base_region_content: &HashMap<&str, &str>,
    ours_region_content: &HashMap<&str, &str>,
    theirs_region_content: &HashMap<&str, &str>,
    base_all: &[SemanticEntity],
    ours_all: &[SemanticEntity],
    theirs_all: &[SemanticEntity],
    stats: &mut MergeStats,
    marker_format: &MarkerFormat,
) -> (ResolvedEntity, ResolutionStrategy) {
    // Helper: get region content (from file lines) for an entity, falling back to entity.content
    let region_content = |entity: &SemanticEntity, map: &HashMap<&str, &str>| -> String {
        map.get(entity.id.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| entity.content.clone())
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

            let ours_modified =
                ours.content_hash != base.content_hash || ours_rc_lazy() != base_rc_lazy();
            let theirs_modified =
                theirs.content_hash != base.content_hash || theirs_rc_lazy() != base_rc_lazy();

            match (ours_modified, theirs_modified) {
                (false, false) => {
                    // Neither changed
                    stats.entities_unchanged += 1;
                    (
                        ResolvedEntity::Clean(entity_to_region_with_content(
                            ours,
                            &region_content(ours, ours_region_content),
                        )),
                        ResolutionStrategy::Unchanged,
                    )
                }
                (true, false) => {
                    // Only ours changed
                    stats.entities_ours_only += 1;
                    (
                        ResolvedEntity::Clean(entity_to_region_with_content(
                            ours,
                            &region_content(ours, ours_region_content),
                        )),
                        ResolutionStrategy::OursOnly,
                    )
                }
                (false, true) => {
                    // Only theirs changed
                    stats.entities_theirs_only += 1;
                    (
                        ResolvedEntity::Clean(entity_to_region_with_content(
                            theirs,
                            &region_content(theirs, theirs_region_content),
                        )),
                        ResolutionStrategy::TheirsOnly,
                    )
                }
                (true, true) => {
                    // Both changed — try intra-entity merge
                    if ours.content_hash == theirs.content_hash {
                        // Same change in both — take ours
                        stats.entities_both_changed_merged += 1;
                        (
                            ResolvedEntity::Clean(entity_to_region_with_content(
                                ours,
                                &region_content(ours, ours_region_content),
                            )),
                            ResolutionStrategy::ContentEqual,
                        )
                    } else {
                        // Try diffy 3-way merge on region content (preserves full syntax)
                        let base_rc = region_content(base, base_region_content);
                        let ours_rc = region_content(ours, ours_region_content);
                        let theirs_rc = region_content(theirs, theirs_region_content);

                        // Rename-aware merge: when one side renamed an entity and the
                        // other modified it, normalize names before 3-way merge so diffy
                        // only sees the structural changes, not name conflicts.
                        let ours_renamed = base.name != ours.name;
                        let theirs_renamed = base.name != theirs.name;
                        let (merge_base_rc, merge_ours_rc, merge_theirs_rc, _final_name) =
                            if ours_renamed && !theirs_renamed {
                                // Ours renamed: normalize base+theirs to use ours' name
                                let nb =
                                    replace_at_word_boundaries(&base_rc, &base.name, &ours.name);
                                let nt = replace_at_word_boundaries(
                                    &theirs_rc,
                                    &theirs.name,
                                    &ours.name,
                                );
                                (nb, ours_rc.clone(), nt, Some(&ours.name))
                            } else if theirs_renamed && !ours_renamed {
                                // Theirs renamed: normalize base+ours to use theirs' name
                                let nb =
                                    replace_at_word_boundaries(&base_rc, &base.name, &theirs.name);
                                let no =
                                    replace_at_word_boundaries(&ours_rc, &ours.name, &theirs.name);
                                (nb, no, theirs_rc.clone(), Some(&theirs.name))
                            } else {
                                (base_rc.clone(), ours_rc.clone(), theirs_rc.clone(), None)
                            };

                        // Rename + modify: if one side renamed and the other modified
                        // the body, conflict. The modifying developer didn't know about
                        // the rename, so the merge needs human/agent review.
                        if (ours_renamed && !theirs_renamed) || (!ours_renamed && theirs_renamed) {
                            let renamed_in_ours = ours_renamed;
                            let (old_name, new_name) = if renamed_in_ours {
                                (base.name.clone(), ours.name.clone())
                            } else {
                                (base.name.clone(), theirs.name.clone())
                            };
                            stats.entities_conflicted += 1;
                            let conflict = EntityConflict {
                                entity_name: base.name.clone(),
                                entity_type: base.entity_type.clone(),
                                kind: ConflictKind::RenameModify {
                                    old_name,
                                    new_name,
                                    renamed_in_ours,
                                },
                                complexity: ConflictComplexity::SyntaxFunctional,
                                ours_content: Some(ours_rc),
                                theirs_content: Some(theirs_rc),
                                base_content: Some(base_rc),
                            };
                            return (
                                ResolvedEntity::Conflict(conflict),
                                ResolutionStrategy::ConflictRenameModify,
                            );
                        }

                        // Whitespace-aware shortcut: if one side only changed
                        // whitespace/formatting, take the other side's content changes.
                        // This handles the common case where one agent reformats while
                        // another makes semantic changes.
                        if is_whitespace_only_diff(&base_rc, &ours_rc) {
                            stats.entities_theirs_only += 1;
                            return (
                                ResolvedEntity::Clean(entity_to_region_with_content(
                                    theirs, &theirs_rc,
                                )),
                                ResolutionStrategy::TheirsOnly,
                            );
                        }
                        if is_whitespace_only_diff(&base_rc, &theirs_rc) {
                            stats.entities_ours_only += 1;
                            return (
                                ResolvedEntity::Clean(entity_to_region_with_content(
                                    ours, &ours_rc,
                                )),
                                ResolutionStrategy::OursOnly,
                            );
                        }

                        // Pick the renamed entity for output (prefer the side that did the rename)
                        let output_entity = if ours_renamed {
                            ours
                        } else if theirs_renamed {
                            theirs
                        } else {
                            ours
                        };

                        match diffy_merge(&merge_base_rc, &merge_ours_rc, &merge_theirs_rc) {
                            Some(merged) => {
                                stats.entities_both_changed_merged += 1;
                                stats.resolved_via_diffy += 1;
                                (
                                    ResolvedEntity::Clean(EntityRegion {
                                        entity_id: output_entity.id.clone(),
                                        entity_name: output_entity.name.clone(),
                                        entity_type: output_entity.entity_type.clone(),
                                        content: merged,
                                        start_line: output_entity.start_line,
                                        end_line: output_entity.end_line,
                                    }),
                                    ResolutionStrategy::DiffyMerged,
                                )
                            }
                            None => {
                                // Strategy 1: decorator/annotation-aware merge
                                // Decorators are unordered annotations — merge them commutatively
                                if let Some(merged) =
                                    try_decorator_aware_merge(&base_rc, &ours_rc, &theirs_rc)
                                {
                                    stats.entities_both_changed_merged += 1;
                                    stats.resolved_via_diffy += 1;
                                    return (
                                        ResolvedEntity::Clean(EntityRegion {
                                            entity_id: ours.id.clone(),
                                            entity_name: ours.name.clone(),
                                            entity_type: ours.entity_type.clone(),
                                            content: merged,
                                            start_line: ours.start_line,
                                            end_line: ours.end_line,
                                        }),
                                        ResolutionStrategy::DecoratorMerged,
                                    );
                                }

                                // Strategy 2: inner entity merge for container types
                                // (LastMerge insight: class members are unordered children)
                                if is_container_entity_type(&ours.entity_type) {
                                    let base_children = in_base
                                        .map(|b| get_child_entities(b, base_all))
                                        .unwrap_or_default();
                                    let ours_children = get_child_entities(ours, ours_all);
                                    let theirs_children = in_theirs
                                        .map(|t| get_child_entities(t, theirs_all))
                                        .unwrap_or_default();
                                    let base_start = in_base.map(|b| b.start_line).unwrap_or(1);
                                    let ours_start = ours.start_line;
                                    let theirs_start = in_theirs.map(|t| t.start_line).unwrap_or(1);
                                    if let Some(inner) = try_inner_entity_merge(
                                        &base_rc,
                                        &ours_rc,
                                        &theirs_rc,
                                        &base_children,
                                        &ours_children,
                                        &theirs_children,
                                        base_start,
                                        ours_start,
                                        theirs_start,
                                        marker_format,
                                    ) {
                                        if inner.has_conflicts {
                                            // Inner merge produced per-member conflicts:
                                            // content has scoped markers for just the conflicted
                                            // members; clean members are merged normally.
                                            stats.entities_conflicted += 1;
                                            stats.resolved_via_inner_merge += 1;
                                            let complexity = classify_conflict(
                                                Some(&base_rc),
                                                Some(&ours_rc),
                                                Some(&theirs_rc),
                                            );
                                            return (
                                                ResolvedEntity::ScopedConflict {
                                                    content: inner.content,
                                                    conflict: EntityConflict {
                                                        entity_name: ours.name.clone(),
                                                        entity_type: ours.entity_type.clone(),
                                                        kind: ConflictKind::BothModified,
                                                        complexity,
                                                        ours_content: Some(ours_rc),
                                                        theirs_content: Some(theirs_rc),
                                                        base_content: Some(base_rc),
                                                    },
                                                },
                                                ResolutionStrategy::InnerMerged,
                                            );
                                        } else {
                                            stats.entities_both_changed_merged += 1;
                                            stats.resolved_via_inner_merge += 1;
                                            return (
                                                ResolvedEntity::Clean(EntityRegion {
                                                    entity_id: ours.id.clone(),
                                                    entity_name: ours.name.clone(),
                                                    entity_type: ours.entity_type.clone(),
                                                    content: inner.content,
                                                    start_line: ours.start_line,
                                                    end_line: ours.end_line,
                                                }),
                                                ResolutionStrategy::InnerMerged,
                                            );
                                        }
                                    }
                                }
                                stats.entities_conflicted += 1;
                                let complexity = classify_conflict(
                                    Some(&base_rc),
                                    Some(&ours_rc),
                                    Some(&theirs_rc),
                                );
                                (
                                    ResolvedEntity::Conflict(EntityConflict {
                                        entity_name: ours.name.clone(),
                                        entity_type: ours.entity_type.clone(),
                                        kind: ConflictKind::BothModified,
                                        complexity,
                                        ours_content: Some(ours_rc),
                                        theirs_content: Some(theirs_rc),
                                        base_content: Some(base_rc),
                                    }),
                                    ResolutionStrategy::ConflictBothModified,
                                )
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
                (
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
                    }),
                    ResolutionStrategy::ConflictModifyDelete,
                )
            } else {
                // Theirs deleted, ours unchanged → accept deletion
                stats.entities_deleted += 1;
                (ResolvedEntity::Deleted, ResolutionStrategy::Deleted)
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
                (
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
                    }),
                    ResolutionStrategy::ConflictModifyDelete,
                )
            } else {
                // Ours deleted, theirs unchanged → accept deletion
                stats.entities_deleted += 1;
                (ResolvedEntity::Deleted, ResolutionStrategy::Deleted)
            }
        }

        // Entity only in ours (added by ours)
        (None, Some(ours), None) => {
            stats.entities_added_ours += 1;
            (
                ResolvedEntity::Clean(entity_to_region_with_content(
                    ours,
                    &region_content(ours, ours_region_content),
                )),
                ResolutionStrategy::AddedOurs,
            )
        }

        // Entity only in theirs (added by theirs)
        (None, None, Some(theirs)) => {
            stats.entities_added_theirs += 1;
            (
                ResolvedEntity::Clean(entity_to_region_with_content(
                    theirs,
                    &region_content(theirs, theirs_region_content),
                )),
                ResolutionStrategy::AddedTheirs,
            )
        }

        // Entity in both ours and theirs but not base (both added)
        (None, Some(ours), Some(theirs)) => {
            if ours.content_hash == theirs.content_hash {
                // Same content added by both → take ours
                stats.entities_added_ours += 1;
                (
                    ResolvedEntity::Clean(entity_to_region_with_content(
                        ours,
                        &region_content(ours, ours_region_content),
                    )),
                    ResolutionStrategy::ContentEqual,
                )
            } else {
                // Different content → conflict
                stats.entities_conflicted += 1;
                let ours_rc = region_content(ours, ours_region_content);
                let theirs_rc = region_content(theirs, theirs_region_content);
                let complexity = classify_conflict(None, Some(&ours_rc), Some(&theirs_rc));
                (
                    ResolvedEntity::Conflict(EntityConflict {
                        entity_name: ours.name.clone(),
                        entity_type: ours.entity_type.clone(),
                        kind: ConflictKind::BothAdded,
                        complexity,
                        ours_content: Some(ours_rc),
                        theirs_content: Some(theirs_rc),
                        base_content: None,
                    }),
                    ResolutionStrategy::ConflictBothAdded,
                )
            }
        }

        // Entity only in base (deleted by both)
        (Some(_), None, None) => {
            stats.entities_deleted += 1;
            (ResolvedEntity::Deleted, ResolutionStrategy::Deleted)
        }

        // Should not happen
        (None, None, None) => (ResolvedEntity::Deleted, ResolutionStrategy::Deleted),
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
/// Returns borrowed references since regions live for the merge duration.
fn build_region_content_map(regions: &[FileRegion]) -> HashMap<&str, &str> {
    regions
        .iter()
        .filter_map(|r| match r {
            FileRegion::Entity(e) => Some((e.entity_id.as_str(), e.content.as_str())),
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
    let a_normalized: Vec<&str> = a
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    let b_normalized: Vec<&str> = b
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
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
    while decorator_end > 0
        && lines
            .get(decorator_end - 1)
            .is_some_and(|l| l.trim().is_empty())
    {
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
    result.ok()
}

/// Try 3-way merge using git merge-file. Returns None on conflict or error.
/// This uses a different diff algorithm than diffy and can sometimes merge
/// cases that diffy cannot (and vice versa).
fn git_merge_string(base: &str, ours: &str, theirs: &str) -> Option<String> {
    let dir = tempfile::tempdir().ok()?;
    let base_path = dir.path().join("base");
    let ours_path = dir.path().join("ours");
    let theirs_path = dir.path().join("theirs");

    std::fs::write(&base_path, base).ok()?;
    std::fs::write(&ours_path, ours).ok()?;
    std::fs::write(&theirs_path, theirs).ok()?;

    let output = Command::new("git")
        .arg("merge-file")
        .arg("-p")
        .arg(&ours_path)
        .arg(&base_path)
        .arg(&theirs_path)
        .output()
        .ok()?;

    if output.status.success() {
        String::from_utf8(output.stdout).ok()
    } else {
        None
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
    marker_format: &MarkerFormat,
) -> (HashMap<String, String>, Vec<EntityConflict>) {
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
    let mut interstitial_conflicts: Vec<EntityConflict> = Vec::new();

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
            // Both changed — check whitespace-only first
            if is_whitespace_only_diff(base_content, ours_content)
                && is_whitespace_only_diff(base_content, theirs_content)
            {
                // Both sides only changed whitespace, take theirs (arbitrary)
                merged.insert(key.to_string(), theirs_content.to_string());
            } else if is_whitespace_only_diff(base_content, ours_content) {
                // Ours is whitespace-only, theirs has real changes
                merged.insert(key.to_string(), theirs_content.to_string());
            } else if is_whitespace_only_diff(base_content, theirs_content) {
                // Theirs is whitespace-only, ours has real changes
                merged.insert(key.to_string(), ours_content.to_string());
            } else if is_import_region(base_content)
                || is_import_region(ours_content)
                || is_import_region(theirs_content)
            {
                // Commutative merge: treat import lines as a set
                let result =
                    merge_imports_commutatively(base_content, ours_content, theirs_content);
                merged.insert(key.to_string(), result);
            } else {
                // Regular line-level merge
                match diffy::merge(base_content, ours_content, theirs_content) {
                    Ok(m) => {
                        merged.insert(key.to_string(), m);
                    }
                    Err(_conflicted) => {
                        // Create a proper conflict instead of silently embedding
                        // raw conflict markers into the output.
                        let complexity = classify_conflict(
                            Some(base_content),
                            Some(ours_content),
                            Some(theirs_content),
                        );
                        let conflict = EntityConflict {
                            entity_name: key.to_string(),
                            entity_type: "interstitial".to_string(),
                            kind: ConflictKind::BothModified,
                            complexity,
                            ours_content: Some(ours_content.to_string()),
                            theirs_content: Some(theirs_content.to_string()),
                            base_content: Some(base_content.to_string()),
                        };
                        merged.insert(key.to_string(), conflict.to_conflict_markers(marker_format));
                        interstitial_conflicts.push(conflict);
                    }
                }
            }
        }
    }

    (merged, interstitial_conflicts)
}

/// Check if a region is predominantly import/use statements.
/// Handles both single-line imports and multi-line import blocks
/// (e.g. `import { type a, type b } from "..."` spread across lines).
fn is_import_region(content: &str) -> bool {
    let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        return false;
    }
    let mut import_count = 0;
    let mut in_multiline_import = false;
    for line in &lines {
        if in_multiline_import {
            import_count += 1;
            let trimmed = line.trim();
            if trimmed.starts_with('}') || trimmed.ends_with(')') {
                in_multiline_import = false;
            }
        } else if is_import_line(line) {
            import_count += 1;
            let trimmed = line.trim();
            // Detect start of multi-line import: `import {` or `import (` without closing on same line
            if (trimmed.contains('{') && !trimmed.contains('}'))
                || (trimmed.starts_with("import (") && !trimmed.contains(')'))
                || (trimmed.starts_with("from ")
                    && trimmed.contains("import (")
                    && !trimmed.contains(')'))
            {
                in_multiline_import = true;
            }
        }
    }
    // If >50% of non-empty lines are imports, treat as import region
    import_count * 2 > lines.len()
}

/// Post-merge cleanup: remove consecutive duplicate lines and normalize blank lines.
///
/// Fixes two classes of merge artifacts:
/// 1. Duplicate lines/blocks that appear when both sides add the same content
///    (e.g. duplicate typedefs, forward declarations)
/// 2. Missing blank lines between entities or declarations, and excessive
///    blank lines (3+ consecutive) collapsed to 2
fn post_merge_cleanup(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut result: Vec<&str> = Vec::with_capacity(lines.len());

    // Pass 1: Remove consecutive duplicate lines that look like declarations or imports.
    // Only dedup lines that are plausibly merge artifacts (imports, exports, forward decls).
    // Preserve intentional duplicates like repeated assertions, assignments, or data lines.
    for line in &lines {
        if line.trim().is_empty() {
            result.push(line);
            continue;
        }
        if let Some(prev) = result.last() {
            if !prev.trim().is_empty() && *prev == *line && looks_like_declaration(line) {
                continue; // skip consecutive exact duplicate of declaration-like line
            }
        }
        result.push(line);
    }

    // Pass 2: Collapse 3+ consecutive blank lines to 2 (one separator blank line).
    let mut final_lines: Vec<&str> = Vec::with_capacity(result.len());
    let mut consecutive_blanks = 0;
    for line in &result {
        if line.trim().is_empty() {
            consecutive_blanks += 1;
            if consecutive_blanks <= 2 {
                final_lines.push(line);
            }
        } else {
            consecutive_blanks = 0;
            final_lines.push(line);
        }
    }

    let mut out = final_lines.join("\n");
    if content.ends_with('\n') && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Check if a line looks like a declaration/import that merge might duplicate.
/// Returns false for lines that could be intentionally repeated (assertions,
/// assignments, data initializers, struct fields, etc.).
fn looks_like_declaration(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("import ")
        || trimmed.starts_with("from ")
        || trimmed.starts_with("use ")
        || trimmed.starts_with("export ")
        || trimmed.starts_with("require(")
        || trimmed.starts_with("#include")
        || trimmed.starts_with("typedef ")
        || trimmed.starts_with("using ")
        || (trimmed.starts_with("pub ") && trimmed.contains("mod "))
}

/// Check if a line is a top-level import/use/require statement.
///
/// Only matches unindented lines to avoid picking up conditional imports
/// inside `if TYPE_CHECKING:` blocks or similar constructs.
fn is_import_line(line: &str) -> bool {
    // Skip indented lines: these are inside conditional blocks (TYPE_CHECKING, etc.)
    if line.starts_with(' ') || line.starts_with('\t') {
        return false;
    }
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

/// Like is_import_line but operates on already-trimmed strings.
/// Used in the line-level safety net where we compare trimmed lines.
fn is_import_line_trimmed(trimmed: &str) -> bool {
    trimmed.starts_with("import ")
        || trimmed.starts_with("from ")
        || trimmed.starts_with("use ")
        || trimmed.starts_with("require(")
        || trimmed.starts_with("const ") && trimmed.contains("require(")
        || trimmed.starts_with("package ")
        || trimmed.starts_with("#include ")
        || trimmed.starts_with("using ")
}

/// A complete import statement (possibly multi-line) as a single unit.
#[derive(Debug, Clone)]
struct ImportStatement {
    /// The full text of the import (may span multiple lines)
    lines: Vec<String>,
    /// The source module (e.g. "./foo", "react", "std::io")
    source: String,
    /// For multi-line imports: the individual specifiers (e.g. ["type a", "type b"])
    specifiers: Vec<String>,
    /// Whether this is a multi-line import block
    is_multiline: bool,
}

/// Extract specifiers from a single-line import statement.
/// e.g. "from X import A, B, C" → ["A", "B", "C"]
///      "import { A, B } from 'X'" → ["A", "B"]
///      "import X from 'Y'" → [] (default import, no named specifiers)
fn parse_single_line_specifiers(trimmed: &str) -> Vec<String> {
    // Python: "from X import A, B, C"
    if let Some(import_pos) = trimmed.find(" import ") {
        let after_import = &trimmed[import_pos + 8..];
        // Skip if it's "import *" or "import (..." (multi-line handled elsewhere)
        if after_import.starts_with('*') || after_import.starts_with('(') {
            return Vec::new();
        }
        return after_import
            .split(',')
            .map(|s| s.trim().trim_end_matches(';').to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }
    // JS/TS: "import { A, B } from 'X'"
    if trimmed.starts_with("import ") {
        if let Some(brace_start) = trimmed.find('{') {
            if let Some(brace_end) = trimmed.find('}') {
                if brace_start < brace_end {
                    let inner = &trimmed[brace_start + 1..brace_end];
                    return inner
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
            }
        }
    }
    Vec::new()
}

/// Parse content into import statements, handling multi-line imports as single units.
fn parse_import_statements(content: &str) -> (Vec<ImportStatement>, Vec<String>) {
    let mut imports: Vec<ImportStatement> = Vec::new();
    let mut non_import_lines: Vec<String> = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        if line.trim().is_empty() {
            non_import_lines.push(line.to_string());
            i += 1;
            continue;
        }

        if is_import_line(line) {
            let trimmed = line.trim();
            // Check for multi-line import: `import {` without `}` on same line
            let starts_multiline = (trimmed.contains('{') && !trimmed.contains('}'))
                || (trimmed.starts_with("import (") && !trimmed.contains(')'))
                || (trimmed.starts_with("from ")
                    && trimmed.contains("import (")
                    && !trimmed.contains(')'));

            if starts_multiline {
                let mut block_lines = vec![line.to_string()];
                let mut specifiers = Vec::new();
                let close_char = if trimmed.contains('{') { '}' } else { ')' };
                i += 1;

                // Collect lines until closing brace/paren
                while i < lines.len() {
                    let inner = lines[i];
                    block_lines.push(inner.to_string());
                    let inner_trimmed = inner.trim();

                    if inner_trimmed.starts_with(close_char) {
                        // This is the closing line (e.g. `} from "./foo"`)
                        break;
                    } else if !inner_trimmed.is_empty() {
                        // This is a specifier line — strip trailing comma
                        let spec = inner_trimmed.trim_end_matches(',').trim().to_string();
                        if !spec.is_empty() {
                            specifiers.push(spec);
                        }
                    }
                    i += 1;
                }

                let full_text = block_lines.join("\n");
                let source = import_source_prefix(&full_text).to_string();
                imports.push(ImportStatement {
                    lines: block_lines,
                    source,
                    specifiers,
                    is_multiline: true,
                });
            } else {
                // Single-line import — also parse specifiers for set-based merging
                let source = import_source_prefix(line).to_string();
                let specifiers = parse_single_line_specifiers(trimmed);
                imports.push(ImportStatement {
                    lines: vec![line.to_string()],
                    source,
                    specifiers,
                    is_multiline: false,
                });
            }
        } else {
            non_import_lines.push(line.to_string());
        }
        i += 1;
    }

    (imports, non_import_lines)
}

/// Merge import blocks commutatively (as unordered sets), preserving grouping.
///
/// Handles both single-line imports and multi-line import blocks.
/// For multi-line imports from the same source, merges specifiers as a set.
/// Single-line imports are merged as before: set union with deletions.
fn merge_imports_commutatively(base: &str, ours: &str, theirs: &str) -> String {
    let (base_imports, _) = parse_import_statements(base);
    let (ours_imports, _) = parse_import_statements(ours);
    let (theirs_imports, _) = parse_import_statements(theirs);

    let has_multiline = base_imports.iter().any(|i| i.is_multiline)
        || ours_imports.iter().any(|i| i.is_multiline)
        || theirs_imports.iter().any(|i| i.is_multiline);

    if has_multiline {
        return merge_imports_with_multiline(
            base,
            ours,
            theirs,
            &base_imports,
            &ours_imports,
            &theirs_imports,
        );
    }

    // Original single-line-only logic
    let base_lines: HashSet<&str> = base.lines().filter(|l| is_import_line(l)).collect();
    let ours_lines: HashSet<&str> = ours.lines().filter(|l| is_import_line(l)).collect();

    let theirs_deleted: HashSet<&str> = base_lines
        .difference(
            &theirs
                .lines()
                .filter(|l| is_import_line(l))
                .collect::<HashSet<&str>>(),
        )
        .copied()
        .collect();

    let theirs_added: Vec<&str> = theirs
        .lines()
        .filter(|l| is_import_line(l) && !base_lines.contains(l) && !ours_lines.contains(l))
        .collect();

    // Build import groups from ours (import lines only)
    let mut groups: Vec<Vec<String>> = Vec::new();
    let mut current_group: Vec<String> = Vec::new();

    for line in ours.lines() {
        if line.trim().is_empty() {
            if !current_group.is_empty() {
                groups.push(current_group);
                current_group = Vec::new();
            }
        } else if is_import_line(line) {
            if theirs_deleted.contains(line) {
                continue;
            }
            current_group.push(line.to_string());
        }
    }
    if !current_group.is_empty() {
        groups.push(current_group);
    }

    for add in &theirs_added {
        let prefix = import_source_prefix(add);
        let mut best_group = if groups.is_empty() {
            0
        } else {
            groups.len() - 1
        };
        let mut matching_line = None;
        for (i, group) in groups.iter().enumerate() {
            if let Some(pos) = group
                .iter()
                .position(|l| is_import_line(l) && import_source_prefix(l) == prefix)
            {
                best_group = i;
                matching_line = Some(pos);
                break;
            }
        }
        if let Some(pos) = matching_line {
            let base_specs: Vec<String> = base_imports
                .iter()
                .filter(|i| i.source == prefix)
                .flat_map(|i| i.specifiers.iter().cloned())
                .collect();
            if let Some(merged) =
                merge_same_source_named_import_line(&groups[best_group][pos], add, &base_specs)
            {
                groups[best_group][pos] = merged;
                continue;
            }
        }
        if best_group < groups.len() {
            groups[best_group].push(add.to_string());
        } else {
            groups.push(vec![add.to_string()]);
        }
    }

    // Sort import lines within each group alphabetically
    for group in &mut groups {
        group.sort_unstable();
    }

    let mut import_lines: Vec<String> = Vec::new();
    for (i, group) in groups.iter().enumerate() {
        if i > 0 {
            import_lines.push(String::new());
        }
        import_lines.extend(group.iter().cloned());
    }

    let import_block = import_lines.join("\n");

    // Split non-import lines into prefix (before first import) and suffix
    // (after last import) to preserve file-leading directives like
    // `// @ts-nocheck`, shebangs, or license headers (fixes #94).
    let extract_prefix_suffix = |content: &str| -> (String, String) {
        let mut prefix = Vec::new();
        let mut suffix = Vec::new();
        let mut seen_import = false;
        let mut last_import_idx = 0;
        let all_lines: Vec<&str> = content.lines().collect();

        for (idx, line) in all_lines.iter().enumerate() {
            if is_import_line(line) {
                seen_import = true;
                last_import_idx = idx;
            }
        }

        for (idx, line) in all_lines.iter().enumerate() {
            if is_import_line(line) || line.trim().is_empty() {
                continue;
            }
            if !seen_import
                || idx
                    < all_lines
                        .iter()
                        .position(|l| is_import_line(l))
                        .unwrap_or(0)
            {
                prefix.push(*line);
            } else if idx > last_import_idx {
                suffix.push(*line);
            }
        }

        (prefix.join("\n"), suffix.join("\n"))
    };

    let (base_prefix, base_suffix) = extract_prefix_suffix(base);
    let (ours_prefix, ours_suffix) = extract_prefix_suffix(ours);
    let (theirs_prefix, theirs_suffix) = extract_prefix_suffix(theirs);

    // Merge prefix lines (directives before imports)
    let mut result = String::new();
    if !base_prefix.is_empty() || !ours_prefix.is_empty() || !theirs_prefix.is_empty() {
        let merged_prefix = match diffy::merge(&base_prefix, &ours_prefix, &theirs_prefix) {
            Ok(m) => m,
            Err(conflicted) => conflicted,
        };
        if !merged_prefix.trim().is_empty() {
            result.push_str(&merged_prefix);
            result.push('\n');
        }
    }

    result.push_str(&import_block);

    // Merge suffix lines (non-import lines after imports)
    if !base_suffix.is_empty() || !ours_suffix.is_empty() || !theirs_suffix.is_empty() {
        let merged_suffix = match diffy::merge(&base_suffix, &ours_suffix, &theirs_suffix) {
            Ok(m) => m,
            Err(conflicted) => conflicted,
        };
        if !merged_suffix.trim().is_empty() {
            result.push('\n');
            result.push('\n');
            result.push_str(&merged_suffix);
        }
    }
    let ours_trailing = ours.len() - ours.trim_end_matches('\n').len();
    let result_trailing = result.len() - result.trim_end_matches('\n').len();
    for _ in result_trailing..ours_trailing {
        result.push('\n');
    }
    result
}

/// Merge imports when multi-line import blocks are involved.
/// Matches imports by source module, merges specifiers as a set.
fn merge_imports_with_multiline(
    _base_raw: &str,
    ours_raw: &str,
    _theirs_raw: &str,
    base_imports: &[ImportStatement],
    ours_imports: &[ImportStatement],
    theirs_imports: &[ImportStatement],
) -> String {
    // Build source → specifier sets for base and theirs.
    // Use entry API to merge specifiers when multiple imports share the same source
    // (e.g. `import type { Foo } from "./foo"` AND `import { type a } from "./foo"`).
    let mut base_specs: HashMap<&str, HashSet<&str>> = HashMap::new();
    for imp in base_imports {
        let entry = base_specs.entry(imp.source.as_str()).or_default();
        for s in &imp.specifiers {
            entry.insert(s.as_str());
        }
    }

    let mut theirs_specs: HashMap<&str, HashSet<&str>> = HashMap::new();
    for imp in theirs_imports {
        let entry = theirs_specs.entry(imp.source.as_str()).or_default();
        for s in &imp.specifiers {
            entry.insert(s.as_str());
        }
    }

    // Single-line import tracking: base lines and theirs-deleted
    let base_single: HashSet<String> = base_imports
        .iter()
        .filter(|i| !i.is_multiline)
        .map(|i| i.lines[0].clone())
        .collect();
    let theirs_single: HashSet<String> = theirs_imports
        .iter()
        .filter(|i| !i.is_multiline)
        .map(|i| i.lines[0].clone())
        .collect();
    let theirs_deleted_single: HashSet<&str> = base_single
        .iter()
        .filter(|l| !theirs_single.contains(l.as_str()))
        .map(|l| l.as_str())
        .collect();

    // Process ours imports, merging in theirs specifiers
    let mut result_parts: Vec<String> = Vec::new();
    let mut handled_theirs_sources: HashSet<&str> = HashSet::new();

    // Walk through ours_raw to preserve formatting (blank lines, comments)
    let lines: Vec<&str> = ours_raw.lines().collect();
    let mut i = 0;
    let mut ours_imp_idx = 0;

    while i < lines.len() {
        let line = lines[i];

        if line.trim().is_empty() {
            result_parts.push(line.to_string());
            i += 1;
            continue;
        }

        if is_import_line(line) {
            let trimmed = line.trim();
            let starts_multiline = (trimmed.contains('{') && !trimmed.contains('}'))
                || (trimmed.starts_with("import (") && !trimmed.contains(')'))
                || (trimmed.starts_with("from ")
                    && trimmed.contains("import (")
                    && !trimmed.contains(')'));

            if starts_multiline && ours_imp_idx < ours_imports.len() {
                let imp = &ours_imports[ours_imp_idx];
                // Find the matching import by source
                let source = imp.source.as_str();
                handled_theirs_sources.insert(source);

                // Merge specifiers: ours + theirs additions - theirs deletions
                let base_spec_set = base_specs.get(source).cloned().unwrap_or_default();
                let theirs_spec_set = theirs_specs.get(source).cloned().unwrap_or_default();
                // Added by theirs: in theirs but not in base
                let theirs_added: HashSet<&str> = theirs_spec_set
                    .difference(&base_spec_set)
                    .copied()
                    .collect();
                // Deleted by theirs: in base but not in theirs
                let theirs_removed: HashSet<&str> = base_spec_set
                    .difference(&theirs_spec_set)
                    .copied()
                    .collect();

                // Final set: ours (in original order) + theirs_added - theirs_removed
                let mut final_specs: Vec<&str> = imp
                    .specifiers
                    .iter()
                    .map(|s| s.as_str())
                    .filter(|s| !theirs_removed.contains(s))
                    .collect();
                for added in &theirs_added {
                    if !final_specs.contains(added) {
                        final_specs.push(added);
                    }
                }

                // Detect indentation from the original block
                let indent = if imp.lines.len() > 1 {
                    let second = &imp.lines[1];
                    &second[..second.len() - second.trim_start().len()]
                } else {
                    "     "
                };

                // Reconstruct multi-line import
                result_parts.push(imp.lines[0].clone()); // `import {`
                for spec in &final_specs {
                    result_parts.push(format!("{}{},", indent, spec));
                }
                // Closing line from ours
                if let Some(last) = imp.lines.last() {
                    result_parts.push(last.clone());
                }

                // Skip past the original multi-line block in ours_raw
                let close_char = if trimmed.contains('{') { '}' } else { ')' };
                i += 1;
                while i < lines.len() {
                    if lines[i].trim().starts_with(close_char) {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                ours_imp_idx += 1;
                continue;
            } else {
                // Single-line import
                if ours_imp_idx < ours_imports.len() {
                    let imp = &ours_imports[ours_imp_idx];
                    let source = imp.source.as_str();
                    handled_theirs_sources.insert(source);
                    ours_imp_idx += 1;

                    // Check if theirs has a multi-line version with more specifiers
                    if let Some(theirs_spec_set) = theirs_specs.get(source) {
                        let base_spec_set = base_specs.get(source).cloned().unwrap_or_default();
                        let theirs_added: HashSet<&str> = theirs_spec_set
                            .difference(&base_spec_set)
                            .copied()
                            .collect();

                        if !theirs_added.is_empty() {
                            // Parse ours single-line specifier from the line text
                            let mut ours_specifiers: Vec<&str> = Vec::new();
                            let trimmed_line = line.trim();
                            // Python: "from X import Y, Z" → extract after "import "
                            if let Some(import_pos) = trimmed_line.find(" import ") {
                                let after_import = &trimmed_line[import_pos + 8..];
                                for spec in after_import.split(',') {
                                    let s = spec.trim().trim_end_matches(';');
                                    if !s.is_empty() {
                                        ours_specifiers.push(s);
                                    }
                                }
                            }
                            // JS/TS: "import X from 'Y'" → X is a default import, not a specifier
                            // Only merge if we found named specifiers
                            if !ours_specifiers.is_empty() {
                                let mut final_specs: Vec<&str> = ours_specifiers.clone();
                                for added in &theirs_added {
                                    if !final_specs.contains(added) {
                                        final_specs.push(added);
                                    }
                                }
                                // Find theirs import to get formatting
                                if let Some(theirs_imp) =
                                    theirs_imports.iter().find(|ti| ti.source == source)
                                {
                                    if theirs_imp.is_multiline {
                                        // Use theirs's multi-line formatting
                                        let indent = if theirs_imp.lines.len() > 1 {
                                            let second = &theirs_imp.lines[1];
                                            &second[..second.len() - second.trim_start().len()]
                                        } else {
                                            "    "
                                        };
                                        // Reconstruct as multi-line using theirs's opening/closing style
                                        result_parts.push(theirs_imp.lines[0].clone());
                                        for spec in &final_specs {
                                            result_parts.push(format!("{}{},", indent, spec));
                                        }
                                        if let Some(last) = theirs_imp.lines.last() {
                                            result_parts.push(last.clone());
                                        }
                                        i += 1;
                                        continue;
                                    }
                                }
                                // Theirs not multi-line but has more specifiers: reconstruct single-line
                                // e.g. "from X import A, B, C"
                                let prefix =
                                    &trimmed_line[..trimmed_line.find(" import ").unwrap() + 8];
                                result_parts.push(format!("{}{}", prefix, final_specs.join(", ")));
                                i += 1;
                                continue;
                            }
                        }
                    }
                } else {
                    // No more ours imports to match
                }
                // Check if theirs deleted this single-line import
                if !theirs_deleted_single.contains(line) {
                    result_parts.push(line.to_string());
                }
            }
        } else {
            result_parts.push(line.to_string());
        }
        i += 1;
    }

    // Add any new imports from theirs that have new sources
    for imp in theirs_imports {
        if handled_theirs_sources.contains(imp.source.as_str()) {
            continue;
        }
        // Truly new import from theirs (source wasn't handled in the main loop)
        for line in &imp.lines {
            result_parts.push(line.clone());
        }
    }

    let mut result = result_parts.join("\n");

    // Non-import lines: use diffy 3-way merge so adds/deletes/edits on
    // either side are handled correctly (fixes #60).
    let extract_non_imports = |content: &str| -> String {
        let (_, non_import_lines) = parse_import_statements(content);
        non_import_lines
            .into_iter()
            .filter(|l| !l.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    };
    let base_ni = extract_non_imports(_base_raw);
    let ours_ni = extract_non_imports(ours_raw);
    let theirs_ni = extract_non_imports(_theirs_raw);
    if !base_ni.is_empty() || !ours_ni.is_empty() || !theirs_ni.is_empty() {
        let merged_ni = match diffy::merge(&base_ni, &ours_ni, &theirs_ni) {
            Ok(m) => m,
            Err(conflicted) => conflicted,
        };
        if !merged_ni.trim().is_empty() {
            result.push('\n');
            result.push('\n');
            result.push_str(&merged_ni);
        }
    }

    let ours_trailing = ours_raw.len() - ours_raw.trim_end_matches('\n').len();
    let result_trailing = result.len() - result.trim_end_matches('\n').len();
    for _ in result_trailing..ours_trailing {
        result.push('\n');
    }
    result
}

fn named_import_parts(line: &str) -> Option<(&str, &str)> {
    let trimmed_start = line.len() - line.trim_start().len();
    let trimmed = &line[trimmed_start..];

    if let Some(open) = trimmed.find('{') {
        let close = trimmed.rfind('}')?;
        let from = trimmed.find(" from ")?;
        if open < close && close < from {
            return Some((
                &line[..trimmed_start + open + 1],
                &line[trimmed_start + close..],
            ));
        }
    }

    if trimmed.starts_with("from ") {
        if let Some(pos) = trimmed.find(" import ") {
            let prefix_end = trimmed_start + pos + " import ".len();
            return Some((&line[..prefix_end], ""));
        }
    }

    None
}

fn merge_same_source_named_import_line(
    ours: &str,
    theirs: &str,
    base_specs: &[String],
) -> Option<String> {
    let ours_specs = parse_single_line_specifiers(ours.trim());
    let theirs_specs = parse_single_line_specifiers(theirs.trim());
    if ours_specs.is_empty() || theirs_specs.is_empty() {
        return None;
    }

    let (ours_prefix, ours_suffix) = named_import_parts(ours)?;
    let (theirs_prefix, theirs_suffix) = named_import_parts(theirs)?;
    if ours_prefix.trim() != theirs_prefix.trim() || ours_suffix.trim() != theirs_suffix.trim() {
        return None;
    }

    let base_set: HashSet<&str> = base_specs.iter().map(String::as_str).collect();
    let theirs_set: HashSet<&str> = theirs_specs.iter().map(String::as_str).collect();
    let theirs_removed: HashSet<&str> = base_set.difference(&theirs_set).copied().collect();

    let mut final_specs: Vec<String> = ours_specs
        .into_iter()
        .filter(|s| !theirs_removed.contains(s.as_str()))
        .collect();

    for spec in theirs_specs {
        if !base_set.contains(spec.as_str()) && !final_specs.contains(&spec) {
            final_specs.push(spec);
        }
    }

    if final_specs.is_empty() {
        return None;
    }

    if ours_suffix.is_empty() {
        Some(format!("{}{}", ours_prefix, final_specs.join(", ")))
    } else {
        Some(format!(
            "{} {} {}",
            ours_prefix,
            final_specs.join(", "),
            ours_suffix.trim_start()
        ))
    }
}

/// Extract the source/module prefix from an import line for group matching.
/// e.g. "from collections import OrderedDict" -> "collections"
///      "import React from 'react'" -> "react"
///      "use std::collections::HashMap;" -> "std::collections"
fn import_source_prefix(line: &str) -> &str {
    // For multi-line imports, search all lines for the source module
    // (e.g. `} from "./foo"` on the closing line)
    for l in line.lines() {
        let trimmed = l.trim();
        // Python: "from X import Y" -> X
        if let Some(rest) = trimmed.strip_prefix("from ") {
            return rest.split_whitespace().next().unwrap_or("");
        }
        // JS/TS closing line: `} from 'Y'` or `} from "Y"`
        if trimmed.starts_with('}') && trimmed.contains("from ") {
            if let Some(quote_start) = trimmed.find(['\'', '"']) {
                let after = &trimmed[quote_start + 1..];
                if let Some(quote_end) = after.find(['\'', '"']) {
                    return &after[..quote_end];
                }
            }
        }
        // JS/TS: "import X from 'Y'" -> Y (between quotes)
        if trimmed.starts_with("import ") {
            if let Some(quote_start) = trimmed.find(['\'', '"']) {
                let after = &trimmed[quote_start + 1..];
                if let Some(quote_end) = after.find(['\'', '"']) {
                    return &after[..quote_end];
                }
            }
        }
        // Rust: "use X::Y;" -> X
        if let Some(rest) = trimmed.strip_prefix("use ") {
            return rest.split("::").next().unwrap_or("").trim_end_matches(';');
        }
    }
    line.trim()
}

/// Fallback to line-level 3-way merge when entity extraction isn't possible.
///
/// Uses Sesame-inspired separator preprocessing (arXiv:2407.18888) to get
/// finer-grained alignment before line-level merge. Inserts newlines around
/// syntactic separators ({, }, ;) so that changes in different code blocks
/// align independently, reducing spurious conflicts.
///
/// Sesame expansion is skipped for data formats (JSON, YAML, TOML, lock files)
/// where `{`, `}`, `;` are structural content rather than code separators.
/// Expanding them destroys alignment and produces far more conflicts (confirmed
/// on GitButler: YAML went from 68 git markers to 192 weave markers with Sesame).
fn line_level_fallback(base: &str, ours: &str, theirs: &str, file_path: &str) -> MergeResult {
    let mut stats = MergeStats::default();
    stats.used_fallback = true;

    // Skip Sesame preprocessing for data formats where {/}/; are content, not separators
    let skip = skip_sesame(file_path);

    if skip {
        // Use git merge-file for data formats so we match git's output exactly.
        // diffy::merge uses a different diff algorithm that can produce more
        // conflict markers on structured data like lock files.
        return git_merge_file(base, ours, theirs, &mut stats);
    }

    // Try Sesame expansion + diffy first, then compare against git merge-file.
    // Use whichever produces fewer conflict markers so we're never worse than git.
    let base_expanded = expand_separators(base);
    let ours_expanded = expand_separators(ours);
    let theirs_expanded = expand_separators(theirs);

    let sesame_result = match diffy::merge(&base_expanded, &ours_expanded, &theirs_expanded) {
        Ok(merged) => {
            let content = collapse_separators(&merged, base);
            Some(MergeResult {
                content: post_merge_cleanup(&content),
                conflicts: vec![],
                warnings: vec![],
                stats: stats.clone(),
                audit: vec![],
            })
        }
        Err(_) => {
            // Sesame expansion conflicted, try plain diffy
            match diffy::merge(base, ours, theirs) {
                Ok(merged) => Some(MergeResult {
                    content: merged,
                    conflicts: vec![],
                    warnings: vec![],
                    stats: stats.clone(),
                    audit: vec![],
                }),
                Err(conflicted) => {
                    let _markers = conflicted
                        .lines()
                        .filter(|l| l.starts_with("<<<<<<<"))
                        .count();
                    let mut s = stats.clone();
                    s.entities_conflicted = 1;
                    Some(MergeResult {
                        content: conflicted,
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
                        stats: s,
                        audit: vec![],
                    })
                }
            }
        }
    };

    // Get git merge-file result as our floor
    let git_result = git_merge_file(base, ours, theirs, &mut stats);

    // Compare: use sesame result only if it has fewer or equal markers
    match sesame_result {
        Some(sesame) if sesame.conflicts.is_empty() && !git_result.conflicts.is_empty() => {
            // Sesame resolved cleanly, git didn't: use sesame
            sesame
        }
        Some(sesame) if !sesame.conflicts.is_empty() && !git_result.conflicts.is_empty() => {
            // Both conflicted: use whichever has fewer markers
            let sesame_markers = sesame
                .content
                .lines()
                .filter(|l| l.starts_with("<<<<<<<"))
                .count();
            let git_markers = git_result
                .content
                .lines()
                .filter(|l| l.starts_with("<<<<<<<"))
                .count();
            if sesame_markers <= git_markers {
                sesame
            } else {
                git_result
            }
        }
        _ => git_result,
    }
}

/// Shell out to `git merge-file` for an exact match with git's line-level merge.
///
/// We use this instead of `diffy::merge` for data formats (lock files, JSON, YAML, TOML)
/// where weave can't improve on git. `diffy` uses a different diff algorithm that can
/// produce more conflict markers on structured data (e.g. 22 markers vs git's 19 on uv.lock).
fn git_merge_file(base: &str, ours: &str, theirs: &str, stats: &mut MergeStats) -> MergeResult {
    let dir = match tempfile::tempdir() {
        Ok(d) => d,
        Err(_) => return diffy_fallback(base, ours, theirs, stats),
    };

    let base_path = dir.path().join("base");
    let ours_path = dir.path().join("ours");
    let theirs_path = dir.path().join("theirs");

    let write_ok = (|| -> std::io::Result<()> {
        std::fs::File::create(&base_path)?.write_all(base.as_bytes())?;
        std::fs::File::create(&ours_path)?.write_all(ours.as_bytes())?;
        std::fs::File::create(&theirs_path)?.write_all(theirs.as_bytes())?;
        Ok(())
    })();

    if write_ok.is_err() {
        return diffy_fallback(base, ours, theirs, stats);
    }

    // git merge-file writes result to the first file (ours) in place
    let output = Command::new("git")
        .arg("merge-file")
        .arg("-p") // print to stdout instead of modifying ours in place
        .arg("--diff3") // include ||||||| base section for jj compatibility
        .arg("-L")
        .arg("ours")
        .arg("-L")
        .arg("base")
        .arg("-L")
        .arg("theirs")
        .arg(&ours_path)
        .arg(&base_path)
        .arg(&theirs_path)
        .output();

    match output {
        Ok(result) => {
            let content = String::from_utf8_lossy(&result.stdout).into_owned();
            if result.status.success() {
                // Exit 0 = clean merge
                MergeResult {
                    content: post_merge_cleanup(&content),
                    conflicts: vec![],
                    warnings: vec![],
                    stats: stats.clone(),
                    audit: vec![],
                }
            } else {
                // Exit >0 = conflicts (exit code = number of conflicts)
                stats.entities_conflicted = 1;
                MergeResult {
                    content,
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
                    stats: stats.clone(),
                    audit: vec![],
                }
            }
        }
        // git not available, fall back to diffy
        Err(_) => diffy_fallback(base, ours, theirs, stats),
    }
}

/// Fallback to diffy::merge when git merge-file is unavailable.
fn diffy_fallback(base: &str, ours: &str, theirs: &str, stats: &mut MergeStats) -> MergeResult {
    match diffy::merge(base, ours, theirs) {
        Ok(merged) => {
            let content = post_merge_cleanup(&merged);
            MergeResult {
                content,
                conflicts: vec![],
                warnings: vec![],
                stats: stats.clone(),
                audit: vec![],
            }
        }
        Err(conflicted) => {
            stats.entities_conflicted = 1;
            MergeResult {
                content: conflicted,
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
                stats: stats.clone(),
                audit: vec![],
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
/// Check if entity list has too many duplicate names, which causes matching to hang.
fn has_excessive_duplicates(entities: &[SemanticEntity]) -> bool {
    let threshold = std::env::var("WEAVE_MAX_DUPLICATES")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(10);
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for e in entities {
        *counts.entry(&e.name).or_default() += 1;
    }
    counts.values().any(|&c| c >= threshold)
}

/// Filter out entities that are nested inside other entities.
/// O(n log n) via sort + stack, replacing the previous O(n^2) approach.
fn filter_nested_entities(mut entities: Vec<SemanticEntity>) -> Vec<SemanticEntity> {
    if entities.len() <= 1 {
        return entities;
    }

    // Sort by start_line ASC, then by end_line DESC (widest span first).
    // A parent entity always appears before its children in this order.
    entities.sort_by(|a, b| {
        a.start_line
            .cmp(&b.start_line)
            .then(b.end_line.cmp(&a.end_line))
    });

    // Stack-based filter: track the end_line of the current outermost entity.
    let mut result: Vec<SemanticEntity> = Vec::with_capacity(entities.len());
    let mut max_end: usize = 0;

    for entity in entities {
        if entity.start_line > max_end || max_end == 0 {
            // Not nested: new top-level entity
            max_end = entity.end_line;
            result.push(entity);
        } else if entity.start_line == result.last().map_or(0, |e| e.start_line)
            && entity.end_line == result.last().map_or(0, |e| e.end_line)
        {
            // Exact same span (e.g. decorated_definition wrapping function_definition)
            result.push(entity);
        }
        // else: strictly nested, skip
    }

    result
}

/// Get child entities of a parent, sorted by start line.
fn get_child_entities<'a>(
    parent: &SemanticEntity,
    all_entities: &'a [SemanticEntity],
) -> Vec<&'a SemanticEntity> {
    let mut children: Vec<&SemanticEntity> = all_entities
        .iter()
        .filter(|e| e.parent_id.as_deref() == Some(&parent.id))
        .collect();
    children.sort_by_key(|e| e.start_line);
    children
}

/// Compute a body hash for rename detection: the entity content with the entity
/// name replaced at word boundaries by a placeholder, so entities with identical
/// bodies but different names produce the same hash.
///
/// Compute Jaccard similarity on whitespace-split tokens of two strings.
/// Returns a value in [0.0, 1.0] where 1.0 means identical token sets.
fn token_jaccard(a: &str, b: &str) -> f64 {
    let tokens_a: HashSet<&str> = a.split_whitespace().collect();
    let tokens_b: HashSet<&str> = b.split_whitespace().collect();
    if tokens_a.is_empty() && tokens_b.is_empty() {
        return 1.0;
    }
    let intersection = tokens_a.intersection(&tokens_b).count();
    let union = tokens_a.union(&tokens_b).count();
    if union == 0 {
        1.0
    } else {
        intersection as f64 / union as f64
    }
}

/// Uses word-boundary matching to avoid partial replacements (e.g. replacing
/// "get" inside "getAll"). Works across all languages since it operates on
/// the content string, not language-specific AST features.
fn body_hash(entity: &SemanticEntity) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let normalized = replace_at_word_boundaries(&entity.content, &entity.name, "__ENTITY__");
    let mut hasher = DefaultHasher::new();
    normalized.hash(&mut hasher);
    hasher.finish()
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
                || (content.is_char_boundary(after_idx) && !is_ident_char(bytes[after_idx]));
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

/// Build a rename map from new_id → base_id using confidence-scored matching.
///
/// Detects when an entity in the branch has the same body as an entity
/// in base but a different name/ID, indicating it was renamed.
/// Uses body_hash (name-stripped content hash) and structural_hash with
/// confidence scoring to resolve ambiguous matches correctly.
fn build_rename_map(
    base_entities: &[SemanticEntity],
    branch_entities: &[SemanticEntity],
) -> HashMap<String, String> {
    let mut rename_map: HashMap<String, String> = HashMap::new();

    let base_ids: HashSet<&str> = base_entities.iter().map(|e| e.id.as_str()).collect();

    // Build body_hash → base entities (multiple can have same hash)
    let mut base_by_body: HashMap<u64, Vec<&SemanticEntity>> = HashMap::new();
    for entity in base_entities {
        base_by_body
            .entry(body_hash(entity))
            .or_default()
            .push(entity);
    }

    // Also keep structural_hash index as fallback
    let mut base_by_structural: HashMap<&str, Vec<&SemanticEntity>> = HashMap::new();
    for entity in base_entities {
        if let Some(ref sh) = entity.structural_hash {
            base_by_structural
                .entry(sh.as_str())
                .or_default()
                .push(entity);
        }
    }

    // Collect all candidate (branch_entity, base_entity, confidence) triples
    struct RenameCandidate<'a> {
        branch: &'a SemanticEntity,
        base: &'a SemanticEntity,
        confidence: f64,
    }
    let mut candidates: Vec<RenameCandidate> = Vec::new();

    for branch_entity in branch_entities {
        if base_ids.contains(branch_entity.id.as_str()) {
            continue;
        }

        let bh = body_hash(branch_entity);

        // Body hash matches
        if let Some(base_entities_for_hash) = base_by_body.get(&bh) {
            for &base_entity in base_entities_for_hash {
                let same_type = base_entity.entity_type == branch_entity.entity_type;
                let same_parent = base_entity.parent_id == branch_entity.parent_id;
                let confidence = match (same_type, same_parent) {
                    (true, true) => 0.95,
                    (true, false) => 0.8,
                    (false, _) => 0.6,
                };
                candidates.push(RenameCandidate {
                    branch: branch_entity,
                    base: base_entity,
                    confidence,
                });
            }
        }

        // Structural hash fallback (lower confidence)
        if let Some(ref sh) = branch_entity.structural_hash {
            if let Some(base_entities_for_sh) = base_by_structural.get(sh.as_str()) {
                for &base_entity in base_entities_for_sh {
                    // Skip if already covered by body hash match
                    if candidates
                        .iter()
                        .any(|c| c.branch.id == branch_entity.id && c.base.id == base_entity.id)
                    {
                        continue;
                    }
                    candidates.push(RenameCandidate {
                        branch: branch_entity,
                        base: base_entity,
                        confidence: 0.6,
                    });
                }
            }
        }

        // Token similarity fallback: catches renames where internal references were also updated
        // (e.g. IDE "rename symbol" changes the name everywhere in the body).
        // Only check same-file, same-type entities not already matched above.
        let has_candidate = candidates.iter().any(|c| c.branch.id == branch_entity.id);
        if !has_candidate {
            for base_entity in base_entities {
                if base_entity.entity_type != branch_entity.entity_type {
                    continue;
                }
                if base_entity.file_path != branch_entity.file_path {
                    continue;
                }
                // Skip if base entity still exists in branch (not actually deleted/renamed)
                if branch_entities.iter().any(|e| e.id == base_entity.id) {
                    continue;
                }
                let similarity = token_jaccard(&base_entity.content, &branch_entity.content);
                if similarity >= 0.7 {
                    let same_parent = base_entity.parent_id == branch_entity.parent_id;
                    let confidence = if same_parent { 0.75 } else { 0.65 };
                    candidates.push(RenameCandidate {
                        branch: branch_entity,
                        base: base_entity,
                        confidence,
                    });
                }
            }
        }
    }

    // Sort by confidence descending, assign greedily
    candidates.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut used_base_ids: HashSet<String> = HashSet::new();
    let mut used_branch_ids: HashSet<String> = HashSet::new();

    for candidate in &candidates {
        if candidate.confidence < 0.6 {
            break;
        }
        if used_base_ids.contains(&candidate.base.id)
            || used_branch_ids.contains(&candidate.branch.id)
        {
            continue;
        }
        // Don't rename if the base entity's ID still exists in branch (it wasn't actually renamed)
        let base_id_in_branch = branch_entities.iter().any(|e| e.id == candidate.base.id);
        if base_id_in_branch {
            continue;
        }
        rename_map.insert(candidate.branch.id.clone(), candidate.base.id.clone());
        used_base_ids.insert(candidate.base.id.clone());
        used_branch_ids.insert(candidate.branch.id.clone());
    }

    rename_map
}

/// Check if an entity type is a container that may benefit from inner entity merge.
fn is_container_entity_type(entity_type: &str) -> bool {
    matches!(
        entity_type,
        "class"
            | "interface"
            | "enum"
            | "impl"
            | "trait"
            | "module"
            | "impl_item"
            | "trait_item"
            | "struct"
            | "union"
            | "namespace"
            | "struct_item"
            | "struct_specifier"
            | "variable"
            | "export"
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

/// Result of an inner entity merge attempt.
struct InnerMergeResult {
    /// Merged content (may contain per-member conflict markers)
    content: String,
    /// Whether any members had conflicts
    has_conflicts: bool,
}

/// Convert sem-core child entities to MemberChunks for inner merge.
///
/// Uses child entity line positions to extract content from the container text,
/// including any leading decorators/annotations that tree-sitter attaches as
/// sibling nodes rather than part of the method node.
fn children_to_chunks(
    children: &[&SemanticEntity],
    container_content: &str,
    container_start_line: usize,
) -> Vec<MemberChunk> {
    if children.is_empty() {
        return Vec::new();
    }

    let lines: Vec<&str> = container_content.lines().collect();
    let mut chunks = Vec::new();

    for (i, child) in children.iter().enumerate() {
        let child_start_idx = child.start_line.saturating_sub(container_start_line);
        // +1 because end_line is inclusive but we need an exclusive upper bound for slicing
        let child_end_idx = child.end_line.saturating_sub(container_start_line) + 1;

        if child_end_idx > lines.len() + 1 || child_start_idx >= lines.len() {
            // Position out of range, fall back to entity content
            chunks.push(MemberChunk {
                name: child.name.clone(),
                content: child.content.clone(),
            });
            continue;
        }
        let child_end_idx = child_end_idx.min(lines.len());

        // Determine the earliest line we can claim (after previous child's end, or body start)
        let floor = if i > 0 {
            children[i - 1]
                .end_line
                .saturating_sub(container_start_line)
                + 1
        } else {
            // First child: start after the container header line (the `{` or `:` line)
            // For Python-style containers, find the declaration line ending with `:`
            // For brace-delimited, find the opening `{` on a declaration line
            let header_end = if is_python_style_container(&lines) {
                lines
                    .iter()
                    .position(|l| {
                        let t = l.trim();
                        (t.starts_with("class ")
                            || t.starts_with("def ")
                            || t.starts_with("async def "))
                            && t.ends_with(':')
                    })
                    .map(|p| p + 1)
                    .unwrap_or(0)
            } else {
                lines
                    .iter()
                    .position(|l| l.contains('{'))
                    .map(|p| p + 1)
                    .unwrap_or(0)
            };
            header_end
        };

        // Scan backwards from child_start_idx to include decorators/annotations/comments
        let mut content_start = child_start_idx;
        while content_start > floor {
            let prev = content_start - 1;
            let trimmed = lines[prev].trim();
            if trimmed.starts_with('@')
                || trimmed.starts_with("#[")
                || trimmed.starts_with("//")
                || trimmed.starts_with("///")
                || trimmed.starts_with("/**")
                || trimmed.starts_with("* ")
                || trimmed == "*/"
            {
                content_start = prev;
            } else if trimmed.is_empty() && content_start > floor + 1 {
                // Allow one blank line between decorator and method
                content_start = prev;
            } else {
                break;
            }
        }

        // Skip leading blank lines
        while content_start < child_start_idx && lines[content_start].trim().is_empty() {
            content_start += 1;
        }

        let chunk_content: String = lines[content_start..child_end_idx].join("\n");
        chunks.push(MemberChunk {
            name: child.name.clone(),
            content: chunk_content,
        });
    }

    chunks
}

/// Generate a scoped conflict marker for a single member within a container merge.
fn scoped_conflict_marker(
    name: &str,
    base: Option<&str>,
    ours: Option<&str>,
    theirs: Option<&str>,
    ours_deleted: bool,
    theirs_deleted: bool,
    fmt: &MarkerFormat,
) -> String {
    let open = "<".repeat(fmt.marker_length);
    let sep = "=".repeat(fmt.marker_length);
    let close = ">".repeat(fmt.marker_length);

    let o = ours.unwrap_or("");
    let t = theirs.unwrap_or("");

    // Narrow conflict markers to just the differing lines
    let ours_lines: Vec<&str> = o.lines().collect();
    let theirs_lines: Vec<&str> = t.lines().collect();
    let (prefix_len, suffix_len) = if ours.is_some() && theirs.is_some() {
        crate::conflict::narrow_conflict_lines(&ours_lines, &theirs_lines)
    } else {
        (0, 0)
    };
    let has_narrowing = prefix_len > 0 || suffix_len > 0;
    let ours_mid = &ours_lines[prefix_len..ours_lines.len() - suffix_len];
    let theirs_mid = &theirs_lines[prefix_len..theirs_lines.len() - suffix_len];

    let mut out = String::new();

    // Emit common prefix as clean text
    if has_narrowing {
        for line in &ours_lines[..prefix_len] {
            out.push_str(line);
            out.push('\n');
        }
    }

    // Opening marker
    if fmt.enhanced {
        if ours_deleted {
            out.push_str(&format!("{} ours ({} deleted)\n", open, name));
        } else {
            out.push_str(&format!("{} ours ({})\n", open, name));
        }
    } else {
        out.push_str(&format!("{} ours\n", open));
    }

    // Ours content (narrowed or full)
    if ours.is_some() {
        if has_narrowing {
            for line in ours_mid {
                out.push_str(line);
                out.push('\n');
            }
        } else {
            out.push_str(o);
            if !o.ends_with('\n') {
                out.push('\n');
            }
        }
    }

    // Base section for diff3 format (standard mode only)
    if !fmt.enhanced {
        let base_marker = "|".repeat(fmt.marker_length);
        out.push_str(&format!("{} base\n", base_marker));
        let b = base.unwrap_or("");
        if has_narrowing {
            let base_lines: Vec<&str> = b.lines().collect();
            let base_prefix = prefix_len.min(base_lines.len());
            let base_suffix = suffix_len.min(base_lines.len().saturating_sub(base_prefix));
            for line in &base_lines[base_prefix..base_lines.len() - base_suffix] {
                out.push_str(line);
                out.push('\n');
            }
        } else {
            out.push_str(b);
            if !b.is_empty() && !b.ends_with('\n') {
                out.push('\n');
            }
        }
    }

    // Separator
    out.push_str(&format!("{}\n", sep));

    // Theirs content (narrowed or full)
    if theirs.is_some() {
        if has_narrowing {
            for line in theirs_mid {
                out.push_str(line);
                out.push('\n');
            }
        } else {
            out.push_str(t);
            if !t.ends_with('\n') {
                out.push('\n');
            }
        }
    }

    // Closing marker
    if fmt.enhanced {
        if theirs_deleted {
            out.push_str(&format!("{} theirs ({} deleted)\n", close, name));
        } else {
            out.push_str(&format!("{} theirs ({})\n", close, name));
        }
    } else {
        out.push_str(&format!("{} theirs\n", close));
    }

    // Emit common suffix as clean text
    if has_narrowing {
        for line in &ours_lines[ours_lines.len() - suffix_len..] {
            out.push_str(line);
            out.push('\n');
        }
    }

    out
}

/// Try recursive inner entity merge for container types (classes, impls, etc.).
///
/// Inspired by LastMerge (arXiv:2507.19687): class members are "unordered children" —
/// reordering them is not a conflict. We chunk the class body into members, match by
/// name, and merge each member independently.
///
/// Returns Some(result) if chunking succeeded, None if we can't parse the container.
/// The result may contain per-member conflict markers (scoped conflicts).
fn try_inner_entity_merge(
    base: &str,
    ours: &str,
    theirs: &str,
    base_children: &[&SemanticEntity],
    ours_children: &[&SemanticEntity],
    theirs_children: &[&SemanticEntity],
    base_start_line: usize,
    ours_start_line: usize,
    theirs_start_line: usize,
    marker_format: &MarkerFormat,
) -> Option<InnerMergeResult> {
    // Try sem-core child entities first (tree-sitter-accurate boundaries),
    // fall back to indentation heuristic if children aren't available.
    // When children_to_chunks produces chunks, try indentation as a fallback
    // if the tree-sitter chunks lead to conflicts (the indentation heuristic
    // can include trailing context that helps diffy merge adjacent changes).
    let use_children = !ours_children.is_empty() || !theirs_children.is_empty();
    let (base_chunks, ours_chunks, theirs_chunks) = if use_children {
        (
            children_to_chunks(base_children, base, base_start_line),
            children_to_chunks(ours_children, ours, ours_start_line),
            children_to_chunks(theirs_children, theirs, theirs_start_line),
        )
    } else {
        (
            extract_member_chunks(base)?,
            extract_member_chunks(ours)?,
            extract_member_chunks(theirs)?,
        )
    };

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
                    merged_members.push(o.to_string());
                } else if b == o {
                    merged_members.push(t.to_string());
                } else if b == t {
                    merged_members.push(o.to_string());
                } else {
                    // Both changed differently: try diffy, then git merge-file, then decorator merge
                    if let Some(merged) = diffy_merge(b, o, t) {
                        merged_members.push(merged);
                    } else if let Some(merged) = git_merge_string(b, o, t) {
                        merged_members.push(merged);
                    } else if let Some(merged) = try_decorator_aware_merge(b, o, t) {
                        merged_members.push(merged);
                    } else {
                        // Emit per-member conflict markers
                        has_conflict = true;
                        merged_members.push(scoped_conflict_marker(
                            name,
                            Some(b),
                            Some(o),
                            Some(t),
                            false,
                            false,
                            marker_format,
                        ));
                    }
                }
            }
            // Deleted by theirs, ours unchanged or not in base
            (Some(b), Some(o), None) => {
                if *b == *o {
                    // Ours unchanged, theirs deleted → accept deletion
                } else {
                    // Ours modified, theirs deleted → per-member conflict
                    has_conflict = true;
                    merged_members.push(scoped_conflict_marker(
                        name,
                        Some(b),
                        Some(o),
                        None,
                        false,
                        true,
                        marker_format,
                    ));
                }
            }
            // Deleted by ours, theirs unchanged or not in base
            (Some(b), None, Some(t)) => {
                if *b == *t {
                    // Theirs unchanged, ours deleted → accept deletion
                } else {
                    // Theirs modified, ours deleted → per-member conflict
                    has_conflict = true;
                    merged_members.push(scoped_conflict_marker(
                        name,
                        Some(b),
                        None,
                        Some(t),
                        true,
                        false,
                        marker_format,
                    ));
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
            // Added by both with different content
            (None, Some(o), Some(t)) => {
                if o == t {
                    merged_members.push(o.to_string());
                } else {
                    has_conflict = true;
                    merged_members.push(scoped_conflict_marker(
                        name,
                        None,
                        Some(o),
                        Some(t),
                        false,
                        false,
                        marker_format,
                    ));
                }
            }
            // Deleted by both
            (Some(_), None, None) => {}
            (None, None, None) => {}
        }
    }

    // Reconstruct: header + merged members + footer
    let mut result = String::new();
    result.push_str(ours_header);
    if !ours_header.ends_with('\n') {
        result.push('\n');
    }

    // Detect if members are single-line (fields, variants) vs multi-line (methods)
    let has_multiline_members = merged_members.iter().any(|m| m.contains('\n'));
    // Check if the original content had blank lines between members
    let original_has_blank_separators = {
        let body = ours_header.len()..ours.rfind(ours_footer).unwrap_or(ours.len());
        let body_content = &ours[body];
        body_content.contains("\n\n")
    };

    for (i, member) in merged_members.iter().enumerate() {
        result.push_str(member);
        if !member.ends_with('\n') {
            result.push('\n');
        }
        // Add blank line between multi-line members only if the original had them
        if i < merged_members.len() - 1
            && has_multiline_members
            && original_has_blank_separators
            && !member.ends_with("\n\n")
        {
            result.push('\n');
        }
    }

    result.push_str(ours_footer);
    if !ours_footer.ends_with('\n') && ours.ends_with('\n') {
        result.push('\n');
    }

    // If children_to_chunks led to conflicts, retry with indentation heuristic.
    // The indentation approach includes trailing blank lines in chunks, giving
    // diffy more context to merge adjacent changes from different branches.
    if has_conflict && use_children {
        if let (Some(bc), Some(oc), Some(tc)) = (
            extract_member_chunks(base),
            extract_member_chunks(ours),
            extract_member_chunks(theirs),
        ) {
            if !bc.is_empty() || !oc.is_empty() || !tc.is_empty() {
                let fallback = try_inner_merge_with_chunks(
                    &bc,
                    &oc,
                    &tc,
                    ours,
                    ours_header,
                    ours_footer,
                    has_multiline_members,
                    marker_format,
                );
                if let Some(fb) = fallback {
                    if !fb.has_conflicts {
                        return Some(fb);
                    }
                }
            }
        }
    }

    Some(InnerMergeResult {
        content: result,
        has_conflicts: has_conflict,
    })
}

/// Inner merge helper using pre-extracted chunks. Used for indentation-heuristic fallback.
fn try_inner_merge_with_chunks(
    base_chunks: &[MemberChunk],
    ours_chunks: &[MemberChunk],
    theirs_chunks: &[MemberChunk],
    ours: &str,
    ours_header: &str,
    ours_footer: &str,
    has_multiline_hint: bool,
    marker_format: &MarkerFormat,
) -> Option<InnerMergeResult> {
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

    let mut all_names: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for chunk in ours_chunks {
        if seen.insert(chunk.name.clone()) {
            all_names.push(chunk.name.clone());
        }
    }
    for chunk in theirs_chunks {
        if seen.insert(chunk.name.clone()) {
            all_names.push(chunk.name.clone());
        }
    }

    let mut merged_members: Vec<String> = Vec::new();
    let mut has_conflict = false;

    for name in &all_names {
        let in_base = base_map.get(name.as_str());
        let in_ours = ours_map.get(name.as_str());
        let in_theirs = theirs_map.get(name.as_str());

        match (in_base, in_ours, in_theirs) {
            (Some(b), Some(o), Some(t)) => {
                if o == t {
                    merged_members.push(o.to_string());
                } else if b == o {
                    merged_members.push(t.to_string());
                } else if b == t {
                    merged_members.push(o.to_string());
                } else if let Some(merged) = diffy_merge(b, o, t) {
                    merged_members.push(merged);
                } else if let Some(merged) = git_merge_string(b, o, t) {
                    merged_members.push(merged);
                } else {
                    has_conflict = true;
                    merged_members.push(scoped_conflict_marker(
                        name,
                        Some(b),
                        Some(o),
                        Some(t),
                        false,
                        false,
                        marker_format,
                    ));
                }
            }
            (Some(b), Some(o), None) => {
                if *b != *o {
                    merged_members.push(o.to_string());
                }
            }
            (Some(b), None, Some(t)) => {
                if *b != *t {
                    merged_members.push(t.to_string());
                }
            }
            (None, Some(o), None) => merged_members.push(o.to_string()),
            (None, None, Some(t)) => merged_members.push(t.to_string()),
            (None, Some(o), Some(t)) => {
                if o == t {
                    merged_members.push(o.to_string());
                } else {
                    has_conflict = true;
                    merged_members.push(scoped_conflict_marker(
                        name,
                        None,
                        Some(o),
                        Some(t),
                        false,
                        false,
                        marker_format,
                    ));
                }
            }
            (Some(_), None, None) | (None, None, None) => {}
        }
    }

    let has_multiline_members =
        has_multiline_hint || merged_members.iter().any(|m| m.contains('\n'));
    let mut result = String::new();
    result.push_str(ours_header);
    if !ours_header.ends_with('\n') {
        result.push('\n');
    }
    for (i, member) in merged_members.iter().enumerate() {
        result.push_str(member);
        if !member.ends_with('\n') {
            result.push('\n');
        }
        if i < merged_members.len() - 1 && has_multiline_members && !member.ends_with("\n\n") {
            result.push('\n');
        }
    }
    result.push_str(ours_footer);
    if !ours_footer.ends_with('\n') && ours.ends_with('\n') {
        result.push('\n');
    }

    Some(InnerMergeResult {
        content: result,
        has_conflicts: has_conflict,
    })
}

/// Extract the header (class declaration) and footer (closing brace) from a container.
/// Supports both brace-delimited (JS/TS/Java/Rust/C) and indentation-based (Python) containers.
/// Detect whether a container entity uses Python-style indentation (`:` terminated
/// declaration) or brace-delimited style (`{`). Only inspects the declaration
/// line(s), not the body, so dict literals / set comprehensions inside methods
/// don't cause a false negative.
fn is_python_style_container(lines: &[&str]) -> bool {
    // Find the first line that looks like a class/def/async def declaration
    let has_colon_decl = lines.iter().any(|l| {
        let trimmed = l.trim();
        (trimmed.starts_with("class ")
            || trimmed.starts_with("def ")
            || trimmed.starts_with("async def "))
            && trimmed.ends_with(':')
    });
    if !has_colon_decl {
        return false;
    }
    // Verify the container itself opens with `:`, not `{`.
    // Look at the declaration line (first line ending with `:` that is class/def).
    // If that line also contains `{` it's not Python style (edge case: shouldn't happen).
    if let Some(decl) = lines.iter().find(|l| {
        let t = l.trim();
        (t.starts_with("class ") || t.starts_with("def ") || t.starts_with("async def "))
            && t.ends_with(':')
    }) {
        !decl.contains('{')
    } else {
        false
    }
}

fn extract_container_wrapper(content: &str) -> Option<(&str, &str)> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() < 2 {
        return None;
    }

    // Check if this is a Python-style container (ends with `:` instead of `{`)
    let is_python_style = is_python_style_container(&lines);

    if is_python_style {
        // Python: header is the `class Foo:` line, no footer
        let header_end = lines.iter().position(|l| l.trim().ends_with(':'))?;
        let header_byte_end: usize = lines[..=header_end].iter().map(|l| l.len() + 1).sum();
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

        let footer_byte_start: usize = lines[..footer_start].iter().map(|l| l.len() + 1).sum();
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
    let is_python_style = is_python_style_container(&lines);

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
            && !trimmed.starts_with("}")
            && trimmed != ","
        {
            // Save previous chunk
            if let Some(name) = current_name.take() {
                // Trim trailing blank lines
                while current_chunk_lines
                    .last()
                    .is_some_and(|l| l.trim().is_empty())
                {
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
        while current_chunk_lines
            .last()
            .is_some_and(|l| l.trim().is_empty())
        {
            current_chunk_lines.pop();
        }
        if !current_chunk_lines.is_empty() {
            chunks.push(MemberChunk {
                name,
                content: current_chunk_lines.join("\n"),
            });
        }
    }

    // Post-process: if any chunk has a brace-only name (anonymous struct literal
    // entries like Go's `{ Name: "x", ... }`), derive a name from the first
    // key-value field inside the chunk to avoid HashMap collisions.
    for chunk in &mut chunks {
        if chunk.name == "{" || chunk.name == "{}" {
            if let Some(better) = derive_name_from_struct_literal(&chunk.content) {
                chunk.name = better;
            }
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

    // Go method receiver: `func (c *Calculator) Add(` -> skip receiver, find name before second `(`
    if trimmed.starts_with("func ") && trimmed.get(5..6) == Some("(") {
        // Skip past the receiver: find closing `)`, then extract name before next `(`
        if let Some(recv_close) = trimmed.find(')') {
            let after_recv = &trimmed[recv_close + 1..];
            if let Some(paren_pos) = after_recv.find('(') {
                let before = after_recv[..paren_pos].trim();
                let name: String = before
                    .chars()
                    .rev()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
                if !name.is_empty() {
                    return name;
                }
            }
        }
    }

    // Strategy 1: For method/function declarations with parentheses,
    // the name is the identifier immediately before `(`.
    // This handles all languages: Java `public int add(`, Rust `pub fn add(`,
    // Python `def add(`, TS `async getUser(`, Go `func add(`, etc.
    if let Some(paren_pos) = trimmed.find('(') {
        let before = trimmed[..paren_pos].trim_end();
        let name: String = before
            .chars()
            .rev()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        if !name.is_empty() {
            return name;
        }
    }

    // Strategy 2: For fields/properties/variants without parens,
    // strip keywords and take the first identifier.
    let mut s = trimmed;
    for keyword in &[
        "export ",
        "public ",
        "private ",
        "protected ",
        "static ",
        "abstract ",
        "async ",
        "override ",
        "readonly ",
        "pub ",
        "pub(crate) ",
        "fn ",
        "def ",
        "get ",
        "set ",
    ] {
        if s.starts_with(keyword) {
            s = &s[keyword.len()..];
        }
    }
    if s.starts_with("fn ") {
        s = &s[3..];
    }

    let name: String = s
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();

    if name.is_empty() {
        trimmed.chars().take(20).collect()
    } else {
        name
    }
}

/// For anonymous struct literal entries (e.g., Go slice entries starting with `{`),
/// derive a name from the first key-value field inside the chunk.
/// E.g., `{ Name: "panelTitleSearch", ... }` → `panelTitleSearch`
fn derive_name_from_struct_literal(content: &str) -> Option<String> {
    for line in content.lines().skip(1) {
        let trimmed = line.trim().trim_end_matches(',');
        // Look for `Key: "value"` or `Key: value` pattern
        if let Some(colon_pos) = trimmed.find(':') {
            let value = trimmed[colon_pos + 1..].trim();
            // Strip quotes from string values
            let value = value.trim_matches('"').trim_matches('\'');
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// Returns true for data/config file formats where Sesame separator expansion
/// (`{`, `}`, `;`) is counterproductive because those chars are structural
/// content rather than code block separators.
///
/// Note: template files like .svelte/.vue are NOT included here because their
/// embedded `<script>` sections contain real code where Sesame helps.
/// Check if content looks binary (contains null bytes in first 8KB).
fn is_binary(content: &str) -> bool {
    content.as_bytes().iter().take(8192).any(|&b| b == 0)
}

/// Check if content already contains git conflict markers.
/// This happens with AU/AA conflicts where git stores markers in stage blobs.
fn has_conflict_markers(content: &str) -> bool {
    content.contains("<<<<<<<") && content.contains(">>>>>>>")
}

fn skip_sesame(file_path: &str) -> bool {
    let path_lower = file_path.to_lowercase();
    let extensions = [
        // Data/config formats
        ".json",
        ".yaml",
        ".yml",
        ".toml",
        ".lock",
        ".xml",
        ".csv",
        ".tsv",
        ".ini",
        ".cfg",
        ".conf",
        ".properties",
        ".env",
        // Markup/document formats
        ".md",
        ".markdown",
        ".txt",
        ".rst",
        ".svg",
        ".html",
        ".htm",
    ];
    extensions.iter().any(|ext| path_lower.ends_with(ext))
}

/// Expand syntactic separators into separate lines for finer merge alignment.
/// Inspired by Sesame (arXiv:2407.18888): isolating separators lets line-based
/// merge tools see block boundaries as independent change units.
/// Uses byte-level iteration since separators ({, }, ;) and string delimiters
/// (", ', `) are all ASCII.
fn expand_separators(content: &str) -> String {
    let bytes = content.as_bytes();
    let mut result = Vec::with_capacity(content.len() * 2);
    let mut in_string = false;
    let mut escape_next = false;
    let mut string_char = b'"';

    for &b in bytes {
        if escape_next {
            result.push(b);
            escape_next = false;
            continue;
        }
        if b == b'\\' && in_string {
            result.push(b);
            escape_next = true;
            continue;
        }
        if !in_string && (b == b'"' || b == b'\'' || b == b'`') {
            in_string = true;
            string_char = b;
            result.push(b);
            continue;
        }
        if in_string && b == string_char {
            in_string = false;
            result.push(b);
            continue;
        }

        if !in_string && (b == b'{' || b == b'}' || b == b';') {
            if result.last() != Some(&b'\n') && !result.is_empty() {
                result.push(b'\n');
            }
            result.push(b);
            result.push(b'\n');
        } else {
            result.push(b);
        }
    }

    // Safe: we only inserted ASCII bytes into valid UTF-8 content
    unsafe { String::from_utf8_unchecked(result) }
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
        assert_eq!(
            replace_at_word_boundaries("fn get() {}", "get", "__E__"),
            "fn __E__() {}"
        );
        // Should NOT replace inside longer identifiers
        assert_eq!(
            replace_at_word_boundaries("fn getAll() {}", "get", "__E__"),
            "fn getAll() {}"
        );
        assert_eq!(
            replace_at_word_boundaries("fn _get() {}", "get", "__E__"),
            "fn _get() {}"
        );
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
        let result = line_level_fallback(base, ours, theirs, "test.rs");
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
        let result = line_level_fallback(base, ours, theirs, "test.rs");
        assert!(!result.is_clean());
        assert!(result.stats.used_fallback);
    }

    #[test]
    fn test_expand_separators() {
        let code = "function foo() { return 1; }";
        let expanded = expand_separators(code);
        // Separators should be on their own lines
        assert!(
            expanded.contains("{\n"),
            "Opening brace should have newline after"
        );
        assert!(
            expanded.contains(";\n"),
            "Semicolons should have newline after"
        );
        assert!(
            expanded.contains("\n}"),
            "Closing brace should have newline before"
        );
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
        assert!(is_import_region(
            "import foo from 'foo';\nimport bar from 'bar';\n"
        ));
        assert!(is_import_region("use std::io;\nuse std::fs;\n"));
        assert!(!is_import_region("let x = 1;\nlet y = 2;\n"));
        // Mixed: 1 import + 2 non-imports → not import region
        assert!(!is_import_region(
            "import foo from 'foo';\nlet x = 1;\nlet y = 2;\n"
        ));
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
    fn test_named_import_parsing_rejects_reversed_braces() {
        let malformed = "import } foo { from 'bar';";
        assert!(parse_single_line_specifiers(malformed).is_empty());
        assert!(named_import_parts(malformed).is_none());
        assert!(named_import_parts("import { foo } from 'bar';").is_some());
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
        assert!(
            !result.contains("import b from 'b';"),
            "Removed import should stay removed"
        );
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
    fn test_commutative_import_merge_preserves_file_directive() {
        // Issue #94: // @ts-nocheck on line 1 must stay above imports
        let base = "// @ts-nocheck\nimport { a } from \"./a\";\n";
        let ours = "// @ts-nocheck\nimport { a } from \"./a\";\nimport { b } from \"./b\";\n";
        let theirs = "// @ts-nocheck\nimport { a } from \"./a\";\nimport { c } from \"./c\";\n";
        let result = merge_imports_commutatively(base, ours, theirs);
        assert!(
            result.contains("// @ts-nocheck"),
            "Directive should be preserved. Got: {:?}",
            result
        );
        let nocheck_pos = result.find("// @ts-nocheck").unwrap();
        let first_import_pos = result.find("import").unwrap();
        assert!(
            nocheck_pos < first_import_pos,
            "// @ts-nocheck must stay before imports. Got: {:?}",
            result
        );
        assert!(result.contains("import { b }"), "ours import missing");
        assert!(result.contains("import { c }"), "theirs import missing");
    }

    #[test]
    fn test_commutative_import_merge_preserves_shebang() {
        // Shebang must stay on line 1
        let base = "#!/usr/bin/env node\nimport { a } from \"./a\";\n";
        let ours = "#!/usr/bin/env node\nimport { a } from \"./a\";\nimport { b } from \"./b\";\n";
        let theirs =
            "#!/usr/bin/env node\nimport { a } from \"./a\";\nimport { c } from \"./c\";\n";
        let result = merge_imports_commutatively(base, ours, theirs);
        assert!(
            result.starts_with("#!/usr/bin/env node\n"),
            "Shebang must be first line. Got: {:?}",
            result
        );
    }

    #[test]
    fn test_entity_merge_ts_nocheck_stays_above_imports() {
        // Issue #94 end-to-end: both branches add a different import,
        // // @ts-nocheck must stay on line 1.
        let base = "// @ts-nocheck\nimport { a } from \"./a\";\n\nexport function f() {}\n";
        let ours =
            "// @ts-nocheck\nimport { a } from \"./a\";\nimport { b } from \"./b\";\n\nexport function f() {}\n";
        let theirs =
            "// @ts-nocheck\nimport { a } from \"./a\";\nimport { c } from \"./c\";\n\nexport function f() {}\n";
        let result = entity_merge(base, ours, theirs, "src/example.ts");
        assert!(
            result.is_clean(),
            "Should merge cleanly. Conflicts: {:?}",
            result.conflicts,
        );
        let nocheck_pos = result.content.find("// @ts-nocheck");
        let first_import_pos = result.content.find("import");
        assert!(
            nocheck_pos.is_some(),
            "// @ts-nocheck must be present. Got:\n{}",
            result.content
        );
        assert!(
            nocheck_pos.unwrap() < first_import_pos.unwrap(),
            "// @ts-nocheck must be before imports. Got:\n{}",
            result.content
        );
    }

    #[test]
    fn test_import_preserved_when_both_edit_declaration() {
        // Issue #95: theirs adds an import AND modifies a type, ours also
        // modifies the type. The import must not be dropped.
        let base = "type T = {\n  a: string;\n  b: string;\n};\n";
        let ours = "type T = {\n  a: string;\n  b: string;\n  c: string;\n};\n";
        let theirs = "import { G } from './g';\n\ntype T = {\n  a: G;\n  b: string;\n};\n";
        let result = entity_merge(base, ours, theirs, "test.ts");
        assert!(
            result.content.contains("import { G }"),
            "Import from theirs must be preserved. Got:\n{}",
            result.content
        );
        assert!(
            result.content.contains("c: string"),
            "Field added by ours must be present. Got:\n{}",
            result.content
        );
    }

    #[test]
    fn test_import_preserved_one_sided_entity_change() {
        // One-sided import + entity change should work (fast path).
        // This already works but guard against regressions.
        let base = "type T = {\n  a: string;\n};\n";
        let ours = base;
        let theirs = "import { G } from './g';\n\ntype T = {\n  a: G;\n};\n";
        let result = entity_merge(base, ours, theirs, "test.ts");
        assert!(
            result.content.contains("import { G }"),
            "Import must be preserved in one-sided case. Got:\n{}",
            result.content
        );
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
        assert!(
            result.content.contains("console.log"),
            "Should contain ours changes"
        );
        assert!(
            result.content.contains("negative"),
            "Should contain theirs changes"
        );
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
        assert!(
            result.content.contains("multiply"),
            "Should contain ours's new method"
        );
        assert!(
            result.content.contains("divide"),
            "Should contain theirs's new method"
        );
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
        assert_eq!(
            chunks.len(),
            2,
            "Should find 2 members, found {:?}",
            chunks.iter().map(|c| &c.name).collect::<Vec<_>>()
        );
        assert_eq!(chunks[0].name, "bar");
        assert_eq!(chunks[1].name, "baz");
    }

    #[test]
    fn test_extract_member_name() {
        assert_eq!(extract_member_name("add(a, b) {"), "add");
        assert_eq!(extract_member_name("fn add(&self, a: i32) -> i32 {"), "add");
        assert_eq!(extract_member_name("def add(self, a, b):"), "add");
        assert_eq!(
            extract_member_name("public static getValue(): number {"),
            "getValue"
        );
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
        eprintln!(
            "TS interface: clean={}, conflicts={:?}",
            result.is_clean(),
            result.conflicts
        );
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
        eprintln!(
            "Rust enum: clean={}, conflicts={:?}",
            result.is_clean(),
            result.conflicts
        );
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
        eprintln!(
            "Method reorder: clean={}, conflicts={:?}",
            result.is_clean(),
            result.conflicts
        );
        eprintln!("Content:\n{}", result.content);
        assert!(
            result.is_clean(),
            "Method reordering + modification should merge. Conflicts: {:?}",
            result.conflicts,
        );
        assert!(
            result.content.contains("console.log(\"fetching\""),
            "Should contain theirs modification"
        );
        assert!(
            result.content.contains("deleteUser"),
            "Should have deleteUser"
        );
        assert!(
            result.content.contains("createUser"),
            "Should have createUser"
        );
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
        eprintln!(
            "Doc comment + body: clean={}, conflicts={:?}",
            result.is_clean(),
            result.conflicts
        );
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
        eprintln!(
            "Guard clauses: clean={}, conflicts={:?}",
            result.is_clean(),
            result.conflicts
        );
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
        eprintln!(
            "Enum modify+add: clean={}, conflicts={:?}",
            result.is_clean(),
            result.conflicts
        );
        eprintln!("Content:\n{}", result.content);
        assert!(
            result.is_clean(),
            "Modify variant + add new variant should merge. Conflicts: {:?}",
            result.conflicts,
        );
        assert!(
            result.content.contains("\"disabled\""),
            "Should have modified Inactive"
        );
        assert!(
            result.content.contains("Deleted"),
            "Should have new Deleted variant"
        );
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
        eprintln!(
            "Config fields: clean={}, conflicts={:?}",
            result.is_clean(),
            result.conflicts
        );
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
        eprintln!(
            "Rust impl: clean={}, conflicts={:?}",
            result.is_clean(),
            result.conflicts
        );
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
    fn test_rust_impl_same_trait_different_types() {
        // Two impl blocks for the same trait but different types.
        // Each branch modifies a different impl. Both should be preserved.
        // Regression: sem-core <0.3.10 named both "Stream", causing collision.
        let base = r#"struct Foo;
struct Bar;

impl Stream for Foo {
    type Item = i32;
    fn poll_next(&self) -> Option<i32> {
        Some(1)
    }
}

impl Stream for Bar {
    type Item = String;
    fn poll_next(&self) -> Option<String> {
        Some("hello".into())
    }
}

fn other() {}
"#;
        let ours = r#"struct Foo;
struct Bar;

impl Stream for Foo {
    type Item = i32;
    fn poll_next(&self) -> Option<i32> {
        let x = compute();
        Some(x + 1)
    }
}

impl Stream for Bar {
    type Item = String;
    fn poll_next(&self) -> Option<String> {
        Some("hello".into())
    }
}

fn other() {}
"#;
        let theirs = r#"struct Foo;
struct Bar;

impl Stream for Foo {
    type Item = i32;
    fn poll_next(&self) -> Option<i32> {
        Some(1)
    }
}

impl Stream for Bar {
    type Item = String;
    fn poll_next(&self) -> Option<String> {
        let s = format!("hello {}", name);
        Some(s)
    }
}

fn other() {}
"#;
        let result = entity_merge(base, ours, theirs, "test.rs");
        assert!(
            result.is_clean(),
            "Same trait, different types should not conflict. Conflicts: {:?}",
            result.conflicts,
        );
        assert!(
            result.content.contains("impl Stream for Foo"),
            "Should have Foo impl"
        );
        assert!(
            result.content.contains("impl Stream for Bar"),
            "Should have Bar impl"
        );
        assert!(
            result.content.contains("compute()"),
            "Should have ours' Foo change"
        );
        assert!(
            result.content.contains("format!"),
            "Should have theirs' Bar change"
        );
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
        assert!(
            result.content.contains("/// Adds two numbers"),
            "Should have ours doc comment"
        );
        assert!(
            result.content.contains("a - b - 1"),
            "Should have theirs body change"
        );
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
        assert!(
            result.content.contains("/// Adds two numbers"),
            "Should have add's doc comment"
        );
        assert!(
            result.content.contains("/// Subtracts b from a"),
            "Should have subtract's doc comment"
        );
    }

    #[test]
    fn test_go_import_block_both_add_different() {
        // Go uses import (...) blocks — both add different imports
        let base = "package main\n\nimport (\n\t\"fmt\"\n\t\"os\"\n)\n\nfunc main() {\n\tfmt.Println(\"hello\")\n}\n";
        let ours = "package main\n\nimport (\n\t\"fmt\"\n\t\"os\"\n\t\"strings\"\n)\n\nfunc main() {\n\tfmt.Println(\"hello\")\n}\n";
        let theirs = "package main\n\nimport (\n\t\"fmt\"\n\t\"os\"\n\t\"io\"\n)\n\nfunc main() {\n\tfmt.Println(\"hello\")\n}\n";
        let result = entity_merge(base, ours, theirs, "main.go");
        eprintln!(
            "Go import block: clean={}, conflicts={:?}",
            result.is_clean(),
            result.conflicts
        );
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
        eprintln!(
            "Python class: clean={}, conflicts={:?}",
            result.is_clean(),
            result.conflicts
        );
        eprintln!("Content:\n{}", result.content);
        assert!(
            result.is_clean(),
            "Both adding methods to Python class should merge. Conflicts: {:?}",
            result.conflicts,
        );
        assert!(result.content.contains("multiply"), "Should have multiply");
        assert!(result.content.contains("divide"), "Should have divide");
    }

    #[test]
    fn test_interstitial_conflict_not_silently_embedded() {
        // Regression test: when interstitial content between entities has a
        // both-modified conflict, merge_interstitials must report it as a real
        // conflict instead of silently embedding raw diffy markers and claiming
        // is_clean=true.
        //
        // Scenario: a barrel export file (index.ts) with comments between
        // export statements. Both sides modify the SAME interstitial comment
        // block differently. The exports are the entities; the comment between
        // them is interstitial content that goes through merge_interstitials
        // → diffy, which cannot auto-merge conflicting edits.
        let base = r#"export { alpha } from "./alpha";

// Section: data utilities
// TODO: add more exports here

export { beta } from "./beta";
"#;
        let ours = r#"export { alpha } from "./alpha";

// Section: data utilities (sorting)
// Sorting helpers for list views

export { beta } from "./beta";
"#;
        let theirs = r#"export { alpha } from "./alpha";

// Section: data utilities (filtering)
// Filtering helpers for search views

export { beta } from "./beta";
"#;
        let result = entity_merge(base, ours, theirs, "index.ts");

        // The key assertions:
        // 1. If the content has conflict markers, is_clean() MUST be false
        let has_markers = result.content.contains("<<<<<<<") || result.content.contains(">>>>>>>");
        if has_markers {
            assert!(
                !result.is_clean(),
                "BUG: is_clean()=true but merged content has conflict markers!\n\
                 stats: {}\nconflicts: {:?}\ncontent:\n{}",
                result.stats,
                result.conflicts,
                result.content
            );
            assert!(
                result.stats.entities_conflicted > 0,
                "entities_conflicted should be > 0 when markers are present"
            );
        }

        // 2. If it was resolved cleanly, no markers should exist
        if result.is_clean() {
            assert!(
                !has_markers,
                "Clean merge should not contain conflict markers!\ncontent:\n{}",
                result.content
            );
        }
    }

    #[test]
    fn test_pre_conflicted_input_not_treated_as_clean() {
        // Regression test for AU/AA conflicts: git can store conflict markers
        // directly into stage blobs. Weave must not return is_clean=true.
        let base = "";
        let theirs = "";
        let ours = r#"/**
 * MIT License
 */

<<<<<<<< HEAD:src/lib/exports/index.ts
export { renderDocToBuffer } from "./doc-exporter";
export type { ExportOptions, ExportMetadata, RenderContext } from "./types";
========
export * from "./editor";
export * from "./types";
>>>>>>>> feature:packages/core/src/editor/index.ts
"#;
        let result = entity_merge(base, ours, theirs, "index.ts");

        assert!(
            !result.is_clean(),
            "Pre-conflicted input must not be reported as clean!\n\
             stats: {}\nconflicts: {:?}",
            result.stats,
            result.conflicts,
        );
        assert!(result.stats.entities_conflicted > 0);
        assert!(!result.conflicts.is_empty());
    }

    #[test]
    fn test_multi_line_signature_classified_as_syntax() {
        // Multi-line parameter list: changing a param should be Syntax, not Functional
        let base = "function process(\n    a: number,\n    b: string\n) {\n    return a;\n}\n";
        let ours = "function process(\n    a: number,\n    b: string,\n    c: boolean\n) {\n    return a;\n}\n";
        let theirs = "function process(\n    a: number,\n    b: number\n) {\n    return a;\n}\n";
        let complexity = crate::conflict::classify_conflict(Some(base), Some(ours), Some(theirs));
        assert_eq!(
            complexity,
            crate::conflict::ConflictComplexity::Syntax,
            "Multi-line signature change should be classified as Syntax, got {:?}",
            complexity
        );
    }

    #[test]
    fn test_grouped_import_merge_preserves_groups() {
        let base = "import os\nimport sys\n\nfrom collections import OrderedDict\nfrom typing import List\n";
        let ours = "import os\nimport sys\nimport json\n\nfrom collections import OrderedDict\nfrom typing import List\n";
        let theirs = "import os\nimport sys\n\nfrom collections import OrderedDict\nfrom collections import defaultdict\nfrom typing import List\n";
        let result = merge_imports_commutatively(base, ours, theirs);
        // json should be in the first group (stdlib), defaultdict in the second (collections)
        let lines: Vec<&str> = result.lines().collect();
        let json_idx = lines.iter().position(|l| l.contains("json"));
        let blank_idx = lines.iter().position(|l| l.trim().is_empty());
        let defaultdict_idx = lines.iter().position(|l| l.contains("defaultdict"));
        assert!(json_idx.is_some(), "json import should be present");
        assert!(
            blank_idx.is_some(),
            "blank line separator should be present"
        );
        assert!(
            defaultdict_idx.is_some(),
            "defaultdict import should be present"
        );
        // json should come before the blank line, defaultdict after
        assert!(
            json_idx.unwrap() < blank_idx.unwrap(),
            "json should be in first group"
        );
        assert!(
            defaultdict_idx.unwrap() > blank_idx.unwrap(),
            "defaultdict should be in second group"
        );
    }

    #[test]
    fn test_configurable_duplicate_threshold() {
        // Create entities with 15 same-name entities
        let entities: Vec<SemanticEntity> = (0..15)
            .map(|i| SemanticEntity {
                id: format!("test::function::test_{}", i),
                file_path: "test.ts".to_string(),
                entity_type: "function".to_string(),
                name: "test".to_string(),
                parent_id: None,
                content: format!("function test() {{ return {}; }}", i),
                content_hash: format!("hash_{}", i),
                structural_hash: None,
                start_line: i * 3 + 1,
                end_line: i * 3 + 3,
                metadata: None,
            })
            .collect();
        // Default threshold (10): should trigger
        assert!(has_excessive_duplicates(&entities));
        // Set threshold to 20: should not trigger
        std::env::set_var("WEAVE_MAX_DUPLICATES", "20");
        assert!(!has_excessive_duplicates(&entities));
        std::env::remove_var("WEAVE_MAX_DUPLICATES");
    }

    #[test]
    fn test_ts_multiline_import_consolidation() {
        // Issue #24: when incoming consolidates two imports into one multi-line import,
        // the `import {` opening line can get dropped.
        let base = "\
import type { Foo } from \"./foo\"
import {
     type a,
     type b,
     type c,
} from \"./foo\"

export function bar() {
    return 1;
}
";
        let ours = base;
        let theirs = "\
import {
     type Foo,
     type a,
     type b,
     type c,
} from \"./foo\"

export function bar() {
    return 1;
}
";
        let result = entity_merge(base, ours, theirs, "test.ts");
        eprintln!(
            "TS import consolidation: clean={}, conflicts={:?}",
            result.is_clean(),
            result.conflicts
        );
        eprintln!("Content:\n{}", result.content);
        // Theirs is the only change, result should match theirs exactly
        assert!(
            result.content.contains("import {"),
            "import {{ must not be dropped"
        );
        assert!(
            result.content.contains("type Foo,"),
            "type Foo must be present"
        );
        assert!(
            result.content.contains("} from \"./foo\""),
            "closing must be present"
        );
        assert!(
            !result.content.contains("import type { Foo }"),
            "old separate import should be removed"
        );
    }

    #[test]
    fn test_ts_multiline_import_both_modify() {
        // Issue #24 variant: both sides modify the import block
        let base = "\
import type { Foo } from \"./foo\"
import {
     type a,
     type b,
     type c,
} from \"./foo\"

export function bar() {
    return 1;
}
";
        // Ours: consolidates imports + adds type d
        let ours = "\
import {
     type Foo,
     type a,
     type b,
     type c,
     type d,
} from \"./foo\"

export function bar() {
    return 1;
}
";
        // Theirs: consolidates imports + adds type e
        let theirs = "\
import {
     type Foo,
     type a,
     type b,
     type c,
     type e,
} from \"./foo\"

export function bar() {
    return 1;
}
";
        let result = entity_merge(base, ours, theirs, "test.ts");
        eprintln!(
            "TS import both modify: clean={}, conflicts={:?}",
            result.is_clean(),
            result.conflicts
        );
        eprintln!("Content:\n{}", result.content);
        assert!(
            result.content.contains("import {"),
            "import {{ must not be dropped"
        );
        assert!(
            result.content.contains("type Foo,"),
            "type Foo must be present"
        );
        assert!(
            result.content.contains("type d,"),
            "ours addition must be present"
        );
        assert!(
            result.content.contains("type e,"),
            "theirs addition must be present"
        );
        assert!(
            result.content.contains("} from \"./foo\""),
            "closing must be present"
        );
    }

    #[test]
    fn test_ts_multiline_import_no_entities() {
        // Issue #24: file with only imports, no other entities
        let base = "\
import type { Foo } from \"./foo\"
import {
     type a,
     type b,
     type c,
} from \"./foo\"
";
        let ours = base;
        let theirs = "\
import {
     type Foo,
     type a,
     type b,
     type c,
} from \"./foo\"
";
        let result = entity_merge(base, ours, theirs, "test.ts");
        eprintln!(
            "TS import no entities: clean={}, conflicts={:?}",
            result.is_clean(),
            result.conflicts
        );
        eprintln!("Content:\n{}", result.content);
        assert!(
            result.content.contains("import {"),
            "import {{ must not be dropped"
        );
        assert!(
            result.content.contains("type Foo,"),
            "type Foo must be present"
        );
    }

    #[test]
    fn test_ts_multiline_import_export_variable() {
        // Issue #24: import block near an export variable entity
        let base = "\
import type { Foo } from \"./foo\"
import {
     type a,
     type b,
     type c,
} from \"./foo\"

export const X = 1;

export function bar() {
    return 1;
}
";
        let ours = "\
import type { Foo } from \"./foo\"
import {
     type a,
     type b,
     type c,
     type d,
} from \"./foo\"

export const X = 1;

export function bar() {
    return 1;
}
";
        let theirs = "\
import {
     type Foo,
     type a,
     type b,
     type c,
} from \"./foo\"

export const X = 2;

export function bar() {
    return 1;
}
";
        let result = entity_merge(base, ours, theirs, "test.ts");
        eprintln!(
            "TS import + export var: clean={}, conflicts={:?}",
            result.is_clean(),
            result.conflicts
        );
        eprintln!("Content:\n{}", result.content);
        assert!(
            result.content.contains("import {"),
            "import {{ must not be dropped"
        );
    }

    #[test]
    fn test_ts_multiline_import_adjacent_to_entity() {
        // Issue #24: import block directly adjacent to entity (no blank line)
        let base = "\
import type { Foo } from \"./foo\"
import {
     type a,
     type b,
     type c,
} from \"./foo\"
export function bar() {
    return 1;
}
";
        let ours = base;
        let theirs = "\
import {
     type Foo,
     type a,
     type b,
     type c,
} from \"./foo\"
export function bar() {
    return 1;
}
";
        let result = entity_merge(base, ours, theirs, "test.ts");
        eprintln!(
            "TS import adjacent: clean={}, conflicts={:?}",
            result.is_clean(),
            result.conflicts
        );
        eprintln!("Content:\n{}", result.content);
        assert!(
            result.content.contains("import {"),
            "import {{ must not be dropped"
        );
        assert!(
            result.content.contains("type Foo,"),
            "type Foo must be present"
        );
    }

    #[test]
    fn test_ts_multiline_import_both_consolidate_differently() {
        // Issue #24: both sides consolidate imports but add different specifiers
        let base = "\
import type { Foo } from \"./foo\"
import {
     type a,
     type b,
} from \"./foo\"

export function bar() {
    return 1;
}
";
        let ours = "\
import {
     type Foo,
     type a,
     type b,
     type c,
} from \"./foo\"

export function bar() {
    return 1;
}
";
        let theirs = "\
import {
     type Foo,
     type a,
     type b,
     type d,
} from \"./foo\"

export function bar() {
    return 1;
}
";
        let result = entity_merge(base, ours, theirs, "test.ts");
        eprintln!(
            "TS both consolidate: clean={}, conflicts={:?}",
            result.is_clean(),
            result.conflicts
        );
        eprintln!("Content:\n{}", result.content);
        assert!(
            result.content.contains("import {"),
            "import {{ must not be dropped"
        );
        assert!(
            result.content.contains("type Foo,"),
            "type Foo must be present"
        );
        assert!(
            result.content.contains("} from \"./foo\""),
            "closing must be present"
        );
    }

    #[test]
    fn test_ts_multiline_import_ours_adds_theirs_consolidates() {
        // Issue #24 variant: ours adds new import, theirs consolidates
        let base = "\
import type { Foo } from \"./foo\"
import {
     type a,
     type b,
     type c,
} from \"./foo\"

export function bar() {
    return 1;
}
";
        // Ours: adds a new specifier to the multiline import
        let ours = "\
import type { Foo } from \"./foo\"
import {
     type a,
     type b,
     type c,
     type d,
} from \"./foo\"

export function bar() {
    return 1;
}
";
        // Theirs: consolidates into one import
        let theirs = "\
import {
     type Foo,
     type a,
     type b,
     type c,
} from \"./foo\"

export function bar() {
    return 1;
}
";
        let result = entity_merge(base, ours, theirs, "test.ts");
        eprintln!(
            "TS import ours-adds theirs-consolidates: clean={}, conflicts={:?}",
            result.is_clean(),
            result.conflicts
        );
        eprintln!("Content:\n{}", result.content);
        assert!(
            result.content.contains("import {"),
            "import {{ must not be dropped"
        );
        assert!(
            result.content.contains("type d,"),
            "ours addition must be present"
        );
        assert!(
            result.content.contains("} from \"./foo\""),
            "closing must be present"
        );
    }

    #[test]
    fn test_ts_multiline_import_multiple_sources_no_closing_leak() {
        // Issue #24 latest repro: multiple multi-line imports from different sources.
        // The `} from "..."` closing lines were leaking into the non-import diffy merge,
        // producing orphaned `} from` lines without their `import {` opening lines.
        let base = "\
import {
    type A,
} from \"./file1\"
import {
    type B,
} from \"./file2\"
import {
    type C,
} from \"./file3\"

export function main() { return 1; }
";
        // Ours: adds a specifier to file1 and a new import from file4
        let ours = "\
import {
    type A,
    type A2,
} from \"./file1\"
import {
    type B,
} from \"./file2\"
import {
    type C,
} from \"./file3\"
import {
    type D,
} from \"./file4\"

export function main() { return 1; }
";
        // Theirs: adds a specifier to file3
        let theirs = "\
import {
    type A,
} from \"./file1\"
import {
    type B,
} from \"./file2\"
import {
    type C,
    type C2,
} from \"./file3\"

export function main() { return 1; }
";
        let result = entity_merge(base, ours, theirs, "test.ts");
        eprintln!("Multiple source imports: clean={}", result.is_clean());
        eprintln!("Content:\n{}", result.content);

        // All specifiers should be present
        assert!(result.content.contains("type A,"), "A must be present");
        assert!(
            result.content.contains("type A2,"),
            "A2 (ours addition) must be present"
        );
        assert!(result.content.contains("type B,"), "B must be present");
        assert!(result.content.contains("type C,"), "C must be present");
        assert!(
            result.content.contains("type C2,"),
            "C2 (theirs addition) must be present"
        );
        assert!(
            result.content.contains("type D,"),
            "D (ours new import) must be present"
        );

        // Count `import {` vs `} from` — they must be balanced
        let open_count = result
            .content
            .lines()
            .filter(|l| l.trim().starts_with("import {"))
            .count();
        let close_count = result
            .content
            .lines()
            .filter(|l| {
                let t = l.trim();
                t.starts_with('}') && t.contains("from ")
            })
            .count();
        assert_eq!(
            open_count, close_count,
            "import {{ and }} from must be balanced: {} opens vs {} closes\n{}",
            open_count, close_count, result.content
        );
    }

    #[test]
    fn test_rename_plus_modify_auto_resolves() {
        // Issue #53: ours renames a variable (IDE rename symbol), theirs modifies it.
        // Should detect the rename via token similarity and merge cleanly.
        let base = r#"export const cubeQueryExecutorTool = tool({
    name: "cubeQueryExecutorTool",
    description: "Execute a cube query",
    schema: z.object({ query: z.string() }),
    execute: async (input) => {
        return await runQuery(input.query);
    },
});
"#;
        // Ours: renamed cubeQueryExecutorTool → cubeQueryTool (all refs updated)
        let ours = r#"export const cubeQueryTool = tool({
    name: "cubeQueryTool",
    description: "Execute a cube query",
    schema: z.object({ query: z.string() }),
    execute: async (input) => {
        return await runQuery(input.query);
    },
});
"#;
        // Theirs: modified the body (added unit inference logic)
        let theirs = r#"export const cubeQueryExecutorTool = tool({
    name: "cubeQueryExecutorTool",
    description: "Execute a cube query with unit inference",
    schema: z.object({ query: z.string(), unit: z.string().optional() }),
    execute: async (input) => {
        const unit = input.unit || inferUnit(input.query);
        return await runQuery(input.query, unit);
    },
});
"#;
        let result = entity_merge(base, ours, theirs, "cubeQueryTool.ts");
        // Rename + modify should conflict (developer who modified didn't know about rename)
        assert_eq!(
            result.conflicts.len(),
            1,
            "Should have exactly one conflict"
        );
        assert!(
            matches!(
                result.conflicts[0].kind,
                ConflictKind::RenameModify {
                    renamed_in_ours: true,
                    ..
                }
            ),
            "Should be a RenameModify conflict, got: {:?}",
            result.conflicts[0].kind
        );
        // Both versions should be present in the conflict
        assert!(
            result.content.contains("cubeQueryTool"),
            "Ours (renamed) should be in conflict markers"
        );
        assert!(
            result.content.contains("unit inference"),
            "Theirs (modified) should be in conflict markers"
        );
    }
}
