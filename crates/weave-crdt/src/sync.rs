use std::collections::HashMap;
use std::path::Path;

use automerge::{transaction::Transactable, ObjType, ReadDoc};
use sem_core::parser::registry::ParserRegistry;
use weave_core::region::{extract_regions, FileRegion};

use crate::anchor::{ordered_entity_ids, Observation};
use crate::content::{entity_value, get_entity_content, read_writes};
use crate::error::Result;
use crate::join::EntityValue;
use crate::merge::CrdtMergeResult;
use crate::ops::{get_str, read_version_vector, upsert_entity};
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

/// Replace an entity's register with the given writes, made mutually
/// concurrent, over the given ancestor.
///
/// This is how a *local* observation (the working tree, or a test) is injected
/// into the register: the working tree is not an agent with a causal history, so
/// its content has to be stated as a write that stands beside the CRDT's rather
/// than after it. Nothing here is on the join path — the join reads the register
/// this function writes.
pub(crate) fn seed_concurrent_writes(
    state: &mut EntityStateDoc,
    entity_id: &str,
    base: &str,
    writes: &[(&str, &str)],
) -> Result<()> {
    let entities = state.entities_id()?;
    let Some((_, entity_obj)) = state.doc.get(&entities, entity_id)? else {
        return Err(crate::error::WeaveError::EntityNotFound(
            entity_id.to_string(),
        ));
    };
    // `base_content` stays a plain register here. It is a smaller instance of
    // the same shape as `writes` — an ancestor two doors both overwrite — and
    // moving it off LWW is out of scope for the register fix below; flagged so
    // the omission is a stated decision, not an oversight.
    state.doc.put(&entity_obj, "base_content", base)?;

    // The clock each seeding agent has already reached, read once before the
    // map is touched: a `working-tree` re-sync must supersede its OWN previous
    // entry ({agent: n} -> {agent: n+1}) while staying concurrent with every
    // other agent's key, whose counter is never written here.
    let mut seen = read_version_vector(&state.doc, &entity_obj);
    for w in read_writes(&state.doc, &entity_obj).values() {
        seen.merge(&w.vv);
    }

    // Reuse the existing `writes` object; do not replace it. Replacing the
    // container with `put_object` is a last-writer-wins overwrite one level up
    // from the per-key union — two replicas that each reseed drop the other's
    // entries, which is the one thing a CRDT may not do (state.rs). This is the
    // same reuse the content door performs (content.rs).
    let writes_obj = match state.doc.get(&entity_obj, "writes")? {
        Some((_, id)) => id,
        None => state.doc.put_object(&entity_obj, "writes", ObjType::Map)?,
    };
    for (agent, content) in writes {
        // One agent id is one causal thread; replacing that agent's own entry
        // is the agent superseding itself, not a lost update.
        let w = state.doc.put_object(&writes_obj, *agent, ObjType::Map)?;
        state.doc.put(&w, "content", *content)?;
        state.doc.put(&w, "deleted", false)?;
        state.doc.put(&w, "hash", "")?;
        let vv = state.doc.put_object(&w, "vv", ObjType::Map)?;
        // Only this agent's own counter, incremented past whatever it had
        // reached: dominates this agent's previous write, concurrent with all
        // others. Multiple agents seeded in one call are therefore mutually
        // concurrent, which is what a seeded conflict needs.
        let next = seen.get(agent) + 1;
        state.doc.put(&vv, *agent, next as i64)?;
    }
    Ok(())
}

