use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use lru::LruCache;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content, ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use sem_core::model::entity::SemanticEntity;
use sem_core::parser::graph::{EntityGraph, EntityInfo};
use sem_core::parser::plugins::create_default_registry;
use sem_core::parser::registry::ParserRegistry;
use tokio::sync::Mutex;

use weave_core::git;
use weave_crdt::{
    claim_entity, detect_potential_conflicts, get_entities_for_file, get_entity_content,
    get_entity_status, merge_file_entities, register_agent, release_entity, resolve_entity,
    resolve_entity_conflict, sync_from_files, update_entity_content, upsert_entity, EntityAddress,
    EntityStateDoc, Resolution,
};

use crate::error::{Reason, ToolError};
use crate::tools::*;

/// Lazily-initialized repo context. Created on first tool call.
struct RepoContext {
    state: Mutex<EntityStateDoc>,
    repo_root: PathBuf,
}

/// LRU cache for parsed entities keyed on (file_path, content_hash).
/// Avoids redundant tree-sitter parses when the same file is accessed multiple times.
type EntityCache = LruCache<(String, u64), Vec<SemanticEntity>>;

fn content_hash_u64(content: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

#[derive(Clone)]
pub(crate) struct WeaveServer {
    context: Arc<Mutex<Option<RepoContext>>>,
    registry: Arc<ParserRegistry>,
    entity_cache: Arc<Mutex<EntityCache>>,
    /// What a merge run by this server may reach outside itself. Granted once,
    /// in `new`, and passed to every merge — nothing below can widen it.
    host: weave_core::host::Host,
    /// Built by `#[tool_router]` and consumed by `#[tool_handler]`'s generated
    /// `list_tools`/`call_tool`, which read it through the trait rather than
    /// through this field — so a non-test build sees no direct read. It is
    /// read directly by `description_tests::catalog`, which is how the tool
    /// catalog gets asserted on; the `allow` covers the shipped build only.
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

impl WeaveServer {
    /// Discover repo root using multiple strategies:
    /// 1. If file_path is absolute, derive repo from that path
    /// 2. WEAVE_REPO env var
    /// 3. CWD-based git discovery
    fn discover_repo_root(file_path_hint: Option<&str>) -> Result<PathBuf, ToolError> {
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
        match git::find_repo_root(Path::new(".")) {
            Ok(root) => return Ok(root),
            // "git is not installed" is a different diagnosis from "you are
            // not in a repository", and only one of the two is fixed by
            // passing a different path. Reporting them as one was the whole
            // cost of a stringly error here.
            Err(e @ git::GitError::NotRunnable { .. }) => return Err(e.into()),
            Err(_) => {}
        }

        Err(Reason::NoRepository.into())
    }

    /// Resolve a file path to (repo_root-relative path, absolute path).
    /// Handles both absolute and relative paths.
    fn resolve_file_path(repo_root: &Path, file_path: &str) -> (String, PathBuf) {
        let p = Path::new(file_path);
        if p.is_absolute() {
            // Convert absolute -> relative to repo root. A direct `strip_prefix`
            // fails when one side is a symlink and the other is git's canonical
            // toplevel (macOS `/var` vs `/private/var`, a symlinked checkout),
            // so retry on the canonicalized pair before giving up. Falling back
            // to the raw absolute path yields `git show <rev>:/abs/...`, which
            // git reads as a repo-root-relative path that never matches — a
            // read that silently answers for the wrong (or no) file.
            let relative = p
                .strip_prefix(repo_root)
                .map(|r| r.to_string_lossy().to_string())
                .ok()
                .or_else(|| {
                    let cr = std::fs::canonicalize(repo_root).ok()?;
                    let cp = std::fs::canonicalize(p).ok()?;
                    cp.strip_prefix(&cr)
                        .ok()
                        .map(|r| r.to_string_lossy().to_string())
                })
                .unwrap_or_else(|| file_path.to_string());
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
    ) -> Result<tokio::sync::MappedMutexGuard<'_, RepoContext>, ToolError> {
        {
            let mut guard = self.context.lock().await;
            if guard.is_none() {
                let repo_root = Self::discover_repo_root(file_path_hint)?;
                let state_path = repo_root.join(".weave").join("state.automerge");
                let state =
                    EntityStateDoc::open(&state_path).map_err(|e| Reason::state(&state_path, e))?;
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

    fn walk_dir(dir: &Path, root: &Path, registry: &ParserRegistry, files: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
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

    fn read_file_at(abs_path: &Path, display_path: &str) -> Result<String, ToolError> {
        std::fs::read_to_string(abs_path).map_err(|source| {
            Reason::Unreadable {
                path: display_path.to_string(),
                source,
            }
            .into()
        })
    }

    /// Resolve an entity address to its ID via weave-crdt's shared resolver.
    ///
    /// Never picks first: on ambiguity the error lists every candidate and
    /// tells the caller which address field to add.
    #[allow(clippy::too_many_arguments)]
    fn resolve_entity_sync(
        registry: &ParserRegistry,
        content: &str,
        file_path: &str,
        entity_name: &str,
        entity_type: Option<&str>,
        parent_name: Option<&str>,
        ordinal: Option<u32>,
    ) -> Result<String, ToolError> {
        let mut address = EntityAddress::by_name(entity_name);
        if let Some(t) = entity_type {
            address = address.with_type(t);
        }
        if let Some(p) = parent_name {
            address = address.with_parent(p);
        }
        if let Some(o) = ordinal {
            address = address.with_ordinal(o);
        }
        match resolve_entity(content, file_path, registry, &address) {
            Resolution::Resolved(id) => Ok(id),
            Resolution::NotFound => Err(Reason::EntityNotFound {
                entity: entity_name.to_string(),
                file: file_path.to_string(),
            }
            .into()),
            resolution @ Resolution::Ambiguous(_) => Err(Reason::EntityAmbiguous(
                resolution.describe_failure(&address, file_path),
            )
            .into()),
        }
    }

    /// Derive the enclosing entity's display name from its ID.
    ///
    /// Private mirror of `weave_crdt::resolve`'s internal `parent_name_of`,
    /// needed here to apply `parent_name` filters to graph nodes during the
    /// repo-wide fallback below (parent IDs look like
    /// `src/lib.rs::class::Animal`, with optional disambiguator suffixes such
    /// as `Animal@L1#1`).
    fn fallback_parent_name_of(entity: &EntityInfo) -> Option<String> {
        let parent_id = entity.parent_id.as_deref()?;
        let last = parent_id.rsplit("::").next().unwrap_or(parent_id);
        let name = match last.split_once('@') {
            Some((name, _)) => name,
            None => last,
        };
        if name.is_empty() {
            None
        } else {
            Some(name.to_string())
        }
    }

    /// Resolve an entity for the read-only graph-analysis tools.
    ///
    /// Primary path is the shared single-file resolver
    /// ([`Self::resolve_entity_sync`]), which refuses ambiguity with a
    /// candidate-listing error. An in-file [`Reason::EntityAmbiguous`] is
    /// TERMINAL: the addressed file already has more than one candidate, so
    /// naming it that precisely is itself sufficient information for the
    /// caller to narrow with `entity_type`/`parent_name`/`ordinal` — falling
    /// back to a repo-wide scan here would either re-derive the same
    /// ambiguity under the misleading label "ambiguous across the repo" (the
    /// candidates aren't spread across the repo, they're all in this one
    /// file) or, worse, silently resolve if some other in-repo candidate set
    /// happened to collapse to a single survivor. Only an in-file `NotFound`
    /// falls back to scanning every graph node with the same progressive
    /// filters (the old name-only lookup fell back from file-scoped to
    /// whole-graph too) — again never picking first: multiple survivors are
    /// an error listing each candidate (with its file), and `ordinal` indexes
    /// survivors in deterministic (file path, start line) order. If nothing
    /// matches anywhere, the resolver's original `NotFound` is surfaced. An
    /// in-file resolution that already succeeded is trusted outright and
    /// never overridden by the repo-wide scan — the addressed file is
    /// sufficient disambiguation on its own.
    #[allow(clippy::too_many_arguments)]
    fn resolve_entity_for_graph(
        registry: &ParserRegistry,
        content: &str,
        rel_path: &str,
        graph: &EntityGraph,
        entity_name: &str,
        entity_type: Option<&str>,
        parent_name: Option<&str>,
        ordinal: Option<u32>,
    ) -> Result<String, ToolError> {
        let resolver_result = Self::resolve_entity_sync(
            registry,
            content,
            rel_path,
            entity_name,
            entity_type,
            parent_name,
            ordinal,
        );

        // The addressed file already disambiguated the entity — trust it.
        // Fall back to the repo-wide scan below ONLY when the in-file
        // resolver found nothing at all (NotFound), so a same-named entity
        // elsewhere in the repo never overrides an unambiguous in-file
        // match. An in-file Ambiguous is terminal — never masked as
        // repo-wide, and never re-collapsed by the graph-wide scan.
        match resolver_result {
            Ok(_) => return resolver_result,
            Err(ToolError {
                reason: Reason::EntityAmbiguous(_),
                ..
            }) => return resolver_result,
            Err(_) => {}
        }

        let mut candidates: Vec<&EntityInfo> = graph
            .entities
            .values()
            .filter(|e| e.name == entity_name)
            .filter(|e| match entity_type {
                Some(t) => e.entity_type == t,
                None => true,
            })
            .filter(|e| match parent_name {
                Some(p) => Self::fallback_parent_name_of(e).as_deref() == Some(p),
                None => true,
            })
            .collect();
        candidates.sort_by(|a, b| {
            (&a.file_path, a.start_line, &a.id).cmp(&(&b.file_path, b.start_line, &b.id))
        });

        match candidates.len() {
            0 => resolver_result,
            // Unambiguous repo-wide: keep the historical cross-file lookup.
            1 => Ok(candidates[0].id.clone()),
            _ => {
                if let Some(o) = ordinal {
                    return candidates
                        .get(o as usize)
                        .map(|e| e.id.clone())
                        .ok_or_else(|| {
                            Reason::EntityAmbiguous(format!(
                                "ordinal {} out of range: {} candidates named '{}' outside '{}'",
                                o,
                                candidates.len(),
                                entity_name,
                                rel_path
                            ))
                            .into()
                        });
                }
                let type_q = entity_type.unwrap_or("<any>");
                let parent_q = parent_name.unwrap_or("<any>");
                let mut msg = format!(
                    "Entity '{}' (type=`{}`, parent=`{}`) is ambiguous across the repo: \
                     {} candidates match.\nCandidates (repo order):",
                    entity_name,
                    type_q,
                    parent_q,
                    candidates.len()
                );
                for c in candidates {
                    msg.push_str(&format!(
                        "\n  {} `{}` in `{}` (parent: {}) at line {}",
                        c.entity_type,
                        c.name,
                        c.file_path,
                        Self::fallback_parent_name_of(c)
                            .as_deref()
                            .unwrap_or("<none>"),
                        c.start_line
                    ));
                }
                msg.push_str(
                    "\nDisambiguate by adding one of: entity_type, parent_name, or \
                     ordinal (0-based, see list above), or point file_path at the \
                     file containing the entity.",
                );
                Err(Reason::EntityAmbiguous(msg).into())
            }
        }
    }

    /// Extract entities with LRU caching. Cache hit skips tree-sitter parse entirely.
    async fn cached_extract_entities(&self, content: &str, rel_path: &str) -> Vec<SemanticEntity> {
        let hash = content_hash_u64(content);
        let key = (rel_path.to_string(), hash);

        // Check cache
        {
            let mut cache = self.entity_cache.lock().await;
            if let Some(entities) = cache.get(&key) {
                return entities.clone();
            }
        }

        // Cache miss: parse
        let Some(plugin) = self.registry.get_plugin(rel_path) else {
            return Vec::new();
        };
        let entities = plugin.extract_entities(content, rel_path);

        // Store in cache
        {
            let mut cache = self.entity_cache.lock().await;
            cache.put(key, entities.clone());
        }

        entities
    }
}

#[tool_router]
impl WeaveServer {
    pub(crate) fn new() -> Self {
        Self {
            context: Arc::new(Mutex::new(None)),
            registry: Arc::new(create_default_registry()),
            entity_cache: Arc::new(Mutex::new(LruCache::new(
                std::num::NonZeroUsize::new(500).unwrap(),
            ))),
            host: weave_core::host::Host {
                line_merge: Some(weave_core::host::git_line_merge),
                ..Default::default()
            },
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "List every semantic entity (function, class, etc.) in a file, with type and line range. Use before weave_claim_entity to find the exact entity name to claim, or to scope any other tool call to one entity instead of a whole file."
    )]
    async fn weave_extract_entities(
        &self,
        Parameters(params): Parameters<ExtractEntitiesParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self.get_context(Some(&params.file_path)).await?;
        let (rel_path, abs_path) = Self::resolve_file_path(&ctx.repo_root, &params.file_path);
        let content = Self::read_file_at(&abs_path, &rel_path)?;

        let entities = self.cached_extract_entities(&content, &rel_path).await;
        if entities.is_empty() && self.registry.get_plugin(&rel_path).is_none() {
            return Err(internal_err(format!("No parser for file: {}", rel_path)));
        }
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

    #[tool(
        description = "Advisory lock on one entity, taken before you edit it — signals other agents to stay out; weave does not enforce it. Call this first when multiple agents share a repo live. The response includes warnings when an entity you depend on, or that depends on you, is already claimed by someone else, so you can wait instead of colliding."
    )]
    async fn weave_claim_entity(
        &self,
        Parameters(params): Parameters<ClaimEntityParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self.get_context(Some(&params.file_path)).await?;
        let (rel_path, abs_path) = Self::resolve_file_path(&ctx.repo_root, &params.file_path);
        let content = Self::read_file_at(&abs_path, &rel_path)?;
        let entity_id = Self::resolve_entity_sync(
            &self.registry,
            &content,
            &rel_path,
            &params.entity_name,
            params.entity_type.as_deref(),
            params.parent_name.as_deref(),
            params.ordinal,
        )?;

        let mut state = ctx.state.lock().await;
        let entities = self.cached_extract_entities(&content, &rel_path).await;
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

        let result =
            claim_entity(&mut state, &params.agent_id, &entity_id).map_err(internal_err)?;

        let _ = state.save();

        // Predictive conflict detection: check if any entity in the
        // dependency chain is claimed by another agent
        let mut dep_warnings: Vec<serde_json::Value> = Vec::new();
        let file_paths = Self::find_supported_files(&ctx.repo_root, &self.registry);
        let (graph, _entities) = EntityGraph::build(&ctx.repo_root, &file_paths, &self.registry);

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
                                    graph_entity.entity_type, params.entity_name,
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
                                    graph_entity.entity_type, params.entity_name,
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
            // The claim's own stable identity. Pass it back to
            // weave_update_entity_content / weave_release_entity to address
            // the claim directly — a rename of the entity between claim and
            // update/release makes the *name* unresolvable against the file's
            // current content, but never invalidates this id.
            "entity_id": entity_id,
            "dependency_warnings": dep_warnings,
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&response).unwrap_or_default(),
        )]))
    }

    #[tool(
        description = "Release a lock taken with weave_claim_entity. Call this once your edit is either written back with weave_update_entity_content or abandoned — a stale claim blocks other agents from claiming the same entity."
    )]
    async fn weave_release_entity(
        &self,
        Parameters(params): Parameters<ReleaseEntityParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self.get_context(Some(&params.file_path)).await?;
        let (rel_path, abs_path) = Self::resolve_file_path(&ctx.repo_root, &params.file_path);
        let content = Self::read_file_at(&abs_path, &rel_path)?;
        // The entity_id from weave_claim_entity addresses the claim directly —
        // name resolution runs against the file's *current* content, so a
        // rename between claim and release would make the old name
        // unresolvable even though the claim is still held.
        let entity_id = match params.entity_id {
            Some(id) => id,
            None => Self::resolve_entity_sync(
                &self.registry,
                &content,
                &rel_path,
                &params.entity_name,
                params.entity_type.as_deref(),
                params.parent_name.as_deref(),
                params.ordinal,
            )?,
        };

        let mut state = ctx.state.lock().await;
        release_entity(&mut state, &params.agent_id, &entity_id).map_err(internal_err)?;
        let _ = state.save();

        Ok(CallToolResult::success(vec![Content::text(
            "Released successfully",
        )]))
    }

    #[tool(
        description = "Every entity in a file with its claim owner, last editor, version, and merge_state. Use before editing a file you didn't just extract entities from, to see at a glance what's claimed and what's already mid-conflict."
    )]
    async fn weave_status(
        &self,
        Parameters(params): Parameters<StatusParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self.get_context(Some(&params.file_path)).await?;
        let (rel_path, abs_path) = Self::resolve_file_path(&ctx.repo_root, &params.file_path);
        let content = Self::read_file_at(&abs_path, &rel_path)?;

        let mut state = ctx.state.lock().await;
        let _ = sync_from_files(
            &mut state,
            &ctx.repo_root,
            std::slice::from_ref(&rel_path),
            &self.registry,
        );

        let entities = get_entities_for_file(&state, &rel_path).map_err(internal_err)?;

        let file_entities = self.cached_extract_entities(&content, &rel_path).await;

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
                    "version_vector": status.map(|s| serde_json::to_value(&s.version_vector).unwrap_or_default()),
                    "merge_state": status.map(|s| s.merge_state.as_str()).unwrap_or("clean"),
                })
            })
            .collect();

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }

    #[tool(
        description = "Who claimed one entity and who last modified it. Use instead of weave_status when you already know the entity's name and just need a fast yes/no before editing it."
    )]
    async fn weave_who_is_editing(
        &self,
        Parameters(params): Parameters<WhoIsEditingParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self.get_context(Some(&params.file_path)).await?;
        let (rel_path, abs_path) = Self::resolve_file_path(&ctx.repo_root, &params.file_path);
        let content = Self::read_file_at(&abs_path, &rel_path)?;
        let entity_id = Self::resolve_entity_sync(
            &self.registry,
            &content,
            &rel_path,
            &params.entity_name,
            params.entity_type.as_deref(),
            params.parent_name.as_deref(),
            params.ordinal,
        )?;

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

    #[tool(
        description = "Scan the whole coordination state for entities more than one agent has claimed or edited. Run this periodically, or right before merging, to catch a collision while it's still two claims rather than a conflict marker. No results means no entity currently has more than one agent's claim on it — it is not a guarantee that a later git merge will be clean."
    )]
    async fn weave_potential_conflicts(
        &self,
        Parameters(params): Parameters<PotentialConflictsParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self.get_context(None).await?;
        let state = ctx.state.lock().await;
        let mut conflicts = detect_potential_conflicts(&state).map_err(internal_err)?;

        if let Some(ref agent_id) = params.agent_id {
            conflicts.retain(|c| c.agents.contains(agent_id));
        }

        let result: Vec<serde_json::Value> = conflicts
            .iter()
            .map(|c| {
                // Try to get version vector for richer conflict info
                let vv = get_entity_status(&state, &c.entity_id)
                    .ok()
                    .map(|s| serde_json::to_value(&s.version_vector).unwrap_or_default());
                let ms = get_entity_status(&state, &c.entity_id)
                    .ok()
                    .map(|s| s.merge_state)
                    .unwrap_or_else(|| "unknown".to_string());
                serde_json::json!({
                    "entity_id": c.entity_id,
                    "entity_name": c.entity_name,
                    "file_path": c.file_path,
                    "agents": c.agents,
                    "version_vector": vv,
                    "merge_state": ms,
                })
            })
            .collect();

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }

    #[tool(
        description = "Dry-run a merge between two branches — writes nothing, just returns per-file clean/conflict verdicts, a confidence rating, and entity-level stats. Use for a fast go/no-go signal before merging. When you intend to act on the result (resolve a conflict, check for cross-file breakage), use weave_findings instead — it's the typed contract, this is the summary."
    )]
    async fn weave_preview_merge(
        &self,
        Parameters(params): Parameters<PreviewMergeParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self.get_context(params.file_path.as_deref()).await?;

        // Run git commands from the repo root
        let merge_base =
            git::find_merge_base(&ctx.repo_root, &params.base_branch, &params.target_branch)
                .map_err(internal_err)?;

        let files = if let Some(ref fp) = params.file_path {
            let (rel, _) = Self::resolve_file_path(&ctx.repo_root, fp);
            vec![rel]
        } else {
            git::get_changed_files(
                &ctx.repo_root,
                &merge_base,
                &params.base_branch,
                &params.target_branch,
            )
            .map_err(internal_err)?
        };

        let mut results = Vec::new();
        for file in &files {
            let base = git::git_show_optional(&ctx.repo_root, &merge_base, file)
                .map_err(ToolError::from)?
                .unwrap_or_default();
            let ours = git::git_show_optional(&ctx.repo_root, &params.base_branch, file)
                .map_err(ToolError::from)?
                .unwrap_or_default();
            let theirs = git::git_show_optional(&ctx.repo_root, &params.target_branch, file)
                .map_err(ToolError::from)?
                .unwrap_or_default();

            if ours == theirs || base == ours || base == theirs {
                continue;
            }

            let merge_result = weave_core::entity_merge_with_registry(
                &base,
                &ours,
                &theirs,
                file,
                &self.registry,
                &weave_core::MarkerFormat::default(),
                &self.host,
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

        let clean_count = results
            .iter()
            .filter(|r| r["clean"].as_bool().unwrap_or(true))
            .count();
        let conflict_count = results.len() - clean_count;
        let overall_confidence = if conflict_count > 0 {
            "conflict"
        } else if results
            .iter()
            .any(|r| r["confidence"].as_str() == Some("medium"))
        {
            "medium"
        } else if results
            .iter()
            .any(|r| r["confidence"].as_str() == Some("high"))
        {
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

    #[tool(
        description = "What one entity calls, references, or imports. Check this before editing or resolving it — if anything in the list is claimed by another agent (weave_who_is_editing), your change may collide with theirs even though the two entities live in different files."
    )]
    async fn weave_get_dependencies(
        &self,
        Parameters(params): Parameters<EntityDepsParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self.get_context(Some(&params.file_path)).await?;
        let (rel_path, abs_path) = Self::resolve_file_path(&ctx.repo_root, &params.file_path);
        let content = Self::read_file_at(&abs_path, &rel_path)?;

        // Build graph from all supported files in the repo
        let file_paths = Self::find_supported_files(&ctx.repo_root, &self.registry);
        let (graph, _entities) = EntityGraph::build(&ctx.repo_root, &file_paths, &self.registry);

        // Shared typed resolver; falls back to a filtered, never-pick-first
        // repo-wide scan (see resolve_entity_for_graph).
        let entity_id = Self::resolve_entity_for_graph(
            &self.registry,
            &content,
            &rel_path,
            &graph,
            &params.entity_name,
            params.entity_type.as_deref(),
            params.parent_name.as_deref(),
            params.ordinal,
        )?;

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

    #[tool(
        description = "Who calls or references one entity (reverse of weave_get_dependencies). Check this before deleting, renaming, or changing the signature of an entity — every result is a caller that may need updating too."
    )]
    async fn weave_get_dependents(
        &self,
        Parameters(params): Parameters<EntityDepsParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self.get_context(Some(&params.file_path)).await?;
        let (rel_path, abs_path) = Self::resolve_file_path(&ctx.repo_root, &params.file_path);
        let content = Self::read_file_at(&abs_path, &rel_path)?;

        let file_paths = Self::find_supported_files(&ctx.repo_root, &self.registry);
        let (graph, _entities) = EntityGraph::build(&ctx.repo_root, &file_paths, &self.registry);

        // Shared typed resolver; falls back to a filtered, never-pick-first
        // repo-wide scan (see resolve_entity_for_graph).
        let entity_id = Self::resolve_entity_for_graph(
            &self.registry,
            &content,
            &rel_path,
            &graph,
            &params.entity_name,
            params.entity_type.as_deref(),
            params.parent_name.as_deref(),
            params.ordinal,
        )?;

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

    #[tool(
        description = "The full transitive blast radius of changing one entity — not just direct callers (weave_get_dependents), every dependent of every dependent. Use before a wide or risky edit, so you know what's affected up front instead of finding out from a DANGLING finding in weave_check afterward."
    )]
    async fn weave_impact_analysis(
        &self,
        Parameters(params): Parameters<ImpactAnalysisParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self.get_context(Some(&params.file_path)).await?;
        let (rel_path, abs_path) = Self::resolve_file_path(&ctx.repo_root, &params.file_path);
        let content = Self::read_file_at(&abs_path, &rel_path)?;

        let file_paths = Self::find_supported_files(&ctx.repo_root, &self.registry);
        let (graph, _entities) = EntityGraph::build(&ctx.repo_root, &file_paths, &self.registry);

        // Shared typed resolver; falls back to a filtered, never-pick-first
        // repo-wide scan (see resolve_entity_for_graph).
        let entity_id = Self::resolve_entity_for_graph(
            &self.registry,
            &content,
            &rel_path,
            &graph,
            &params.entity_name,
            params.entity_type.as_deref(),
            params.parent_name.as_deref(),
            params.ordinal,
        )?;

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

    #[tool(
        description = "Announce yourself in the CRDT coordination state, with an agent_id and the branch you're on. Call this once before claiming or editing anything, so your claims and edits are attributed to an agent other agents can see and wait on."
    )]
    async fn weave_agent_register(
        &self,
        Parameters(params): Parameters<AgentRegisterParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self.get_context(None).await?;
        let mut state = ctx.state.lock().await;
        register_agent(
            &mut state,
            &params.agent_id,
            &params.agent_id,
            &params.branch,
        )
        .map_err(internal_err)?;
        let _ = state.save();

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Agent '{}' registered on branch '{}'",
            params.agent_id, params.branch
        ))]))
    }

    #[tool(
        description = "Refresh your agent's liveness timestamp and replace the list of entities you currently hold. Call periodically while working (after weave_agent_register), and whenever the set of entities you are editing changes, so other agents' weave_status and weave_potential_conflicts calls reflect what you actually hold. Note: weave does not currently reap agents that stop heartbeating, so a crashed agent's claims stay visible until released."
    )]
    async fn weave_agent_heartbeat(
        &self,
        Parameters(params): Parameters<AgentHeartbeatParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self.get_context(None).await?;
        let mut state = ctx.state.lock().await;
        weave_crdt::agent_heartbeat(&mut state, &params.agent_id, &params.working_on)
            .map_err(internal_err)?;
        let _ = state.save();

        Ok(CallToolResult::success(vec![Content::text("OK")]))
    }

    #[tool(
        description = "Entity-level diff between two refs — which functions/classes were added, modified, deleted, or renamed, not which lines changed. Use to scope a review or a merge preview to what actually changed structurally, before running weave_preview_merge or weave_findings on it."
    )]
    async fn weave_diff(
        &self,
        Parameters(params): Parameters<DiffParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self.get_context(params.file_path.as_deref()).await?;

        let target_ref = params.target_ref.as_deref().unwrap_or("HEAD");

        let files = if let Some(ref fp) = params.file_path {
            // Relativize against the repository the context discovered — the
            // same root every content read below runs in — rather than
            // re-deriving a root from the path and leaving the reads ambient.
            let (rel, _) = Self::resolve_file_path(&ctx.repo_root, fp);
            vec![rel]
        } else {
            git::diff_files(&ctx.repo_root, &params.base_ref, target_ref).map_err(internal_err)?
        };

        let mut all_changes = Vec::new();

        for file in &files {
            // skip unsupported files
            let Some(plugin) = self.registry.get_plugin(file) else {
                continue;
            };

            let base_content = git::git_show_optional(&ctx.repo_root, &params.base_ref, file)
                .map_err(ToolError::from)?
                .unwrap_or_default();
            let target_content = git::git_show_optional(&ctx.repo_root, target_ref, file)
                .map_err(ToolError::from)?
                .unwrap_or_default();

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

    #[tool(
        description = "Parse a file that already has weave conflict markers (after a merge driver run) into structured JSON: each conflicted entity's name, type, complexity, and refused_by — the same guard name the marker itself prints, machine-readable instead of scraped from comment text."
    )]
    async fn weave_merge_summary(
        &self,
        Parameters(params): Parameters<MergeSummaryParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self.get_context(Some(&params.file_path)).await?;
        let (_rel_path, abs_path) = Self::resolve_file_path(&ctx.repo_root, &params.file_path);

        let content = Self::read_file_at(&abs_path, &params.file_path)?;
        let conflicts = weave_core::parse_weave_conflicts(&content);

        let json_conflicts: Vec<serde_json::Value> = conflicts
            .iter()
            .map(|c| {
                serde_json::json!({
                    "entity": c.entity_name,
                    "kind": c.entity_kind,
                    "complexity": format!("{}", c.complexity),
                    "confidence": c.confidence,
                    "refused_by": c.refusal,
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

    #[tool(
        description = "Run a merge between two branches (writes nothing) and return, per entity, which resolution strategy weave used or would use (unchanged, diffy_merged, inner_merged, conflict, ...). Use to debug or audit why an entity did or didn't conflict; for the typed agent-facing findings document, use weave_findings instead."
    )]
    async fn weave_merge_audit(
        &self,
        Parameters(params): Parameters<MergeAuditParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self.get_context(params.file_path.as_deref()).await?;

        let merge_base =
            git::find_merge_base(&ctx.repo_root, &params.base_branch, &params.target_branch)
                .map_err(internal_err)?;

        let files = if let Some(ref fp) = params.file_path {
            let (rel, _) = Self::resolve_file_path(&ctx.repo_root, fp);
            vec![rel]
        } else {
            git::get_changed_files(
                &ctx.repo_root,
                &merge_base,
                &params.base_branch,
                &params.target_branch,
            )
            .map_err(internal_err)?
        };

        let mut results = Vec::new();
        for file in &files {
            let base = git::git_show_optional(&ctx.repo_root, &merge_base, file)
                .map_err(ToolError::from)?
                .unwrap_or_default();
            let ours = git::git_show_optional(&ctx.repo_root, &params.base_branch, file)
                .map_err(ToolError::from)?
                .unwrap_or_default();
            let theirs = git::git_show_optional(&ctx.repo_root, &params.target_branch, file)
                .map_err(ToolError::from)?
                .unwrap_or_default();

            if ours == theirs || base == ours || base == theirs {
                continue;
            }

            let merge_result = weave_core::entity_merge_with_registry(
                &base,
                &ours,
                &theirs,
                file,
                &self.registry,
                &weave_core::MarkerFormat::default(),
                &self.host,
            );

            let audit: Vec<serde_json::Value> = merge_result
                .audit
                .iter()
                .map(|a| serde_json::to_value(a).unwrap_or_default())
                .collect();

            results.push(serde_json::json!({
                "file": file,
                "clean": merge_result.is_clean(),
                "confidence": merge_result.stats.confidence(),
                "stats": merge_result.stats,
                "entities": audit,
            }));
        }

        let summary = serde_json::json!({
            "files_analyzed": results.len(),
            "results": results,
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&summary).unwrap_or_default(),
        )]))
    }

    #[tool(
        description = "Run a merge between two branches (writes nothing) and return a weave-findings document per file that actually diverges between them: conflicts, semantic warnings (SHADOW), derived binding breakage (DANGLING/DUP), the per-entity op trail, and rename facts. This is the agent-facing read contract for a two-branch comparison — prefer it over weave_preview_merge, weave_merge_audit, and weave_validate_merge, which each return a subset of the same information in an older shape. Each returned document states `clean: true` explicitly when a diverging file has no findings; a file where one side matches the base or both sides agree isn't diverging and is left out of `results` entirely — `files_analyzed` is the count actually compared. For a check against the CURRENT working tree instead of two branch tips, use weave_check."
    )]
    async fn weave_findings(
        &self,
        Parameters(params): Parameters<FindingsParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self.get_context(params.file_path.as_deref()).await?;

        let merge_base =
            git::find_merge_base(&ctx.repo_root, &params.base_branch, &params.target_branch)
                .map_err(internal_err)?;

        let files = if let Some(ref fp) = params.file_path {
            let (rel, _) = Self::resolve_file_path(&ctx.repo_root, fp);
            vec![rel]
        } else {
            git::get_changed_files(
                &ctx.repo_root,
                &merge_base,
                &params.base_branch,
                &params.target_branch,
            )
            .map_err(internal_err)?
        };

        let mut documents = Vec::new();
        for file in &files {
            let base = git::git_show_optional(&ctx.repo_root, &merge_base, file)
                .map_err(ToolError::from)?
                .unwrap_or_default();
            let ours = git::git_show_optional(&ctx.repo_root, &params.base_branch, file)
                .map_err(ToolError::from)?
                .unwrap_or_default();
            let theirs = git::git_show_optional(&ctx.repo_root, &params.target_branch, file)
                .map_err(ToolError::from)?
                .unwrap_or_default();

            // Nothing to compose: one side is the base, or both agree.
            if ours == theirs || base == ours || base == theirs {
                continue;
            }

            let result = weave_core::entity_merge_with_registry(
                &base,
                &ours,
                &theirs,
                file,
                &self.registry,
                &weave_core::MarkerFormat::default(),
                &self.host,
            );
            // The revisions travel with the references: `path@rev` is one
            // `git show` away for the reader, which is the whole point of not
            // carrying the bytes.
            let revs = crate::findings::Revs {
                base: Some(merge_base.clone()),
                ours: Some(params.base_branch.clone()),
                theirs: Some(params.target_branch.clone()),
            };
            documents.push(crate::findings::build(file, &result, &ours, &theirs, &revs));
        }

        let out = serde_json::json!({
            "schema": "weave-findings",
            "schema_version": crate::findings::SCHEMA_VERSION,
            "files_analyzed": documents.len(),
            "results": documents,
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&out).unwrap_or_default(),
        )]))
    }

    #[tool(
        description = "Cross-file binding check, repo-wide. A merge driver runs once per FILE and therefore cannot see a rename in a.py whose surviving caller lives in b.py — both files merge cleanly and the program is broken. This tool re-derives the same def/use evidence over the WHOLE repo and returns weave-findings documents (scope=repo, schema 1.1.0) for the cross-file DANGLING and SHADOW classes only. Defaults to HEAD × MERGE_HEAD — call it right after a merge to catch what the per-file driver couldn't see — or pass base/ours/theirs to check two arbitrary revisions before merging. `files_with_findings: 0` is stated explicitly and means weave found no cross-file breakage between these revisions, not that nothing was checked. Note: this is the repo-scope half of the `weave check` CLI command; it does not verify markers, unanimous-line loss, or duplicate lines in the working tree — those aren't reachable over MCP yet."
    )]
    async fn weave_check(
        &self,
        Parameters(params): Parameters<CheckParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self.get_context(None).await?;
        let root = ctx.repo_root.clone();

        let (base, ours, theirs) = weave_cli::gitscan::trees(
            &root,
            params.base.as_deref(),
            params.ours.as_deref(),
            params.theirs.as_deref(),
        )
        .map_err(internal_err)?;

        let documents = weave_cli::repo_scope::check(&base, &ours, &theirs);
        let total = weave_cli::repo_scope::total_findings(&documents);

        let out = serde_json::json!({
            "schema": "weave-findings",
            "schema_version": weave_cli::wire::SCHEMA_VERSION_1_1,
            "scope": "repo",
            "files_with_findings": documents.len(),
            "findings_total": total,
            "results": documents,
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&out).unwrap_or_default(),
        )]))
    }

    #[tool(
        description = "Detect when an auto-merged entity references another entity that was also modified — the risk that two independently-clean edits don't actually work together. This is a subset of what weave_findings returns (as SHADOW findings); prefer weave_findings unless you specifically want just these warnings without the rest of the document."
    )]
    async fn weave_validate_merge(
        &self,
        Parameters(params): Parameters<ValidateMergeParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self.get_context(params.file_path.as_deref()).await?;

        let merge_base =
            git::find_merge_base(&ctx.repo_root, &params.base_branch, &params.target_branch)
                .map_err(internal_err)?;

        let files = if let Some(ref fp) = params.file_path {
            let (rel, _) = Self::resolve_file_path(&ctx.repo_root, fp);
            vec![rel]
        } else {
            git::get_changed_files(
                &ctx.repo_root,
                &merge_base,
                &params.base_branch,
                &params.target_branch,
            )
            .map_err(internal_err)?
        };

        // Collect modified entities from both branches
        let mut modified_entities = Vec::new();
        for file in &files {
            let base_content = git::git_show_optional(&ctx.repo_root, &merge_base, file)
                .map_err(ToolError::from)?
                .unwrap_or_default();
            let ours_content = git::git_show_optional(&ctx.repo_root, &params.base_branch, file)
                .map_err(ToolError::from)?
                .unwrap_or_default();
            let theirs_content =
                git::git_show_optional(&ctx.repo_root, &params.target_branch, file)
                    .map_err(ToolError::from)?
                    .unwrap_or_default();

            if let Some(plugin) = self.registry.get_plugin(file) {
                let base_entities = plugin.extract_entities(&base_content, file);
                let ours_entities = plugin.extract_entities(&ours_content, file);
                let theirs_entities = plugin.extract_entities(&theirs_content, file);

                // Find entities modified in ours or theirs vs base
                for entity in ours_entities.iter().chain(theirs_entities.iter()) {
                    // Internal diff plumbing, not user-addressed: both sides enumerate
                    // the file's entities across revisions, so there is no user-supplied
                    // address for the resolver.
                    let base_match = base_entities.iter().find(|b| b.name == entity.name);
                    // No match in base means a new entity, so treat it as modified.
                    let is_modified =
                        base_match.is_none_or(|b| b.content_hash != entity.content_hash);
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

    #[tool(
        description = "Write your edit of one entity into the CRDT coordination state — not the file on disk; this is the live multi-agent layer, separate from a git merge. Call once you've decided the entity's new content, whether or not you claimed it first. Bumps your agent's version vector so weave_merge_file and other agents' weave_status calls see the update."
    )]
    async fn weave_update_entity_content(
        &self,
        Parameters(params): Parameters<UpdateEntityContentParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self.get_context(Some(&params.file_path)).await?;
        let (rel_path, abs_path) = Self::resolve_file_path(&ctx.repo_root, &params.file_path);
        let content = Self::read_file_at(&abs_path, &rel_path)?;
        // The entity_id from weave_claim_entity addresses the claim directly —
        // name resolution runs against the file's *current* content, so a
        // rename between claim and update would make the old name
        // unresolvable even though the claim is still held.
        let entity_id = match params.entity_id {
            Some(ref id) => id.clone(),
            None => Self::resolve_entity_sync(
                &self.registry,
                &content,
                &rel_path,
                &params.entity_name,
                params.entity_type.as_deref(),
                params.parent_name.as_deref(),
                params.ordinal,
            )?,
        };

        // Compute content hash
        let hash = format!("{:x}", content_hash_u64(&params.content));

        let mut state = ctx.state.lock().await;

        // Ensure entity exists in CRDT
        let entities = self.cached_extract_entities(&content, &rel_path).await;
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

        update_entity_content(
            &mut state,
            &params.agent_id,
            &entity_id,
            &params.content,
            &hash,
        )
        .map_err(internal_err)?;
        let _ = state.save();

        let status = get_entity_content(&state, &entity_id).map_err(internal_err)?;

        let response = serde_json::json!({
            "entity": params.entity_name,
            "content_hash": hash,
            "version_vector": serde_json::to_value(&status.version_vector).unwrap_or_default(),
            "version": status.version_vector.total(),
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&response).unwrap_or_default(),
        )]))
    }

    #[tool(
        description = "Read what the CRDT currently holds for one entity: content, version vector, merge_state, and — when it's mid-conflict — each side's content. Use to see what another agent wrote (via weave_update_entity_content) before writing your own version, or to inspect a conflict weave_merge_file flagged before calling weave_resolve_conflict."
    )]
    async fn weave_get_entity_content(
        &self,
        Parameters(params): Parameters<GetEntityContentParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self.get_context(Some(&params.file_path)).await?;
        let (rel_path, abs_path) = Self::resolve_file_path(&ctx.repo_root, &params.file_path);
        let content = Self::read_file_at(&abs_path, &rel_path)?;
        let entity_id = Self::resolve_entity_sync(
            &self.registry,
            &content,
            &rel_path,
            &params.entity_name,
            params.entity_type.as_deref(),
            params.parent_name.as_deref(),
            params.ordinal,
        )?;

        let state = ctx.state.lock().await;
        match get_entity_content(&state, &entity_id) {
            Ok(status) => {
                let response = serde_json::json!({
                    "entity": status.name,
                    "content": status.content,
                    "base_content": status.base_content,
                    "content_hash": status.content_hash,
                    "version_vector": serde_json::to_value(&status.version_vector).unwrap_or_default(),
                    "merge_state": status.merge_state,
                    "conflict_ours": status.conflict_ours,
                    "conflict_theirs": status.conflict_theirs,
                    "conflict_base": status.conflict_base,
                });
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&response).unwrap_or_default(),
                )]))
            }
            Err(_) => Ok(CallToolResult::success(vec![Content::text(
                serde_json::json!({
                    "entity": params.entity_name,
                    "content": "",
                    "merge_state": "clean",
                    "version_vector": {},
                })
                .to_string(),
            )])),
        }
    }

    #[tool(
        description = "Run weave's entity-level merge over one file's CRDT state: syncs from what's on disk, then auto-merges entities where version vectors allow and marks the rest conflicted. This is the CRDT-coordination merge, not the git merge driver — use it when agents are editing live through weave_update_entity_content rather than on separate branches. Follow a conflicted result with weave_get_entity_content then weave_resolve_conflict."
    )]
    async fn weave_merge_file(
        &self,
        Parameters(params): Parameters<MergeFileParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self.get_context(Some(&params.file_path)).await?;
        let (rel_path, _abs_path) = Self::resolve_file_path(&ctx.repo_root, &params.file_path);

        let mut state = ctx.state.lock().await;

        // Sync from working tree first
        let _ = sync_from_files(
            &mut state,
            &ctx.repo_root,
            std::slice::from_ref(&rel_path),
            &self.registry,
        );

        let result = merge_file_entities(&state, &rel_path).map_err(internal_err)?;
        let _ = state.save();

        let response = serde_json::json!({
            "file_path": result.file_path,
            "entities_auto_merged": result.entities_auto_merged,
            "entities_conflicted": result.entities_conflicted,
            "clean": result.entities_conflicted == 0,
            "merged_content": result.merged_content,
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&response).unwrap_or_default(),
        )]))
    }

    #[tool(
        description = "Resolve an entity weave_merge_file flagged as conflicted: supply the final content and weave merges the version vectors and clears the conflict. Use weave_get_entity_content first to read both sides before deciding."
    )]
    async fn weave_resolve_conflict(
        &self,
        Parameters(params): Parameters<ResolveConflictParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = self.get_context(Some(&params.file_path)).await?;
        let (rel_path, abs_path) = Self::resolve_file_path(&ctx.repo_root, &params.file_path);
        let content = Self::read_file_at(&abs_path, &rel_path)?;
        let entity_id = Self::resolve_entity_sync(
            &self.registry,
            &content,
            &rel_path,
            &params.entity_name,
            params.entity_type.as_deref(),
            params.parent_name.as_deref(),
            params.ordinal,
        )?;

        let hash = format!("{:x}", content_hash_u64(&params.resolved_content));

        let mut state = ctx.state.lock().await;
        resolve_entity_conflict(
            &mut state,
            &params.agent_id,
            &entity_id,
            &params.resolved_content,
            &hash,
        )
        .map_err(internal_err)?;
        let _ = state.save();

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::json!({
                "entity": params.entity_name,
                "resolved": true,
                "content_hash": hash,
            })
            .to_string(),
        )]))
    }
}

