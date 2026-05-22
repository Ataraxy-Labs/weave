use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use colored::Colorize;

const SUPPORTED_EXTENSIONS: &[&str] = &[
    "*.ts", "*.tsx", "*.js", "*.mjs", "*.cjs", "*.jsx", "*.py", "*.go", "*.rs", "*.java", "*.c",
    "*.h", "*.cpp", "*.cc", "*.cxx", "*.hpp", "*.hh", "*.hxx", "*.rb", "*.cs", "*.php", "*.swift",
    "*.ex", "*.exs", "*.sh", "*.f90", "*.f95", "*.f03", "*.f08", "*.xml", "*.plist", "*.svg",
    "*.csproj", "*.fsproj", "*.vbproj", "*.json", "*.yaml", "*.yml", "*.toml", "*.md", "*.scala",
    "*.sc", "*.sbt", "*.kojo", "*.mill", "*.dart",
];

pub fn run(driver_path: Option<&str>, local: bool) -> Result<(), Box<dyn std::error::Error>> {
    // Verify we're in a git repo
    let git_dir = Path::new(".git");
    if !git_dir.exists() {
        return Err("Not in a git repository. Run `weave setup` from the repo root.".into());
    }

    // Resolve driver binary path
    let driver = if let Some(p) = driver_path {
        p.to_string()
    } else {
        // Try to find weave-driver in PATH or next to weave binary
        which_driver()?
    };

    // Verify driver exists
    if !Path::new(&driver).exists()
        && Command::new("which")
            .arg(&driver)
            .output()
            .map(|o| !o.status.success())
            .unwrap_or(true)
    {
        eprintln!(
            "{} Driver binary '{}' not found. Build with `cargo build --release` first.",
            "warning:".yellow().bold(),
            driver
        );
    }

    // Configure git merge driver
    let driver_cmd = format!("{} %O %A %B %L %P", driver);
    let status = Command::new("git")
        .args(["config", "merge.weave.name", "Entity-level semantic merge"])
        .status()?;
    if !status.success() {
        return Err("Failed to set merge.weave.name".into());
    }

    let status = Command::new("git")
        .args(["config", "merge.weave.driver", &driver_cmd])
        .status()?;
    if !status.success() {
        return Err("Failed to set merge.weave.driver".into());
    }

    println!(
        "{} Configured git merge driver: {}",
        "✓".green().bold(),
        driver_cmd
    );

    // Update git attributes
    let (attributes_label, attributes_path) = attributes_target(local)?;
    if local {
        if let Some(parent) = attributes_path.parent() {
            fs::create_dir_all(parent)?;
        }
    }

    let mut existing = if attributes_path.exists() {
        fs::read_to_string(&attributes_path)?
    } else {
        String::new()
    };

    let added = add_supported_patterns(&mut existing);

    if added > 0 {
        fs::write(&attributes_path, &existing)?;
        println!(
            "{} Updated {} ({} patterns added)",
            "✓".green().bold(),
            attributes_label,
            added
        );
    } else {
        println!(
            "{} {} already configured",
            "✓".green().bold(),
            attributes_label
        );
    }

    println!(
        "\n{} Weave is ready. Merge conflicts will now be resolved at the entity level.",
        "Done!".green().bold()
    );

    Ok(())
}

pub fn unsetup() -> Result<(), Box<dyn std::error::Error>> {
    let git_dir = Path::new(".git");
    if !git_dir.exists() {
        return Err("Not in a git repository. Run `weave unsetup` from the repo root.".into());
    }

    // Remove git config for weave merge driver
    let _ = Command::new("git")
        .args(["config", "--remove-section", "merge.weave"])
        .status();

    println!(
        "{} Removed weave merge driver from git config",
        "✓".green().bold()
    );

    // Remove weave patterns from tracked and local git attributes.
    clean_attributes(Path::new(".gitattributes"), ".gitattributes")?;
    if let Ok(local_attributes_path) = git_path("info/attributes") {
        clean_attributes(&local_attributes_path, ".git/info/attributes")?;
    }

    println!(
        "\n{} Weave has been removed. Git will use its default merge strategy.",
        "Done!".green().bold()
    );

    Ok(())
}

