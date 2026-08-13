//! Post-merge semantic validation.
//!
//! After a syntactically clean merge, entities may still be semantically
//! incompatible. For example, if function A calls function B and both were
//! modified by different agents, the merge may succeed syntactically but B's
//! contract (return type, parameters, side effects) may have changed in ways
//! that break A.
//!
//! This module produces the merge's `SemanticWarning` list. Most of it is
//! advisory in the strict sense — the merge succeeded, and these are things
//! about the result a reader should know. But `WarningKind` has since grown
//! past that: `CompositionLicensed` records a conflict weave *resolved* under
//! disjoint composition, with the two read/write sets that licensed it, and renders as
//! `resolved: …` rather than as a warning. So the list is better read as
//! "everything weave has to say about this merge beyond the bytes" than as a
//! warning channel. Either way it never fails a merge.

use std::collections::BTreeSet;

use sem_core::parser::graph::EntityGraph;
use sem_core::parser::registry::ParserRegistry;

/// A warning about a potentially unsafe merge.
#[derive(Debug, Clone)]
pub struct SemanticWarning {
    /// The entity that was auto-merged and may be at risk.
    pub entity_name: String,
    pub entity_type: String,
    pub file_path: String,
    /// The kind of semantic risk detected.
    pub kind: WarningKind,
    /// Related entities involved in the risk.
    pub related: Vec<RelatedEntity>,
}

#[derive(Debug, Clone)]
pub enum WarningKind {
    /// Entity references another entity that was also modified in this merge.
    /// The referenced entity's contract may have changed.
    DependencyAlsoModified,
    /// Entity is depended on by another entity that was also modified.
    /// The dependent may have adapted to old behavior.
    DependentAlsoModified,
    /// The merged output failed to parse — syntactically broken merge result.
    ParseFailedAfterMerge,
    /// A conflict was resolved under disjoint composition: the two sides inserted
    /// code into the same slot, and neither block writes what the other writes
    /// or reads. The composition is licensed, not guessed — and the evidence
    /// that licensed it travels with the finding so a reader can check it.
    CompositionLicensed {
        /// Names our side's inserted block binds.
        ours_binds: Vec<String>,
        /// Names their side's inserted block binds.
        theirs_binds: Vec<String>,
    },
    /// The FRAME of a conflicted output — every line outside every conflict box
    /// — states a line more times than any union of the two sides could put
    /// there.
    ///
    /// This is the one region of a conflicted file that no marker invites the
    /// reader to check and that no resolution removes, and until the checks
    /// ranged over it (`crate::frame`) nothing looked. A duplicated method
    /// definition living there is exactly the shape of bug this catches — one a
    /// reader has to delete by hand once they notice the merge shipped broken
    /// code outside its own markers.
    ///
    /// Advisory, always: the file already needs a human, so this changes no
    /// verdict. It is a pointer, computed on the finished bytes rather than on
    /// the derivation, because the derivation is only as good as the parse
    /// behind it.
    ConflictFrameDuplicate {
        /// The line the frame over-states.
        line: String,
        /// How many times the frame states it.
        found: usize,
        /// The most times an honest union of the two sides could.
        allowed: usize,
    },
    /// A container (class, impl, object, trait, enum) merged clean, but each
    /// side changed or added a DIFFERENT set of sibling members. The merge is
    /// correct at every member — nothing conflicted — yet the two sides' work
    /// now coexists in one container though neither author saw the other's.
    ///
    /// This is not a risk weave can decide (deciding whether two disjoint edits
    /// were MEANT to coexist is not something a merge tool can know), so it is
    /// the lowest register on this channel: advisory, never a warning and never
    /// a refusal. It surfaces one derivable fact — the container and each side's
    /// changed/added members — and asks a human to confirm the coexistence.
    ///
    /// Fires only when both sides changed/added members and those two member
    /// sets are disjoint (different siblings). A member both sides touched is
    /// either a conflict already or an identical edit, and is not co-occupancy.
    SiblingCoChange {
        /// Members ours added that theirs did not touch.
        ours_added: Vec<String>,
        /// Existing members ours changed substantively.
        ours_changed: Vec<String>,
        /// Members theirs added that ours did not touch.
        theirs_added: Vec<String>,
        /// Existing members theirs changed substantively.
        theirs_changed: Vec<String>,
    },
}

#[derive(Debug, Clone)]
pub struct RelatedEntity {
    pub name: String,
    pub entity_type: String,
    pub file_path: String,
}

