use weave_crdt::{
    agent_heartbeat, claim_entity, cleanup_stale_agents, detect_potential_conflicts,
    get_agent_status, get_entities_for_file, get_entity_status, record_modification,
    register_agent, release_entity, set_agent_last_seen, upsert_entity, ClaimResult,
    EntityStateDoc,
};

fn setup_state_with_entity(entity_id: &str, name: &str, file_path: &str) -> EntityStateDoc {
    let mut state = EntityStateDoc::new_memory().unwrap();
    upsert_entity(
        &mut state, entity_id, name, "function", file_path, "hash123",
    )
    .unwrap();
    state
}

// ── Claim / Release tests ──

#[test]
fn test_claim_entity_success() {
    let mut state = setup_state_with_entity("eid1", "my_func", "src/lib.rs");
    let result = claim_entity(&mut state, "agent-1", "eid1").unwrap();
    assert_eq!(result, ClaimResult::Claimed);
}

#[test]
fn test_claim_entity_already_owned_by_self() {
    let mut state = setup_state_with_entity("eid1", "my_func", "src/lib.rs");
    claim_entity(&mut state, "agent-1", "eid1").unwrap();
    let result = claim_entity(&mut state, "agent-1", "eid1").unwrap();
    assert_eq!(result, ClaimResult::AlreadyOwnedBySelf);
}

#[test]
fn test_claim_entity_already_claimed_by_other() {
    let mut state = setup_state_with_entity("eid1", "my_func", "src/lib.rs");
    claim_entity(&mut state, "agent-1", "eid1").unwrap();
    let result = claim_entity(&mut state, "agent-2", "eid1").unwrap();
    assert_eq!(
        result,
        ClaimResult::AlreadyClaimed {
            by: "agent-1".into()
        }
    );
}

#[test]
fn test_claim_nonexistent_entity() {
    let mut state = EntityStateDoc::new_memory().unwrap();
    let result = claim_entity(&mut state, "agent-1", "nonexistent");
    assert!(result.is_err());
}

#[test]
fn test_release_entity() {
    let mut state = setup_state_with_entity("eid1", "my_func", "src/lib.rs");
    claim_entity(&mut state, "agent-1", "eid1").unwrap();

    release_entity(&mut state, "agent-1", "eid1").unwrap();

    // Should be claimable again
    let result = claim_entity(&mut state, "agent-2", "eid1").unwrap();
    assert_eq!(result, ClaimResult::Claimed);
}

#[test]
fn test_release_by_non_owner_is_noop() {
    let mut state = setup_state_with_entity("eid1", "my_func", "src/lib.rs");
    claim_entity(&mut state, "agent-1", "eid1").unwrap();

    // Agent-2 tries to release — should be a no-op
    release_entity(&mut state, "agent-2", "eid1").unwrap();

    // Agent-1 should still own it
    let result = claim_entity(&mut state, "agent-2", "eid1").unwrap();
    assert_eq!(
        result,
        ClaimResult::AlreadyClaimed {
            by: "agent-1".into()
        }
    );
}

// ── Entity status tests ──

#[test]
fn test_get_entity_status() {
    let mut state = setup_state_with_entity("eid1", "my_func", "src/lib.rs");
    claim_entity(&mut state, "agent-1", "eid1").unwrap();

    let status = get_entity_status(&state, "eid1").unwrap();
    assert_eq!(status.name, "my_func");
    assert_eq!(status.entity_type, "function");
    assert_eq!(status.claimed_by, Some("agent-1".to_string()));
}

#[test]
fn test_get_entities_for_file() {
    let mut state = EntityStateDoc::new_memory().unwrap();
    upsert_entity(&mut state, "e1", "func_a", "function", "src/lib.rs", "h1").unwrap();
    upsert_entity(&mut state, "e2", "func_b", "function", "src/lib.rs", "h2").unwrap();
    upsert_entity(&mut state, "e3", "func_c", "function", "src/other.rs", "h3").unwrap();

    let entities = get_entities_for_file(&state, "src/lib.rs").unwrap();
    assert_eq!(entities.len(), 2);
    let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"func_a"));
    assert!(names.contains(&"func_b"));
}

// ── Record modification tests ──

