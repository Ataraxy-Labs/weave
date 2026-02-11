use std::path::{Path, PathBuf};
use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content, ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use sem_core::parser::graph::EntityGraph;
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
    /// Discover repo root using multiple strategies:
    /// 1. If file_path is absolute, derive repo from that path
    /// 2. WEAVE_REPO env var
    /// 3. CWD-based git discovery
    fn discover_repo_root(file_path_hint: Option<&str>) -> Result<PathBuf, String> {
        // Strategy 1: Absolute file path -> git -C <parent> rev-parse
        if let Some(fp) = file_path_hint {
            let p = Path::new(fp);
            if p.is_absolute() {
                if let Ok(root) = git::find_repo_root_from_path(p) {
                    return Ok(root);
                }
            }
        }

        // Strategy 2: WEAVE_REPO env var
        if let Ok(repo) = std::env::var("WEAVE_REPO") {
            let p = PathBuf::from(&repo);
            if p.is_dir() {
                return Ok(p);
            }
        }

        // Strategy 3: CWD-based discovery
        if let Ok(root) = git::find_repo_root() {
            return Ok(root);
        }

        Err(
            "Cannot find git repository. Either:\n\
             - Pass an absolute file path (e.g. /Users/you/project/src/lib.ts)\n\
             - Set WEAVE_REPO env var to the repo root\n\
             - Run weave-mcp from within a git repo"
                .to_string(),
        )
    }

    /// Resolve a file path to (repo_root-relative path, absolute path).
    /// Handles both absolute and relative paths.
    fn resolve_file_path(repo_root: &Path, file_path: &str) -> (String, PathBuf) {
        let p = Path::new(file_path);
        if p.is_absolute() {
            // Convert absolute -> relative to repo root
            let relative = p
                .strip_prefix(repo_root)
                .map(|r| r.to_string_lossy().to_string())
                .unwrap_or_else(|_| file_path.to_string());
            (relative, p.to_path_buf())
        } else {
            // Already relative, resolve to absolute
            (file_path.to_string(), repo_root.join(file_path))
        }
    }

    /// Lazily initialize repo context, using file_path as a hint for repo discovery.
    async fn get_context(
        &self,
        file_path_hint: Option<&str>,
    ) -> Result<tokio::sync::MappedMutexGuard<'_, RepoContext>, String> {
        {
            let mut guard = self.context.lock().await;
            if guard.is_none() {
                let repo_root = Self::discover_repo_root(file_path_hint)?;
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

    /// Find all files in the repo that have a supported parser.
    fn find_supported_files(root: &Path, registry: &ParserRegistry) -> Vec<String> {
        let mut files = Vec::new();
        Self::walk_dir(root, root, registry, &mut files);
        files.sort();
        files
    }

    fn walk_dir(
        dir: &Path,
        root: &Path,
        registry: &ParserRegistry,
        files: &mut Vec<String>,
    ) {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with('.')
                    || name == "node_modules"
                    || name == "target"
                    || name == "__pycache__"
                    || name == "venv"
                {
                    continue;
                }
            }
            if path.is_dir() {
                Self::walk_dir(&path, root, registry, files);
            } else if let Ok(rel) = path.strip_prefix(root) {
                let rel_str = rel.to_string_lossy().to_string();
                if registry.get_plugin(&rel_str).is_some() {
                    files.push(rel_str);
                }
            }
        }
    }

    fn read_file_at(abs_path: &Path, display_path: &str) -> Result<String, String> {
        std::fs::read_to_string(abs_path)
            .map_err(|e| format!("Failed to read {}: {}", display_path, e))
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
        let ctx = self
            .get_context(Some(&params.file_path))
            .await
            .map_err(internal_err)?;
        let (rel_path, abs_path) =
            Self::resolve_file_path(&ctx.repo_root, &params.file_path);
        let content = Self::read_file_at(&abs_path, &rel_path).map_err(internal_err)?;

        let plugin = self
            .registry
            .get_plugin(&rel_path)
            .ok_or_else(|| internal_err(format!("No parser for file: {}", rel_path)))?;

        let entities = plugin.extract_entities(&content, &rel_path);
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

    #[tool(description = "Claim an entity before editing it. Advisory lock that signals to other agents you're working on this entity. Returns predictive warnings if related entities are claimed by other agents.")]
    async fn weave_claim_entity(
        &self,
        Parameters(params): Parameters<ClaimEntityParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self
            .get_context(Some(&params.file_path))
            .await
            .map_err(internal_err)?;
        let (rel_path, abs_path) =
            Self::resolve_file_path(&ctx.repo_root, &params.file_path);
        let content = Self::read_file_at(&abs_path, &rel_path).map_err(internal_err)?;
        let entity_id =
            Self::resolve_entity_sync(&self.registry, &content, &rel_path, &params.entity_name)
                .map_err(internal_err)?;

        let mut state = ctx.state.lock().await;
        let plugin = self
            .registry
            .get_plugin(&rel_path)
            .ok_or_else(|| internal_err("No parser for file"))?;
        let entities = plugin.extract_entities(&content, &rel_path);
        if let Some(e) = entities.iter().find(|e| e.id == entity_id) {
            let _ = upsert_entity(
                &mut state,
                &e.id,
                &e.name,
                &e.entity_type,
                &rel_path,
                &e.content_hash,
            );
        }

        let result = claim_entity(&mut state, &params.agent_id, &entity_id)
            .map_err(|e| internal_err(e.to_string()))?;

        let _ = state.save();

        // Predictive conflict detection: check if any entity in the
        // dependency chain is claimed by another agent
        let mut dep_warnings: Vec<serde_json::Value> = Vec::new();
        let file_paths = Self::find_supported_files(&ctx.repo_root, &self.registry);
        let graph = EntityGraph::build(&ctx.repo_root, &file_paths, &self.registry);

        // Find graph entity matching our claimed entity
        if let Some(graph_entity) = graph
            .entities
            .values()
            .find(|e| e.name == params.entity_name && e.file_path == rel_path)
        {
            // Check dependencies (what we call)
            let deps = graph.get_dependencies(&graph_entity.id);
            for dep in &deps {
                if let Ok(status) = get_entity_status(&state, &dep.id) {
                    if let Some(ref claimed_by) = status.claimed_by {
                        if claimed_by != &params.agent_id {
                            dep_warnings.push(serde_json::json!({
                                "type": "dependency_claimed",
                                "message": format!(
                                    "{} `{}` depends on {} `{}` which is claimed by agent `{}`",
                                    "function", params.entity_name,
                                    dep.entity_type, dep.name, claimed_by
                                ),
                                "entity": dep.name,
                                "file": dep.file_path,
                                "claimed_by": claimed_by,
                            }));
                        }
                    }
                }
            }

            // Check dependents (who calls us)
            let dependents = graph.get_dependents(&graph_entity.id);
            for dep in &dependents {
                if let Ok(status) = get_entity_status(&state, &dep.id) {
                    if let Some(ref claimed_by) = status.claimed_by {
                        if claimed_by != &params.agent_id {
                            dep_warnings.push(serde_json::json!({
                                "type": "dependent_claimed",
                                "message": format!(
                                    "{} `{}` is used by {} `{}` which is claimed by agent `{}`",
                                    "function", params.entity_name,
                                    dep.entity_type, dep.name, claimed_by
                                ),
                                "entity": dep.name,
                                "file": dep.file_path,
                                "claimed_by": claimed_by,
                            }));
                        }
                    }
                }
            }
        }

        let response = serde_json::json!({
            "result": serde_json::to_value(&result).unwrap_or_default(),
            "dependency_warnings": dep_warnings,
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&response).unwrap_or_default(),
        )]))
    }

    #[tool(description = "Release a previously claimed entity after you're done editing it")]
    async fn weave_release_entity(
        &self,
        Parameters(params): Parameters<ReleaseEntityParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self
            .get_context(Some(&params.file_path))
            .await
            .map_err(internal_err)?;
        let (rel_path, abs_path) =
            Self::resolve_file_path(&ctx.repo_root, &params.file_path);
        let content = Self::read_file_at(&abs_path, &rel_path).map_err(internal_err)?;
        let entity_id =
            Self::resolve_entity_sync(&self.registry, &content, &rel_path, &params.entity_name)
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
        let ctx = self
            .get_context(Some(&params.file_path))
            .await
            .map_err(internal_err)?;
        let (rel_path, abs_path) =
            Self::resolve_file_path(&ctx.repo_root, &params.file_path);
        let content = Self::read_file_at(&abs_path, &rel_path).map_err(internal_err)?;

        let mut state = ctx.state.lock().await;
        let _ = sync_from_files(
            &mut state,
            &ctx.repo_root,
            &[rel_path.clone()],
            &self.registry,
        );

        let entities = get_entities_for_file(&state, &rel_path)
            .map_err(|e| internal_err(e.to_string()))?;

        let plugin = self.registry.get_plugin(&rel_path);
        let file_entities = plugin
            .map(|p| p.extract_entities(&content, &rel_path))
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
        let ctx = self
            .get_context(Some(&params.file_path))
            .await
            .map_err(internal_err)?;
        let (rel_path, abs_path) =
            Self::resolve_file_path(&ctx.repo_root, &params.file_path);
        let content = Self::read_file_at(&abs_path, &rel_path).map_err(internal_err)?;
        let entity_id =
            Self::resolve_entity_sync(&self.registry, &content, &rel_path, &params.entity_name)
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
        let ctx = self.get_context(None).await.map_err(internal_err)?;
        let state = ctx.state.lock().await;
        let mut conflicts =
            detect_potential_conflicts(&state).map_err(|e| internal_err(e.to_string()))?;

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
        let ctx = self
            .get_context(params.file_path.as_deref())
            .await
            .map_err(internal_err)?;

        // Run git commands from the repo root
        let merge_base = git::find_merge_base(&params.base_branch, &params.target_branch)
            .map_err(|e| internal_err(e.to_string()))?;

        let files = if let Some(ref fp) = params.file_path {
            let (rel, _) = Self::resolve_file_path(&ctx.repo_root, fp);
            vec![rel]
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
                        "complexity": format!("{}", c.complexity),
                    })
                })
                .collect();

            let warnings: Vec<String> = merge_result
                .warnings
                .iter()
                .map(|w| format!("{}", w))
                .collect();

            results.push(serde_json::json!({
                "file": file,
                "clean": merge_result.is_clean(),
                "confidence": merge_result.stats.confidence(),
                "stats": {
                    "unchanged": merge_result.stats.entities_unchanged,
                    "ours_only": merge_result.stats.entities_ours_only,
                    "theirs_only": merge_result.stats.entities_theirs_only,
                    "auto_merged": merge_result.stats.entities_both_changed_merged,
                    "added_ours": merge_result.stats.entities_added_ours,
                    "added_theirs": merge_result.stats.entities_added_theirs,
                    "deleted": merge_result.stats.entities_deleted,
                    "conflicted": merge_result.stats.entities_conflicted,
                    "resolved_via_diffy": merge_result.stats.resolved_via_diffy,
                    "resolved_via_inner_merge": merge_result.stats.resolved_via_inner_merge,
                },
                "conflicts": conflicts,
                "warnings": warnings,
            }));
        }

        let clean_count = results.iter().filter(|r| r["clean"].as_bool().unwrap_or(true)).count();
        let conflict_count = results.len() - clean_count;
        let overall_confidence = if conflict_count > 0 {
            "conflict"
        } else if results.iter().any(|r| r["confidence"].as_str() == Some("medium")) {
            "medium"
        } else if results.iter().any(|r| r["confidence"].as_str() == Some("high")) {
            "high"
        } else {
            "very_high"
        };

        let summary = serde_json::json!({
            "files_analyzed": results.len(),
            "files_clean": clean_count,
            "files_with_conflicts": conflict_count,
            "overall_confidence": overall_confidence,
            "results": results,
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&summary).unwrap_or_default(),
        )]))
    }

    #[tool(description = "Get entities that the given entity depends on (calls, references, imports)")]
    async fn weave_get_dependencies(
        &self,
        Parameters(params): Parameters<EntityDepsParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self
            .get_context(Some(&params.file_path))
            .await
            .map_err(internal_err)?;
        let (rel_path, _abs_path) =
            Self::resolve_file_path(&ctx.repo_root, &params.file_path);

        // Build graph from all supported files in the repo
        let file_paths = Self::find_supported_files(&ctx.repo_root, &self.registry);
        let graph = EntityGraph::build(&ctx.repo_root, &file_paths, &self.registry);

        // Find the entity by name in the target file
        let entity_id = graph
            .entities
            .values()
            .find(|e| e.name == params.entity_name && e.file_path == rel_path)
            .or_else(|| graph.entities.values().find(|e| e.name == params.entity_name))
            .map(|e| e.id.clone())
            .ok_or_else(|| internal_err(format!("Entity '{}' not found in graph", params.entity_name)))?;

        let deps = graph.get_dependencies(&entity_id);
        let result: Vec<serde_json::Value> = deps
            .iter()
            .map(|d| {
                serde_json::json!({
                    "name": d.name,
                    "type": d.entity_type,
                    "file": d.file_path,
                    "lines": [d.start_line, d.end_line],
                })
            })
            .collect();

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&serde_json::json!({
                "entity": params.entity_name,
                "file": rel_path,
                "dependencies": result,
            }))
            .unwrap_or_default(),
        )]))
    }

    #[tool(description = "Get entities that depend on the given entity (reverse dependencies — who calls/references it)")]
    async fn weave_get_dependents(
        &self,
        Parameters(params): Parameters<EntityDepsParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self
            .get_context(Some(&params.file_path))
            .await
            .map_err(internal_err)?;
        let (rel_path, _abs_path) =
            Self::resolve_file_path(&ctx.repo_root, &params.file_path);

        let file_paths = Self::find_supported_files(&ctx.repo_root, &self.registry);
        let graph = EntityGraph::build(&ctx.repo_root, &file_paths, &self.registry);

        let entity_id = graph
            .entities
            .values()
            .find(|e| e.name == params.entity_name && e.file_path == rel_path)
            .or_else(|| graph.entities.values().find(|e| e.name == params.entity_name))
            .map(|e| e.id.clone())
            .ok_or_else(|| internal_err(format!("Entity '{}' not found in graph", params.entity_name)))?;

        let deps = graph.get_dependents(&entity_id);
        let result: Vec<serde_json::Value> = deps
            .iter()
            .map(|d| {
                serde_json::json!({
                    "name": d.name,
                    "type": d.entity_type,
                    "file": d.file_path,
                    "lines": [d.start_line, d.end_line],
                })
            })
            .collect();

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&serde_json::json!({
                "entity": params.entity_name,
                "file": rel_path,
                "dependents": result,
            }))
            .unwrap_or_default(),
        )]))
    }

    #[tool(description = "Impact analysis: if this entity changes, what else might be affected? Returns all transitive dependents.")]
    async fn weave_impact_analysis(
        &self,
        Parameters(params): Parameters<ImpactAnalysisParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self
            .get_context(Some(&params.file_path))
            .await
            .map_err(internal_err)?;
        let (rel_path, _abs_path) =
            Self::resolve_file_path(&ctx.repo_root, &params.file_path);

        let file_paths = Self::find_supported_files(&ctx.repo_root, &self.registry);
        let graph = EntityGraph::build(&ctx.repo_root, &file_paths, &self.registry);

        let entity_id = graph
            .entities
            .values()
            .find(|e| e.name == params.entity_name && e.file_path == rel_path)
            .or_else(|| graph.entities.values().find(|e| e.name == params.entity_name))
            .map(|e| e.id.clone())
            .ok_or_else(|| internal_err(format!("Entity '{}' not found in graph", params.entity_name)))?;

        let impact = graph.impact_analysis(&entity_id);
        let result: Vec<serde_json::Value> = impact
            .iter()
            .map(|d| {
                serde_json::json!({
                    "name": d.name,
                    "type": d.entity_type,
                    "file": d.file_path,
                    "lines": [d.start_line, d.end_line],
                })
            })
            .collect();

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&serde_json::json!({
                "entity": params.entity_name,
                "file": rel_path,
                "total_affected": result.len(),
                "affected_entities": result,
            }))
            .unwrap_or_default(),
        )]))
    }

    #[tool(description = "Register an agent in weave's coordination state")]
    async fn weave_agent_register(
        &self,
        Parameters(params): Parameters<AgentRegisterParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self.get_context(None).await.map_err(internal_err)?;
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
        let ctx = self.get_context(None).await.map_err(internal_err)?;
        let mut state = ctx.state.lock().await;
        weave_crdt::agent_heartbeat(&mut state, &params.agent_id, &params.working_on)
            .map_err(|e| internal_err(e.to_string()))?;
        let _ = state.save();

        Ok(CallToolResult::success(vec![Content::text("OK")]))
    }

    #[tool(description = "Semantic diff between two refs: shows entity-level changes (added, modified, deleted, renamed) instead of line-level diffs")]
    async fn weave_diff(
        &self,
        Parameters(params): Parameters<DiffParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let _ctx = self
            .get_context(params.file_path.as_deref())
            .await
            .map_err(internal_err)?;

        let target_ref = params.target_ref.as_deref().unwrap_or("HEAD");

        let files = if let Some(ref fp) = params.file_path {
            let p = Path::new(fp);
            if p.is_absolute() {
                let root = git::find_repo_root_from_path(p)
                    .map_err(|e| internal_err(e.to_string()))?;
                let rel = p.strip_prefix(&root)
                    .map(|r| r.to_string_lossy().to_string())
                    .unwrap_or_else(|_| fp.clone());
                vec![rel]
            } else {
                vec![fp.clone()]
            }
        } else {
            git::diff_files(&params.base_ref, target_ref)
                .map_err(|e| internal_err(e.to_string()))?
        };

        let mut all_changes = Vec::new();

        for file in &files {
            let plugin = match self.registry.get_plugin(file) {
                Some(p) => p,
                None => continue, // skip unsupported files
            };

            let base_content = git::git_show(&params.base_ref, file).unwrap_or_default();
            let target_content = git::git_show(target_ref, file).unwrap_or_default();

            let base_entities = plugin.extract_entities(&base_content, file);
            let target_entities = plugin.extract_entities(&target_content, file);

            let match_result = sem_core::model::identity::match_entities(
                &base_entities,
                &target_entities,
                file,
                None,
                None,
                None,
            );

            for change in match_result.changes {
                all_changes.push(serde_json::json!({
                    "file": file,
                    "entity_name": change.entity_name,
                    "entity_type": change.entity_type,
                    "change_type": change.change_type.to_string(),
                }));
            }
        }

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&serde_json::json!({
                "base_ref": params.base_ref,
                "target_ref": target_ref,
                "files_analyzed": files.len(),
                "total_changes": all_changes.len(),
                "changes": all_changes,
            }))
            .unwrap_or_default(),
        )]))
    }

    #[tool(description = "Parse weave conflict markers in a file and return a structured summary with entity names, conflict types, confidence levels, and resolution hints")]
    async fn weave_merge_summary(
        &self,
        Parameters(params): Parameters<MergeSummaryParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self
            .get_context(Some(&params.file_path))
            .await
            .map_err(internal_err)?;
        let (_rel_path, abs_path) =
            Self::resolve_file_path(&ctx.repo_root, &params.file_path);

        let content = Self::read_file_at(&abs_path, &params.file_path).map_err(internal_err)?;
        let conflicts = weave_core::parse_weave_conflicts(&content);

        let json_conflicts: Vec<serde_json::Value> = conflicts
            .iter()
            .map(|c| {
                serde_json::json!({
                    "entity": c.entity_name,
                    "kind": c.entity_kind,
                    "complexity": format!("{}", c.complexity),
                    "confidence": c.confidence,
                    "hint": c.hint,
                })
            })
            .collect();

        let output = serde_json::json!({
            "file": params.file_path,
            "conflict_count": conflicts.len(),
            "conflicts": json_conflicts,
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&output).unwrap_or_default(),
        )]))
    }

    #[tool(description = "Validate a merge for semantic risks: detect when auto-merged entities reference other entities that were also modified")]
    async fn weave_validate_merge(
        &self,
        Parameters(params): Parameters<ValidateMergeParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self
            .get_context(params.file_path.as_deref())
            .await
            .map_err(internal_err)?;

        let merge_base = git::find_merge_base(&params.base_branch, &params.target_branch)
            .map_err(|e| internal_err(e.to_string()))?;

        let files = if let Some(ref fp) = params.file_path {
            let (rel, _) = Self::resolve_file_path(&ctx.repo_root, fp);
            vec![rel]
        } else {
            git::get_changed_files(&merge_base, &params.base_branch, &params.target_branch)
                .map_err(|e| internal_err(e.to_string()))?
        };

        // Collect modified entities from both branches
        let mut modified_entities = Vec::new();
        for file in &files {
            let base_content = git::git_show(&merge_base, file).unwrap_or_default();
            let ours_content = git::git_show(&params.base_branch, file).unwrap_or_default();
            let theirs_content = git::git_show(&params.target_branch, file).unwrap_or_default();

            if let Some(plugin) = self.registry.get_plugin(file) {
                let base_entities = plugin.extract_entities(&base_content, file);
                let ours_entities = plugin.extract_entities(&ours_content, file);
                let theirs_entities = plugin.extract_entities(&theirs_content, file);

                // Find entities modified in ours or theirs vs base
                for entity in ours_entities.iter().chain(theirs_entities.iter()) {
                    let base_match = base_entities.iter().find(|b| b.name == entity.name);
                    let is_modified = match base_match {
                        Some(b) => b.content_hash != entity.content_hash,
                        None => true, // new entity
                    };
                    if is_modified {
                        modified_entities.push(weave_core::ModifiedEntity {
                            name: entity.name.clone(),
                            file_path: file.clone(),
                        });
                    }
                }
            }
        }

        // Deduplicate
        modified_entities.sort_by(|a, b| (&a.file_path, &a.name).cmp(&(&b.file_path, &b.name)));
        modified_entities.dedup_by(|a, b| a.file_path == b.file_path && a.name == b.name);

        let all_files = Self::find_supported_files(&ctx.repo_root, &self.registry);
        let warnings = weave_core::validate_merge(
            &ctx.repo_root,
            &all_files,
            &modified_entities,
            &self.registry,
        );

        let result: Vec<serde_json::Value> = warnings
            .iter()
            .map(|w| {
                serde_json::json!({
                    "entity": w.entity_name,
                    "entity_type": w.entity_type,
                    "file": w.file_path,
                    "warning": w.to_string(),
                    "related": w.related.iter().map(|r| serde_json::json!({
                        "name": r.name,
                        "type": r.entity_type,
                        "file": r.file_path,
                    })).collect::<Vec<_>>(),
                })
            })
            .collect();

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&serde_json::json!({
                "modified_entities": modified_entities.len(),
                "warnings": result.len(),
                "details": result,
            }))
            .unwrap_or_default(),
        )]))
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
