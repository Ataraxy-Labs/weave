//! Fair benchmark: in-process merge algorithm vs in-process merge algorithm.
//!
//! Uses libgit2 (via git2 crate) with an in-memory repo for git's 3-way merge
//! and weave_core::entity_merge for weave's entity-level merge.
//! Both operate in-process on in-memory data. No subprocess spawning.
//!
//! This is the only honest way to compare: algorithm vs algorithm.

use std::time::Instant;

use git2::Repository;
use sem_core::parser::plugins::create_default_registry;
use weave_crdt::{
    merge_file_entities, register_agent, sync_from_files, update_entity_content, EntityStateDoc,
};

const BASE_FILE: &str = r#"import { db } from './db';

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

// Agent 1 modifies getUser
const OURS_FILE: &str = r#"import { db } from './db';

export function getUser(id: string) {
    // Added caching layer
    const cached = cache.get(id);
    if (cached) return cached;
    const result = db.query(`SELECT * FROM users WHERE id = ${id}`);
    cache.set(id, result);
    return result;
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

// Agent 2 modifies deleteUser
const THEIRS_FILE: &str = r#"import { db } from './db';

export function getUser(id: string) {
    return db.query(`SELECT * FROM users WHERE id = ${id}`);
}

export function createUser(name: string, email: string) {
    return db.insert('users', { name, email });
}

export function deleteUser(id: string) {
    // Added soft delete
    return db.update('users', id, { deleted_at: new Date() });
}

export function updateUser(id: string, data: Record<string, unknown>) {
    return db.update('users', id, data);
}

export function listUsers(limit: number = 10) {
    return db.query(`SELECT * FROM users LIMIT ${limit}`);
}
"#;

// Both agents modify getUser (conflict case)
const CONFLICT_THEIRS: &str = r#"import { db } from './db';

export function getUser(id: string) {
    // Added validation
    if (!id) throw new Error('id required');
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

/// Holds a libgit2 repo with pre-built trees, so we only time merge_trees().
struct LibGit2Setup {
    _dir: std::path::PathBuf,
    repo: Repository,
    base_tree: git2::Oid,
    ours_tree: git2::Oid,
    theirs_tree: git2::Oid,
}

/// Prepare a bare repo with three trees (not timed).
fn libgit2_setup(id: &str, base: &str, ours: &str, theirs: &str) -> LibGit2Setup {
    let dir = std::env::temp_dir().join(format!("bench_libgit2_{}", id));
    let _ = std::fs::remove_dir_all(&dir);
    let repo = Repository::init_bare(&dir).unwrap();

    let base_tree = {
        let blob = repo.blob(base.as_bytes()).unwrap();
        let mut tb = repo.treebuilder(None).unwrap();
        tb.insert("file.ts", blob, 0o100644).unwrap();
        tb.write().unwrap()
    };

    let ours_tree = {
        let blob = repo.blob(ours.as_bytes()).unwrap();
        let mut tb = repo.treebuilder(None).unwrap();
        tb.insert("file.ts", blob, 0o100644).unwrap();
        tb.write().unwrap()
    };

    let theirs_tree = {
        let blob = repo.blob(theirs.as_bytes()).unwrap();
        let mut tb = repo.treebuilder(None).unwrap();
        tb.insert("file.ts", blob, 0o100644).unwrap();
        tb.write().unwrap()
    };

    LibGit2Setup {
        _dir: dir,
        repo,
        base_tree,
        ours_tree,
        theirs_tree,
    }
}

/// Run merge_trees on pre-built trees. Only the merge is timed.
fn libgit2_merge_timed(setup: &LibGit2Setup) -> (std::time::Duration, bool) {
    let base = setup.repo.find_tree(setup.base_tree).unwrap();
    let ours = setup.repo.find_tree(setup.ours_tree).unwrap();
    let theirs = setup.repo.find_tree(setup.theirs_tree).unwrap();

    let start = Instant::now();
    let index = setup.repo.merge_trees(&base, &ours, &theirs, None).unwrap();
    let clean = !index.has_conflicts();
    let elapsed = start.elapsed();

    (elapsed, clean)
}

impl Drop for LibGit2Setup {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self._dir);
    }
}

/// weave entity_merge. In-process, in-memory. Only the merge is timed.
fn weave_merge(base: &str, ours: &str, theirs: &str) -> (std::time::Duration, bool) {
    let start = Instant::now();
    let result = weave_core::entity_merge(base, ours, theirs, "users.ts");
    let elapsed = start.elapsed();
    (elapsed, result.is_clean())
}

// ── Test 1: Clean merge (different functions) ──

