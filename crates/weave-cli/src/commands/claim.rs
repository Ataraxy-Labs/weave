use colored::Colorize;
use sem_core::parser::plugins::create_default_registry;
use weave_core::git::find_repo_root;
use weave_crdt::{claim_entity, resolve_entity_id, upsert_entity, ClaimResult, EntityStateDoc};

pub(crate) fn run(
    agent_id: &str,
    file_path: &str,
    entity_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = find_repo_root(std::path::Path::new("."))?;
    let state_path = repo_root.join(".weave").join("state.automerge");
    let mut state = EntityStateDoc::open(&state_path)?;
    let registry = create_default_registry();

    // Read file content
    let content = std::fs::read_to_string(repo_root.join(file_path))?;

    // Resolve entity name to ID
    let entity_id = resolve_entity_id(&content, file_path, entity_name, &registry)
        .ok_or_else(|| format!("Entity '{}' not found in '{}'", entity_name, file_path))?;

    // Ensure entity exists in state
    let plugin = registry
        .get_plugin(file_path)
        .ok_or("No parser for this file type")?;
    let entities = plugin.extract_entities(&content, file_path);
    if let Some(e) = entities.iter().find(|e| e.id == entity_id) {
        upsert_entity(
            &mut state,
            &e.id,
            &e.name,
            &e.entity_type,
            file_path,
            &e.content_hash,
        )?;
    }

    // Claim
    let result = claim_entity(&mut state, agent_id, &entity_id)?;
    state.save()?;

    match result {
        ClaimResult::Claimed => {
            println!(
                "{} Entity '{}' claimed by '{}'",
                "✓".green().bold(),
                entity_name,
                agent_id
            );
        }
        ClaimResult::AlreadyOwnedBySelf => {
            println!("Entity '{}' already claimed by you.", entity_name);
        }
        ClaimResult::AlreadyClaimed { by } => {
            println!(
                "{} Entity '{}' is already claimed by '{}'",
                "✗".red().bold(),
                entity_name,
                by
            );
        }
    }

    Ok(())
}
