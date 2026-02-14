use std::fs;
use std::path::Path;
use std::process::Command;

use colored::Colorize;

const SUPPORTED_EXTENSIONS: &[&str] = &[
    "*.ts", "*.tsx", "*.js", "*.jsx", "*.py", "*.go", "*.rs",
    "*.java", "*.c", "*.h", "*.cpp", "*.cc", "*.cxx", "*.hpp", "*.hh", "*.hxx",
    "*.rb", "*.cs", "*.php",
    "*.f90", "*.f95", "*.f03", "*.f08",
    "*.json", "*.yaml", "*.yml", "*.toml", "*.md",
];

pub fn run(driver_path: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    // Verify we're in a git repo
    let git_dir = Path::new(".git");
    if !git_dir.exists() {
        return Err("Not in a git repository. Run `weave-cli setup` from the repo root.".into());
    }

    // Resolve driver binary path
    let driver = if let Some(p) = driver_path {
        p.to_string()
    } else {
        // Try to find weave-driver in PATH or next to weave binary
        which_driver()?
    };

    // Verify driver exists
    if !Path::new(&driver).exists() && Command::new("which").arg(&driver).output()
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

    // Update .gitattributes
    let gitattributes_path = Path::new(".gitattributes");
    let mut existing = if gitattributes_path.exists() {
        fs::read_to_string(gitattributes_path)?
    } else {
        String::new()
    };

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

    if added > 0 {
        fs::write(gitattributes_path, &existing)?;
        println!(
            "{} Updated .gitattributes ({} patterns added)",
            "✓".green().bold(),
            added
        );
    } else {
        println!(
            "{} .gitattributes already configured",
            "✓".green().bold(),
        );
    }

    println!(
        "\n{} Weave is ready. Merge conflicts will now be resolved at the entity level.",
        "Done!".green().bold()
    );

    Ok(())
}

fn which_driver() -> Result<String, Box<dyn std::error::Error>> {
    // Check if weave-driver is next to current executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("weave-driver");
            if candidate.exists() {
                return Ok(candidate.to_string_lossy().to_string());
            }
        }
    }

    // Check PATH
    if let Ok(output) = Command::new("which").arg("weave-driver").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Ok(path);
            }
        }
    }

    Ok("weave-driver".to_string())
}
