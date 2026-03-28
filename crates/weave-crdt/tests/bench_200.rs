//! Benchmark: 200-function file, git subprocess vs weave CRDT.
//! Each agent makes a real code modification (add validation, caching,
//! error handling, logging, refactored returns). Not just a comment.

use std::path::Path;
use std::process::Command;
use std::time::Instant;

use sem_core::parser::plugins::create_default_registry;
use weave_crdt::{
    merge_file_entities, register_agent, sync_from_files, update_entity_content, EntityStateDoc,
};

fn generate_200_func_file() -> String {
    let mut file = String::from(
        "import { db } from './db';\nimport { logger } from './logger';\nimport { cache } from './cache';\n\n",
    );
    for i in 0..200 {
        let table = match i % 5 {
            0 => "users",
            1 => "orders",
            2 => "products",
            3 => "sessions",
            _ => "events",
        };
        file.push_str(&format!(
            "export function handler{}(req: Request, res: Response) {{\n",
            i
        ));
        file.push_str(&format!("    const id = req.params.id;\n"));
        file.push_str(&format!(
            "    const data = db.query('SELECT * FROM {} WHERE id = ?', [id]);\n",
            table
        ));
        file.push_str(&format!(
            "    if (!data) return res.status(404).json({{ error: '{} not found' }});\n",
            table
        ));
        file.push_str(&format!(
            "    return res.json({{ result: data, table: '{}' }});\n",
            table
        ));
        file.push_str("}\n\n");
    }
    file
}

/// Generate a realistic agent modification for a function.
/// Different agents make different kinds of changes.
fn agent_modification(agent_idx: usize, func_idx: usize) -> String {
    let table = match func_idx % 5 {
        0 => "users",
        1 => "orders",
        2 => "products",
        3 => "sessions",
        _ => "events",
    };

    match agent_idx % 7 {
        // Add input validation
        0 => format!(
            "export function handler{}(req: Request, res: Response) {{\n\
            \x20   const id = req.params.id;\n\
            \x20   if (!id || typeof id !== 'string') {{\n\
            \x20       return res.status(400).json({{ error: 'Invalid id parameter' }});\n\
            \x20   }}\n\
            \x20   if (id.length > 128) {{\n\
            \x20       return res.status(400).json({{ error: 'Id too long' }});\n\
            \x20   }}\n\
            \x20   const data = db.query('SELECT * FROM {} WHERE id = ?', [id]);\n\
            \x20   if (!data) return res.status(404).json({{ error: '{} not found' }});\n\
            \x20   return res.json({{ result: data, table: '{}' }});\n\
            }}",
            func_idx, table, table, table
        ),
        // Add caching layer
        1 => format!(
            "export function handler{}(req: Request, res: Response) {{\n\
            \x20   const id = req.params.id;\n\
            \x20   const cacheKey = `{}:${{id}}`;\n\
            \x20   const cached = cache.get(cacheKey);\n\
            \x20   if (cached) {{\n\
            \x20       return res.json({{ result: cached, table: '{}', fromCache: true }});\n\
            \x20   }}\n\
            \x20   const data = db.query('SELECT * FROM {} WHERE id = ?', [id]);\n\
            \x20   if (!data) return res.status(404).json({{ error: '{} not found' }});\n\
            \x20   cache.set(cacheKey, data, {{ ttl: 300 }});\n\
            \x20   return res.json({{ result: data, table: '{}' }});\n\
            }}",
            func_idx, table, table, table, table, table
        ),
        // Add error handling with try/catch
        2 => format!(
            "export function handler{}(req: Request, res: Response) {{\n\
            \x20   try {{\n\
            \x20       const id = req.params.id;\n\
            \x20       const data = db.query('SELECT * FROM {} WHERE id = ?', [id]);\n\
            \x20       if (!data) return res.status(404).json({{ error: '{} not found' }});\n\
            \x20       return res.json({{ result: data, table: '{}' }});\n\
            \x20   }} catch (err) {{\n\
            \x20       logger.error('handler{} failed', {{ error: err.message, params: req.params }});\n\
            \x20       return res.status(500).json({{ error: 'Internal server error' }});\n\
            \x20   }}\n\
            }}",
            func_idx, table, table, table, func_idx
        ),
        // Add logging and metrics
        3 => format!(
            "export function handler{}(req: Request, res: Response) {{\n\
            \x20   const start = Date.now();\n\
            \x20   const id = req.params.id;\n\
            \x20   logger.info('handler{} called', {{ id, ip: req.ip }});\n\
            \x20   const data = db.query('SELECT * FROM {} WHERE id = ?', [id]);\n\
            \x20   const duration = Date.now() - start;\n\
            \x20   logger.info('handler{} query complete', {{ id, duration }});\n\
            \x20   if (!data) {{\n\
            \x20       logger.warn('handler{} not found', {{ id }});\n\
            \x20       return res.status(404).json({{ error: '{} not found' }});\n\
            \x20   }}\n\
            \x20   return res.json({{ result: data, table: '{}', queryTime: duration }});\n\
            }}",
            func_idx, func_idx, table, func_idx, func_idx, table, table
        ),
        // Add pagination support
        4 => format!(
            "export function handler{}(req: Request, res: Response) {{\n\
            \x20   const id = req.params.id;\n\
            \x20   const page = parseInt(req.query.page || '1', 10);\n\
            \x20   const limit = Math.min(parseInt(req.query.limit || '20', 10), 100);\n\
            \x20   const offset = (page - 1) * limit;\n\
            \x20   const data = db.query('SELECT * FROM {} WHERE id = ? LIMIT ? OFFSET ?', [id, limit, offset]);\n\
            \x20   const total = db.query('SELECT COUNT(*) as count FROM {} WHERE id = ?', [id]);\n\
            \x20   if (!data) return res.status(404).json({{ error: '{} not found' }});\n\
            \x20   return res.json({{ result: data, table: '{}', page, limit, total: total.count }});\n\
            }}",
            func_idx, table, table, table, table
        ),
        // Add rate limiting check
        5 => format!(
            "export function handler{}(req: Request, res: Response) {{\n\
            \x20   const clientIp = req.ip || req.headers['x-forwarded-for'];\n\
            \x20   const rateKey = `ratelimit:handler{}:${{clientIp}}`;\n\
            \x20   const requests = cache.incr(rateKey);\n\
            \x20   if (requests === 1) cache.expire(rateKey, 60);\n\
            \x20   if (requests > 100) {{\n\
            \x20       return res.status(429).json({{ error: 'Too many requests' }});\n\
            \x20   }}\n\
            \x20   const id = req.params.id;\n\
            \x20   const data = db.query('SELECT * FROM {} WHERE id = ?', [id]);\n\
            \x20   if (!data) return res.status(404).json({{ error: '{} not found' }});\n\
            \x20   return res.json({{ result: data, table: '{}' }});\n\
            }}",
            func_idx, func_idx, table, table, table
        ),
        // Refactor to async/await
        _ => format!(
            "export async function handler{}(req: Request, res: Response) {{\n\
            \x20   const id = req.params.id;\n\
            \x20   const data = await db.queryAsync('SELECT * FROM {} WHERE id = ?', [id]);\n\
            \x20   if (!data) {{\n\
            \x20       return res.status(404).json({{ error: '{} not found' }});\n\
            \x20   }}\n\
            \x20   const enriched = await enrichData(data, '{}');\n\
            \x20   return res.json({{ result: enriched, table: '{}' }});\n\
            }}",
            func_idx, table, table, table, table
        ),
    }
}