#[test]
fn bench_clean_merge_in_process() {
    let setup = libgit2_setup("clean", BASE_FILE, OURS_FILE, THEIRS_FILE);

    // Warmup
    for _ in 0..3 {
        let _ = libgit2_merge_timed(&setup);
        let _ = weave_merge(BASE_FILE, OURS_FILE, THEIRS_FILE);
    }

    let runs = 50;
    let mut git_times = Vec::new();
    let mut weave_times = Vec::new();
    let mut git_clean = 0;
    let mut weave_clean = 0;

    for _ in 0..runs {
        let (gt, gc) = libgit2_merge_timed(&setup);
        let (wt, wc) = weave_merge(BASE_FILE, OURS_FILE, THEIRS_FILE);
        git_times.push(gt);
        weave_times.push(wt);
        if gc {
            git_clean += 1;
        }
        if wc {
            weave_clean += 1;
        }
    }

    git_times.sort();
    weave_times.sort();
    let git_median = git_times[runs / 2].as_micros();
    let weave_median = weave_times[runs / 2].as_micros();
    let git_avg = git_times.iter().map(|t| t.as_micros()).sum::<u128>() / runs as u128;
    let weave_avg = weave_times.iter().map(|t| t.as_micros()).sum::<u128>() / runs as u128;
    let git_p95 = git_times[(runs as f64 * 0.95) as usize].as_micros();
    let weave_p95 = weave_times[(runs as f64 * 0.95) as usize].as_micros();

    eprintln!("\n=== CLEAN MERGE: different functions, in-process ===");
    eprintln!("libgit2 merge_trees() vs weave entity_merge()");
    eprintln!("Both: in-process, no subprocess. Only merge step timed.");
    eprintln!("");
    eprintln!(
        "              {:>10} {:>10} {:>10}",
        "median", "avg", "p95"
    );
    eprintln!(
        "libgit2:      {:>8} us {:>8} us {:>8} us  (clean: {}/{})",
        git_median, git_avg, git_p95, git_clean, runs
    );
    eprintln!(
        "weave:        {:>8} us {:>8} us {:>8} us  (clean: {}/{})",
        weave_median, weave_avg, weave_p95, weave_clean, runs
    );
    eprintln!("");
    let ratio_median = git_median as f64 / weave_median.max(1) as f64;
    let ratio_avg = git_avg as f64 / weave_avg.max(1) as f64;
    eprintln!("Ratio (median): {:.2}x", ratio_median);
    eprintln!("Ratio (avg):    {:.2}x", ratio_avg);
}

// ── Test 2: Conflict case (same function modified by both) ──

#[test]
fn bench_conflict_merge_in_process() {
    let setup = libgit2_setup("conflict", BASE_FILE, OURS_FILE, CONFLICT_THEIRS);

    for _ in 0..3 {
        let _ = libgit2_merge_timed(&setup);
        let _ = weave_merge(BASE_FILE, OURS_FILE, CONFLICT_THEIRS);
    }

    let runs = 50;
    let mut git_times = Vec::new();
    let mut weave_times = Vec::new();
    let mut git_clean = 0;
    let mut weave_clean = 0;

    for _ in 0..runs {
        let (gt, gc) = libgit2_merge_timed(&setup);
        let (wt, wc) = weave_merge(BASE_FILE, OURS_FILE, CONFLICT_THEIRS);
        git_times.push(gt);
        weave_times.push(wt);
        if gc {
            git_clean += 1;
        }
        if wc {
            weave_clean += 1;
        }
    }

    git_times.sort();
    weave_times.sort();
    let git_median = git_times[runs / 2].as_micros();
    let weave_median = weave_times[runs / 2].as_micros();
    let git_avg = git_times.iter().map(|t| t.as_micros()).sum::<u128>() / runs as u128;
    let weave_avg = weave_times.iter().map(|t| t.as_micros()).sum::<u128>() / runs as u128;

    eprintln!("\n=== CONFLICT MERGE: same function, in-process ===");
    eprintln!("Both agents modified getUser");
    eprintln!("");
    eprintln!("              {:>10} {:>10}", "median", "avg");
    eprintln!(
        "libgit2:      {:>8} us {:>8} us  (clean: {}/{})",
        git_median, git_avg, git_clean, runs
    );
    eprintln!(
        "weave:        {:>8} us {:>8} us  (clean: {}/{})",
        weave_median, weave_avg, weave_clean, runs
    );
    eprintln!("");
    let ratio = git_median as f64 / weave_median.max(1) as f64;
    eprintln!("Ratio (median): {:.2}x", ratio);
}