#[test]
fn test_record_modification() {
    let mut state = setup_state_with_entity("eid1", "my_func", "src/lib.rs");
    record_modification(&mut state, "agent-1", "eid1", "newhash").unwrap();

    let status = get_entity_status(&state, "eid1").unwrap();
    assert_eq!(status.content_hash, "newhash");
    assert_eq!(status.last_modified_by, Some("agent-1".to_string()));
    assert_eq!(status.version, 1);
}

#[test]
fn test_record_modification_increments_version() {
    let mut state = setup_state_with_entity("eid1", "my_func", "src/lib.rs");
    record_modification(&mut state, "agent-1", "eid1", "h1").unwrap();
    record_modification(&mut state, "agent-2", "eid1", "h2").unwrap();

    let status = get_entity_status(&state, "eid1").unwrap();
    assert_eq!(status.version, 2);
    assert_eq!(status.last_modified_by, Some("agent-2".to_string()));
}

// ── Agent tests ──

#[test]
fn test_register_and_get_agent() {
    let mut state = EntityStateDoc::new_memory().unwrap();
    register_agent(&mut state, "agent-1", "Agent One", "feature-branch").unwrap();

    let status = get_agent_status(&state, "agent-1").unwrap();
    assert_eq!(status.name, "Agent One");
    assert_eq!(status.branch, "feature-branch");
    assert_eq!(status.status, "active");
    assert!(status.working_on.is_empty());
}

#[test]
fn test_agent_heartbeat() {
    let mut state = EntityStateDoc::new_memory().unwrap();
    register_agent(&mut state, "agent-1", "Agent One", "main").unwrap();

    agent_heartbeat(
        &mut state,
        "agent-1",
        &["eid1".to_string(), "eid2".to_string()],
    )
    .unwrap();

    let status = get_agent_status(&state, "agent-1").unwrap();
    assert_eq!(status.working_on, vec!["eid1", "eid2"]);
}

#[test]
fn test_agent_not_found() {
    let state = EntityStateDoc::new_memory().unwrap();
    let result = get_agent_status(&state, "nonexistent");
    assert!(result.is_err());
}

// ── Stale cleanup tests ──

#[test]
fn test_cleanup_stale_agents() {
    let mut state = setup_state_with_entity("eid1", "my_func", "src/lib.rs");
    register_agent(&mut state, "agent-1", "Agent One", "main").unwrap();
    claim_entity(&mut state, "agent-1", "eid1").unwrap();

    // Set agent's last_seen to 0 (very old)
    set_agent_last_seen(&mut state, "agent-1", 0).unwrap();

    let stale = cleanup_stale_agents(&mut state, 1000).unwrap();
    assert_eq!(stale, vec!["agent-1"]);

    // Entity should be released
    let status = get_entity_status(&state, "eid1").unwrap();
    assert!(status.claimed_by.is_none());

    // Agent should be marked stale
    let agent = get_agent_status(&state, "agent-1").unwrap();
    assert_eq!(agent.status, "stale");
}

// ── Conflict detection tests ──

#[test]
fn test_detect_no_conflicts() {
    let mut state = setup_state_with_entity("eid1", "func_a", "src/lib.rs");
    register_agent(&mut state, "agent-1", "A1", "main").unwrap();
    register_agent(&mut state, "agent-2", "A2", "feature").unwrap();

    // Different entities
    upsert_entity(&mut state, "eid2", "func_b", "function", "src/lib.rs", "h2").unwrap();
    agent_heartbeat(&mut state, "agent-1", &["eid1".to_string()]).unwrap();
    agent_heartbeat(&mut state, "agent-2", &["eid2".to_string()]).unwrap();

    let conflicts = detect_potential_conflicts(&state).unwrap();
    assert!(conflicts.is_empty());
}

#[test]
fn test_detect_potential_conflict() {
    let mut state = setup_state_with_entity("eid1", "func_a", "src/lib.rs");
    register_agent(&mut state, "agent-1", "A1", "main").unwrap();
    register_agent(&mut state, "agent-2", "A2", "feature").unwrap();

    // Both working on same entity
    agent_heartbeat(&mut state, "agent-1", &["eid1".to_string()]).unwrap();
    agent_heartbeat(&mut state, "agent-2", &["eid1".to_string()]).unwrap();

    let conflicts = detect_potential_conflicts(&state).unwrap();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].entity_id, "eid1");
    assert!(conflicts[0].agents.contains(&"agent-1".to_string()));
    assert!(conflicts[0].agents.contains(&"agent-2".to_string()));
}