#[tool_handler]
impl ServerHandler for WeaveServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "Weave: entity-level semantic merge for Git, plus live multi-agent coordination. \
                 Two independent tool groups. (1) Merge analysis — weave_findings, weave_check, \
                 weave_preview_merge, weave_diff, weave_merge_audit, weave_validate_merge — read \
                 git refs or the working tree directly; no setup needed. Start with weave_findings \
                 after (or before) a merge between two branches, or weave_check for cross-file \
                 binding risk a per-file merge driver can't see. (2) Live coordination — \
                 weave_claim_entity, weave_release_entity, weave_status, weave_who_is_editing, \
                 weave_potential_conflicts, weave_update_entity_content, weave_get_entity_content, \
                 weave_merge_file, weave_resolve_conflict — track edits in a shared CRDT \
                 (.weave/state.automerge) for agents editing the same repo at the same time. Call \
                 weave_agent_register once before using any of these.",
        )
    }
}

fn internal_err(msg: impl ToString) -> rmcp::ErrorData {
    rmcp::ErrorData::internal_error(msg.to_string(), None)
}

#[cfg(test)]
mod resolve_entity_for_graph_tests {
    use super::*;
    use std::fs;

    /// Writes `files` (relative path -> content) under a fresh temp dir and
    /// returns the root. Each test gets its own directory (PID + a counter)
    /// so parallel `cargo test` runs never collide.
    fn write_fixture(name: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "weave-mcp-resolve-graph-test-{}-{}-{}",
            std::process::id(),
            name,
            n
        ));
        fs::create_dir_all(&root).expect("create fixture dir");
        for (rel, content) in files {
            let path = root.join(rel);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create fixture subdir");
            }
            fs::write(&path, content).expect("write fixture file");
        }
        root
    }

    /// An entity that is perfectly unambiguous WITHIN its own file must
    /// resolve to that file's entity, even when a same-named entity exists
    /// elsewhere in the repo. Specifying `file_path` is supposed to be
    /// sufficient disambiguation on its own — the repo-wide fallback exists
    /// only for entities the in-file resolver could not find at all.
    #[test]
    fn in_file_unambiguous_match_wins_over_cross_file_namesake() {
        let root = write_fixture(
            "namesake",
            &[
                ("a.ts", "function helper(): string {\n  return \"a\";\n}\n"),
                ("b.ts", "function helper(): string {\n  return \"b\";\n}\n"),
            ],
        );

        let registry = create_default_registry();
        let file_paths = vec!["a.ts".to_string(), "b.ts".to_string()];
        let (graph, _entities) = EntityGraph::build(&root, &file_paths, &registry);

        let content_a = fs::read_to_string(root.join("a.ts")).unwrap();

        let result = WeaveServer::resolve_entity_for_graph(
            &registry, &content_a, "a.ts", &graph, "helper", None, None, None,
        );

        let expected_id = graph
            .entities
            .values()
            .find(|e| e.name == "helper" && e.file_path == "a.ts")
            .map(|e| e.id.clone())
            .expect("fixture entity must be in the graph");

        match result {
            Ok(id) => assert_eq!(
                id, expected_id,
                "an entity unambiguous within the addressed file must resolve to it, \
                 not be reported ambiguous just because a same-named entity exists \
                 in a different file"
            ),
            Err(e) => panic!(
                "expected Ok({expected_id}), got Err({e}) — an entity unambiguous within \
                 the addressed file must resolve to it, not be reported ambiguous just \
                 because a same-named entity exists in a different file"
            ),
        }

        let _ = fs::remove_dir_all(&root);
    }

    /// When the addressed file has no entity of that name at all, the
    /// historical repo-wide lookup still kicks in and resolves it if it's
    /// unique elsewhere in the repo.
    #[test]
    fn falls_back_repo_wide_when_absent_from_target_file() {
        let root = write_fixture(
            "fallback-unique",
            &[
                ("a.ts", "function onlyInA(): string {\n  return \"a\";\n}\n"),
                (
                    "b.ts",
                    "function elsewhereOnly(): string {\n  return \"b\";\n}\n",
                ),
            ],
        );

        let registry = create_default_registry();
        let file_paths = vec!["a.ts".to_string(), "b.ts".to_string()];
        let (graph, _entities) = EntityGraph::build(&root, &file_paths, &registry);
        let content_a = fs::read_to_string(root.join("a.ts")).unwrap();

        let result = WeaveServer::resolve_entity_for_graph(
            &registry,
            &content_a,
            "a.ts",
            &graph,
            "elsewhereOnly",
            None,
            None,
            None,
        );

        let expected_id = graph
            .entities
            .values()
            .find(|e| e.name == "elsewhereOnly")
            .map(|e| e.id.clone())
            .expect("fixture entity must be in the graph");

        assert_eq!(result.unwrap(), expected_id);

        let _ = fs::remove_dir_all(&root);
    }

    /// When the addressed file has no entity of that name, and the name is
    /// ambiguous elsewhere in the repo, the fallback refuses to pick first.
    #[test]
    fn repo_wide_fallback_refuses_ambiguity() {
        let root = write_fixture(
            "fallback-ambiguous",
            &[
                ("a.ts", "function onlyInA(): string {\n  return \"a\";\n}\n"),
                ("b.ts", "function dup(): string {\n  return \"b\";\n}\n"),
                ("c.ts", "function dup(): string {\n  return \"c\";\n}\n"),
            ],
        );

        let registry = create_default_registry();
        let file_paths = vec!["a.ts".to_string(), "b.ts".to_string(), "c.ts".to_string()];
        let (graph, _entities) = EntityGraph::build(&root, &file_paths, &registry);
        let content_a = fs::read_to_string(root.join("a.ts")).unwrap();

        let result = WeaveServer::resolve_entity_for_graph(
            &registry, &content_a, "a.ts", &graph, "dup", None, None, None,
        );

        let Err(e) = result else {
            panic!("expected ambiguous error, got {result:?}");
        };
        let msg = e.to_string();
        assert!(msg.contains("ambiguous across the repo"), "{msg}");
        assert!(msg.contains("b.ts") && msg.contains("c.ts"), "{msg}");

        let _ = fs::remove_dir_all(&root);
    }

    /// Ambiguity that exists PURELY within the addressed file (no cross-file
    /// duplicate at all, and the file is the only one in the graph) must be
    /// reported as an in-file ambiguity — naming the file and listing both
    /// candidates with their parents — and must NOT fall through to the
    /// repo-wide scan and come back mislabeled "ambiguous across the repo"
    /// when both candidates are actually in that one file.
    #[test]
    fn in_file_ambiguity_is_terminal_and_not_mislabeled_as_repo_wide() {
        let root = write_fixture(
            "in-file-only-ambiguous",
            &[(
                "dup.ts",
                "class Animal {\n  run(): string { return \"a\"; }\n}\n\nclass Robot {\n  run(): string { return \"r\"; }\n}\n",
            )],
        );

        let registry = create_default_registry();
        let file_paths = vec!["dup.ts".to_string()];
        let (graph, _entities) = EntityGraph::build(&root, &file_paths, &registry);
        let content = fs::read_to_string(root.join("dup.ts")).unwrap();

        let result = WeaveServer::resolve_entity_for_graph(
            &registry, &content, "dup.ts", &graph, "run", None, None, None,
        );

        let Err(e) = result else {
            panic!("expected ambiguous error, got {result:?}");
        };
        let msg = e.to_string();
        assert!(
            !msg.contains("ambiguous across the repo"),
            "in-file-only ambiguity must not be labeled repo-wide: {msg}"
        );
        assert!(
            msg.contains("dup.ts"),
            "message should name the file: {msg}"
        );
        assert!(
            msg.contains("Animal") && msg.contains("Robot"),
            "both candidates' parents should be listed: {msg}"
        );

        let _ = fs::remove_dir_all(&root);
    }
}

