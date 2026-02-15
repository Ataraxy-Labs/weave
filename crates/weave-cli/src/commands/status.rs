use colored::Colorize;
use sem_core::parser::plugins::create_default_registry;
use weave_core::git::find_repo_root;
use weave_crdt::{get_agent_status, get_entities_for_file, sync_from_files, EntityStateDoc};

pub fn run(
    file: Option<&str>,
    agent: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = find_repo_root()?;
    let state_path = repo_root.join(".weave").join("state.automerge");
    let mut state = EntityStateDoc::open(&state_path)?;

    if let Some(agent_id) = agent {
        // Show agent status
        match get_agent_status(&state, agent_id) {
            Ok(status) => {
                println!("Agent: {}", status.agent_id.bold());
                println!("  Status: {}", status.status);
                println!("  Branch: {}", status.branch);
                println!("  Working on: {}", if status.working_on.is_empty() {
                    "(nothing)".to_string()
                } else {
                    status.working_on.join(", ")
                });
            }
            Err(_) => {
                println!("Agent '{}' not found in state.", agent_id);
            }
        }
        return Ok(());
    }

    if let Some(file_path) = file {
        let registry = create_default_registry();
        // Sync from file first
        let _ = sync_from_files(&mut state, &repo_root, &[file_path.to_string()], &registry);

        let entities = get_entities_for_file(&state, file_path)?;
        if entities.is_empty() {
            println!("No entities tracked for '{}'", file_path);
            return Ok(());
        }

        println!("{}", file_path.bold());
        for e in &entities {
            let claim_info = if let Some(ref by) = e.claimed_by {
                format!(" [claimed by {}]", by.yellow())
            } else {
                String::new()
            };
            let mod_info = if let Some(ref by) = e.last_modified_by {
                format!(" (last modified by {})", by)
            } else {
                String::new()
            };
            println!(
                "  {} {} `{}` v{}{}{}",
                "•".cyan(),
                e.entity_type,
                e.name,
                e.version,
                claim_info,
                mod_info,
            );
        }
    } else {
        println!("Usage: weave-cli status --file <path> or --agent <id>");
    }

    Ok(())
}
