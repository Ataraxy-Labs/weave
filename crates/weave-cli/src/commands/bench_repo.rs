use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

use weave_core::entity_merge_with_registry;
use sem_core::parser::plugins::create_default_registry;

const SUPPORTED_EXTENSIONS: &[&str] = &[
    "ts", "tsx", "js", "jsx", "py", "rs", "go", "java", "c", "cpp", "cc", "h", "hpp", "rb", "cs",
];

struct Stats {
    merge_commits: usize,
    files_tested: usize,
    both_clean: usize,
    weave_wins: usize,
    both_conflict: usize,
    regressions: usize,
    matches_human: usize,
    differs_from_human: usize,
}

pub fn run(repo_path: &str, limit: usize, show_diff: bool) -> Result<(), Box<dyn std::error::Error>> {
    let repo = Path::new(repo_path).canonicalize()?;
    // Support both regular and bare repos
    let is_git = repo.join(".git").exists() || repo.join("HEAD").exists();
    if !is_git {
        return Err(format!("{} is not a git repository", repo_path).into());
    }

    let repo_name = repo.file_name().unwrap_or_default().to_string_lossy();
    println!("weave real-world benchmark");
    println!("==========================");
    println!("repo: {} ({})", repo_name, repo.display());
    println!("scanning up to {} merge commits\n", limit);

    let output = Command::new("git")
        .args(["log", "--merges", "--format=%H", &format!("-{}", limit)])
        .current_dir(&repo)
        .output()?;

    let merge_commits: Vec<String> = String::from_utf8(output.stdout)?
        .lines()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    println!("found {} merge commits\n", merge_commits.len());

    let registry = create_default_registry();

    let mut stats = Stats {
        merge_commits: merge_commits.len(),
        files_tested: 0,
        both_clean: 0,
        weave_wins: 0,
        both_conflict: 0,
        regressions: 0,
        matches_human: 0,
        differs_from_human: 0,
    };

    for (i, merge_commit) in merge_commits.iter().enumerate() {
        // Get the two parents
        let output = Command::new("git")
            .args(["rev-parse", &format!("{}^1", merge_commit), &format!("{}^2", merge_commit)])
            .current_dir(&repo)
            .output()?;
        let parents: Vec<String> = String::from_utf8(output.stdout)?
            .lines()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        if parents.len() != 2 {
            continue; // skip octopus merges
        }
        let (p1, p2) = (&parents[0], &parents[1]);

        // Get merge base
        let output = Command::new("git")
            .args(["merge-base", p1, p2])
            .current_dir(&repo)
            .output()?;
        let base = String::from_utf8(output.stdout)?.trim().to_string();
        if base.is_empty() {
            continue;
        }

        // Files changed in each branch relative to base
        let files_p1 = changed_files(&repo, &base, p1)?;
        let files_p2 = changed_files(&repo, &base, p2)?;

        // Only care about files touched by BOTH branches
        let both_touched: Vec<&String> = files_p1.intersection(&files_p2).collect();

        for file in both_touched {
            let ext = Path::new(file).extension().and_then(|e| e.to_str()).unwrap_or("");
            if !SUPPORTED_EXTENSIONS.contains(&ext) {
                continue;
            }

            // Get all four versions: base, ours (p1), theirs (p2), human (merge commit)
            let (base_content, ours, theirs, human) = match (
                git_show(&repo, &base, file),
                git_show(&repo, p1, file),
                git_show(&repo, p2, file),
                git_show(&repo, merge_commit, file),
            ) {
                (Some(b), Some(o), Some(t), Some(h)) => (b, o, t, h),
                _ => continue, // file added/deleted on one side
            };

            // Skip large or binary files
            if base_content.len() > 1_000_000 || base_content.contains('\0') {
                continue;
            }

            // If both sides made identical changes, skip (trivial merge)
            if ours == theirs {
                continue;
            }

            stats.files_tested += 1;

            let git_clean = diffy::merge(&base_content, &ours, &theirs).is_ok();
            let weave_result = entity_merge_with_registry(&base_content, &ours, &theirs, file, &registry);
            // Check both the conflicts vec AND the actual content for markers
            let weave_clean = weave_result.is_clean()
                && !weave_result.content.contains("<<<<<<<")
                && !weave_result.content.contains(">>>>>>>");

            match (weave_clean, git_clean) {
                (true, true) => stats.both_clean += 1,
                (false, false) => stats.both_conflict += 1,
                (false, true) => {
                    stats.regressions += 1;
                    println!("  REGR   {}  {}", short(merge_commit), file);
                }
                (true, false) => {
                    stats.weave_wins += 1;
                    if normalize(&weave_result.content) == normalize(&human) {
                        stats.matches_human += 1;
                        println!("  MATCH  {}  {}", short(merge_commit), file);
                    } else {
                        stats.differs_from_human += 1;
                        println!("  DIFF   {}  {}", short(merge_commit), file);
                        if show_diff {
                            print_inline_diff(&weave_result.content, &human);
                        }
                    }
                }
            }
        }

        if (i + 1) % 100 == 0 {
            eprint!("\r  processed {}/{} merges...", i + 1, merge_commits.len());
        }
    }

    eprintln!();
    print_results(&stats, &repo_name);
    Ok(())
}

