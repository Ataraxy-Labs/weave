use weave_crdt::{
    get_entity_content, get_entity_status, record_modification, resolve_entity_conflict,
    set_entity_conflict, update_entity_content, upsert_entity, EntityStateDoc, VersionVector,
};

fn setup() -> EntityStateDoc {
    EntityStateDoc::new_memory().unwrap()
}

fn setup_with_entity(entity_id: &str, name: &str) -> EntityStateDoc {
    let mut state = setup();
    upsert_entity(&mut state, entity_id, name, "function", "src/lib.rs", "hash0").unwrap();
    state
}

// ── Content operations ──

#[test]
fn test_store_and_read_content() {
    let mut state = setup_with_entity("eid1", "my_func");
    update_entity_content(&mut state, "agent-1", "eid1", "fn my_func() { 42 }", "h1").unwrap();

    let status = get_entity_content(&state, "eid1").unwrap();
    assert_eq!(status.content, "fn my_func() { 42 }");
    assert_eq!(status.content_hash, "h1");
    assert_eq!(status.merge_state, "clean");
}

#[test]
fn test_vv_increment_on_content_update() {
    let mut state = setup_with_entity("eid1", "my_func");
    update_entity_content(&mut state, "agent-1", "eid1", "v1", "h1").unwrap();
    update_entity_content(&mut state, "agent-1", "eid1", "v2", "h2").unwrap();

    let status = get_entity_content(&state, "eid1").unwrap();
    assert_eq!(status.version_vector.get("agent-1"), 2);
    assert_eq!(status.version_vector.total(), 2);
}

#[test]
fn test_base_content_tracking() {
    let mut state = setup_with_entity("eid1", "my_func");

    // First update sets content, base_content is empty initially
    update_entity_content(&mut state, "agent-1", "eid1", "version_1", "h1").unwrap();
    let s1 = get_entity_content(&state, "eid1").unwrap();
    // base_content should be empty since the entity had empty content before
    assert_eq!(s1.base_content, "");

    // Second update: base_content should now be set to "version_1"
    update_entity_content(&mut state, "agent-1", "eid1", "version_2", "h2").unwrap();
    let s2 = get_entity_content(&state, "eid1").unwrap();
    assert_eq!(s2.content, "version_2");
    assert_eq!(s2.base_content, "version_1");
}

#[test]
fn test_multiple_agents_content_update() {
    let mut state = setup_with_entity("eid1", "my_func");

    update_entity_content(&mut state, "agent-1", "eid1", "a1_content", "h1").unwrap();
    update_entity_content(&mut state, "agent-2", "eid1", "a2_content", "h2").unwrap();

    let status = get_entity_content(&state, "eid1").unwrap();
    assert_eq!(status.version_vector.get("agent-1"), 1);
    assert_eq!(status.version_vector.get("agent-2"), 1);
    assert_eq!(status.version_vector.total(), 2);
    // Last writer wins in Automerge
    assert_eq!(status.content, "a2_content");
}

#[test]
fn test_content_update_clears_conflict_state() {
    let mut state = setup_with_entity("eid1", "my_func");

    // Set up content first so entity has some state
    update_entity_content(&mut state, "agent-1", "eid1", "orig", "h0").unwrap();

    // Manually set conflict state via the internal API
    set_entity_conflict(
        &mut state,
        "eid1",
        "ours_content",
        "theirs_content",
        "base_content",
        "agent-1",
        "agent-2",
    )
    .unwrap();

    let status = get_entity_content(&state, "eid1").unwrap();
    assert_eq!(status.merge_state, "conflict");

    // Writing new content should clear the conflict
    update_entity_content(&mut state, "agent-1", "eid1", "resolved", "h_resolved").unwrap();
    let status2 = get_entity_content(&state, "eid1").unwrap();
    assert_eq!(status2.merge_state, "clean");
    assert_eq!(status2.content, "resolved");
}

// ── Conflict resolution ──

#[test]
fn test_resolve_entity_conflict() {
    let mut state = setup_with_entity("eid1", "my_func");
    update_entity_content(&mut state, "agent-1", "eid1", "orig", "h0").unwrap();

    // Set conflict
    set_entity_conflict(
        &mut state,
        "eid1",
        "ours_content",
        "theirs_content",
        "base_content",
        "agent-1",
        "agent-2",
    )
    .unwrap();

    // Resolve
    resolve_entity_conflict(&mut state, "agent-3", "eid1", "merged_content", "hm").unwrap();

    let status = get_entity_content(&state, "eid1").unwrap();
    assert_eq!(status.merge_state, "clean");
    assert_eq!(status.content, "merged_content");
    assert!(status.conflict_ours.is_none());
    assert!(status.conflict_theirs.is_none());
    assert_eq!(status.version_vector.get("agent-3"), 1);
}