fn git_cmd(dir: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "test")
        .env("GIT_AUTHOR_EMAIL", "test@test.com")
        .env("GIT_COMMITTER_NAME", "test")
        .env("GIT_COMMITTER_EMAIL", "test@test.com")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn bench_git(base_content: &str, num_agents: usize) -> (std::time::Duration, usize) {
    let tmp = std::env::temp_dir().join(format!("bench200_git_{}", num_agents));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    // Init repo
    git_cmd(&tmp, &["init"]);
    git_cmd(&tmp, &["checkout", "-b", "main"]);
    std::fs::write(tmp.join("api.ts"), base_content).unwrap();
    git_cmd(&tmp, &["add", "api.ts"]);
    git_cmd(&tmp, &["commit", "-m", "initial"]);

    let start = Instant::now();

    // Each agent: branch, make a real edit, commit
    for i in 0..num_agents {
        let branch = format!("agent-{}", i);
        git_cmd(&tmp, &["checkout", "main"]);
        git_cmd(&tmp, &["checkout", "-b", &branch]);

        let content = std::fs::read_to_string(tmp.join("api.ts")).unwrap();
        let func_idx = i % 200;
        let old_func_header = format!("export function handler{}(", func_idx);
        // Also match async variant
        let old_func_header_async = format!("export async function handler{}(", func_idx);

        // Find the full old function and replace it
        let new_func = agent_modification(i, func_idx);
        let modified = replace_function(&content, func_idx, &new_func);

        std::fs::write(tmp.join("api.ts"), &modified).unwrap();
        git_cmd(&tmp, &["add", "api.ts"]);
        git_cmd(
            &tmp,
            &["commit", "-m", &format!("agent-{} modifies handler{}", i, func_idx)],
        );
    }

    // Merge all branches sequentially into main
    git_cmd(&tmp, &["checkout", "main"]);
    let mut clean = 0;
    for i in 0..num_agents {
        let branch = format!("agent-{}", i);
        if git_cmd(&tmp, &["merge", &branch, "--no-edit"]) {
            clean += 1;
        } else {
            git_cmd(&tmp, &["merge", "--abort"]);
        }
    }

    let elapsed = start.elapsed();
    let _ = std::fs::remove_dir_all(&tmp);
    (elapsed, clean)
}