fn changed_files(repo: &Path, base: &str, head: &str) -> Result<HashSet<String>, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(["diff", "--name-only", "--diff-filter=M", &format!("{}..{}", base, head)])
        .current_dir(repo)
        .output()?;
    Ok(String::from_utf8(output.stdout)?
        .lines()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect())
}

fn git_show(repo: &Path, rev: &str, file: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["show", &format!("{}:{}", rev, file)])
        .current_dir(repo)
        .output()
        .ok()?;
    if output.status.success() {
        String::from_utf8(output.stdout).ok()
    } else {
        None
    }
}

fn short(hash: &str) -> &str {
    &hash[..8.min(hash.len())]
}

fn normalize(s: &str) -> String {
    s.lines().map(|l| l.trim_end()).collect::<Vec<_>>().join("\n")
}

fn print_inline_diff(weave: &str, human: &str) {
    let weave_lines: Vec<&str> = weave.lines().collect();
    let human_lines: Vec<&str> = human.lines().collect();
    let max = weave_lines.len().max(human_lines.len());
    let mut diffs = 0;
    for i in 0..max {
        let w = weave_lines.get(i).copied().unwrap_or("");
        let h = human_lines.get(i).copied().unwrap_or("");
        if w.trim_end() != h.trim_end() {
            if diffs == 0 {
                println!("         --- weave / +++ human ---");
            }
            println!("    L{:>4} - {}", i + 1, w);
            println!("    L{:>4} + {}", i + 1, h);
            diffs += 1;
            if diffs >= 10 {
                println!("    ... ({} more lines differ)", max - i - 1);
                break;
            }
        }
    }
    println!();
}

fn print_results(s: &Stats, repo_name: &str) {
    let total_git_conflicts = s.weave_wins + s.both_conflict;

    println!("results: {}", repo_name);
    println!("{}", "=".repeat(40));
    println!("merge commits:          {}", s.merge_commits);
    println!("files tested:           {}", s.files_tested);
    println!();
    println!("both clean:             {}", s.both_clean);
    println!("weave wins:             {}", s.weave_wins);
    println!("both conflict:          {}", s.both_conflict);
    println!("regressions:            {}", s.regressions);
    println!();

    if s.weave_wins > 0 {
        println!("of {} weave wins:", s.weave_wins);
        println!(
            "  matches human:  {} ({:.0}%)",
            s.matches_human,
            s.matches_human as f64 / s.weave_wins as f64 * 100.0
        );
        println!(
            "  differs:        {} ({:.0}%)",
            s.differs_from_human,
            s.differs_from_human as f64 / s.weave_wins as f64 * 100.0
        );
    }

    if total_git_conflicts > 0 {
        println!(
            "\nweave resolved {}/{} git conflicts ({:.0}%)",
            s.weave_wins,
            total_git_conflicts,
            s.weave_wins as f64 / total_git_conflicts as f64 * 100.0
        );
    }

    if s.regressions > 0 {
        println!("\nWARNING: {} regressions (git clean, weave conflict)", s.regressions);
    }
}