/// Sync entities from working tree files into CRDT state.
pub fn sync_from_files(
    state: &mut EntityStateDoc,
    repo_root: &Path,
    file_paths: &[String],
    registry: &ParserRegistry,
) -> Result<usize> {
    let mut count = 0;

    for file_path in file_paths {
        let full_path = repo_root.join(file_path);
        // A file named for sync that is not on disk is a deletion — skip it.
        // Any OTHER read failure (a permission error, a non-UTF-8 file, a
        // directory in its place) is not a deletion and must not be silently
        // swallowed: it is propagated so the caller learns the sync was partial
        // rather than reading a hole as "nothing changed" (weave-ka7).
        let content = match std::fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.into()),
        };

        // No parser for this file type.
        let Some(plugin) = registry.get_plugin(file_path) else {
            continue;
        };

        let entities = plugin.extract_entities(&content, file_path);

        for entity in &entities {
            upsert_entity(
                state,
                &entity.id,
                &entity.name,
                &entity.entity_type,
                file_path,
                &entity.content_hash,
            )?;

            // The parser key resolves to the entity it names, which after a
            // rename announced on the op channel is the entity that already
            // holds the history rather than a fresh one. Resolved once, here,
            // and used for every write below — a second lookup by the raw key
            // is how the old row and the new one came to coexist.
            let eid = crate::identity::entity_id_for(state, &entity.id);

            // Reconcile the file's content against the CRDT with a 3-way rule so
            // a local (unmaterialized) entity edit is never silently clobbered by
            // a re-sync. `base` is the content at the last sync (the common
            // ancestor), the CRDT's joined value is the possibly-locally-edited
            // side, and `entity.content` is the file's current content.
            let entities_map = state.entities_id()?;
            if let Ok(Some((_, entity_obj))) = state.doc.get(&entities_map, eid.as_str()) {
                let base = get_str(&state.doc, &entity_obj, "base_content").unwrap_or_default();
                let crdt_content = entity_value(&state.doc, &entity_obj).materialize();
                let file_content = entity.content.clone();

                match reconcile_entity_content(&base, &crdt_content, &file_content) {
                    ContentReconcile::Seed => {
                        // First sight: the file IS the ancestor.
                        seed_concurrent_writes(
                            state,
                            &eid,
                            &file_content,
                            &[("working-tree", &file_content)],
                        )?;
                    }
                    ContentReconcile::FastForward => {
                        // The working tree moved with no local edit. Record it
                        // as an ordinary write by the `working-tree` thread over
                        // the SAME ancestor — do NOT reset the ancestor to the
                        // new file. A concurrent agent edit on another replica
                        // shares that ancestor; advancing base to the new file
                        // here makes the working-tree side of the eventual
                        // three-way merge equal base and be dropped — silent
                        // loss (weave-xxs). base advances lawfully via
                        // AdvanceBase once a sync observes file == crdt. The
                        // cost is that two external edits with no stabilising
                        // sync between them surface as a conflict rather than a
                        // second fast-forward — which weave prefers to a
                        // vanished write (a conflict is recoverable; a dropped
                        // write is not).
                        seed_concurrent_writes(
                            state,
                            &eid,
                            &base,
                            &[("working-tree", &file_content)],
                        )?;
                    }
                    ContentReconcile::AdvanceBase => {
                        state
                            .doc
                            .put(&entity_obj, "base_content", file_content.as_str())?;
                    }
                    // File unchanged since last sync: keep the local CRDT edit.
                    ContentReconcile::PreserveLocal => {}
                    ContentReconcile::Conflict => {
                        seed_concurrent_writes(
                            state,
                            &eid,
                            &base,
                            &[("crdt", &crdt_content), ("working-tree", &file_content)],
                        )?;
                    }
                }
            }

            count += 1;
        }

        // Placement, through the one anchor rule. A sync is the door that saw
        // the whole file, so it is the door with positional evidence — and
        // `place` is monotone, so it settles the evidence-free anchors the
        // other doors wrote rather than overwriting a settled answer. `sync`
        // used to carry its own second rule (`assign_anchors`) that overwrote
        // whatever `ops` had decided, letting two placement rules disagree
        // about the same anchor depending on which door ran last.
        let ids: Vec<String> = entities
            .iter()
            .map(|e| crate::identity::entity_id_for(state, &e.id))
            .collect();
        crate::anchor::place(state, file_path, Observation::File(&ids))?;

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

/// Join every entity of a file and report what the join produced.
///
/// This *is* the merge now. The previous implementation counted version-vector
/// entries, never called `entity_merge`, and took a `&ParserRegistry` it did not
/// use — it detected that a concurrent write had happened without ever
/// producing a joined result, which is not a join.
pub fn merge_file_entities(state: &EntityStateDoc, file_path: &str) -> Result<CrdtMergeResult> {
    let ids = ordered_entity_ids(state, file_path);
    if ids.is_empty() {
        return Ok(CrdtMergeResult {
            file_path: file_path.to_string(),
            entities_auto_merged: 0,
            entities_conflicted: 0,
            merged_content: None,
        });
    }
    let entities = state.entities_id()?;
    let mut auto_merged = 0;
    let mut conflicted = 0;
    for id in &ids {
        let Some((_, obj)) = state.doc.get(&entities, id.as_str())? else {
            continue;
        };
        match entity_value(&state.doc, &obj) {
            EntityValue::Conflict { .. } => conflicted += 1,
            EntityValue::Content(_) => auto_merged += 1,
            EntityValue::Absent | EntityValue::Tombstone => {}
        }
    }

    Ok(CrdtMergeResult {
        file_path: file_path.to_string(),
        entities_auto_merged: auto_merged,
        entities_conflicted: conflicted,
        merged_content: Some(reconstruct_file_from_crdt(state, file_path)?),
    })
}

/// Extract entity IDs from a single file (for lookups).
pub fn extract_entity_ids(
    content: &str,
    file_path: &str,
    registry: &ParserRegistry,
) -> Vec<(String, String, String)> {
    let Some(plugin) = registry.get_plugin(file_path) else {
        return Vec::new();
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

/// Reconstruct a file from CRDT state: anchor order + interstitials + the
/// joined value of every entity.
pub fn reconstruct_file_from_crdt(state: &EntityStateDoc, file_path: &str) -> Result<String> {
    let entity_order = ordered_entity_ids(state, file_path);
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

        // An entity in the anchor order whose content cannot be read is not a
        // blank line to skip past — dropping it silently reconstructs a file
        // missing code. Propagate it (weave-ka7).
        let status = get_entity_content(state, entity_id)?;
        output.push_str(&status.content);
        if !status.content.is_empty() && !status.content.ends_with('\n') {
            output.push('\n');
        }
    }

    // File footer interstitial
    let footer_key = format!("{}::file_footer", file_path);
    if let Some(footer) = interstitials.get(&footer_key) {
        output.push_str(footer);
    }

    Ok(output)
}

/// Read all interstitials for a file from the CRDT.
fn read_file_interstitials(state: &EntityStateDoc, file_path: &str) -> HashMap<String, String> {
    let Ok(inter_map) = state.file_interstitials_id() else {
        return HashMap::new();
    };

    let prefix = format!("{}::", file_path);
    state
        .doc
        .keys(&inter_map)
        .filter(|key| key.starts_with(&prefix))
        .filter_map(|key| get_str(&state.doc, &inter_map, &key).map(|val| (key, val)))
        .collect()
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