/// Validate a merge result for semantic risks.
///
/// Takes the set of entity names that were auto-merged (modified by one or both
/// branches) and uses the entity dependency graph to check for cross-references
/// between modified entities.
pub fn validate_merge(
    repo_root: &std::path::Path,
    file_paths: &[String],
    modified_entities: &[ModifiedEntity],
    registry: &ParserRegistry,
) -> Vec<SemanticWarning> {
    if modified_entities.len() < 2 {
        return vec![];
    }

    // Build the dependency graph
    let (graph, _entities) = EntityGraph::build(repo_root, file_paths, registry);

    // Build a set of modified entity IDs for quick lookup.
    //
    // Sorted, and `min_by` on the id rather than `find`: `graph.entities` is a
    // `HashMap`, so both "which of two same-named entities in one file wins"
    // and "in what order are the warnings emitted" would otherwise be decided
    // by the process's random hash seed, and the warning list is user-facing
    // output that has to be deterministic.
    let modified_ids: BTreeSet<String> = modified_entities
        .iter()
        .filter_map(|me| {
            graph
                .entities
                .values()
                .filter(|e| e.name == me.name && e.file_path == me.file_path)
                .min_by(|a, b| a.id.cmp(&b.id))
                .map(|e| e.id.clone())
        })
        .collect();

    let mut warnings = Vec::new();

    for entity_id in &modified_ids {
        let Some(entity) = graph.entities.get(entity_id) else {
            continue;
        };

        // Check: does this entity depend on another modified entity?
        let deps = graph.get_dependencies(entity_id);
        for dep in &deps {
            if modified_ids.contains(&dep.id) {
                warnings.push(SemanticWarning {
                    entity_name: entity.name.clone(),
                    entity_type: entity.entity_type.clone(),
                    file_path: entity.file_path.clone(),
                    kind: WarningKind::DependencyAlsoModified,
                    related: vec![RelatedEntity {
                        name: dep.name.clone(),
                        entity_type: dep.entity_type.clone(),
                        file_path: dep.file_path.clone(),
                    }],
                });
            }
        }

        // Check: is this entity depended on by another modified entity?
        let dependents = graph.get_dependents(entity_id);
        for dep in &dependents {
            if modified_ids.contains(&dep.id) && dep.id != *entity_id {
                // Only add if we haven't already covered this from the other direction
                let already_covered = warnings.iter().any(|w| {
                    matches!(&w.kind, WarningKind::DependencyAlsoModified)
                        && w.entity_name == dep.name
                        && w.related.iter().any(|r| r.name == entity.name)
                });
                if !already_covered {
                    warnings.push(SemanticWarning {
                        entity_name: entity.name.clone(),
                        entity_type: entity.entity_type.clone(),
                        file_path: entity.file_path.clone(),
                        kind: WarningKind::DependentAlsoModified,
                        related: vec![RelatedEntity {
                            name: dep.name.clone(),
                            entity_type: dep.entity_type.clone(),
                            file_path: dep.file_path.clone(),
                        }],
                    });
                }
            }
        }
    }

    warnings
}

/// A modified entity descriptor, used as input to validation.
#[derive(Debug, Clone)]
pub struct ModifiedEntity {
    pub name: String,
    pub file_path: String,
}

impl std::fmt::Display for SemanticWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            WarningKind::DependencyAlsoModified => {
                write!(
                    f,
                    "warning: {} `{}` was modified and references {} `{}` which was also modified",
                    self.entity_type,
                    self.entity_name,
                    self.related[0].entity_type,
                    self.related[0].name,
                )
            }
            WarningKind::DependentAlsoModified => {
                write!(
                    f,
                    "warning: {} `{}` was modified and is used by {} `{}` which was also modified",
                    self.entity_type,
                    self.entity_name,
                    self.related[0].entity_type,
                    self.related[0].name,
                )
            }
            WarningKind::ParseFailedAfterMerge => {
                write!(
                    f,
                    "warning: merged output for `{}` failed to parse — result may be syntactically broken",
                    self.file_path,
                )
            }
            WarningKind::CompositionLicensed {
                ours_binds,
                theirs_binds,
            } => {
                write!(
                    f,
                    "resolved: {} `{}` — both sides inserted into one slot and their read/write sets are disjoint (ours binds {}; theirs binds {})",
                    self.entity_type,
                    self.entity_name,
                    ours_binds.join(", "),
                    theirs_binds.join(", "),
                )
            }
            WarningKind::ConflictFrameDuplicate {
                line,
                found,
                allowed,
            } => {
                write!(
                    f,
                    "warning: outside every conflict marker, `{}` states {:?} {}x — no union of the two sides states it more than {}x, so every resolution of this file carries the extra copy",
                    self.file_path, line, found, allowed,
                )
            }
            WarningKind::SiblingCoChange {
                ours_added,
                ours_changed,
                theirs_added,
                theirs_changed,
            } => {
                write!(
                    f,
                    "advisory: cleanly merged {} `{}` — both sides changed different siblings ({}; {}); confirm they are meant to coexist",
                    self.entity_type,
                    self.entity_name,
                    co_change_side_phrase("ours", ours_added, ours_changed),
                    co_change_side_phrase("theirs", theirs_added, theirs_changed),
                )
            }
        }
    }
}

