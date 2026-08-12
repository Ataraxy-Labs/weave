use std::sync::Arc;

use weave_core::conflict::MergeStats;
use weave_core::entity_merge_with_registry;
use weave_core::MergeResult;

use crate::comment::format_comment;
use crate::error::{GitHubError, Reason};
use crate::github::GitHubClient;
use crate::webhook::PrEvent;
use crate::AppState;

/// Per-file merge result.
pub struct FileMergeResult {
    pub path: String,
    pub result: MergeResult,
}

/// Handle a pull_request event end-to-end.
pub async fn handle_pull_request(state: &AppState, pr: &PrEvent) -> Result<(), GitHubError> {
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

    // Merge each file. The host was granted once, when the server started.
    let host = state.host;
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
                &host,
            )
        })
        .await
        .map_err(|e| GitHubError::new("merge", Reason::MergeTaskLost(e)))?;

        // Accumulate stats. The fold belongs to the type, in the crate that
        // owns the table — this crate used to sum it field by field, which is
        // how nine fields came to have two writing modules and how
        // `references_rewritten` came to be missing from every PR total.
        total_stats.absorb(&result.stats);

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
///
/// Two things can make an attempt inconclusive, and they are not the same
/// thing: GitHub has not finished computing the answer yet (a successful call
/// returning `None`), or the call itself did not get through. Both are worth
/// another attempt inside the same budget. A 401, a 404 or a body we could not
/// read are not — they will say the same thing in eight seconds — so they
/// return immediately. That distinction is exactly what `String` errors could
/// not express here.
async fn poll_mergeable(
    gh: &GitHubClient,
    owner: &str,
    repo: &str,
    pr_number: u64,
) -> Result<Option<bool>, GitHubError> {
    let mut last_transient: Option<GitHubError> = None;
    for attempt in 0..5 {
        match gh.get_pr_mergeable(owner, repo, pr_number).await {
            Ok(Some(mergeable)) => return Ok(Some(mergeable)),
            // Computed, but not yet known: back off and ask again.
            Ok(None) => {}
            Err(e) if e.is_transient() => last_transient = Some(e),
            Err(e) => return Err(e),
        }
        tokio::time::sleep(std::time::Duration::from_secs(2u64.pow(attempt))).await;
    }
    match last_transient {
        Some(e) => Err(e),
        None => Ok(None),
    }
}
