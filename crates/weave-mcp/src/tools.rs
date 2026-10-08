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
    #[schemars(
        description = "Optional: entity kind to disambiguate same-named entities (e.g. 'function', 'class', 'method')"
    )]
    pub entity_type: Option<String>,
    #[schemars(
        description = "Optional: name of the enclosing entity (e.g. the class owning a method) to disambiguate same-named entities"
    )]
    pub parent_name: Option<String>,
    #[schemars(
        description = "Optional: 0-based occurrence index among matching entities in file order, for overloads or duplicate names"
    )]
    pub ordinal: Option<u32>,
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
    #[schemars(
        description = "Optional: the entity_id returned by weave_claim_entity. Addresses the claim directly, independent of the entity's current name — a rename between claim and release does not break it. When present, name-based resolution is skipped entirely."
    )]
    pub entity_id: Option<String>,
    #[schemars(
        description = "Optional: entity kind to disambiguate same-named entities (e.g. 'function', 'class', 'method')"
    )]
    pub entity_type: Option<String>,
    #[schemars(
        description = "Optional: name of the enclosing entity (e.g. the class owning a method) to disambiguate same-named entities"
    )]
    pub parent_name: Option<String>,
    #[schemars(
        description = "Optional: 0-based occurrence index among matching entities in file order, for overloads or duplicate names"
    )]
    pub ordinal: Option<u32>,
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
    #[schemars(
        description = "Optional: entity kind to disambiguate same-named entities (e.g. 'function', 'class', 'method')"
    )]
    pub entity_type: Option<String>,
    #[schemars(
        description = "Optional: name of the enclosing entity (e.g. the class owning a method) to disambiguate same-named entities"
    )]
    pub parent_name: Option<String>,
    #[schemars(
        description = "Optional: 0-based occurrence index among matching entities in file order, for overloads or duplicate names"
    )]
    pub ordinal: Option<u32>,
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
    #[schemars(
        description = "Optional: entity kind to disambiguate same-named entities (e.g. 'function', 'class', 'method')"
    )]
    pub entity_type: Option<String>,
    #[schemars(
        description = "Optional: name of the enclosing entity (e.g. the class owning a method) to disambiguate same-named entities"
    )]
    pub parent_name: Option<String>,
    #[schemars(
        description = "Optional: 0-based occurrence index among matching entities in file order, for overloads or duplicate names"
    )]
    pub ordinal: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ImpactAnalysisParams {
    #[schemars(description = "Path to the file containing the entity")]
    pub file_path: String,
    #[schemars(description = "Name of the entity to analyze impact for")]
    pub entity_name: String,
    #[schemars(
        description = "Optional: entity kind to disambiguate same-named entities (e.g. 'function', 'class', 'method')"
    )]
    pub entity_type: Option<String>,
    #[schemars(
        description = "Optional: name of the enclosing entity (e.g. the class owning a method) to disambiguate same-named entities"
    )]
    pub parent_name: Option<String>,
    #[schemars(
        description = "Optional: 0-based occurrence index among matching entities in file order, for overloads or duplicate names"
    )]
    pub ordinal: Option<u32>,
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
    #[schemars(
        description = "Optional: the entity_id returned by weave_claim_entity. Addresses the claim directly, independent of the entity's current name — a rename between claim and update does not break it. When present, name-based resolution is skipped entirely."
    )]
    pub entity_id: Option<String>,
    #[schemars(
        description = "Optional, sent together with ours_content: the WHOLE FILE exactly as this agent read it before computing the edit. Enables the concurrent-edit backstop: when the file on disk has drifted from this snapshot, weave runs an entity-level 3-way merge (base = this snapshot, ours = ours_content, theirs = current disk) instead of letting the edit silently overwrite concurrent work. Omit both fields for the previous behavior."
    )]
    pub base_content: Option<String>,
    #[schemars(
        description = "Optional, sent together with base_content: the WHOLE FILE as this agent intends it — base_content with this entity's new content spliced in. Used as the 'ours' side of the drift merge. weave never writes the file; on a clean merge the response carries merged_content for the caller to write."
    )]
    pub ours_content: Option<String>,
    #[schemars(
        description = "Optional: entity kind to disambiguate same-named entities (e.g. 'function', 'class', 'method')"
    )]
    pub entity_type: Option<String>,
    #[schemars(
        description = "Optional: name of the enclosing entity (e.g. the class owning a method) to disambiguate same-named entities"
    )]
    pub parent_name: Option<String>,
    #[schemars(
        description = "Optional: 0-based occurrence index among matching entities in file order, for overloads or duplicate names"
    )]
    pub ordinal: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct GetEntityContentParams {
    #[schemars(description = "Path to the file containing the entity")]
    pub file_path: String,
    #[schemars(description = "Name of the entity to read")]
    pub entity_name: String,
    #[schemars(
        description = "Optional: entity kind to disambiguate same-named entities (e.g. 'function', 'class', 'method')"
    )]
    pub entity_type: Option<String>,
    #[schemars(
        description = "Optional: name of the enclosing entity (e.g. the class owning a method) to disambiguate same-named entities"
    )]
    pub parent_name: Option<String>,
    #[schemars(
        description = "Optional: 0-based occurrence index among matching entities in file order, for overloads or duplicate names"
    )]
    pub ordinal: Option<u32>,
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
    #[schemars(
        description = "Optional: entity kind to disambiguate same-named entities (e.g. 'function', 'class', 'method')"
    )]
    pub entity_type: Option<String>,
    #[schemars(
        description = "Optional: name of the enclosing entity (e.g. the class owning a method) to disambiguate same-named entities"
    )]
    pub parent_name: Option<String>,
    #[schemars(
        description = "Optional: 0-based occurrence index among matching entities in file order, for overloads or duplicate names"
    )]
    pub ordinal: Option<u32>,
}

