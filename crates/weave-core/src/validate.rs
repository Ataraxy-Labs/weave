//! Post-merge semantic validation.
//!
//! After a syntactically clean merge, entities may still be semantically
//! incompatible. For example, if function A calls function B and both were
//! modified by different agents, the merge may succeed syntactically but B's
//! contract (return type, parameters, side effects) may have changed in ways
//! that break A.
//!
//! This module flags such "semantic risk" cases as warnings, not errors.
//! The merge still succeeds — this is advisory.

use std::collections::HashSet;

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

    // Build a set of modified entity IDs for quick lookup
    let modified_ids: HashSet<String> = modified_entities
        .iter()
        .filter_map(|me| {
            graph
                .entities
                .values()
                .find(|e| e.name == me.name && e.file_path == me.file_path)
                .map(|e| e.id.clone())
        })
        .collect();

    let mut warnings = Vec::new();

    for entity_id in &modified_ids {
        let entity = match graph.entities.get(entity_id) {
            Some(e) => e,
            None => continue,
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
        }
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
