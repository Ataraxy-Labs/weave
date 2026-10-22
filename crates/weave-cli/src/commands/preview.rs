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
        vec![repo_relative(&repo_root, fp)?]
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
    let mut compared = 0;

    for file in &files {
        let base_content = git::git_show_optional(&repo_root, &merge_base, file)?;
        let ours_content = git::git_show_optional(&repo_root, head, file)?;
        let theirs_content = git::git_show_optional(&repo_root, branch, file)?;
        // A named file on no side is a wrong path, not a clean merge.
        if file_path.is_some()
            && base_content.is_none()
            && ours_content.is_none()
            && theirs_content.is_none()
        {
            return Err(format!("`{file}` exists at none of base/ours/theirs").into());
        }
        let base_content = base_content.unwrap_or_default();
        let ours_content = ours_content.unwrap_or_default();
        let theirs_content = theirs_content.unwrap_or_default();

        if ours_content == theirs_content
            || base_content == ours_content
            || base_content == theirs_content
        {
            continue;
        }
        compared += 1;

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
    if compared == 0 {
        println!(
            "{} Nothing to compare: no file needed a three-way merge (each was changed on one \
             side only, or identically on both)",
            "•".cyan().bold()
        );
    } else if total_conflicts == 0 {
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

/// `file` as a path from the repository root: an absolute path inside it, or
/// a path relative to the current directory, as a shell user types it.
fn repo_relative(
    repo_root: &std::path::Path,
    file: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let outside = || format!("`{file}` is outside the repository");
    let p = std::path::Path::new(file);
    let joined = if p.is_absolute() {
        let root = std::fs::canonicalize(repo_root)?;
        let abs = p
            .parent()
            .and_then(|d| std::fs::canonicalize(d).ok())
            .map(|d| d.join(p.file_name().unwrap_or_default()))
            .unwrap_or_else(|| p.to_path_buf());
        abs.strip_prefix(&root)
            .map_err(|_| outside())?
            .to_path_buf()
    } else {
        let out = std::process::Command::new("git")
            .args(["rev-parse", "--show-prefix"])
            .output()?;
        std::path::PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()).join(p)
    };
    let mut parts: Vec<String> = Vec::new();
    for c in joined.components() {
        match c {
            std::path::Component::Normal(s) => parts.push(s.to_string_lossy().to_string()),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                parts.pop().ok_or_else(outside)?;
            }
            _ => return Err(outside().into()),
        }
    }
    Ok(parts.join("/"))
}
