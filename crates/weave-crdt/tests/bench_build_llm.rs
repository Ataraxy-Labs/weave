//! CRDT benchmark for the "build features" scenario.
//! Each agent produced a full modified app.ts. We extract entities from
//! each agent's version, find what changed vs base, and write those
//! entity changes to the CRDT.

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
    rewritten: Option<String>,
}

#[test]
fn bench_crdt_build() {
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
    let plugin = registry.get_plugin("app.ts").unwrap();

    // Extract base entities
    let base_entities = plugin.extract_entities(&data.base_file, "app.ts");

    let tmp = std::env::temp_dir().join("bench_crdt_build");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::write(tmp.join("app.ts"), &data.base_file).unwrap();

    let start = Instant::now();

    let mut state = EntityStateDoc::new_memory().unwrap();
    sync_from_files(&mut state, &tmp, &["app.ts".to_string()], &registry).unwrap();

    // For each agent: extract entities from their version, find changes, write to CRDT
    for rewrite in &data.rewrites {
        let agent_code = match &rewrite.rewritten {
            Some(c) if !c.is_empty() => {
                // Strip markdown fences if present
                let trimmed = c.trim();
                if trimmed.starts_with("```") {
                    let lines: Vec<&str> = trimmed.lines().collect();
                    if lines.last().map(|l| l.trim()) == Some("```") {
                        lines[1..lines.len() - 1].join("\n")
                    } else {
                        lines[1..].join("\n")
                    }
                } else {
                    c.clone()
                }
            }
            _ => continue,
        };

        let agent_id = format!("agent-{}", rewrite.agent_idx);
        register_agent(&mut state, &agent_id, &agent_id, "main").unwrap();

        let agent_entities = plugin.extract_entities(&agent_code, "app.ts");

        // Find entities that changed vs base (by name comparison)
        for agent_ent in &agent_entities {
            // Find matching base entity by name
            let base_match = base_entities.iter().find(|b| b.name == agent_ent.name);

            match base_match {
                Some(base_ent) if base_ent.content != agent_ent.content => {
                    // Entity was modified - write new content
                    update_entity_content(
                        &mut state,
                        &agent_id,
                        &base_ent.id,
                        &agent_ent.content,
                        &format!("h_{}_{}", rewrite.agent_idx, agent_ent.name),
                    )
                    .unwrap();
                }
                None => {
                    // New entity added by this agent - we'd need upsert
                    // For now, skip (CRDT tracks existing entities from sync)
                }
                _ => {
                    // Unchanged, skip
                }
            }
        }
    }

    let result = merge_file_entities(&mut state, "app.ts", &registry).unwrap();
    let elapsed = start.elapsed();

    let _ = std::fs::remove_dir_all(&tmp);

    let ms = elapsed.as_millis();
    let clean = result.entities_auto_merged;

    eprintln!("CRDT_RESULT:{}:{}", ms, clean);
}
