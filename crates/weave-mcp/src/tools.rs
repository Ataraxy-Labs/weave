//! The arguments each tool accepts, and nothing else.
//!
//! `deny_unknown_fields` on every one of them is the point of this module.
//! The decode is rmcp's, and an absent `arguments` object becomes `{}` before
//! it ever reaches serde — so a struct whose fields are all optional (weave_check
//! was one) accepted a call with nothing in it, or with nothing but misspelled
//! keys, and answered confidently about revisions the caller never named. A
//! caller who wrote `filepath` for `file_path` now gets `invalid_params`
//! naming the field, instead of a good answer to a question they did not ask.

use serde::Deserialize;

// ── Tool parameter structs ──

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExtractEntitiesParams {
    #[schemars(description = "Path to the file (relative to repo root)")]
    pub file_path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ClaimEntityParams {
    #[schemars(description = "Agent identifier (e.g. 'agent-1')")]
    pub agent_id: String,
    #[schemars(description = "Path to the file containing the entity")]
    pub file_path: String,
    #[schemars(description = "Name of the entity to claim")]
    pub entity_name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReleaseEntityParams {
    #[schemars(description = "Agent identifier")]
    pub agent_id: String,
    #[schemars(description = "Path to the file containing the entity")]
    pub file_path: String,
    #[schemars(description = "Name of the entity to release")]
    pub entity_name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct StatusParams {
    #[schemars(description = "Path to the file to check status for")]
    pub file_path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct WhoIsEditingParams {
    #[schemars(description = "Path to the file")]
    pub file_path: String,
    #[schemars(description = "Name of the entity to check")]
    pub entity_name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PotentialConflictsParams {
    #[schemars(description = "Optional: filter conflicts to those involving this agent")]
    pub agent_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PreviewMergeParams {
    #[schemars(description = "Base branch to merge from (e.g. 'main')")]
    pub base_branch: String,
    #[schemars(description = "Target branch to merge into (e.g. 'feature-x')")]
    pub target_branch: String,
    #[schemars(description = "Optional: preview only this file")]
    pub file_path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AgentRegisterParams {
    #[schemars(description = "Agent identifier")]
    pub agent_id: String,
    #[schemars(description = "Branch the agent is working on")]
    pub branch: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AgentHeartbeatParams {
    #[schemars(description = "Agent identifier")]
    pub agent_id: String,
    #[schemars(description = "List of entity IDs the agent is currently working on")]
    pub working_on: Vec<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct EntityDepsParams {
    #[schemars(description = "Path to the file containing the entity")]
    pub file_path: String,
    #[schemars(description = "Name of the entity to analyze")]
    pub entity_name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ImpactAnalysisParams {
    #[schemars(description = "Path to the file containing the entity")]
    pub file_path: String,
    #[schemars(description = "Name of the entity to analyze impact for")]
    pub entity_name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ValidateMergeParams {
    #[schemars(description = "Base branch (e.g. 'main')")]
    pub base_branch: String,
    #[schemars(description = "Target branch to validate merge of")]
    pub target_branch: String,
    #[schemars(description = "Optional: validate only this file")]
    pub file_path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct MergeSummaryParams {
    #[schemars(description = "Path to a file containing weave conflict markers")]
    pub file_path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct DiffParams {
    #[schemars(
        description = "Base ref to compare from (branch, tag, or commit hash, e.g. 'main')"
    )]
    pub base_ref: String,
    #[schemars(
        description = "Target ref to compare to (branch, tag, or commit hash, e.g. 'feature-x'). Defaults to HEAD."
    )]
    pub target_ref: Option<String>,
    #[schemars(description = "Optional: diff only this file")]
    pub file_path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct MergeAuditParams {
    #[schemars(description = "Base branch to merge from (e.g. 'main')")]
    pub base_branch: String,
    #[schemars(description = "Target branch to merge into (e.g. 'feature-x')")]
    pub target_branch: String,
    #[schemars(description = "Optional: audit only this file")]
    pub file_path: Option<String>,
}

// ── New v2 tools ──

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct UpdateEntityContentParams {
    #[schemars(description = "Agent identifier")]
    pub agent_id: String,
    #[schemars(description = "Path to the file containing the entity")]
    pub file_path: String,
    #[schemars(description = "Name of the entity to update")]
    pub entity_name: String,
    #[schemars(description = "New source code content for the entity")]
    pub content: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct GetEntityContentParams {
    #[schemars(description = "Path to the file containing the entity")]
    pub file_path: String,
    #[schemars(description = "Name of the entity to read")]
    pub entity_name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct MergeFileParams {
    #[schemars(description = "Path to the file to merge")]
    pub file_path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct FindingsParams {
    #[schemars(description = "Base branch to merge from (e.g. 'main')")]
    pub base_branch: String,
    #[schemars(description = "Target branch to merge into (e.g. 'feature-x')")]
    pub target_branch: String,
    #[schemars(description = "Optional: analyze only this file")]
    pub file_path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CheckParams {
    #[schemars(
        description = "Merge base revision. Optional: defaults to the merge base of ours and theirs."
    )]
    pub base: Option<String>,
    #[schemars(description = "Our side (branch, tag or SHA). Optional: defaults to HEAD.")]
    pub ours: Option<String>,
    #[schemars(
        description = "Their side (branch, tag or SHA). Optional: defaults to MERGE_HEAD, i.e. the merge currently in progress."
    )]
    pub theirs: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResolveConflictParams {
    #[schemars(description = "Agent identifier")]
    pub agent_id: String,
    #[schemars(description = "Path to the file containing the entity")]
    pub file_path: String,
    #[schemars(description = "Name of the conflicted entity to resolve")]
    pub entity_name: String,
    #[schemars(description = "Resolved source code content")]
    pub resolved_content: String,
}
