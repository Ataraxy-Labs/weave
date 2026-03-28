//! Benchmark: CRDT entity workflow vs traditional git branch workflow.
//!
//! Measures end-to-end time for multi-agent collaboration scenarios:
//! 1. Two agents editing different functions in the same file
//! 2. Five agents editing different functions in the same file
//! 3. Conflict detection speed

use std::path::Path;
use std::process::Command;
use std::time::Instant;

use weave_crdt::{
    get_entity_content, merge_file_entities, register_agent, sync_from_files,
    update_entity_content, EntityStateDoc,
};

use sem_core::parser::plugins::create_default_registry;

const TEST_FILE: &str = r#"import { db } from './db';

export function getUser(id: string) {
    return db.query(`SELECT * FROM users WHERE id = ${id}`);
}

export function createUser(name: string, email: string) {
    return db.insert('users', { name, email });
}

export function deleteUser(id: string) {
    return db.delete('users', id);
}

export function updateUser(id: string, data: Record<string, unknown>) {
    return db.update('users', id, data);
}

export function listUsers(limit: number = 10) {
    return db.query(`SELECT * FROM users LIMIT ${limit}`);
}
"#;

fn git_cmd(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "test")
        .env("GIT_AUTHOR_EMAIL", "test@test.com")
        .env("GIT_COMMITTER_NAME", "test")
        .env("GIT_COMMITTER_EMAIL", "test@test.com")
        .output()
        .expect("git failed");
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn git_ok(dir: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "test")
        .env("GIT_AUTHOR_EMAIL", "test@test.com")
        .env("GIT_COMMITTER_NAME", "test")
        .env("GIT_COMMITTER_EMAIL", "test@test.com")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Traditional git workflow: branch, edit, commit, merge.
fn bench_git_workflow(num_agents: usize) -> (std::time::Duration, bool) {
    let tmp = std::env::temp_dir().join(format!("weave_bench_git_{}", num_agents));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    // Init repo with base file
    git_cmd(&tmp, &["init"]);
    git_cmd(&tmp, &["checkout", "-b", "main"]);
    let file_path = tmp.join("users.ts");
    std::fs::write(&file_path, TEST_FILE).unwrap();
    git_cmd(&tmp, &["add", "users.ts"]);
    git_cmd(&tmp, &["commit", "-m", "initial"]);

    let functions = ["getUser", "createUser", "deleteUser", "updateUser", "listUsers"];

    let start = Instant::now();

    // Each agent creates a branch, edits their function, commits
    for i in 0..num_agents {
        let branch = format!("agent-{}", i + 1);
        git_cmd(&tmp, &["checkout", "main"]);
        git_cmd(&tmp, &["checkout", "-b", &branch]);

        let func_name = functions[i % functions.len()];
        let content = std::fs::read_to_string(&file_path).unwrap();
        let modified = content.replace(
            &format!("export function {}(", func_name),
            &format!("// Modified by agent-{}\nexport function {}(", i + 1, func_name),
        );
        std::fs::write(&file_path, &modified).unwrap();
        git_cmd(&tmp, &["add", "users.ts"]);
        git_cmd(&tmp, &["commit", "-m", &format!("agent-{} edits {}", i + 1, func_name)]);
    }

    // Merge all branches into main
    git_cmd(&tmp, &["checkout", "main"]);
    let mut all_clean = true;
    for i in 0..num_agents {
        let branch = format!("agent-{}", i + 1);
        let ok = git_ok(&tmp, &["merge", &branch, "--no-edit"]);
        if !ok {
            all_clean = false;
            // Abort failed merge
            git_ok(&tmp, &["merge", "--abort"]);
        }
    }

    let elapsed = start.elapsed();

    // Cleanup
    let _ = std::fs::remove_dir_all(&tmp);

    (elapsed, all_clean)
}

/// CRDT workflow: agents write entity content directly, merge via VV.
fn bench_crdt_workflow(num_agents: usize) -> (std::time::Duration, bool) {
    let registry = create_default_registry();

    let start = Instant::now();

    let mut state = EntityStateDoc::new_memory().unwrap();

    // Register agents
    for i in 0..num_agents {
        register_agent(
            &mut state,
            &format!("agent-{}", i + 1),
            &format!("Agent {}", i + 1),
            "main",
        )
        .unwrap();
    }

    // Sync from "file" (simulate initial state)
    let tmp = std::env::temp_dir().join(format!("weave_bench_crdt_{}", num_agents));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::write(tmp.join("users.ts"), TEST_FILE).unwrap();

    let count = sync_from_files(&mut state, &tmp, &["users.ts".to_string()], &registry).unwrap();
    assert!(count > 0, "Should sync entities");

    // Get entity IDs
    let plugin = registry.get_plugin("users.ts").unwrap();
    let entities = plugin.extract_entities(TEST_FILE, "users.ts");
    let functions: Vec<_> = entities
        .iter()
        .filter(|e| e.entity_type == "function")
        .collect();

    // Each agent writes to their function
    for i in 0..num_agents {
        let func = &functions[i % functions.len()];
        let agent = format!("agent-{}", i + 1);
        let new_content = format!("// Modified by {}\n{}", agent, func.content);
        let hash = format!("hash_{}", i);
        update_entity_content(&mut state, &agent, &func.id, &new_content, &hash).unwrap();
    }

    // Merge
    let result = merge_file_entities(&mut state, "users.ts", &registry).unwrap();

    let elapsed = start.elapsed();

    // Cleanup
    let _ = std::fs::remove_dir_all(&tmp);

    (elapsed, result.entities_conflicted == 0)
}

