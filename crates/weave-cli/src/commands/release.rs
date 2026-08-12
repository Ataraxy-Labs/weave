use colored::Colorize;
use sem_core::parser::plugins::create_default_registry;
use weave_core::git::find_repo_root;
use weave_crdt::{release_entity, resolve_entity_id, EntityStateDoc};

pub(crate) fn run(
    agent_id: &str,
    file_path: &str,
    entity_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = find_repo_root(std::path::Path::new("."))?;
    let state_path = repo_root.join(".weave").join("state.automerge");
    let mut state = EntityStateDoc::open(&state_path)?;
    let registry = create_default_registry();

    let content = std::fs::read_to_string(repo_root.join(file_path))?;
    let entity_id = resolve_entity_id(&content, file_path, entity_name, &registry)
        .ok_or_else(|| format!("Entity '{}' not found in '{}'", entity_name, file_path))?;

    release_entity(&mut state, agent_id, &entity_id)?;
    state.save()?;

    println!(
        "{} Entity '{}' released by '{}'",
        "✓".green().bold(),
        entity_name,
        agent_id
    );

    Ok(())
}
