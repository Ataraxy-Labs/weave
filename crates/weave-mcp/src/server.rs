use std::path::PathBuf;
use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content, ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use sem_core::parser::plugins::create_default_registry;
use sem_core::parser::registry::ParserRegistry;
use tokio::sync::Mutex;

use weave_core::git;
use weave_crdt::{
    claim_entity, detect_potential_conflicts, get_entities_for_file, get_entity_status,
    register_agent, release_entity, resolve_entity_id, sync_from_files, upsert_entity,
    EntityStateDoc,
};

use crate::tools::*;

/// Lazily-initialized repo context. Created on first tool call.
struct RepoContext {
    state: Mutex<EntityStateDoc>,
    repo_root: PathBuf,
}

#[derive(Clone)]
pub struct WeaveServer {
    context: Arc<Mutex<Option<RepoContext>>>,
    registry: Arc<ParserRegistry>,
    tool_router: ToolRouter<Self>,
}

impl WeaveServer {
    /// Lazily initialize and return the repo context.
    async fn get_context(&self) -> Result<tokio::sync::MappedMutexGuard<'_, RepoContext>, String> {
        {
            let mut guard = self.context.lock().await;
            if guard.is_none() {
                let repo_root = git::find_repo_root()
                    .map_err(|_| "Not inside a git repository. Run weave-mcp from within a git repo, or cd into one first.".to_string())?;
                let state_path = repo_root.join(".weave").join("state.automerge");
                let state = EntityStateDoc::open(&state_path)
                    .map_err(|e| format!("Failed to open CRDT state: {}", e))?;
                *guard = Some(RepoContext {
                    state: Mutex::new(state),
                    repo_root,
                });
            }
        }
        let guard = self.context.lock().await;
        Ok(tokio::sync::MutexGuard::map(guard, |opt| {
            opt.as_mut().unwrap()
        }))
    }

    fn read_file_sync(repo_root: &PathBuf, file_path: &str) -> Result<String, String> {
        let full = repo_root.join(file_path);
        std::fs::read_to_string(&full).map_err(|e| format!("Failed to read {}: {}", file_path, e))
    }

    fn resolve_entity_sync(
        registry: &ParserRegistry,
        content: &str,
        file_path: &str,
        entity_name: &str,
    ) -> Result<String, String> {
        resolve_entity_id(content, file_path, entity_name, registry)
            .ok_or_else(|| format!("Entity '{}' not found in '{}'", entity_name, file_path))
    }
}

#[tool_router]
impl WeaveServer {
    pub fn new() -> Self {
        Self {
            context: Arc::new(Mutex::new(None)),
            registry: Arc::new(create_default_registry()),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "List all semantic entities (functions, classes, etc.) in a file with their types and line ranges")]
    async fn weave_extract_entities(
        &self,
        Parameters(params): Parameters<ExtractEntitiesParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self.get_context().await.map_err(internal_err)?;
        let content = Self::read_file_sync(&ctx.repo_root, &params.file_path).map_err(internal_err)?;

        let plugin = self
            .registry
            .get_plugin(&params.file_path)
            .ok_or_else(|| internal_err(format!("No parser for file: {}", params.file_path)))?;

        let entities = plugin.extract_entities(&content, &params.file_path);
        let result: Vec<serde_json::Value> = entities
            .iter()
            .map(|e| {
                serde_json::json!({
                    "id": e.id,
                    "name": e.name,
                    "type": e.entity_type,
                    "start_line": e.start_line,
                    "end_line": e.end_line,
                })
            })
            .collect();

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }

