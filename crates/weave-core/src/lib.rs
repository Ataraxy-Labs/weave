#![allow(clippy::too_many_arguments)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::nonminimal_bool)]

pub mod conflict;
pub mod git;
pub mod merge;
pub mod reconstruct;
pub mod region;
pub mod validate;

pub use conflict::{parse_weave_conflicts, MarkerFormat, ParsedConflict};
pub use merge::{
    entity_merge, entity_merge_fmt, entity_merge_with_registry, EntityAudit, MergeResult,
    ResolutionStrategy,
};
pub use validate::{validate_merge, ModifiedEntity, SemanticWarning};