#[test]
fn test_resolve_non_conflict_fails() {
    let mut state = setup_with_entity("eid1", "my_func");
    update_entity_content(&mut state, "agent-1", "eid1", "content", "h1").unwrap();

    let result = resolve_entity_conflict(&mut state, "agent-1", "eid1", "new", "h2");
    assert!(result.is_err());
}

// ── Version vector in record_modification ──

#[test]
fn test_record_modification_uses_vv() {
    let mut state = setup_with_entity("eid1", "my_func");

    record_modification(&mut state, "agent-1", "eid1", "h1").unwrap();
    record_modification(&mut state, "agent-2", "eid1", "h2").unwrap();
    record_modification(&mut state, "agent-1", "eid1", "h3").unwrap();

    let status = get_entity_status(&state, "eid1").unwrap();
    assert_eq!(status.version_vector.get("agent-1"), 2);
    assert_eq!(status.version_vector.get("agent-2"), 1);
    assert_eq!(status.version, 3); // total
}

// ── Migration tests ──

#[test]
fn test_new_doc_has_schema_v2() {
    let mut state = setup();
    // New docs should have v2 structures initialized
    // Verify indirectly by syncing (which uses file_entity_order and file_interstitials)
    // If they don't exist, upsert would fail
    upsert_entity(&mut state, "eid1", "func", "function", "test.rs", "h1").unwrap();
    let status = get_entity_status(&state, "eid1").unwrap();
    assert_eq!(status.merge_state, "clean");
}

#[test]
fn test_upsert_creates_v2_fields() {
    let mut state = setup();
    upsert_entity(&mut state, "eid1", "func", "function", "lib.rs", "h1").unwrap();

    let status = get_entity_status(&state, "eid1").unwrap();
    assert_eq!(status.version, 0);
    assert_eq!(status.merge_state, "clean");
    assert!(status.version_vector.is_empty());

    let content_status = get_entity_content(&state, "eid1").unwrap();
    assert_eq!(content_status.content, "");
    assert_eq!(content_status.base_content, "");
}

#[test]
fn test_migration_preserves_existing_data() {
    let mut state = setup_with_entity("eid1", "my_func");
    record_modification(&mut state, "agent-1", "eid1", "modified_hash").unwrap();

    let status = get_entity_status(&state, "eid1").unwrap();
    assert_eq!(status.name, "my_func");
    assert_eq!(status.content_hash, "modified_hash");
    assert!(status.version >= 1);
}

#[test]
fn test_migration_idempotent() {
    let path = std::env::temp_dir().join("weave_test_migration_idempotent.automerge");
    // Clean up from previous run if needed
    let _ = std::fs::remove_file(&path);

    // Create and populate a doc via open (saves to disk)
    {
        let mut state = EntityStateDoc::open(&path).unwrap();
        upsert_entity(&mut state, "eid1", "my_func", "function", "src/lib.rs", "h0").unwrap();
        update_entity_content(&mut state, "agent-1", "eid1", "content_v1", "h1").unwrap();
        state.save().unwrap();
    }

    // Reopen (triggers migrate_if_needed)
    let state2 = EntityStateDoc::open(&path).unwrap();
    let status = get_entity_content(&state2, "eid1").unwrap();
    assert_eq!(status.content, "content_v1");
    assert_eq!(status.version_vector.get("agent-1"), 1);

    // Clean up
    let _ = std::fs::remove_file(&path);
}

// ── Version vector concurrent detection ──

#[test]
fn test_concurrent_version_vectors_detected() {
    let mut vv_a = VersionVector::new();
    vv_a.increment("agent-1");
    vv_a.increment("agent-1");

    let mut vv_b = VersionVector::new();
    vv_b.increment("agent-2");

    // Neither dominates: concurrent
    assert!(vv_a.partial_cmp(&vv_b).is_none());

    // After merge, both are dominated
    let mut merged = vv_a.clone();
    merged.merge(&vv_b);
    assert_eq!(merged.get("agent-1"), 2);
    assert_eq!(merged.get("agent-2"), 1);
    assert_eq!(merged.total(), 3);
}

// ── Entity status includes new fields ──

#[test]
fn test_entity_status_includes_vv_and_merge_state() {
    let mut state = setup_with_entity("eid1", "my_func");
    update_entity_content(&mut state, "agent-1", "eid1", "content", "h1").unwrap();

    let status = get_entity_status(&state, "eid1").unwrap();
    assert_eq!(status.version_vector.get("agent-1"), 1);
    assert_eq!(status.merge_state, "clean");
    assert_eq!(status.version, 1);
}