fn attributes_target(local: bool) -> Result<(&'static str, PathBuf), Box<dyn std::error::Error>> {
    if local {
        Ok((".git/info/attributes", git_path("info/attributes")?))
    } else {
        Ok((".gitattributes", PathBuf::from(".gitattributes")))
    }
}

fn add_supported_patterns(existing: &mut String) -> usize {
    let mut added = 0;
    for ext in SUPPORTED_EXTENSIONS {
        let pattern = format!("{} merge=weave", ext);
        if !existing.contains(&pattern) {
            if !existing.is_empty() && !existing.ends_with('\n') {
                existing.push('\n');
            }
            existing.push_str(&pattern);
            existing.push('\n');
            added += 1;
        }
    }
    added
}

fn clean_attributes(path: &Path, label: &str) -> Result<(), Box<dyn std::error::Error>> {
    if !path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(path)?;
    let filtered: Vec<&str> = content
        .lines()
        .filter(|line| !line.contains("merge=weave"))
        .collect();
    let new_content = filtered.join("\n");
    if filtered.is_empty() || new_content.trim().is_empty() {
        fs::remove_file(path)?;
        println!(
            "{} Removed {} (was only weave patterns)",
            "✓".green().bold(),
            label
        );
    } else {
        let mut out = new_content;
        if !out.ends_with('\n') {
            out.push('\n');
        }
        fs::write(path, out)?;
        println!(
            "{} Cleaned weave patterns from {}",
            "✓".green().bold(),
            label
        );
    }

    Ok(())
}

fn git_path(requested_path: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(["rev-parse", "--git-path", requested_path])
        .output()?;
    if !output.status.success() {
        return Err(format!("Failed to resolve git path '{}'", requested_path).into());
    }

    let path = String::from_utf8(output.stdout)?.trim().to_string();
    if path.is_empty() {
        return Err(format!("Git returned an empty path for '{}'", requested_path).into());
    }

    Ok(PathBuf::from(path))
}

fn which_driver() -> Result<String, Box<dyn std::error::Error>> {
    // Check PATH first — this returns the stable symlink path on package managers
    // like Homebrew, avoiding breakage when the versioned Cellar path changes on
    // upgrade (see https://github.com/Ataraxy-Labs/weave/issues/91).
    if let Ok(output) = Command::new("which").arg("weave-driver").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Ok(path);
            }
        }
    }

    // Fall back to checking next to current executable (e.g. local cargo build)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("weave-driver");
            if candidate.exists() {
                return Ok(candidate.to_string_lossy().to_string());
            }
        }
    }

    Ok("weave-driver".to_string())
}

#[cfg(test)]
mod tests {
    use super::{add_supported_patterns, SUPPORTED_EXTENSIONS};

    #[test]
    fn add_supported_patterns_adds_all_supported_patterns() {
        let mut existing = String::new();

        let added = add_supported_patterns(&mut existing);

        assert_eq!(added, SUPPORTED_EXTENSIONS.len());
        for ext in SUPPORTED_EXTENSIONS {
            assert!(existing.contains(&format!("{} merge=weave", ext)));
        }
    }

    #[test]
    fn add_supported_patterns_is_idempotent() {
        let mut existing = String::from("*.ts merge=weave\n");

        let first_added = add_supported_patterns(&mut existing);
        let after_first = existing.clone();
        let second_added = add_supported_patterns(&mut existing);

        assert_eq!(first_added, SUPPORTED_EXTENSIONS.len() - 1);
        assert_eq!(second_added, 0);
        assert_eq!(existing, after_first);
    }
}
