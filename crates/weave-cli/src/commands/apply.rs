use colored::Colorize;
use weave_core::git::find_repo_root;
use weave_crdt::{reconstruct_file_from_crdt, EntityStateDoc};

/// Materialize entity edits from the CRDT back onto the working files.
///
/// This is weave's write side: the CRDT is the authoritative source of an
/// entity's content, and this projects that content onto disk, reassembling
/// the file from its entities plus the interstitial text between them. It is
/// the counterpart to editing entities in the CRDT (via the MCP tools or
/// `weave claim` flows): edit entities, then `weave apply` to write the files.
pub(crate) fn run(files: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if files.is_empty() {
        return Err("Specify one or more files to apply, e.g. `weave apply src/lib.rs`".into());
    }

    let repo_root = find_repo_root(std::path::Path::new("."))?;
    let state_path = repo_root.join(".weave").join("state.automerge");
    let state = EntityStateDoc::open(&state_path)?;

    let mut applied = 0usize;
    for file in files {
        let reconstructed = reconstruct_file_from_crdt(&state, file)?;
        if reconstructed.is_empty() {
            println!(
                "  {} {} — no entities tracked in the CRDT, skipping",
                "!".yellow().bold(),
                file
            );
            continue;
        }

        let target = repo_root.join(file);
        let before = std::fs::read_to_string(&target).unwrap_or_default();
        if before == reconstructed {
            println!("  {} {} — already up to date", "·".dimmed(), file);
            continue;
        }

        std::fs::write(&target, &reconstructed)?;
        applied += 1;
        println!("  {} {} — materialized from CRDT", "✓".green().bold(), file);
    }

    println!(
        "{} {} file(s) written from the CRDT.",
        "weave apply:".bold(),
        applied
    );
    Ok(())
}
