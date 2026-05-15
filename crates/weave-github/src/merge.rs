use std::sync::Arc;

use weave_core::conflict::MergeStats;
use weave_core::entity_merge_with_registry;
use weave_core::MergeResult;

use crate::comment::format_comment;
use crate::github::GitHubClient;
use crate::webhook::PrEvent;
use crate::AppState;

/// Per-file merge result.
pub struct FileMergeResult {
    pub path: String,
    pub result: MergeResult,
}

/// Handle a pull_request event end-to-end.
pub async fn handle_pull_request(state: &AppState, pr: &PrEvent) -> Result<(), String> {
    let gh = GitHubClient::for_installation(&state.config, pr.installation_id).await?;

    // Poll mergeable status (GitHub computes it async)
    let mergeable = poll_mergeable(&gh, &pr.owner, &pr.repo, pr.pr_number).await?;

    if mergeable == Some(true) {
        // No conflicts, post a green check
        gh.create_check_run(
            &pr.owner,
            &pr.repo,
            &pr.head_sha,
            "success",
            "No conflicts",
            "This PR has no merge conflicts.",
        )
        .await?;
        return Ok(());
    }

    // Get merge base and changed files
    let compare = gh
        .compare(&pr.owner, &pr.repo, &pr.base_sha, &pr.head_sha)
        .await?;
    let merge_base = &compare.merge_base_commit.sha;

    let files = compare.files.unwrap_or_default();
    if files.is_empty() {
        return Ok(());
    }

    // Filter to files with supported parsers
    let registry = Arc::clone(&state.registry);
    let supported_files: Vec<String> = files
        .iter()
        .filter(|f| f.status == "modified" || f.status == "added")
        .filter(|f| registry.get_plugin(&f.filename).is_some())
        .map(|f| f.filename.clone())
        .collect();

    if supported_files.is_empty() {
        gh.create_check_run(
            &pr.owner,
            &pr.repo,
            &pr.head_sha,
            "neutral",
            "No supported files",
            "No files with supported languages found in this PR.",
        )
        .await?;
        return Ok(());
    }

    // Merge each file
    let mut file_results = Vec::new();
    let mut total_stats = MergeStats::default();

    for path in &supported_files {
        let (base_content, ours_content, theirs_content) = tokio::try_join!(
            gh.get_file_content(&pr.owner, &pr.repo, path, merge_base),
            gh.get_file_content(&pr.owner, &pr.repo, path, &pr.head_sha),
            gh.get_file_content(&pr.owner, &pr.repo, path, &pr.base_sha),
        )?;

        let base = base_content.unwrap_or_default();
        let ours = ours_content.unwrap_or_default();
        let theirs = theirs_content.unwrap_or_default();
        let file_path = path.clone();
        let reg = Arc::clone(&registry);

        let result = tokio::task::spawn_blocking(move || {
            entity_merge_with_registry(
                &base,
                &ours,
                &theirs,
                &file_path,
                &reg,
                &weave_core::MarkerFormat::default(),
            )
        })
        .await
        .map_err(|e| format!("merge task panicked: {e}"))?;

        // Accumulate stats
        total_stats.entities_unchanged += result.stats.entities_unchanged;
        total_stats.entities_ours_only += result.stats.entities_ours_only;
        total_stats.entities_theirs_only += result.stats.entities_theirs_only;
        total_stats.entities_both_changed_merged += result.stats.entities_both_changed_merged;
        total_stats.entities_conflicted += result.stats.entities_conflicted;
        total_stats.entities_added_ours += result.stats.entities_added_ours;
        total_stats.entities_added_theirs += result.stats.entities_added_theirs;
        total_stats.entities_deleted += result.stats.entities_deleted;
        total_stats.semantic_warnings += result.stats.semantic_warnings;
        total_stats.resolved_via_diffy += result.stats.resolved_via_diffy;
        total_stats.resolved_via_inner_merge += result.stats.resolved_via_inner_merge;
        if result.stats.used_fallback {
            total_stats.used_fallback = true;
        }

        file_results.push(FileMergeResult {
            path: path.clone(),
            result,
        });
    }

    // Format and post comment
    let comment = format_comment(&file_results, &total_stats);
    gh.post_comment(&pr.owner, &pr.repo, pr.pr_number, &comment)
        .await?;

    // Post check run
    let (conclusion, title) = if total_stats.has_conflicts() {
        (
            "neutral",
            format!("{} conflict(s) remain", total_stats.entities_conflicted),
        )
    } else {
        (
            "success",
            format!(
                "All {} entities resolved cleanly",
                total_stats.entities_both_changed_merged
                    + total_stats.entities_ours_only
                    + total_stats.entities_theirs_only
                    + total_stats.entities_unchanged
            ),
        )
    };

    gh.create_check_run(
        &pr.owner,
        &pr.repo,
        &pr.head_sha,
        conclusion,
        &title,
        &comment,
    )
    .await?;

    Ok(())
}

/// Poll GitHub for the PR's mergeable status, retrying up to 5 times.
async fn poll_mergeable(
    gh: &GitHubClient,
    owner: &str,
    repo: &str,
    pr_number: u64,
) -> Result<Option<bool>, String> {
    for attempt in 0..5 {
        let mergeable = gh.get_pr_mergeable(owner, repo, pr_number).await?;
        if mergeable.is_some() {
            return Ok(mergeable);
        }
        // GitHub hasn't computed it yet, back off
        tokio::time::sleep(std::time::Duration::from_secs(2u64.pow(attempt))).await;
    }
    Ok(None)
}
