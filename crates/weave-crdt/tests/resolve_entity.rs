//! Tests for the address-based entity resolver ([`weave_crdt::resolve_entity`]).
//!
//! Regression coverage for the old name-only `resolve_entity_id`, which
//! silently picked the first same-named entity the parser extracted.

use sem_core::parser::plugins::create_default_registry;
use weave_crdt::{resolve_entity, resolve_entity_or_error, EntityAddress, Resolution};

/// Fixture: two classes in one file, each with a method called `run`.
const DUP_METHODS_TS: &str = r#"class Animal {
  run(): string {
    return "animal runs";
  }
}

class Robot {
  run(): string {
    return "robot runs";
  }
}
"#;

/// Fixture: two top-level functions with the same name (an overload-style
/// duplicate).
const DUP_OVERLOADS_PY: &str = r#"def process(data):
    return data

def process(data):
    return data.upper()
"#;

/// Fixture: a top-level function and a class method share the name `run` —
/// same name, DIFFERENT entity kinds, no `parent_name` in common. Exercises
/// disambiguation by `entity_type` alone, distinct from the same-kind
/// duplicates in [`DUP_METHODS_TS`].
const DUP_KINDS_TS: &str = r#"function run(): string {
  return "top-level run";
}

class Robot {
  run(): string {
    return "robot runs";
  }
}
"#;

fn resolve(address: &EntityAddress<'_>) -> Resolution {
    let registry = create_default_registry();
    resolve_entity(DUP_METHODS_TS, "dup_methods.ts", &registry, address)
}

// ── Name-only resolution of duplicated names is AMBIGUOUS ──

#[test]
fn name_only_resolve_of_duplicated_method_is_ambiguous() {
    let result = resolve(&EntityAddress::by_name("run"));

    let Resolution::Ambiguous(candidates) = result else {
        panic!("expected Ambiguous, got {result:?}");
    };
    assert_eq!(candidates.len(), 2);

    // Candidates are listed in file order with 0-based ordinals.
    assert_eq!(candidates[0].ordinal, 0);
    assert_eq!(candidates[1].ordinal, 1);
    assert_eq!(
        candidates[0].entity_id,
        "dup_methods.ts::class::Animal::run"
    );
    assert_eq!(candidates[1].entity_id, "dup_methods.ts::class::Robot::run");

    assert_eq!(candidates[0].entity_type, "method");
    assert_eq!(
        candidates[0].parent.as_deref(),
        Some("Animal"),
        "parent name should be derived from parent_id"
    );
    assert_eq!(candidates[1].parent.as_deref(), Some("Robot"));

    // Snippets identify the candidates' source.
    assert!(candidates[0].snippet.contains("run"));
    assert!(candidates[1].snippet.contains("run"));
}

#[test]
fn name_only_resolve_of_duplicate_top_level_functions_is_ambiguous() {
    let registry = create_default_registry();
    let result = resolve_entity(
        DUP_OVERLOADS_PY,
        "dup_overloads.py",
        &registry,
        &EntityAddress::by_name("process"),
    );

    let Resolution::Ambiguous(candidates) = result else {
        panic!("expected Ambiguous, got {result:?}");
    };
    assert_eq!(candidates.len(), 2);
    assert_ne!(candidates[0].entity_id, candidates[1].entity_id);
}

// ── Disambiguation via parent_name ──

#[test]
fn name_and_parent_resolves_the_correct_entity() {
    let result = resolve(&EntityAddress::by_name("run").with_parent("Robot"));

    assert_eq!(
        result,
        Resolution::Resolved("dup_methods.ts::class::Robot::run".to_string())
    );
}

#[test]
fn name_and_other_parent_resolves_the_first_classs_method() {
    let result = resolve(&EntityAddress::by_name("run").with_parent("Animal"));

    assert_eq!(
        result,
        Resolution::Resolved("dup_methods.ts::class::Animal::run".to_string())
    );
}

#[test]
fn unknown_parent_is_not_found() {
    let result = resolve(&EntityAddress::by_name("run").with_parent("Spaceship"));
    assert_eq!(result, Resolution::NotFound);
}

// ── Disambiguation via ordinal ──

#[test]
fn name_and_ordinal_resolves_the_correct_entity() {
    let first = resolve(&EntityAddress::by_name("run").with_ordinal(0));
    let second = resolve(&EntityAddress::by_name("run").with_ordinal(1));

    assert_eq!(
        first,
        Resolution::Resolved("dup_methods.ts::class::Animal::run".to_string())
    );
    assert_eq!(
        second,
        Resolution::Resolved("dup_methods.ts::class::Robot::run".to_string())
    );
}

