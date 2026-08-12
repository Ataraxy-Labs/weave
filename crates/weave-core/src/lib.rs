#![allow(clippy::too_many_arguments)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::nonminimal_bool)]

pub mod binding;
pub mod conflict;
pub(crate) mod container;
pub mod diagnose;
pub mod explain;
pub(crate) mod expression;
pub mod frame;
pub mod git;
pub mod host;
pub mod merge;
pub mod region;
pub(crate) mod statement;
pub mod stats;
pub(crate) mod subsumption;
pub mod v2;
pub mod validate;

pub use conflict::{parse_weave_conflicts, MarkerFormat, ParsedConflict};
pub use merge::{
    entity_merge, entity_merge_fmt, entity_merge_with_registry, EntityAudit, MergeResult,
    ResolutionStrategy,
};
pub use validate::{validate_merge, ModifiedEntity, SemanticWarning};