    #[tool(description = "Claim an entity before editing it. Advisory lock that signals to other agents you're working on this entity.")]
    async fn weave_claim_entity(
        &self,
        Parameters(params): Parameters<ClaimEntityParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self.get_context().await.map_err(internal_err)?;
        let content = Self::read_file_sync(&ctx.repo_root, &params.file_path).map_err(internal_err)?;
        let entity_id = Self::resolve_entity_sync(&self.registry, &content, &params.file_path, &params.entity_name)
            .map_err(internal_err)?;

        let mut state = ctx.state.lock().await;
        let plugin = self
            .registry
            .get_plugin(&params.file_path)
            .ok_or_else(|| internal_err("No parser for file"))?;
        let entities = plugin.extract_entities(&content, &params.file_path);
        if let Some(e) = entities.iter().find(|e| e.id == entity_id) {
            let _ = upsert_entity(
                &mut state,
                &e.id,
                &e.name,
                &e.entity_type,
                &params.file_path,
                &e.content_hash,
            );
        }

        let result = claim_entity(&mut state, &params.agent_id, &entity_id)
            .map_err(|e| internal_err(e.to_string()))?;

        let _ = state.save();

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string(&result).unwrap_or_default(),
        )]))
    }

    #[tool(description = "Release a previously claimed entity after you're done editing it")]
    async fn weave_release_entity(
        &self,
        Parameters(params): Parameters<ReleaseEntityParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self.get_context().await.map_err(internal_err)?;
        let content = Self::read_file_sync(&ctx.repo_root, &params.file_path).map_err(internal_err)?;
        let entity_id = Self::resolve_entity_sync(&self.registry, &content, &params.file_path, &params.entity_name)
            .map_err(internal_err)?;

        let mut state = ctx.state.lock().await;
        release_entity(&mut state, &params.agent_id, &entity_id)
            .map_err(|e| internal_err(e.to_string()))?;
        let _ = state.save();

        Ok(CallToolResult::success(vec![Content::text(
            "Released successfully",
        )]))
    }

    #[tool(description = "Show entity status for a file: all entities with their claim and modification status")]
    async fn weave_status(
        &self,
        Parameters(params): Parameters<StatusParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self.get_context().await.map_err(internal_err)?;
        let content = Self::read_file_sync(&ctx.repo_root, &params.file_path).map_err(internal_err)?;

        let mut state = ctx.state.lock().await;
        let _ = sync_from_files(
            &mut state,
            &ctx.repo_root,
            &[params.file_path.clone()],
            &self.registry,
        );

        let entities = get_entities_for_file(&state, &params.file_path)
            .map_err(|e| internal_err(e.to_string()))?;

        let plugin = self.registry.get_plugin(&params.file_path);
        let file_entities = plugin
            .map(|p| p.extract_entities(&content, &params.file_path))
            .unwrap_or_default();

        let result: Vec<serde_json::Value> = file_entities
            .iter()
            .map(|fe| {
                let status = entities.iter().find(|s| s.entity_id == fe.id);
                serde_json::json!({
                    "name": fe.name,
                    "type": fe.entity_type,
                    "start_line": fe.start_line,
                    "end_line": fe.end_line,
                    "claimed_by": status.and_then(|s| s.claimed_by.as_ref()),
                    "last_modified_by": status.and_then(|s| s.last_modified_by.as_ref()),
                    "version": status.map(|s| s.version).unwrap_or(0),
                })
            })
            .collect();

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }

    #[tool(description = "Check if anyone is currently editing a specific entity")]
    async fn weave_who_is_editing(
        &self,
        Parameters(params): Parameters<WhoIsEditingParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self.get_context().await.map_err(internal_err)?;
        let content = Self::read_file_sync(&ctx.repo_root, &params.file_path).map_err(internal_err)?;
        let entity_id = Self::resolve_entity_sync(&self.registry, &content, &params.file_path, &params.entity_name)
            .map_err(internal_err)?;

        let state = ctx.state.lock().await;
        match get_entity_status(&state, &entity_id) {
            Ok(status) => {
                let result = serde_json::json!({
                    "entity": params.entity_name,
                    "claimed_by": status.claimed_by,
                    "last_modified_by": status.last_modified_by,
                    "version": status.version,
                });
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&result).unwrap_or_default(),
                )]))
            }
            Err(_) => Ok(CallToolResult::success(vec![Content::text(
                serde_json::json!({
                    "entity": params.entity_name,
                    "claimed_by": null,
                    "last_modified_by": null,
                    "version": 0,
                })
                .to_string(),
            )])),
        }
    }

    #[tool(description = "Detect entities being worked on by multiple agents — potential merge conflicts")]
    async fn weave_potential_conflicts(
        &self,
        Parameters(params): Parameters<PotentialConflictsParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self.get_context().await.map_err(internal_err)?;
        let state = ctx.state.lock().await;
        let mut conflicts = detect_potential_conflicts(&state)
            .map_err(|e| internal_err(e.to_string()))?;

        if let Some(ref agent_id) = params.agent_id {
            conflicts.retain(|c| c.agents.contains(agent_id));
        }

        let result: Vec<serde_json::Value> = conflicts
            .iter()
            .map(|c| {
                serde_json::json!({
                    "entity_id": c.entity_id,
                    "entity_name": c.entity_name,
                    "file_path": c.file_path,
                    "agents": c.agents,
                })
            })
            .collect();

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }

    #[tool(description = "Preview what a merge between two branches would look like using weave's entity-level analysis")]
    async fn weave_preview_merge(
        &self,
        Parameters(params): Parameters<PreviewMergeParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        // This tool needs a git repo
        let _ = self.get_context().await.map_err(internal_err)?;

        let merge_base =
            git::find_merge_base(&params.base_branch, &params.target_branch)
                .map_err(|e| internal_err(e.to_string()))?;

        let files = if let Some(ref fp) = params.file_path {
            vec![fp.clone()]
        } else {
            git::get_changed_files(&merge_base, &params.base_branch, &params.target_branch)
                .map_err(|e| internal_err(e.to_string()))?
        };

        let mut results = Vec::new();
        for file in &files {
            let base = git::git_show(&merge_base, file).unwrap_or_default();
            let ours = git::git_show(&params.base_branch, file).unwrap_or_default();
            let theirs = git::git_show(&params.target_branch, file).unwrap_or_default();

            if ours == theirs || base == ours || base == theirs {
                continue;
            }

            let merge_result = weave_core::entity_merge_with_registry(
                &base,
                &ours,
                &theirs,
                file,
                &self.registry,
            );

            let conflicts: Vec<serde_json::Value> = merge_result
                .conflicts
                .iter()
                .map(|c| {
                    serde_json::json!({
                        "entity_type": c.entity_type,
                        "entity_name": c.entity_name,
                        "kind": format!("{}", c.kind),
                    })
                })
                .collect();

            results.push(serde_json::json!({
                "file": file,
                "clean": merge_result.is_clean(),
                "stats": format!("{}", merge_result.stats),
                "conflicts": conflicts,
            }));
        }

        let summary = serde_json::json!({
            "files_analyzed": results.len(),
            "files_with_conflicts": results.iter().filter(|r| !r["clean"].as_bool().unwrap_or(true)).count(),
            "results": results,
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&summary).unwrap_or_default(),
        )]))
    }

    #[tool(description = "Register an agent in weave's coordination state")]
    async fn weave_agent_register(
        &self,
        Parameters(params): Parameters<AgentRegisterParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self.get_context().await.map_err(internal_err)?;
        let mut state = ctx.state.lock().await;
        register_agent(
            &mut state,
            &params.agent_id,
            &params.agent_id,
            &params.branch,
        )
        .map_err(|e| internal_err(e.to_string()))?;
        let _ = state.save();

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Agent '{}' registered on branch '{}'",
            params.agent_id, params.branch
        ))]))
    }

    #[tool(description = "Send a heartbeat to keep agent status active and update what entities it's working on")]
    async fn weave_agent_heartbeat(
        &self,
        Parameters(params): Parameters<AgentHeartbeatParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self.get_context().await.map_err(internal_err)?;
        let mut state = ctx.state.lock().await;
        weave_crdt::agent_heartbeat(&mut state, &params.agent_id, &params.working_on)
            .map_err(|e| internal_err(e.to_string()))?;
        let _ = state.save();

        Ok(CallToolResult::success(vec![Content::text("OK")]))
    }
}

#[tool_handler]
impl ServerHandler for WeaveServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Weave MCP server for entity-level semantic merge coordination. \
                 Agents can claim entities before editing, check who is editing what, \
                 detect potential conflicts, and preview merges."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

fn internal_err(msg: impl ToString) -> rmcp::ErrorData {
    rmcp::ErrorData::internal_error(msg.to_string(), None)
}
