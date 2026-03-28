pub mod content;
pub mod error;
pub mod merge;
pub mod ops;
pub mod state;
pub mod sync;

pub use content::{
    get_entity_content, resolve_entity_conflict, update_entity_content, EntityContentStatus,
};
pub use error::{Result, WeaveError};
pub use merge::{CrdtMergeResult, MergeState, VersionVector};
pub use ops::{
    agent_heartbeat, claim_entity, cleanup_stale_agents, detect_potential_conflicts,
    get_agent_status, get_entities_for_file, get_entity_status, record_modification,
    register_agent, release_entity, upsert_entity, AgentStatus, ClaimResult, EntityStatus,
    PotentialConflict,
};
pub use state::EntityStateDoc;
pub use sync::{extract_entity_ids, merge_file_entities, reconstruct_file_from_crdt, resolve_entity_id, sync_from_files};

#[cfg(any(test, feature = "test-helpers"))]
pub use content::set_entity_conflict;
#[cfg(any(test, feature = "test-helpers"))]
pub use ops::set_agent_last_seen;