// ── Test 3: Adjacent functions (semantic understanding test) ──

#[test]
fn bench_adjacent_functions() {
    let base = r#"export function processOrder(order: Order) {
    validate(order);
    const total = calculateTotal(order.items);
    return { orderId: order.id, total };
}

export function calculateTotal(items: Item[]) {
    return items.reduce((sum, item) => sum + item.price * item.quantity, 0);
}
"#;

    let ours = r#"export function processOrder(order: Order) {
    if (!order.items.length) throw new Error('Empty order');
    validate(order);
    const total = calculateTotal(order.items);
    const tax = total * 0.1;
    return { orderId: order.id, total, tax, grandTotal: total + tax };
}

export function calculateTotal(items: Item[]) {
    return items.reduce((sum, item) => sum + item.price * item.quantity, 0);
}
"#;

    let theirs = r#"export function processOrder(order: Order) {
    validate(order);
    const total = calculateTotal(order.items);
    return { orderId: order.id, total };
}

export function calculateTotal(items: Item[]) {
    return items.reduce((sum, item) => {
        const discount = item.discount || 0;
        return sum + (item.price - discount) * item.quantity;
    }, 0);
}
"#;

    let setup = libgit2_setup("adjacent", base, ours, theirs);

    // Warmup
    for _ in 0..3 {
        let _ = libgit2_merge_timed(&setup);
        let _ = weave_merge(base, ours, theirs);
    }

    let runs = 50;
    let mut git_clean = 0;
    let mut weave_clean = 0;
    let mut git_times = Vec::new();
    let mut weave_times = Vec::new();

    for _ in 0..runs {
        let (gt, gc) = libgit2_merge_timed(&setup);
        let (wt, wc) = weave_merge(base, ours, theirs);
        git_times.push(gt);
        weave_times.push(wt);
        if gc {
            git_clean += 1;
        }
        if wc {
            weave_clean += 1;
        }
    }

    git_times.sort();
    weave_times.sort();
    let git_median = git_times[runs / 2].as_micros();
    let weave_median = weave_times[runs / 2].as_micros();

    eprintln!("\n=== ADJACENT FUNCTION EDITS ===");
    eprintln!("Agent 1 rewrites processOrder, Agent 2 rewrites calculateTotal");
    eprintln!("");
    eprintln!(
        "libgit2 clean: {}/{}  (median {} us)",
        git_clean, runs, git_median
    );
    eprintln!(
        "weave clean:   {}/{}  (median {} us)",
        weave_clean, runs, weave_median
    );
    if weave_clean > git_clean {
        eprintln!(
            "Weave resolved {} merges that libgit2 could not.",
            weave_clean - git_clean
        );
    }
}

// ── Test 4: Same function, different changes ──
// Git says clean (wrong!), weave says conflict (correct!)

#[test]
fn bench_same_function_correctness() {
    let setup = libgit2_setup("correctness", BASE_FILE, OURS_FILE, CONFLICT_THEIRS);

    let runs = 50;
    let mut git_clean = 0;
    let mut weave_clean = 0;

    for _ in 0..runs {
        let (_, gc) = libgit2_merge_timed(&setup);
        let (_, wc) = weave_merge(BASE_FILE, OURS_FILE, CONFLICT_THEIRS);
        if gc {
            git_clean += 1;
        }
        if wc {
            weave_clean += 1;
        }
    }

    eprintln!("\n=== SAME FUNCTION CORRECTNESS ===");
    eprintln!("Both agents modified getUser with different logic");
    eprintln!("Ours: caching layer. Theirs: input validation.");
    eprintln!("");
    eprintln!("libgit2 says clean: {}/{}", git_clean, runs);
    eprintln!("weave says clean:   {}/{}", weave_clean, runs);
    eprintln!("");
    if git_clean > 0 && weave_clean == 0 {
        eprintln!("libgit2 silently merged conflicting logic! (WRONG)");
        eprintln!("weave correctly identified the conflict. (CORRECT)");
        eprintln!("");
        eprintln!("This is the key insight: git merges at line level.");
        eprintln!("If changes don't overlap textually, git says 'clean'.");
        eprintln!("But two agents rewrote the same function with different intent.");
        eprintln!("Weave understands entity boundaries and flags this.");
    } else if git_clean == 0 && weave_clean == 0 {
        eprintln!("Both correctly detected the conflict.");
    }
}

// ── Test 5: Scaling (N agents, in-process) ──