/// One side of a sibling co-change, phrased once for every channel that renders
/// it: `ours added `a`, `b`, changed `c``. The single owner of the wording, so
/// the merge driver's warning line, the findings suggestion and `weave check`'s
/// review line cannot drift apart.
pub fn co_change_side_phrase(side: &str, added: &[String], changed: &[String]) -> String {
    let list = |ms: &[String]| {
        ms.iter()
            .map(|m| format!("`{m}`"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    match (added.is_empty(), changed.is_empty()) {
        (false, false) => format!("{side} added {}, changed {}", list(added), list(changed)),
        (false, true) => format!("{side} added {}", list(added)),
        (true, false) => format!("{side} changed {}", list(changed)),
        // Not reachable: the advisory fires only when this side touched a
        // member. Stated rather than panicked, so a future caller cannot make
        // it a crash.
        (true, true) => format!("{side} changed nothing"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn setup_test_repo() -> TempDir {
        let dir = TempDir::new().unwrap();

        // Create a TS file where function A calls function B
        let ts_content = r#"export function validateInput(input: string): boolean {
    return input.length > 0;
}

export function processData(input: string): string {
    if (!validateInput(input)) {
        throw new Error("invalid");
    }
    return input.toUpperCase();
}

export function unrelated(): number {
    return 42;
}
"#;
        let ts_path = dir.path().join("module.ts");
        let mut f = std::fs::File::create(&ts_path).unwrap();
        f.write_all(ts_content.as_bytes()).unwrap();

        dir
    }

    #[test]
    fn test_no_warnings_single_entity() {
        let dir = setup_test_repo();
        let registry = sem_core::parser::plugins::create_default_registry();
        let warnings = validate_merge(
            dir.path(),
            &["module.ts".to_string()],
            &[ModifiedEntity {
                name: "unrelated".to_string(),
                file_path: "module.ts".to_string(),
            }],
            &registry,
        );
        assert!(warnings.is_empty(), "Single entity should have no warnings");
    }

    #[test]
    fn test_warning_when_caller_and_callee_both_modified() {
        let dir = setup_test_repo();
        let registry = sem_core::parser::plugins::create_default_registry();
        let warnings = validate_merge(
            dir.path(),
            &["module.ts".to_string()],
            &[
                ModifiedEntity {
                    name: "validateInput".to_string(),
                    file_path: "module.ts".to_string(),
                },
                ModifiedEntity {
                    name: "processData".to_string(),
                    file_path: "module.ts".to_string(),
                },
            ],
            &registry,
        );
        assert!(
            !warnings.is_empty(),
            "Should warn when caller and callee both modified. Warnings: {:?}",
            warnings
        );
        // processData calls validateInput, so there should be a warning
        let has_dep_warning = warnings.iter().any(|w| {
            w.entity_name == "processData"
                && matches!(w.kind, WarningKind::DependencyAlsoModified)
                && w.related.iter().any(|r| r.name == "validateInput")
        });
        assert!(
            has_dep_warning,
            "Should warn that processData depends on validateInput"
        );
    }

    #[test]
    fn test_no_warning_unrelated_entities() {
        let dir = setup_test_repo();
        let registry = sem_core::parser::plugins::create_default_registry();
        let warnings = validate_merge(
            dir.path(),
            &["module.ts".to_string()],
            &[
                ModifiedEntity {
                    name: "validateInput".to_string(),
                    file_path: "module.ts".to_string(),
                },
                ModifiedEntity {
                    name: "unrelated".to_string(),
                    file_path: "module.ts".to_string(),
                },
            ],
            &registry,
        );
        // validateInput and unrelated don't reference each other
        let cross_warnings: Vec<_> = warnings
            .iter()
            .filter(|w| {
                (w.entity_name == "validateInput"
                    && w.related.iter().any(|r| r.name == "unrelated"))
                    || (w.entity_name == "unrelated"
                        && w.related.iter().any(|r| r.name == "validateInput"))
            })
            .collect();
        assert!(
            cross_warnings.is_empty(),
            "Unrelated entities should not trigger cross-warnings"
        );
    }
}