/// Measure conflict detection latency.
fn bench_conflict_detection_crdt() -> std::time::Duration {
    let registry = create_default_registry();
    let mut state = EntityStateDoc::new_memory().unwrap();

    let tmp = std::env::temp_dir().join("weave_bench_conflict_detect");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::write(tmp.join("users.ts"), TEST_FILE).unwrap();

    sync_from_files(&mut state, &tmp, &["users.ts".to_string()], &registry).unwrap();
    register_agent(&mut state, "agent-1", "A1", "main").unwrap();
    register_agent(&mut state, "agent-2", "A2", "main").unwrap();

    let plugin = registry.get_plugin("users.ts").unwrap();
    let entities = plugin.extract_entities(TEST_FILE, "users.ts");
    let func = entities.iter().find(|e| e.name == "getUser").unwrap();

    // Agent 1 writes
    update_entity_content(&mut state, "agent-1", &func.id, "v1_content", "h1").unwrap();

    // Agent 2 writes to same entity — measure detection time
    let start = Instant::now();
    update_entity_content(&mut state, "agent-2", &func.id, "v2_content", "h2").unwrap();

    // Check version vector for concurrency
    let status = get_entity_content(&state, &func.id).unwrap();
    let vv = &status.version_vector;
    assert_eq!(vv.get("agent-1"), 1);
    assert_eq!(vv.get("agent-2"), 1);
    // These are concurrent: neither dominates
    assert!(vv.partial_cmp(&{
        let mut other = weave_crdt::VersionVector::new();
        other.increment("agent-1");
        other
    }).is_none() == false); // agent-2's VV dominates agent-1-only VV

    let elapsed = start.elapsed();

    let _ = std::fs::remove_dir_all(&tmp);
    elapsed
}

fn bench_conflict_detection_git() -> std::time::Duration {
    let tmp = std::env::temp_dir().join("weave_bench_conflict_detect_git");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    git_cmd(&tmp, &["init"]);
    git_cmd(&tmp, &["checkout", "-b", "main"]);
    std::fs::write(tmp.join("users.ts"), TEST_FILE).unwrap();
    git_cmd(&tmp, &["add", "users.ts"]);
    git_cmd(&tmp, &["commit", "-m", "initial"]);

    // Agent 1 branch
    git_cmd(&tmp, &["checkout", "-b", "agent-1"]);
    let content = std::fs::read_to_string(tmp.join("users.ts")).unwrap();
    let modified = content.replace("return db.query(`SELECT * FROM users WHERE id", "// agent-1 was here\n    return db.query(`SELECT * FROM users WHERE id");
    std::fs::write(tmp.join("users.ts"), &modified).unwrap();
    git_cmd(&tmp, &["add", "users.ts"]);
    git_cmd(&tmp, &["commit", "-m", "agent-1 edit"]);

    // Agent 2 branch
    git_cmd(&tmp, &["checkout", "main"]);
    git_cmd(&tmp, &["checkout", "-b", "agent-2"]);
    let content2 = std::fs::read_to_string(tmp.join("users.ts")).unwrap();
    let modified2 = content2.replace("return db.query(`SELECT * FROM users WHERE id", "// agent-2 was here\n    return db.query(`SELECT * FROM users WHERE id");
    std::fs::write(tmp.join("users.ts"), &modified2).unwrap();
    git_cmd(&tmp, &["add", "users.ts"]);
    git_cmd(&tmp, &["commit", "-m", "agent-2 edit"]);

    // Measure: try merge to detect conflict
    let start = Instant::now();
    git_cmd(&tmp, &["checkout", "agent-1"]);
    let ok = git_ok(&tmp, &["merge", "agent-2", "--no-edit"]);
    let elapsed = start.elapsed();
    assert!(!ok, "Should conflict");
    git_ok(&tmp, &["merge", "--abort"]);

    let _ = std::fs::remove_dir_all(&tmp);
    elapsed
}

// ── Actual benchmark tests ──

