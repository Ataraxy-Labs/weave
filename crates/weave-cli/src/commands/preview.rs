use colored::Colorize;
use sem_core::parser::plugins::create_default_registry;
use weave_core::entity_merge_with_registry;
use weave_core::git;

pub(crate) fn run(
    branch: &str,
    file_path: Option<&str>,
    host: &weave_core::host::Host,
) -> Result<(), Box<dyn std::error::Error>> {
    let head = "HEAD";
    // Name the repository every git read runs in, rather than leaning on the
    // process working directory.
    let repo_root = git::find_repo_root(std::path::Path::new("."))?;
    let merge_base = git::find_merge_base(&repo_root, head, branch)?;

    let files = if let Some(fp) = file_path {
        vec![fp.to_string()]
    } else {
        git::get_changed_files(&repo_root, &merge_base, head, branch)?
    };

    if files.is_empty() {
        println!(
            "{} No files with changes in both branches.",
            "✓".green().bold()
        );
        return Ok(());
    }

    let registry = create_default_registry();
    let mut total_conflicts = 0;
    let mut total_auto_resolved = 0;

    for file in &files {
        let base_content = git::git_show(&repo_root, &merge_base, file).unwrap_or_default();
        let ours_content = git::git_show(&repo_root, head, file).unwrap_or_default();
        let theirs_content = git::git_show(&repo_root, branch, file).unwrap_or_default();

        if ours_content == theirs_content
            || base_content == ours_content
            || base_content == theirs_content
        {
            continue;
        }

        let result = entity_merge_with_registry(
            &base_content,
            &ours_content,
            &theirs_content,
            file,
            &registry,
            &weave_core::MarkerFormat::default(),
            host,
        );

        let status = if result.is_clean() {
            total_auto_resolved += 1;
            format!("{}", "auto-resolved".green())
        } else {
            total_conflicts += result.conflicts.len();
            format!(
                "{} conflict(s)",
                result.conflicts.len().to_string().red().bold()
            )
        };

        println!("  {} — {}", file, status);
        println!("    {}", result.stats);

        for conflict in &result.conflicts {
            println!(
                "    {} {} `{}`: {}",
                "✗".red(),
                conflict.entity_type,
                conflict.entity_name,
                conflict.kind
            );
        }
    }

    println!();
    if total_conflicts == 0 {
        println!(
            "{} Merge would be clean ({} file(s) auto-resolved by weave)",
            "✓".green().bold(),
            total_auto_resolved
        );
    } else {
        println!(
            "{} Merge would have {} entity-level conflict(s) ({} file(s) auto-resolved)",
            "✗".red().bold(),
            total_conflicts,
            total_auto_resolved
        );
    }

    Ok(())
}