/// Replace a function by index in the file content.
fn replace_function(content: &str, func_idx: usize, new_func: &str) -> String {
    let marker = format!("export function handler{}(", func_idx);
    let async_marker = format!("export async function handler{}(", func_idx);

    let start_pos = content
        .find(&marker)
        .or_else(|| content.find(&async_marker));

    match start_pos {
        Some(start) => {
            // Find the end of the function (closing brace at column 0)
            let rest = &content[start..];
            let mut depth = 0;
            let mut end_offset = 0;
            for (i, ch) in rest.char_indices() {
                if ch == '{' {
                    depth += 1;
                } else if ch == '}' {
                    depth -= 1;
                    if depth == 0 {
                        end_offset = i + 1;
                        break;
                    }
                }
            }
            let mut result = String::new();
            result.push_str(&content[..start]);
            result.push_str(new_func);
            result.push_str(&content[start + end_offset..]);
            result
        }
        None => content.to_string(),
    }
}

fn bench_crdt(base_content: &str, num_agents: usize) -> (std::time::Duration, usize) {
    let registry = create_default_registry();
    let tmp = std::env::temp_dir().join(format!("bench200_crdt_{}", num_agents));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::write(tmp.join("api.ts"), base_content).unwrap();

    let start = Instant::now();

    let mut state = EntityStateDoc::new_memory().unwrap();
    sync_from_files(&mut state, &tmp, &["api.ts".to_string()], &registry).unwrap();

    let plugin = registry.get_plugin("api.ts").unwrap();
    let entities = plugin.extract_entities(base_content, "api.ts");
    let functions: Vec<_> = entities
        .iter()
        .filter(|e| e.entity_type == "function")
        .collect();

    for i in 0..num_agents {
        let func_idx = i % 200;
        let func = &functions[func_idx];
        let agent = format!("agent-{}", i);
        register_agent(&mut state, &agent, &agent, "main").unwrap();
        let new_content = agent_modification(i, func_idx);
        update_entity_content(&mut state, &agent, &func.id, &new_content, &format!("h{}", i))
            .unwrap();
    }

    let result = merge_file_entities(&mut state, "api.ts", &registry).unwrap();

    let elapsed = start.elapsed();
    let _ = std::fs::remove_dir_all(&tmp);
    let clean = result.entities_auto_merged;
    (elapsed, clean)
}

#[test]
fn bench_200_functions() {
    let base = generate_200_func_file();
    let line_count = base.lines().count();

    eprintln!("\n=== 200-FUNCTION FILE BENCHMARK ===");
    eprintln!("File: 200 TypeScript handler functions ({} lines)", line_count);
    eprintln!("Each agent rewrites a different function body (validation, caching, error handling, logging, pagination, rate limiting, or async refactor)");
    eprintln!("Git: branch + rewrite function + commit + sequential merge (real subprocess)");
    eprintln!("CRDT: entity write + single merge pass (in-memory)");
    eprintln!("");
    eprintln!(
        "{:<8} {:>12} {:>12} {:>8} {:>16} {:>16}",
        "Agents", "Git(ms)", "CRDT(ms)", "Ratio", "Git clean", "CRDT clean"
    );

    for num_agents in [2, 5, 10, 20, 50] {
        let (git_t, git_c) = bench_git(&base, num_agents);
        let (crdt_t, crdt_c) = bench_crdt(&base, num_agents);

        let ratio = git_t.as_millis() as f64 / crdt_t.as_millis().max(1) as f64;
        eprintln!(
            "{:<8} {:>10} ms {:>10} ms {:>7.1}x {:>16} {:>16}",
            num_agents,
            git_t.as_millis(),
            crdt_t.as_millis(),
            ratio,
            format!("{}/{}", git_c, num_agents),
            format!("{}/{}", crdt_c, num_agents),
        );
    }
}
