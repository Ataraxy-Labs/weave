pub mod conflict;
pub mod git;
pub mod merge;
pub mod reconstruct;
pub mod region;
pub mod validate;

pub use conflict::{parse_weave_conflicts, ParsedConflict};
pub use merge::{entity_merge, entity_merge_with_registry, MergeResult};
pub use validate::{validate_merge, ModifiedEntity, SemanticWarning};
