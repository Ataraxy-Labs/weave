#![allow(clippy::too_many_arguments)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::nonminimal_bool)]

pub mod binding;
pub(crate) mod changelog;
pub mod conflict;
pub(crate) mod container;
pub mod datafile;
pub(crate) mod dataunion;
pub(crate) mod determinate;
pub mod diagnose;
pub(crate) mod elements;
pub mod explain;
pub mod frame;
pub mod git;
pub mod host;
pub(crate) mod insertion;
pub mod layout;
pub mod merge;
pub mod region;
pub(crate) mod statement;
pub mod stats;
pub(crate) mod subsumption;
pub mod v2;
pub mod validate;
pub mod verify;

pub use conflict::{parse_weave_conflicts, MarkerFormat, ParsedConflict};
pub use merge::{
    entity_merge, entity_merge_fmt, entity_merge_with_registry, supported_merge_extensions,
    EntityAudit, MergeResult, ResolutionStrategy, DECLINED_EXTENSIONS, LINE_RULE_EXTENSIONS,
};
pub use validate::{validate_merge, ModifiedEntity, SemanticWarning};
