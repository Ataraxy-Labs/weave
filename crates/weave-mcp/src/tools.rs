use serde::Deserialize;

// ── Tool parameter structs ──

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ExtractEntitiesParams {
    #[schemars(description = "Path to the file (relative to repo root)")]
    pub file_path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ClaimEntityParams {
    #[schemars(description = "Agent identifier (e.g. 'agent-1')")]
    pub agent_id: String,
    #[schemars(description = "Path to the file containing the entity")]
    pub file_path: String,
    #[schemars(description = "Name of the entity to claim")]
    pub entity_name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReleaseEntityParams {
    #[schemars(description = "Agent identifier")]
    pub agent_id: String,
    #[schemars(description = "Path to the file containing the entity")]
    pub file_path: String,
    #[schemars(description = "Name of the entity to release")]
    pub entity_name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct StatusParams {
    #[schemars(description = "Path to the file to check status for")]
    pub file_path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct WhoIsEditingParams {
    #[schemars(description = "Path to the file")]
    pub file_path: String,
    #[schemars(description = "Name of the entity to check")]
    pub entity_name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PotentialConflictsParams {
    #[schemars(description = "Optional: filter conflicts to those involving this agent")]
    pub agent_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PreviewMergeParams {
    #[schemars(description = "Base branch to merge from (e.g. 'main')")]
    pub base_branch: String,
    #[schemars(description = "Target branch to merge into (e.g. 'feature-x')")]
    pub target_branch: String,
    #[schemars(description = "Optional: preview only this file")]
    pub file_path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AgentRegisterParams {
    #[schemars(description = "Agent identifier")]
    pub agent_id: String,
    #[schemars(description = "Branch the agent is working on")]
    pub branch: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AgentHeartbeatParams {
    #[schemars(description = "Agent identifier")]
    pub agent_id: String,
    #[schemars(description = "List of entity IDs the agent is currently working on")]
    pub working_on: Vec<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EntityDepsParams {
    #[schemars(description = "Path to the file containing the entity")]
    pub file_path: String,
    #[schemars(description = "Name of the entity to analyze")]
    pub entity_name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ImpactAnalysisParams {
    #[schemars(description = "Path to the file containing the entity")]
    pub file_path: String,
    #[schemars(description = "Name of the entity to analyze impact for")]
    pub entity_name: String,
}
