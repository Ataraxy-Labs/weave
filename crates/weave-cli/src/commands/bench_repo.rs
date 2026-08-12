use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

use sem_core::parser::plugins::create_default_registry;
use weave_core::entity_merge_with_registry;

const SUPPORTED_EXTENSIONS: &[&str] = &[
    "ts", "tsx", "js", "jsx", "py", "rs", "go", "java", "c", "cpp", "cc", "h", "hpp", "rb", "cs",
    "dart",
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

#[derive(serde::Serialize)]
struct CaseRecord {
    commit: String,
    file: String,
    category: String, // "match", "diff", "regression"
    dir: String,      // relative path in save dir
}

#[derive(serde::Serialize)]
struct BenchResults {
    repo: String,
    merge_commits: usize,
    files_tested: usize,
    both_clean: usize,
    weave_wins: usize,
    both_conflict: usize,
    regressions: usize,
    matches_human: usize,
    differs_from_human: usize,
    resolution_rate: f64,
    human_match_rate: f64,
    cases: Vec<CaseRecord>,
}

pub(crate) fn run(
    repo_path: &str,
    limit: usize,
    show_diff: bool,
    save_dir: Option<&str>,
    host: &weave_core::host::Host,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = Path::new(repo_path).canonicalize()?;
    // Support both regular and bare repos
    let is_git = repo.join(".git").exists() || repo.join("HEAD").exists();
    if !is_git {
        return Err(format!("{} is not a git repository", repo_path).into());
    }

    let repo_name = repo
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    println!("weave real-world benchmark");
    println!("==========================");
    println!("repo: {} ({})", repo_name, repo.display());
    println!("scanning up to {} merge commits\n", limit);

    // Create save directory if requested
    if let Some(dir) = save_dir {
        std::fs::create_dir_all(dir)?;
    }

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

    let mut cases: Vec<CaseRecord> = Vec::new();

    for (i, merge_commit) in merge_commits.iter().enumerate() {
        // Get the two parents
        let output = Command::new("git")
            .args([
                "rev-parse",
                &format!("{}^1", merge_commit),
                &format!("{}^2", merge_commit),
            ])
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
            let ext = Path::new(file)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            if !SUPPORTED_EXTENSIONS.contains(&ext) {
                continue;
            }

            // Get all four versions: base, ours (p1), theirs (p2), human (merge commit)
            // A None anywhere means the file was added/deleted on one side.
            let (Some(base_content), Some(ours), Some(theirs), Some(human)) = (
                git_show(&repo, &base, file),
                git_show(&repo, p1, file),
                git_show(&repo, p2, file),
                git_show(&repo, merge_commit, file),
            ) else {
                continue;
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
            let weave_result = entity_merge_with_registry(
                &base_content,
                &ours,
                &theirs,
                file,
                &registry,
                &weave_core::MarkerFormat::default(),
                host,
            );
            // Cleanliness is a question about the decisions, and `is_clean()`
            // is the one place that answers it. The marker scan this used to do
            // was the v1 workaround for a result whose conflicts vec and
            // content could disagree; in v2 both are produced from the same
            // dispositions, so the scan can only introduce a second opinion —
            // one that a fixture containing the literal marker string flips.
            let weave_clean = weave_result.is_clean();

            match (weave_clean, git_clean) {
                (true, true) => stats.both_clean += 1,
                (false, false) => stats.both_conflict += 1,
                (false, true) => {
                    stats.regressions += 1;
                    println!("  REGR   {}  {}", short(merge_commit), file);
                    if let Some(dir) = save_dir {
                        let case_dir = save_case(
                            dir,
                            merge_commit,
                            file,
                            &base_content,
                            &ours,
                            &theirs,
                            &human,
                            &weave_result.content,
                        )?;
                        cases.push(CaseRecord {
                            commit: merge_commit.clone(),
                            file: file.clone(),
                            category: "regression".to_string(),
                            dir: case_dir,
                        });
                    }
                }
                (true, false) => {
                    stats.weave_wins += 1;
                    if normalize(&weave_result.content) == normalize(&human) {
                        stats.matches_human += 1;
                        println!("  MATCH  {}  {}", short(merge_commit), file);
                        if let Some(dir) = save_dir {
                            let case_dir = save_case(
                                dir,
                                merge_commit,
                                file,
                                &base_content,
                                &ours,
                                &theirs,
                                &human,
                                &weave_result.content,
                            )?;
                            cases.push(CaseRecord {
                                commit: merge_commit.clone(),
                                file: file.clone(),
                                category: "match".to_string(),
                                dir: case_dir,
                            });
                        }
                    } else {
                        stats.differs_from_human += 1;
                        println!("  DIFF   {}  {}", short(merge_commit), file);
                        if show_diff {
                            print_inline_diff(&weave_result.content, &human);
                        }
                        if let Some(dir) = save_dir {
                            let case_dir = save_case(
                                dir,
                                merge_commit,
                                file,
                                &base_content,
                                &ours,
                                &theirs,
                                &human,
                                &weave_result.content,
                            )?;
                            cases.push(CaseRecord {
                                commit: merge_commit.clone(),
                                file: file.clone(),
                                category: "diff".to_string(),
                                dir: case_dir,
                            });
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

    // Write results.json
    if let Some(dir) = save_dir {
        let total_git_conflicts = stats.weave_wins + stats.both_conflict;
        let results = BenchResults {
            repo: repo_name.clone(),
            merge_commits: stats.merge_commits,
            files_tested: stats.files_tested,
            both_clean: stats.both_clean,
            weave_wins: stats.weave_wins,
            both_conflict: stats.both_conflict,
            regressions: stats.regressions,
            matches_human: stats.matches_human,
            differs_from_human: stats.differs_from_human,
            resolution_rate: if total_git_conflicts > 0 {
                stats.weave_wins as f64 / total_git_conflicts as f64 * 100.0
            } else {
                0.0
            },
            human_match_rate: if stats.weave_wins > 0 {
                stats.matches_human as f64 / stats.weave_wins as f64 * 100.0
            } else {
                0.0
            },
            cases,
        };
        let json = serde_json::to_string_pretty(&results)?;
        std::fs::write(Path::new(dir).join("results.json"), &json)?;
        println!("\nsaved {} cases to {}/", results.cases.len(), dir);
    }

    Ok(())
}

/// Save a test case to disk: base, ours, theirs, human, weave
#[allow(clippy::too_many_arguments)]
fn save_case(
    save_dir: &str,
    commit: &str,
    file: &str,
    base: &str,
    ours: &str,
    theirs: &str,
    human: &str,
    weave: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // Create a directory name from commit + file
    let safe_file = file.replace('/', "_");
    let dir_name = format!("{}_{}", short(commit), safe_file);
    let case_path = Path::new(save_dir).join(&dir_name);
    std::fs::create_dir_all(&case_path)?;

    std::fs::write(case_path.join("base"), base)?;
    std::fs::write(case_path.join("ours"), ours)?;
    std::fs::write(case_path.join("theirs"), theirs)?;
    std::fs::write(case_path.join("human"), human)?;
    std::fs::write(case_path.join("weave"), weave)?;

    Ok(dir_name)
}

fn changed_files(
    repo: &Path,
    base: &str,
    head: &str,
) -> Result<HashSet<String>, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args([
            "diff",
            "--name-only",
            "--diff-filter=M",
            &format!("{}..{}", base, head),
        ])
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
    s.lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
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
        println!(
            "\nWARNING: {} regressions (git clean, weave conflict)",
            s.regressions
        );
    }
}
