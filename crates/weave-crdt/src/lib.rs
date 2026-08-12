//! weave-crdt: entity-level CRDT coordination.
//!
//! **One door.** Every module below is `pub(crate)`; the list of `pub use`
//! statements in this file *is* the crate's external surface, and it is the
//! only way in. The modules used to be `pub` as well, so every item had two
//! paths — `weave_crdt::reconstruct_file_from_crdt` and
//! `weave_crdt::sync::reconstruct_file_from_crdt` reached the same function.
//! A surface with two spellings is a surface nobody can measure: callers
//! drift onto whichever path they found first, and the two paths are free to
//! diverge without either caller noticing.
//!
//! The `test-helpers` block at the bottom is the second half of that promise:
//! items only tests need and no product door does are exported only when
//! that feature is on, so the shipped crate does not publish authority
//! nothing outside a test ever exercises.

pub(crate) mod anchor;
pub(crate) mod content;
pub(crate) mod error;
pub(crate) mod identity;
pub(crate) mod join;
pub(crate) mod merge;
pub(crate) mod ops;
pub(crate) mod state;
pub(crate) mod sync;

pub use anchor::{anchor_of, ordered_entity_ids, Anchor};
pub use content::{
    get_entity_content, resolve_entity_conflict, update_entity_content, EntityContentStatus,
};
pub use error::{Result, WeaveError};
pub use join::{apply_op, join, value_of, EntityOp, EntityValue, Write};
pub use merge::{CrdtMergeResult, VersionVector};
pub use ops::{
    agent_heartbeat, claim_entity, cleanup_stale_agents, detect_potential_conflicts,
    get_agent_status, get_entities_for_file, get_entity_status, register_agent, release_entity,
    upsert_entity, AgentStatus, ClaimResult, EntityStatus, PotentialConflict,
};
pub use state::EntityStateDoc;
pub use sync::{
    extract_entity_ids, merge_file_entities, reconstruct_file_from_crdt, resolve_entity_id,
    sync_from_files,
};

// Test doors: they put a replica into a state only a fixture needs to reach,
// and the `test-helpers` feature is what the conformance suite in the companion
// workspace turns on to get them. `EntityStateDoc::new_memory` is gated the
// same way, on the item itself.
#[cfg(any(test, feature = "test-helpers"))]
pub use content::set_entity_conflict;
#[cfg(any(test, feature = "test-helpers"))]
pub use ops::set_agent_last_seen;