// ── Upsert tests ──

#[test]
fn test_upsert_entity_create() {
    let mut state = EntityStateDoc::new_memory().unwrap();
    upsert_entity(
        &mut state,
        "eid1",
        "my_func",
        "function",
        "src/lib.rs",
        "hash1",
    )
    .unwrap();

    let status = get_entity_status(&state, "eid1").unwrap();
    assert_eq!(status.name, "my_func");
    assert_eq!(status.entity_type, "function");
    assert_eq!(status.content_hash, "hash1");
    assert_eq!(status.version, 0);
}

#[test]
fn test_upsert_entity_update_preserves_claims() {
    let mut state = EntityStateDoc::new_memory().unwrap();
    upsert_entity(
        &mut state,
        "eid1",
        "my_func",
        "function",
        "src/lib.rs",
        "hash1",
    )
    .unwrap();
    claim_entity(&mut state, "agent-1", "eid1").unwrap();

    // Upsert again with new content hash
    upsert_entity(
        &mut state,
        "eid1",
        "my_func",
        "function",
        "src/lib.rs",
        "hash2",
    )
    .unwrap();

    let status = get_entity_status(&state, "eid1").unwrap();
    assert_eq!(status.content_hash, "hash2");
    assert_eq!(status.claimed_by, Some("agent-1".to_string())); // Claim preserved
}

// ── Two-agent integration test ──

#[test]
fn test_two_agents_different_entities_no_conflict() {
    let mut state = EntityStateDoc::new_memory().unwrap();

    // Create two entities
    upsert_entity(
        &mut state,
        "f::process_data",
        "process_data",
        "function",
        "src/lib.rs",
        "h1",
    )
    .unwrap();
    upsert_entity(
        &mut state,
        "f::validate_input",
        "validate_input",
        "function",
        "src/lib.rs",
        "h2",
    )
    .unwrap();

    // Register two agents
    register_agent(&mut state, "claude-1", "Claude-1", "feature-a").unwrap();
    register_agent(&mut state, "claude-2", "Claude-2", "feature-b").unwrap();

    // Each claims a different entity
    let r1 = claim_entity(&mut state, "claude-1", "f::process_data").unwrap();
    let r2 = claim_entity(&mut state, "claude-2", "f::validate_input").unwrap();
    assert_eq!(r1, ClaimResult::Claimed);
    assert_eq!(r2, ClaimResult::Claimed);

    // No conflicts
    let conflicts = detect_potential_conflicts(&state).unwrap();
    assert!(conflicts.is_empty());
}

#[test]
fn test_two_agents_same_entity_warning() {
    let mut state = EntityStateDoc::new_memory().unwrap();

    upsert_entity(
        &mut state,
        "f::process_data",
        "process_data",
        "function",
        "src/lib.rs",
        "h1",
    )
    .unwrap();

    register_agent(&mut state, "claude-1", "Claude-1", "feature-a").unwrap();
    register_agent(&mut state, "claude-2", "Claude-2", "feature-b").unwrap();

    // Agent 1 claims
    let r1 = claim_entity(&mut state, "claude-1", "f::process_data").unwrap();
    assert_eq!(r1, ClaimResult::Claimed);

    // Agent 2 tries to claim — gets warning
    let r2 = claim_entity(&mut state, "claude-2", "f::process_data").unwrap();
    assert_eq!(
        r2,
        ClaimResult::AlreadyClaimed {
            by: "claude-1".into()
        }
    );

    // Both report working on it
    agent_heartbeat(&mut state, "claude-1", &["f::process_data".to_string()]).unwrap();
    agent_heartbeat(&mut state, "claude-2", &["f::process_data".to_string()]).unwrap();

    // Conflict detected
    let conflicts = detect_potential_conflicts(&state).unwrap();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].entity_name, "process_data");
}