#[cfg(test)]
mod description_tests {
    use super::*;

    /// The full tool catalog, name -> description, as an agent actually sees
    /// it over MCP (`tools/list`). A change here is a change to what every
    /// consumer reads, so this list is the thing to diff, not the 22
    /// individual `#[tool(description = ...)]` attributes scattered through
    /// this file.
    fn catalog() -> Vec<(String, String)> {
        let server = WeaveServer::new();
        let mut tools: Vec<(String, String)> = server
            .tool_router
            .list_all()
            .into_iter()
            .map(|t| {
                (
                    t.name.to_string(),
                    t.description.map(|d| d.to_string()).unwrap_or_default(),
                )
            })
            .collect();
        tools.sort();
        tools
    }

    #[test]
    fn every_tool_has_a_nonempty_description() {
        for (name, desc) in catalog() {
            assert!(!desc.is_empty(), "{name} has no description");
        }
    }

    /// Every description names when to reach for the tool or how to read its
    /// output, not just what it computes — the point of a tool description is
    /// to substitute for a manual an agent will never read: agents read tool
    /// descriptions and point-of-need lines, not docs. This is a floor, not a
    /// style check: it requires one of a handful of task-oriented words per
    /// description rather than asserting exact wording, so it survives future
    /// rewording as long as the rewrite stays task-oriented.
    #[test]
    fn every_description_is_action_shaped() {
        const SIGNAL: &[&str] = &[
            "use ",
            "call ",
            "before ",
            "after ",
            "prefer ",
            "check this",
            "run this",
            "start with",
        ];
        for (name, desc) in catalog() {
            let lower = desc.to_lowercase();
            assert!(
                SIGNAL.iter().any(|s| lower.contains(s)),
                "{name}'s description doesn't say when to use it: {desc:?}"
            );
        }
    }

