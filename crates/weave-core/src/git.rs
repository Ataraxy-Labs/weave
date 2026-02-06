use std::path::{Path, PathBuf};
use std::process::Command;

/// Find the root of the git repository by walking up from the current directory.
pub fn find_repo_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()?;
    if !output.status.success() {
        return Err("Not inside a git repository".into());
    }
    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(PathBuf::from(root))
}

/// Find the root of the git repository that contains the given path.
/// Uses `git -C <dir> rev-parse --show-toplevel` so it works regardless of CWD.
pub fn find_repo_root_from_path(path: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent()
            .map(|p| if p.as_os_str().is_empty() { Path::new(".") } else { p })
            .unwrap_or(Path::new("."))
            .to_path_buf()
    };
    let output = Command::new("git")
        .args(["-C", &dir.to_string_lossy(), "rev-parse", "--show-toplevel"])
        .output()?;
    if !output.status.success() {
        return Err(format!("Not inside a git repository: {}", dir.display()).into());
    }
    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(PathBuf::from(root))
}

/// Find the merge base between two refs.
pub fn find_merge_base(head: &str, branch: &str) -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(["merge-base", head, branch])
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "Failed to find merge base between '{}' and '{}'. Are both branches valid?",
            head, branch
        )
        .into());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Show file content at a given revision.
pub fn git_show(rev: &str, file: &str) -> Result<String, Box<dyn std::error::Error>> {
    let spec = format!("{}:{}", rev, file);
    let output = Command::new("git").args(["show", &spec]).output()?;
    if !output.status.success() {
        return Err(format!("git show {} failed", spec).into());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Get files changed in both branches relative to their merge base.
pub fn get_changed_files(
    merge_base: &str,
    head: &str,
    branch: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let ours_output = Command::new("git")
        .args(["diff", "--name-only", merge_base, head])
        .output()?;
    let ours_files: std::collections::HashSet<String> =
        String::from_utf8_lossy(&ours_output.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect();

    let theirs_output = Command::new("git")
        .args(["diff", "--name-only", merge_base, branch])
        .output()?;
    let theirs_files: std::collections::HashSet<String> =
        String::from_utf8_lossy(&theirs_output.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect();

    let mut both: Vec<String> = ours_files.intersection(&theirs_files).cloned().collect();
    both.sort();
    Ok(both)
}

/// Read a file from the working tree relative to a root path.
pub fn read_file(root: &Path, file_path: &str) -> Result<String, Box<dyn std::error::Error>> {
    let full = root.join(file_path);
    Ok(std::fs::read_to_string(full)?)
}