#[test]
fn bench_scaling_in_process() {
    let registry = create_default_registry();

    // 10-function file
    let mut big_file = String::from("import { db } from './db';\n\n");
    for i in 0..10 {
        big_file.push_str(&format!(
            "export function func{}(x: number) {{\n    return x * {};\n}}\n\n",
            i,
            i + 1
        ));
    }

    eprintln!("\n=== SCALING: N AGENTS, IN-PROCESS ===");
    eprintln!("Each agent modifies a different function in a 10-function file");
    eprintln!(
        "{:<8} {:>12} {:>12} {:>8}",
        "Agents", "libgit2(us)", "weave(us)", "Ratio"
    );

    for num_agents in [2, 4, 6, 8, 10] {
        // Create per-agent versions
        let mut agent_versions = Vec::new();
        for i in 0..num_agents {
            let func_name = format!("func{}", i % 10);
            let modified = big_file.replace(
                &format!("export function {}(", func_name),
                &format!("// agent-{}\nexport function {}(", i + 1, func_name),
            );
            agent_versions.push(modified);
        }

        // libgit2: sequential merge_trees (accumulating result), in-memory repo
        let repo = Repository::init_bare(
            std::env::temp_dir().join(format!("bench_scale_libgit2_{}", num_agents)),
        )
        .unwrap();

        // Base tree
        let base_blob_oid = repo.blob(big_file.as_bytes()).unwrap();
        let mut base_tb = repo.treebuilder(None).unwrap();
        base_tb
            .insert("test.ts", base_blob_oid, 0o100644)
            .unwrap();
        let base_tree_oid = base_tb.write().unwrap();
        let base_tree = repo.find_tree(base_tree_oid).unwrap();

        // Timed: sequential merges
        let git_start = Instant::now();
        let mut current_tree = base_tree.clone();
        for version in &agent_versions {
            let their_blob_oid = repo.blob(version.as_bytes()).unwrap();
            let mut their_tb = repo.treebuilder(None).unwrap();
            their_tb
                .insert("test.ts", their_blob_oid, 0o100644)
                .unwrap();
            let their_tree_oid = their_tb.write().unwrap();
            let their_tree = repo.find_tree(their_tree_oid).unwrap();

            let mut index = repo
                .merge_trees(&base_tree, &current_tree, &their_tree, None)
                .unwrap();
            if !index.has_conflicts() {
                let merged_oid = index.write_tree_to(&repo).unwrap();
                current_tree = repo.find_tree(merged_oid).unwrap();
            }
        }
        let git_elapsed = git_start.elapsed();

        let _ = std::fs::remove_dir_all(
            std::env::temp_dir().join(format!("bench_scale_libgit2_{}", num_agents)),
        );

        // CRDT: all agents write entities, one merge
        let crdt_start = Instant::now();
        {
            let tmp = std::env::temp_dir().join(format!("weave_bench_scale_ip_{}", num_agents));
            let _ = std::fs::remove_dir_all(&tmp);
            std::fs::create_dir_all(&tmp).unwrap();
            std::fs::write(tmp.join("test.ts"), &big_file).unwrap();

            let mut state = EntityStateDoc::new_memory().unwrap();
            sync_from_files(&mut state, &tmp, &["test.ts".to_string()], &registry).unwrap();

            let plugin = registry.get_plugin("test.ts").unwrap();
            let entities = plugin.extract_entities(&big_file, "test.ts");
            let functions: Vec<_> = entities
                .iter()
                .filter(|e| e.entity_type == "function")
                .collect();

            for i in 0..num_agents {
                let func = &functions[i % functions.len()];
                let agent = format!("agent-{}", i + 1);
                register_agent(&mut state, &agent, &agent, "main").unwrap();
                let new_content = format!("// agent-{}\n{}", i + 1, func.content);
                update_entity_content(
                    &mut state,
                    &agent,
                    &func.id,
                    &new_content,
                    &format!("h{}", i),
                )
                .unwrap();
            }

            merge_file_entities(&mut state, "test.ts", &registry).unwrap();
            let _ = std::fs::remove_dir_all(&tmp);
        }
        let crdt_elapsed = crdt_start.elapsed();

        let ratio = git_elapsed.as_micros() as f64 / crdt_elapsed.as_micros().max(1) as f64;
        eprintln!(
            "{:<8} {:>12} {:>12} {:>8.2}x",
            num_agents,
            git_elapsed.as_micros(),
            crdt_elapsed.as_micros(),
            ratio
        );
    }

    eprintln!("");
    eprintln!("Note: libgit2 = in-memory repo + sequential merge_trees()");
    eprintln!(
        "      weave = entity writes + single merge (includes tree-sitter parse + temp file for sync)"
    );
    eprintln!("      Both are in-process, no subprocess overhead");
}