    /// Stale vocabulary that should never survive a rewrite: "resolution
    /// hints" was the old advice-sentence-in-the-marker feature; it was
    /// removed and replaced by `refused_by` (the guard name). A description
    /// that still promises "hints" is describing a feature that no longer
    /// exists.
    #[test]
    fn no_description_promises_removed_features() {
        for (name, desc) in catalog() {
            let lower = desc.to_lowercase();
            assert!(
                !lower.contains("resolution hint"),
                "{name} still advertises removed resolution-hint text: {desc:?}"
            );
        }
    }

    /// Fixed count, on purpose: adding, removing, or renaming a tool is a
    /// catalog change an agent's tool list will show immediately, and this
    /// test exists so it can never happen silently alongside an unrelated
    /// description edit. Bump the number and update the surrounding docs
    /// (SKILL.md, README, docs/llms.txt) in the same change.
    #[test]
    fn tool_count_is_pinned() {
        let names: Vec<String> = catalog().into_iter().map(|(n, _)| n).collect();
        assert_eq!(
            names.len(),
            22,
            "tool count changed ({names:?}) — update SKILL.md/README/docs/llms.txt in the same commit"
        );
    }
}

#[cfg(test)]
mod claim_by_id_tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    /// A fresh git repo per test (PID + counter, same isolation convention as
    /// `resolve_entity_for_graph_tests::write_fixture`) — the handlers
    /// discover the repo root from the absolute file path, so no env vars.
    fn git_fixture(name: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "weave-mcp-claim-by-id-test-{}-{}-{}",
            std::process::id(),
            name,
            n
        ));
        fs::create_dir_all(&root).expect("create fixture dir");
        let ok = Command::new("git")
            .args(["init", "-q"])
            .current_dir(&root)
            .status()
            .expect("run git init")
            .success();
        assert!(ok, "git init failed");
        for (rel, content) in files {
            fs::write(root.join(rel), content).expect("write fixture file");
        }
        root
    }

    fn text_of(result: &CallToolResult) -> String {
        result
            .content
            .first()
            .and_then(|c| c.as_text())
            .map(|t| t.text.clone())
            .unwrap_or_default()
    }

    const ORIGINAL: &str = "function foo(): string {\n  return \"x\";\n}\n";
    const RENAMED: &str = "function bar(): string {\n  return \"x\";\n}\n";

    /// Claim `foo`, capture the entity_id from the response, rename the
    /// entity on disk. The claim-time id must keep addressing the claim:
    /// update + release by entity_id succeed where the stale name cannot.
    #[tokio::test]
    async fn update_and_release_by_entity_id_survive_a_rename() {
        let root = git_fixture("survives-rename", &[("a.ts", ORIGINAL)]);
        let abs = root.join("a.ts").to_string_lossy().to_string();
        let server = WeaveServer::new();

        let claim = server
            .weave_claim_entity(Parameters(ClaimEntityParams {
                agent_id: "agent-1".into(),
                file_path: abs.clone(),
                entity_name: "foo".into(),
                entity_type: None,
                parent_name: None,
                ordinal: None,
            }))
            .await
            .expect("claim succeeds");
        let claim_json: serde_json::Value =
            serde_json::from_str(&text_of(&claim)).expect("claim response is json");
        let entity_id = claim_json["entity_id"]
            .as_str()
            .expect("claim response carries entity_id")
            .to_string();

        // The rename that breaks name-based resolution.
        fs::write(root.join("a.ts"), RENAMED).expect("rename on disk");

        let update = server
            .weave_update_entity_content(Parameters(UpdateEntityContentParams {
                agent_id: "agent-1".into(),
                file_path: abs.clone(),
                entity_name: "foo".into(), // stale — the id must win
                content: RENAMED.into(),
                entity_id: Some(entity_id.clone()),
                entity_type: None,
                parent_name: None,
                ordinal: None,
            }))
            .await;
        assert!(
            update.is_ok(),
            "update by entity_id must survive the rename, got: {:?}",
            update.err()
        );

        let release = server
            .weave_release_entity(Parameters(ReleaseEntityParams {
                agent_id: "agent-1".into(),
                file_path: abs,
                entity_name: "foo".into(), // stale — the id must win
                entity_id: Some(entity_id),
                entity_type: None,
                parent_name: None,
                ordinal: None,
            }))
            .await;
        assert!(
            release.is_ok(),
            "release by entity_id must survive the rename, got: {:?}",
            release.err()
        );
    }

    /// Pins today's name-based behavior: without entity_id, a rename between
    /// claim and update/release makes the old name unresolvable and the call
    /// fails with an entity-not-found error. This is the failure the
    /// entity_id parameter exists to remove — kept as the contract for
    /// callers that only send entity_name.
    #[tokio::test]
    async fn stale_name_still_fails_after_a_rename() {
        let root = git_fixture("stale-name", &[("a.ts", ORIGINAL)]);
        let abs = root.join("a.ts").to_string_lossy().to_string();
        let server = WeaveServer::new();

        server
            .weave_claim_entity(Parameters(ClaimEntityParams {
                agent_id: "agent-1".into(),
                file_path: abs.clone(),
                entity_name: "foo".into(),
                entity_type: None,
                parent_name: None,
                ordinal: None,
            }))
            .await
            .expect("claim succeeds");

        fs::write(root.join("a.ts"), RENAMED).expect("rename on disk");

        let update = server
            .weave_update_entity_content(Parameters(UpdateEntityContentParams {
                agent_id: "agent-1".into(),
                file_path: abs,
                entity_name: "foo".into(),
                content: RENAMED.into(),
                entity_id: None,
                entity_type: None,
                parent_name: None,
                ordinal: None,
            }))
            .await;
        let err = update.expect_err("stale name must not resolve after a rename");
        assert!(
            err.message.contains("not found"),
            "expected an entity-not-found error, got: {}",
            err.message
        );
    }
}
