use colored::Colorize;
use sem_core::parser::plugins::create_default_registry;
use weave_core::git::find_repo_root;
use weave_crdt::{release_entity, resolve_entity_or_error, EntityAddress, EntityStateDoc};

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
    // Resolve entity name to ID (ambiguous names are an error, never pick-first)
    let address = EntityAddress::by_name(entity_name);
    let entity_id = resolve_entity_or_error(&content, file_path, &registry, &address)
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

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