// ── The listed verbs, one per `weave` command ──

/// How much of a previewed merge to return.
#[derive(Debug, Default, Clone, Copy, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PreviewDetail {
    /// Per-file clean/conflict verdicts, a confidence rating and entity stats.
    #[default]
    Summary,
    /// A weave-findings document per diverging file: conflicts, semantic
    /// warnings and the guard that refused each one.
    Findings,
    /// Per entity, the resolution strategy weave used or would use.
    Entities,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PreviewParams {
    #[schemars(description = "Base branch to merge from (e.g. 'main')")]
    pub base_branch: String,
    #[schemars(description = "Target branch to merge into (e.g. 'feature-x')")]
    pub target_branch: String,
    #[schemars(description = "Optional: preview only this file")]
    pub file_path: Option<String>,
    #[schemars(
        description = "Optional: 'summary' (default) for per-file verdicts, 'findings' for the typed findings document per diverging file, 'entities' for the strategy weave used on each entity"
    )]
    pub detail: Option<PreviewDetail>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExplainParams {
    #[schemars(description = "The conflicted file to explain")]
    pub file_path: String,
    #[schemars(
        description = "Optional: true to summarize the weave conflict markers already in the file instead of reading the merge's three stages from the index (works outside a merge in progress)"
    )]
    pub markers: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct LandParams {
    #[schemars(
        description = "Optional: merge base revision. With ours/theirs, the merge is read from revisions and nothing is written"
    )]
    pub base: Option<String>,
    #[schemars(description = "Optional: our side (branch, tag or SHA)")]
    pub ours: Option<String>,
    #[schemars(description = "Optional: their side (branch, tag or SHA)")]
    pub theirs: Option<String>,
    #[schemars(
        description = "Optional: judge this revision's files as the merge's answer (re-checking a merge commit)"
    )]
    pub result: Option<String>,
    #[schemars(
        description = "Optional: a command that resolves a file weave cannot. It gets base/ours/theirs/conflicted text as JSON on stdin and prints the resolved file, or DELETE / KEEP / CANNOT[: reason]"
    )]
    pub resolver: Option<String>,
    #[schemars(description = "Optional: seconds the resolver may take per file (default 120)")]
    pub resolver_timeout: Option<u64>,
    #[schemars(
        description = "Optional: in the working tree, label every file but write nothing (default false)"
    )]
    pub dry_run: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PatchParams {
    #[schemars(
        description = "The file the patch is for. Without ops: its current content is the changed side, unless changed_content is given. With ops: the target the ops are applied to"
    )]
    pub file_path: String,
    #[schemars(
        description = "Optional: an ops document from an earlier weave_patch call. Present: apply it to file_path. Absent: extract the ops from base_content to the file"
    )]
    pub ops: Option<String>,
    #[schemars(
        description = "The file before the change. Required to extract; when applying, the base the ops were extracted against (omit when the ops embed it)"
    )]
    pub base_content: Option<String>,
    #[schemars(
        description = "Optional, extract only: the changed file, when it is not what is on disk at file_path"
    )]
    pub changed_content: Option<String>,
    #[schemars(
        description = "Optional, extract only: embed base_content in the ops so they apply three-way with nothing else (default true)"
    )]
    pub embed_base: Option<bool>,
    #[schemars(
        description = "Optional, apply only: write a clean result to file_path (default false: return the content)"
    )]
    pub write: Option<bool>,
}
