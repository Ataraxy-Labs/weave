//! The arguments each tool accepts, and nothing else.
//!
//! `deny_unknown_fields` on every one of them is the point of this module.
//! The decode is rmcp's, and an absent `arguments` object becomes `{}` before
//! it ever reaches serde — so a struct whose fields are all optional (weave_check
//! was one) accepted a call with nothing in it, or with nothing but misspelled
//! keys, and answered confidently about revisions the caller never named. A
//! caller who wrote `filepath` for `file_path` now gets `invalid_params`
//! naming the field, instead of a good answer to a question they did not ask.

use serde::{Deserialize, Serialize};

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
    #[schemars(
        description = "Ours: the branch being merged into, i.e. the one you are on (e.g. 'main')"
    )]
    pub base_branch: String,
    #[schemars(description = "Theirs: the branch being merged in (e.g. 'feature-x')")]
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
    #[schemars(
        description = "Ours: the branch being merged into, i.e. the one you are on (e.g. 'main')"
    )]
    pub base_branch: String,
    #[schemars(description = "Theirs: the branch being merged in (e.g. 'feature-x')")]
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
    #[schemars(
        description = "Ours: the branch being merged into, i.e. the one you are on (e.g. 'main')"
    )]
    pub base_branch: String,
    #[schemars(description = "Theirs: the branch being merged in (e.g. 'feature-x')")]
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
    #[schemars(
        description = "Ours: the branch being merged into, i.e. the one you are on (e.g. 'main')"
    )]
    pub base_branch: String,
    #[schemars(description = "Theirs: the branch being merged in (e.g. 'feature-x')")]
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
    #[schemars(description = "Optional: seconds the resolver may take per file (default 900)")]
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

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct EditSpec {
    #[schemars(description = "The file, relative to the repository root")]
    pub file: String,
    #[schemars(
        description = "The entity to change: a name (`Class.method` for a member), or {name, entity_type?, parent_name?, ordinal?}. Omit for a text edit of the whole file or to create a file"
    )]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity: Option<serde_json::Value>,
    #[schemars(
        description = "Optional verb about entity: replace (with content), insert_after / insert_before (content is the new entity), delete"
    )]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub op: Option<String>,
    #[schemars(
        description = "Optional: the entity's kind (function, method, class...) when names collide"
    )]
    #[serde(
        default,
        alias = "entity_type",
        skip_serializing_if = "Option::is_none"
    )]
    pub kind: Option<String>,
    #[schemars(description = "Optional: 0-based occurrence among the entities that still match")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ordinal: Option<usize>,
    #[schemars(
        description = "New source. With entity: its whole new text. With after/before/append: the entity to add. Alone, for a file that does not exist: the new file"
    )]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[schemars(
        description = "Text to replace, inside entity when one is named, else in the whole file. Must match exactly once unless all is true"
    )]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old: Option<String>,
    #[schemars(description = "What old becomes")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new: Option<String>,
    #[schemars(description = "Optional: replace every match of old")]
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub all: bool,
    #[schemars(description = "Add content as a new entity right after this one")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[schemars(description = "Add content as a new entity right before this one")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[schemars(
        description = "Add content as the last member of this class: a new method needs no anchor and no read"
    )]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub into: Option<String>,
    #[schemars(description = "Add content at the end of the file")]
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub append: bool,
    #[schemars(description = "Delete entity")]
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub delete: bool,
    #[schemars(
        description = "Optional: let a replace rename the entity or change its kind or visibility (refused otherwise)"
    )]
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub allow_signature_change: bool,
    #[schemars(
        description = "Rename entity (defined in file) to this name, together with every identifier spelled like it in the repository's code: one edit instead of one per call site. Strings and comments are left alone; files without a grammar that mention it are listed in notes"
    )]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rename: Option<String>,
    #[schemars(
        description = "Optional: the entity's version from sem_find (v:...): the edit is refused if the entity changed since that read, instead of overwriting someone else's change"
    )]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expect: Option<String>,
    #[schemars(
        description = "With rename: also rename the name where it appears as a whole word in comments and in docs (Markdown, reStructuredText, text files); strings are left alone"
    )]
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub everywhere: bool,
    #[schemars(
        description = "With rename: only in files matching these globs, e.g. [\"src/**\", \"tests/**\"]. Default: the whole repository"
    )]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope: Vec<String>,
    #[schemars(
        description = "Put entity back the way it was at this git ref (e.g. \"HEAD~3\", a branch or commit): only that entity changes, through the same guards as a replace. Refused if the entity did not exist at that ref"
    )]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revert: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct EditParams {
    #[schemars(description = "The edits, applied in order; all or nothing")]
    pub edits: Vec<EditSpec>,
    #[schemars(description = "Optional: report what would change and write nothing")]
    pub dry_run: Option<bool>,
    #[schemars(
        description = "Optional: a build or test command to run once the edits are written (e.g. \"go test ./...\"); its exit_code and the last 16KB of its output come back in this same response, so editing and checking take one call. Runs in the repo root, stopped after 10 minutes"
    )]
    pub then: Option<String>,
}