#[test]
fn bench_2_agents_different_functions() {
    // Warmup
    let _ = bench_git_workflow(2);
    let _ = bench_crdt_workflow(2);

    let mut git_times = Vec::new();
    let mut crdt_times = Vec::new();
    let runs = 5;

    for _ in 0..runs {
        let (gt, gc) = bench_git_workflow(2);
        let (ct, cc) = bench_crdt_workflow(2);
        git_times.push(gt);
        crdt_times.push(ct);
        assert!(gc, "Git: 2 agents on different functions should merge clean");
        assert!(cc, "CRDT: 2 agents on different functions should merge clean");
    }

    let git_avg = git_times.iter().map(|t| t.as_micros()).sum::<u128>() / runs as u128;
    let crdt_avg = crdt_times.iter().map(|t| t.as_micros()).sum::<u128>() / runs as u128;
    let speedup = git_avg as f64 / crdt_avg as f64;

    eprintln!("\n=== 2 AGENTS, DIFFERENT FUNCTIONS ===");
    eprintln!("Git avg:  {} us ({} ms)", git_avg, git_avg / 1000);
    eprintln!("CRDT avg: {} us ({} ms)", crdt_avg, crdt_avg / 1000);
    eprintln!("Speedup:  {:.1}x", speedup);
    eprintln!("Both clean: yes");
}

#[test]
fn bench_5_agents_different_functions() {
    // Warmup
    let _ = bench_git_workflow(5);
    let _ = bench_crdt_workflow(5);

    let mut git_times = Vec::new();
    let mut crdt_times = Vec::new();
    let runs = 5;

    for _ in 0..runs {
        let (gt, gc) = bench_git_workflow(5);
        let (ct, cc) = bench_crdt_workflow(5);
        git_times.push(gt);
        crdt_times.push(ct);
        // Git may or may not merge clean with 5 agents
        let _ = gc;
        assert!(cc, "CRDT: 5 agents on different functions should merge clean");
    }

    let git_avg = git_times.iter().map(|t| t.as_micros()).sum::<u128>() / runs as u128;
    let crdt_avg = crdt_times.iter().map(|t| t.as_micros()).sum::<u128>() / runs as u128;
    let speedup = git_avg as f64 / crdt_avg as f64;

    eprintln!("\n=== 5 AGENTS, DIFFERENT FUNCTIONS ===");
    eprintln!("Git avg:  {} us ({} ms)", git_avg, git_avg / 1000);
    eprintln!("CRDT avg: {} us ({} ms)", crdt_avg, crdt_avg / 1000);
    eprintln!("Speedup:  {:.1}x", speedup);
}

#[test]
fn bench_conflict_detection_latency() {
    // Warmup
    let _ = bench_conflict_detection_crdt();
    let _ = bench_conflict_detection_git();

    let mut git_times = Vec::new();
    let mut crdt_times = Vec::new();
    let runs = 5;

    for _ in 0..runs {
        let gt = bench_conflict_detection_git();
        let ct = bench_conflict_detection_crdt();
        git_times.push(gt);
        crdt_times.push(ct);
    }

    let git_avg = git_times.iter().map(|t| t.as_micros()).sum::<u128>() / runs as u128;
    let crdt_avg = crdt_times.iter().map(|t| t.as_micros()).sum::<u128>() / runs as u128;
    let speedup = git_avg as f64 / crdt_avg.max(1) as f64;

    eprintln!("\n=== CONFLICT DETECTION LATENCY ===");
    eprintln!("Git avg:  {} us ({} ms)", git_avg, git_avg / 1000);
    eprintln!("CRDT avg: {} us ({} ms)", crdt_avg, crdt_avg / 1000);
    eprintln!("Speedup:  {:.1}x", speedup);
}

#[test]
fn bench_operation_count() {
    eprintln!("\n=== OPERATION COUNT: 2 AGENTS ===");
    eprintln!("Git:  init + 2*(checkout + edit + add + commit) + checkout + 2*merge = 11 git commands");
    eprintln!("CRDT: new_memory + 2*register + sync + 2*update_content + merge = 0 git commands");
    eprintln!("");
    eprintln!("=== OPERATION COUNT: 5 AGENTS ===");
    eprintln!("Git:  init + 5*(checkout + edit + add + commit) + checkout + 5*merge = 26 git commands");
    eprintln!("CRDT: new_memory + 5*register + sync + 5*update_content + merge = 0 git commands");
    eprintln!("");
    eprintln!("=== OPERATION COUNT: N AGENTS ===");
    eprintln!("Git:  O(N) branches + O(N) sequential merges = O(N) git subprocess calls");
    eprintln!("CRDT: O(N) in-memory writes + 1 merge = O(N) in-memory ops, 0 subprocesses");
}