#[test]
fn ordinal_out_of_range_is_not_found() {
    let result = resolve(&EntityAddress::by_name("run").with_ordinal(2));
    assert_eq!(result, Resolution::NotFound);
}

// ── Disambiguation via entity_type ──

#[test]
fn entity_type_filter_narrows_but_can_still_be_ambiguous() {
    // Both duplicates are methods: type alone doesn't disambiguate...
    let result = resolve(&EntityAddress::by_name("run").with_type("method"));
    assert!(matches!(result, Resolution::Ambiguous(ref c) if c.len() == 2));

    // ...but it must not leak across kinds: no CLASS is named `run`.
    let result = resolve(&EntityAddress::by_name("run").with_type("class"));
    assert_eq!(result, Resolution::NotFound);
}

#[test]
fn entity_type_alone_disambiguates_a_function_from_a_same_named_method() {
    // Adversarial case: `run` names both a top-level function and an
    // unrelated class's method. Unlike DUP_METHODS_TS (both candidates the
    // same kind), `entity_type` alone is sufficient here — no parent_name
    // needed.
    let registry = create_default_registry();

    let function_result = resolve_entity(
        DUP_KINDS_TS,
        "dup_kinds.ts",
        &registry,
        &EntityAddress::by_name("run").with_type("function"),
    );
    assert_eq!(
        function_result,
        Resolution::Resolved("dup_kinds.ts::function::run".to_string())
    );

    let method_result = resolve_entity(
        DUP_KINDS_TS,
        "dup_kinds.ts",
        &registry,
        &EntityAddress::by_name("run").with_type("method"),
    );
    assert_eq!(
        method_result,
        Resolution::Resolved("dup_kinds.ts::class::Robot::run".to_string())
    );

    // Without a type filter, the function and method both survive: ambiguous.
    let unfiltered_result = resolve_entity(
        DUP_KINDS_TS,
        "dup_kinds.ts",
        &registry,
        &EntityAddress::by_name("run"),
    );
    assert!(matches!(unfiltered_result, Resolution::Ambiguous(ref c) if c.len() == 2));
}

// ── NotFound ──

#[test]
fn unknown_name_is_not_found() {
    let result = resolve(&EntityAddress::by_name("nonexistent"));
    assert_eq!(result, Resolution::NotFound);
}

#[test]
fn unknown_file_type_is_not_found() {
    let registry = create_default_registry();
    let result = resolve_entity(
        DUP_METHODS_TS,
        "dup_methods.unknown_extension",
        &registry,
        &EntityAddress::by_name("run"),
    );
    assert_eq!(result, Resolution::NotFound);
}

// ── Error rendering (shared by MCP + CLI) ──

#[test]
fn error_message_lists_candidates_and_tells_caller_what_to_add() {
    let registry = create_default_registry();
    let address = EntityAddress::by_name("run");
    let err = resolve_entity_or_error(DUP_METHODS_TS, "dup_methods.ts", &registry, &address)
        .expect_err("ambiguous resolution must be an error");

    assert!(err.contains("ambiguous"), "message: {err}");
    assert!(err.contains("[0]"), "message: {err}");
    assert!(err.contains("[1]"), "message: {err}");
    assert!(
        err.contains("Animal") && err.contains("Robot"),
        "message: {err}"
    );
    assert!(
        err.contains("parent_name") && err.contains("ordinal"),
        "error should tell the caller which field to add: {err}"
    );
}

#[test]
fn error_message_for_unknown_name_says_not_found() {
    let registry = create_default_registry();
    let err = resolve_entity_or_error(
        DUP_METHODS_TS,
        "dup_methods.ts",
        &registry,
        &EntityAddress::by_name("nonexistent"),
    )
    .expect_err("unknown name must be an error");

    assert!(err.contains("not found"), "message: {err}");
}

#[test]
fn resolved_entity_passes_through_or_error_helper() {
    let registry = create_default_registry();
    let id = resolve_entity_or_error(
        DUP_METHODS_TS,
        "dup_methods.ts",
        &registry,
        &EntityAddress::by_name("run").with_parent("Animal"),
    )
    .unwrap();
    assert_eq!(id, "dup_methods.ts::class::Animal::run");
}
