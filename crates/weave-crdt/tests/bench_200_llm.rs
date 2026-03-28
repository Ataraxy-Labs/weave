//! CRDT benchmark that reads real LLM-generated rewrites from a JSON file.
//! Called by bench_real_agents.py via `cargo test`.

use std::time::Instant;

use sem_core::parser::plugins::create_default_registry;
use weave_crdt::{
    merge_file_entities, register_agent, sync_from_files, update_entity_content, EntityStateDoc,
};

#[derive(serde::Deserialize)]
struct BenchInput {
    base_file: String,
    num_agents: usize,
    rewrites: Vec<Rewrite>,
}

#[derive(serde::Deserialize)]
struct Rewrite {
    agent_idx: usize,
    func_idx: usize,
    rewritten: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

#[test]
fn bench_crdt_from_json() {
    let rewrite_path = match std::env::var("CRDT_REWRITE_FILE") {
        Ok(p) => p,
        Err(_) => {
            eprintln!("CRDT_RESULT:0:0");
            return;
        }
    };

    let data: BenchInput =
        serde_json::from_str(&std::fs::read_to_string(&rewrite_path).unwrap()).unwrap();

    let registry = create_default_registry();
    let tmp = std::env::temp_dir().join("bench_crdt_llm");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::write(tmp.join("api.ts"), &data.base_file).unwrap();

    let start = Instant::now();

    let mut state = EntityStateDoc::new_memory().unwrap();
    sync_from_files(&mut state, &tmp, &["api.ts".to_string()], &registry).unwrap();

    let plugin = registry.get_plugin("api.ts").unwrap();
    let entities = plugin.extract_entities(&data.base_file, "api.ts");
    let functions: Vec<_> = entities
        .iter()
        .filter(|e| e.entity_type == "function")
        .collect();

    for rewrite in &data.rewrites {
        if rewrite.error.is_some() {
            continue;
        }
        let content = match &rewrite.rewritten {
            Some(c) if !c.is_empty() => c,
            _ => continue,
        };

        let func_idx = rewrite.func_idx;
        if func_idx >= functions.len() {
            continue;
        }

        let func = &functions[func_idx];
        let agent = format!("agent-{}", rewrite.agent_idx);
        register_agent(&mut state, &agent, &agent, "main").unwrap();
        update_entity_content(
            &mut state,
            &agent,
            &func.id,
            content,
            &format!("h{}", rewrite.agent_idx),
        )
        .unwrap();
    }

    let result = merge_file_entities(&mut state, "api.ts", &registry).unwrap();
    let elapsed = start.elapsed();

    let _ = std::fs::remove_dir_all(&tmp);

    let ms = elapsed.as_millis();
    let clean = result.entities_auto_merged;

    // Output for Python to parse
    eprintln!("CRDT_RESULT:{}:{}", ms, clean);
}
